use std::{sync::Arc, time::Duration};

use rustok_api::{HostRuntimeContext, PortActor, PortContext, PortErrorKind};
use rustok_core::MigrationSource;
use rustok_groups::{GroupsModerationSubjectAdapterFactory, GroupsModule};
use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionEffect, ModerationDecisionEffectAction,
    ModerationDecisionKind, ModerationReasonCode, ModerationScopeKind, ModerationScopeRef,
    ModerationSubjectAdapterFactory, ModerationSubjectCommandPort, ModerationSubjectKind,
    ModerationSubjectRef, moderation_scope_claim,
};
use rustok_outbox::OutboxModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const POSTGRES_URL_ENV: &str = "RUSTOK_GROUPS_TEST_POSTGRES_URL";
const MODERATION_ACTOR: &str = "rustok-moderation";

#[derive(Clone, Copy)]
struct MembershipFixture {
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    owner_membership_id: Uuid,
    member_id: Uuid,
    member_membership_id: Uuid,
}

fn schema_url(base: &str, schema: &str) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}options=-csearch_path%3D{schema}%2Cpublic")
}

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .max_connections(8)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("Groups moderation PostgreSQL connection should open")
}

async fn install_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Outbox migration should apply for Groups moderation evidence");
    }
    for migration in GroupsModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for moderation evidence");
    }
}

fn fresh_fixture() -> MembershipFixture {
    MembershipFixture {
        tenant_id: Uuid::new_v4(),
        group_id: Uuid::new_v4(),
        owner_id: Uuid::new_v4(),
        owner_membership_id: Uuid::new_v4(),
        member_id: Uuid::new_v4(),
        member_membership_id: Uuid::new_v4(),
    }
}

async fn seed_fixture(db: &DatabaseConnection, fixture: MembershipFixture, handle: &str) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{}', '{}', '{}', '{handle}', 2);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{}', '{}', '{}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{}', '{}', '{}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
        fixture.group_id,
        fixture.tenant_id,
        fixture.owner_id,
        fixture.owner_membership_id,
        fixture.tenant_id,
        fixture.group_id,
        fixture.owner_id,
        fixture.member_membership_id,
        fixture.tenant_id,
        fixture.group_id,
        fixture.member_id,
    ))
    .await
    .expect("Groups moderation PostgreSQL fixture should seed");
}

fn adapter(db: DatabaseConnection) -> Arc<dyn ModerationSubjectCommandPort> {
    GroupsModerationSubjectAdapterFactory
        .build(&HostRuntimeContext::new(db))
        .expect("Groups moderation adapter should materialize")
}

fn application_context(
    fixture: MembershipFixture,
    decision_id: Uuid,
    correlation_suffix: &str,
) -> PortContext {
    let scope = ModerationScopeRef {
        kind: ModerationScopeKind::Group,
        id: Some(fixture.group_id),
    };
    PortContext::new(
        fixture.tenant_id.to_string(),
        PortActor::service(MODERATION_ACTOR),
        "und",
        format!(
            "groups-moderation-postgres:{correlation_suffix}:{}",
            Uuid::new_v4()
        ),
    )
    .with_causation_id(decision_id.to_string())
    .with_claim(moderation_scope_claim(&scope).expect("valid Groups moderation scope claim"))
    .with_idempotency_key(decision_id.to_string())
    .with_deadline(Duration::from_secs(30))
}

fn suspend_command(
    fixture: MembershipFixture,
    decision_id: Uuid,
    revision: i64,
    hash_byte: char,
) -> ApplyModerationDecisionCommand {
    ApplyModerationDecisionCommand {
        decision_id,
        subject: ModerationSubjectRef {
            module: "groups".to_string(),
            kind: ModerationSubjectKind::GroupMembership,
            id: fixture.member_membership_id,
            revision,
        },
        decision_kind: ModerationDecisionKind::SuspendSubject,
        reason_code: ModerationReasonCode::Harassment,
        effect: ModerationDecisionEffect::v1(ModerationDecisionEffectAction::SuspendSubject {
            effective_until: None,
        })
        .expect("valid suspension effect"),
        decision_hash: hash_byte.to_string().repeat(64),
    }
}

async fn group_snapshot(db: &DatabaseConnection, fixture: MembershipFixture) -> (i64, i64) {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT version, member_count FROM groups WHERE tenant_id = '{}' AND id = '{}'",
                fixture.tenant_id, fixture.group_id
            ),
        ))
        .await
        .expect("group snapshot query should succeed")
        .expect("group should exist");
    (
        row.try_get("", "version")
            .expect("group version should decode"),
        row.try_get("", "member_count")
            .expect("member count should decode"),
    )
}

async fn membership_revision(db: &DatabaseConnection, fixture: MembershipFixture) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT revision FROM group_memberships WHERE tenant_id = '{}' AND id = '{}'",
                fixture.tenant_id, fixture.member_membership_id
            ),
        ))
        .await
        .expect("membership revision query should succeed")
        .expect("membership should exist");
    row.try_get("", "revision")
        .expect("membership revision should decode")
}

async fn scalar_count(db: &DatabaseConnection, sql: String) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .expect("count query should succeed")
        .expect("count row should exist");
    row.try_get("", "count").expect("count should decode")
}

async fn assert_single_moderation_mutation(
    db: &DatabaseConnection,
    fixture: MembershipFixture,
    decision_id: Uuid,
    decision_hash: &str,
) {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT source_kind, moderation_decision_id, moderation_decision_hash, actor_kind, actor_id, revision, revoked_at IS NULL AS active FROM group_membership_enforcements WHERE tenant_id = '{}' AND membership_id = '{}'",
                fixture.tenant_id, fixture.member_membership_id
            ),
        ))
        .await
        .expect("moderation enforcement query should succeed")
        .expect("moderation enforcement row should exist");
    let source_kind: String = row
        .try_get("", "source_kind")
        .expect("source kind should decode");
    let stored_decision_id: Uuid = row
        .try_get("", "moderation_decision_id")
        .expect("moderation decision id should decode");
    let stored_decision_hash: String = row
        .try_get("", "moderation_decision_hash")
        .expect("moderation decision hash should decode");
    let actor_kind: String = row
        .try_get("", "actor_kind")
        .expect("actor kind should decode");
    let actor_id: String = row.try_get("", "actor_id").expect("actor id should decode");
    let enforcement_revision: i64 = row
        .try_get("", "revision")
        .expect("enforcement revision should decode");
    let active: bool = row
        .try_get("", "active")
        .expect("active marker should decode");

    assert_eq!(source_kind, "moderation_decision");
    assert_eq!(stored_decision_id, decision_id);
    assert_eq!(stored_decision_hash, decision_hash);
    assert_eq!(actor_kind, "service");
    assert_eq!(actor_id, MODERATION_ACTOR);
    assert_eq!(enforcement_revision, 1);
    assert!(active);
    assert_eq!(group_snapshot(db, fixture).await, (2, 2));
    assert_eq!(membership_revision(db, fixture).await, 2);
    assert_eq!(
        scalar_count(
            db,
            format!(
                "SELECT COUNT(*) AS count FROM group_audit_entries WHERE tenant_id = '{}' AND group_id = '{}' AND action = 'group.membership_suspended'",
                fixture.tenant_id, fixture.group_id
            ),
        )
        .await,
        1
    );
    assert_eq!(
        scalar_count(
            db,
            format!(
                "SELECT COUNT(*) AS count FROM group_domain_events WHERE tenant_id = '{}' AND aggregate_type = 'membership' AND aggregate_id = '{}' AND event_type = 'groups.membership.suspended'",
                fixture.tenant_id, fixture.member_membership_id
            ),
        )
        .await,
        1
    );
}

async fn run_apply_and_lost_response_replay(db: &DatabaseConnection) {
    let fixture = fresh_fixture();
    seed_fixture(db, fixture, "moderation-postgres-replay").await;
    let adapter = adapter(db.clone());
    let decision_id = Uuid::new_v4();
    let command = suspend_command(fixture, decision_id, 1, 'a');

    let first = adapter
        .apply_moderation_decision(
            application_context(fixture, decision_id, "first"),
            command.clone(),
        )
        .await
        .expect("Groups moderation suspension should apply");
    assert_eq!(first.decision_id, decision_id);
    assert_eq!(first.subject, command.subject);
    assert_eq!(first.applied_revision, 2);
    assert_single_moderation_mutation(db, fixture, decision_id, &command.decision_hash).await;

    let replay = adapter
        .apply_moderation_decision(
            application_context(fixture, decision_id, "lost-response-replay"),
            command.clone(),
        )
        .await
        .expect("completed producer receipt must replay before reading the now-revisioned subject");
    assert_eq!(replay, first);
    assert_single_moderation_mutation(db, fixture, decision_id, &command.decision_hash).await;

    let mut changed = command;
    changed.reason_code = ModerationReasonCode::Threats;
    let conflict = adapter
        .apply_moderation_decision(
            application_context(fixture, decision_id, "changed-request"),
            changed,
        )
        .await
        .expect_err("same decision id with changed Groups producer request must conflict");
    assert_eq!(conflict.kind, PortErrorKind::Conflict);
    assert!(!conflict.retryable);
    assert_single_moderation_mutation(db, fixture, decision_id, &"a".repeat(64)).await;
}

async fn run_concurrent_revision_fence(db: &DatabaseConnection) {
    let fixture = fresh_fixture();
    seed_fixture(db, fixture, "moderation-postgres-concurrency").await;
    let adapter = adapter(db.clone());
    let decision_a = Uuid::new_v4();
    let decision_b = Uuid::new_v4();
    let command_a = suspend_command(fixture, decision_a, 1, 'b');
    let command_b = suspend_command(fixture, decision_b, 1, 'c');

    let left = adapter.apply_moderation_decision(
        application_context(fixture, decision_a, "concurrent-a"),
        command_a.clone(),
    );
    let right = adapter.apply_moderation_decision(
        application_context(fixture, decision_b, "concurrent-b"),
        command_b.clone(),
    );
    let (left, right) = tokio::join!(left, right);

    let (winner_id, winner_hash, winner, loser_id, loser_command, loser) = match (left, right) {
        (Ok(winner), Err(loser)) => (
            decision_a,
            command_a.decision_hash.as_str(),
            winner,
            decision_b,
            command_b,
            loser,
        ),
        (Err(loser), Ok(winner)) => (
            decision_b,
            command_b.decision_hash.as_str(),
            winner,
            decision_a,
            command_a,
            loser,
        ),
        other => panic!("exactly one concurrent moderation decision must win: {other:?}"),
    };

    assert_eq!(winner.applied_revision, 2);
    assert_eq!(loser.kind, PortErrorKind::Conflict);
    assert_eq!(loser.code, "groups.moderation_subject_revision_conflict");
    assert!(!loser.retryable);
    assert_single_moderation_mutation(db, fixture, winner_id, winner_hash).await;

    let replayed_loser = adapter
        .apply_moderation_decision(
            application_context(fixture, loser_id, "failed-replay"),
            loser_command,
        )
        .await
        .expect_err("non-retryable stale moderation decision must replay its failed receipt");
    assert_eq!(replayed_loser.kind, PortErrorKind::Conflict);
    assert_eq!(
        replayed_loser.code,
        "groups.moderation_subject_revision_conflict"
    );
    assert_single_moderation_mutation(db, fixture, winner_id, winner_hash).await;
}

#[tokio::test]
#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]
async fn moderation_membership_adapter_replay_and_concurrency_postgres() {
    let base_url = std::env::var(POSTGRES_URL_ENV)
        .expect("RUSTOK_GROUPS_TEST_POSTGRES_URL must be configured");
    let schema_name = format!("groups_moderation_{}", Uuid::new_v4().simple());
    let admin_db = connect(&base_url).await;
    admin_db
        .execute_unprepared(&format!("CREATE SCHEMA {schema_name}"))
        .await
        .expect("isolated Groups moderation schema should create");
    let scoped_url = schema_url(&base_url, &schema_name);
    let db = connect(&scoped_url).await;
    install_schema(&db).await;

    run_apply_and_lost_response_replay(&db).await;
    run_concurrent_revision_fence(&db).await;

    drop(db);
    admin_db
        .execute_unprepared(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await
        .expect("isolated Groups moderation schema should drop");
}
