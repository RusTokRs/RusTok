#![cfg(feature = "mod-groups")]

use std::time::Duration;

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_groups::{
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementCommandService, GroupRole,
    RevokeGroupMembershipSuspensionRequest, SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use tempfile::TempDir;
use uuid::Uuid;

#[derive(Clone, Copy)]
struct GroupFixture {
    group_id: Uuid,
    owner_id: Uuid,
    admin_id: Uuid,
    moderator_id: Uuid,
    member_a_id: Uuid,
    member_b_id: Uuid,
    member_c_id: Uuid,
    owner_membership_id: Uuid,
    admin_membership_id: Uuid,
    moderator_membership_id: Uuid,
    member_a_membership_id: Uuid,
    member_b_membership_id: Uuid,
    member_c_membership_id: Uuid,
}

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("Groups direct enforcement runtime SQLite connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for direct enforcement runtime evidence");
    }
}

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp
        .path()
        .join("groups-membership-enforcement-runtime.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

fn fresh_fixture() -> GroupFixture {
    GroupFixture {
        group_id: Uuid::new_v4(),
        owner_id: Uuid::new_v4(),
        admin_id: Uuid::new_v4(),
        moderator_id: Uuid::new_v4(),
        member_a_id: Uuid::new_v4(),
        member_b_id: Uuid::new_v4(),
        member_c_id: Uuid::new_v4(),
        owner_membership_id: Uuid::new_v4(),
        admin_membership_id: Uuid::new_v4(),
        moderator_membership_id: Uuid::new_v4(),
        member_a_membership_id: Uuid::new_v4(),
        member_b_membership_id: Uuid::new_v4(),
        member_c_membership_id: Uuid::new_v4(),
    }
}

async fn seed_group(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    fixture: GroupFixture,
    handle: &str,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{}', '{tenant_id}', '{}', '{handle}', 6);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{}', '{}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{}', '{}', 'admin', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{}', '{}', 'moderator', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{}', '{}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{}', '{}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{}', '{}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
        fixture.group_id,
        fixture.owner_id,
        fixture.owner_membership_id,
        fixture.group_id,
        fixture.owner_id,
        fixture.admin_membership_id,
        fixture.group_id,
        fixture.admin_id,
        fixture.moderator_membership_id,
        fixture.group_id,
        fixture.moderator_id,
        fixture.member_a_membership_id,
        fixture.group_id,
        fixture.member_a_id,
        fixture.member_b_membership_id,
        fixture.group_id,
        fixture.member_b_id,
        fixture.member_c_membership_id,
        fixture.group_id,
        fixture.member_c_id,
    ))
    .await
    .expect("Groups direct enforcement runtime fixture should seed");
}

fn write_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    idempotency_key: &str,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("groups-enforcement-runtime-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(5))
    .with_idempotency_key(idempotency_key)
}

fn platform_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    idempotency_key: &str,
) -> PortContext {
    write_context(tenant_id, actor_id, idempotency_key).with_claim("groups:moderate")
}

async fn group_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
) -> (i64, i64) {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT version, member_count FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("group snapshot query should succeed")
        .expect("group should exist");
    (
        row.try_get("", "version").expect("group version should decode"),
        row.try_get("", "member_count")
            .expect("member_count should decode"),
    )
}

async fn membership_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> (String, String, i64) {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT role, status, revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("membership snapshot query should succeed")
        .expect("membership should exist");
    (
        row.try_get("", "role").expect("membership role should decode"),
        row.try_get("", "status")
            .expect("membership status should decode"),
        row.try_get("", "revision")
            .expect("membership revision should decode"),
    )
}

async fn enforcement_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
) -> i64 {
    scalar_count(
        db,
        format!(
            "SELECT COUNT(*) AS count FROM group_membership_enforcements WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}'"
        ),
    )
    .await
}

async fn ledger_counts(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
) -> (i64, i64, i64) {
    let audit = scalar_count(
        db,
        format!(
            "SELECT COUNT(*) AS count FROM group_audit_entries WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}'"
        ),
    )
    .await;
    let events = scalar_count(
        db,
        format!(
            "SELECT COUNT(*) AS count FROM group_domain_events WHERE tenant_id = '{tenant_id}'"
        ),
    )
    .await;
    let receipts = scalar_count(
        db,
        format!(
            "SELECT COUNT(*) AS count FROM group_command_receipts WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}'"
        ),
    )
    .await;
    (audit, events, receipts)
}

async fn scalar_count(db: &DatabaseConnection, sql: String) -> i64 {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .expect("count query should succeed")
        .expect("count row should exist");
    row.try_get("", "count").expect("count should decode")
}

async fn assert_no_material_change(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    fixture: GroupFixture,
) {
    assert_eq!(group_snapshot(db, tenant_id, fixture.group_id).await, (1, 6));
    for user_id in [
        fixture.owner_id,
        fixture.admin_id,
        fixture.moderator_id,
        fixture.member_a_id,
        fixture.member_b_id,
        fixture.member_c_id,
    ] {
        assert_eq!(
            membership_snapshot(db, tenant_id, fixture.group_id, user_id)
                .await
                .2,
            1
        );
    }
    assert_eq!(enforcement_count(db, tenant_id, fixture.group_id).await, 0);
    assert_eq!(ledger_counts(db, tenant_id, fixture.group_id).await, (0, 0, 0));
}

async fn suspend(
    service: &GroupMembershipEnforcementCommandService,
    context: PortContext,
    group_id: Uuid,
    target_user_id: Uuid,
    expected_membership_revision: i64,
    reason_code: &str,
) -> Result<rustok_groups::GroupMembershipEnforcementMutationResult, rustok_api::PortError> {
    GroupMembershipEnforcementCommandPort::suspend_membership(
        service,
        context,
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id,
            expected_membership_revision,
            reason_code: reason_code.to_string(),
            effective_until: None,
        },
    )
    .await
}

async fn revoke(
    service: &GroupMembershipEnforcementCommandService,
    context: PortContext,
    group_id: Uuid,
    target_user_id: Uuid,
    expected_membership_revision: i64,
    reason_code: &str,
) -> Result<rustok_groups::GroupMembershipEnforcementMutationResult, rustok_api::PortError> {
    GroupMembershipEnforcementCommandPort::revoke_membership_suspension(
        service,
        context,
        RevokeGroupMembershipSuspensionRequest {
            group_id,
            target_user_id,
            expected_membership_revision,
            reason_code: reason_code.to_string(),
        },
    )
    .await
}

async fn assert_ledger_fact(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    fixture: GroupFixture,
    membership_id: Uuid,
    actor_id: Uuid,
    idempotency_key: &str,
    audit_action: &str,
    event_type: &str,
    command_type: &str,
) {
    assert_eq!(
        scalar_count(
            db,
            format!(
                "SELECT COUNT(*) AS count FROM group_audit_entries WHERE tenant_id = '{tenant_id}' AND group_id = '{}' AND actor_user_id = '{actor_id}' AND target_user_id IS NOT NULL AND action = '{audit_action}'",
                fixture.group_id
            ),
        )
        .await,
        1
    );
    assert_eq!(
        scalar_count(
            db,
            format!(
                "SELECT COUNT(*) AS count FROM group_domain_events WHERE tenant_id = '{tenant_id}' AND aggregate_type = 'membership' AND aggregate_id = '{membership_id}' AND actor_id = '{actor_id}' AND event_type = '{event_type}'"
            ),
        )
        .await,
        1
    );
    assert_eq!(
        scalar_count(
            db,
            format!(
                "SELECT COUNT(*) AS count FROM group_command_receipts WHERE tenant_id = '{tenant_id}' AND group_id = '{}' AND actor_user_id = '{actor_id}' AND idempotency_key = '{idempotency_key}' AND command_type = '{command_type}'",
                fixture.group_id
            ),
        )
        .await,
        1
    );
}

#[tokio::test]
async fn direct_enforcement_denials_leave_no_owner_side_effects_sqlite() {
    let temp = tempfile::tempdir().expect("temporary runtime directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    install_groups_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let fixture = fresh_fixture();
    seed_group(&db, tenant_id, fixture, "enforcement-runtime-denials").await;
    let service = GroupMembershipEnforcementCommandService::new(db.clone());

    let self_target = suspend(
        &service,
        write_context(tenant_id, fixture.moderator_id, "self-target"),
        fixture.group_id,
        fixture.moderator_id,
        1,
        "security_self_target",
    )
    .await
    .expect_err("direct enforcement must reject self-targeting");
    assert_eq!(self_target.kind, PortErrorKind::Forbidden);
    assert_eq!(
        self_target.code,
        "groups.membership_enforcement_self_target"
    );
    assert!(!self_target.retryable);

    let platform_actor = Uuid::new_v4();
    let owner_target = suspend(
        &service,
        platform_context(tenant_id, platform_actor, "owner-target"),
        fixture.group_id,
        fixture.owner_id,
        1,
        "security_owner_target",
    )
    .await
    .expect_err("platform moderation must not suspend the group owner");
    assert_eq!(owner_target.kind, PortErrorKind::Forbidden);
    assert_eq!(
        owner_target.code,
        "groups.membership_enforcement_owner_protected"
    );
    assert!(!owner_target.retryable);

    let hierarchy = suspend(
        &service,
        write_context(tenant_id, fixture.moderator_id, "moderator-vs-admin"),
        fixture.group_id,
        fixture.admin_id,
        1,
        "security_hierarchy",
    )
    .await
    .expect_err("moderator must not enforce an administrator");
    assert_eq!(hierarchy.kind, PortErrorKind::Forbidden);
    assert_eq!(hierarchy.code, "groups.manager_required");
    assert!(!hierarchy.retryable);

    let member_actor = suspend(
        &service,
        write_context(tenant_id, fixture.member_a_id, "member-vs-member"),
        fixture.group_id,
        fixture.member_b_id,
        1,
        "security_member_actor",
    )
    .await
    .expect_err("ordinary member must not enforce another member");
    assert_eq!(member_actor.kind, PortErrorKind::Forbidden);
    assert_eq!(member_actor.code, "groups.manager_required");
    assert!(!member_actor.retryable);

    assert_no_material_change(&db, tenant_id, fixture).await;

    drop(service);
    drop(db);
    drop(temp);
}

#[tokio::test]
async fn direct_enforcement_hierarchy_and_platform_bypass_are_exact_sqlite() {
    let temp = tempfile::tempdir().expect("temporary hierarchy directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    install_groups_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let fixture = fresh_fixture();
    seed_group(&db, tenant_id, fixture, "enforcement-runtime-hierarchy").await;
    let service = GroupMembershipEnforcementCommandService::new(db.clone());

    let owner_suspend_admin = suspend(
        &service,
        write_context(tenant_id, fixture.owner_id, "owner-suspend-admin"),
        fixture.group_id,
        fixture.admin_id,
        1,
        "hierarchy_owner_admin",
    )
    .await
    .expect("owner should suspend administrator");
    assert_eq!(owner_suspend_admin.membership_revision, 2);
    assert_eq!(owner_suspend_admin.group_version, 2);
    assert_eq!(owner_suspend_admin.member_count, 6);
    let owner_revoke_admin = revoke(
        &service,
        write_context(tenant_id, fixture.owner_id, "owner-revoke-admin"),
        fixture.group_id,
        fixture.admin_id,
        2,
        "hierarchy_owner_admin_release",
    )
    .await
    .expect("owner should revoke direct administrator suspension");
    assert_eq!(owner_revoke_admin.membership_revision, 3);
    assert_eq!(owner_revoke_admin.group_version, 3);

    let admin_suspend_moderator = suspend(
        &service,
        write_context(tenant_id, fixture.admin_id, "admin-suspend-moderator"),
        fixture.group_id,
        fixture.moderator_id,
        1,
        "hierarchy_admin_moderator",
    )
    .await
    .expect("administrator should suspend moderator");
    assert_eq!(admin_suspend_moderator.membership_revision, 2);
    assert_eq!(admin_suspend_moderator.group_version, 4);
    let admin_revoke_moderator = revoke(
        &service,
        write_context(tenant_id, fixture.admin_id, "admin-revoke-moderator"),
        fixture.group_id,
        fixture.moderator_id,
        2,
        "hierarchy_admin_moderator_release",
    )
    .await
    .expect("administrator should revoke direct moderator suspension");
    assert_eq!(admin_revoke_moderator.membership_revision, 3);
    assert_eq!(admin_revoke_moderator.group_version, 5);

    let moderator_suspend_member = suspend(
        &service,
        write_context(tenant_id, fixture.moderator_id, "moderator-suspend-member"),
        fixture.group_id,
        fixture.member_a_id,
        1,
        "hierarchy_moderator_member",
    )
    .await
    .expect("moderator should suspend ordinary member");
    assert_eq!(moderator_suspend_member.membership_revision, 2);
    assert_eq!(moderator_suspend_member.group_version, 6);
    let moderator_revoke_member = revoke(
        &service,
        write_context(tenant_id, fixture.moderator_id, "moderator-revoke-member"),
        fixture.group_id,
        fixture.member_a_id,
        2,
        "hierarchy_moderator_member_release",
    )
    .await
    .expect("moderator should revoke direct member suspension");
    assert_eq!(moderator_revoke_member.membership_revision, 3);
    assert_eq!(moderator_revoke_member.group_version, 7);

    let platform_actor = Uuid::new_v4();
    let platform_suspend = suspend(
        &service,
        platform_context(tenant_id, platform_actor, "platform-suspend-member"),
        fixture.group_id,
        fixture.member_b_id,
        1,
        "hierarchy_platform_member",
    )
    .await
    .expect("platform groups:moderate user may act without local membership");
    assert_eq!(platform_suspend.membership_revision, 2);
    assert_eq!(platform_suspend.group_version, 8);
    let platform_revoke = revoke(
        &service,
        platform_context(tenant_id, platform_actor, "platform-revoke-member"),
        fixture.group_id,
        fixture.member_b_id,
        2,
        "hierarchy_platform_member_release",
    )
    .await
    .expect("platform groups:moderate user may revoke direct-local suspension");
    assert_eq!(platform_revoke.membership_revision, 3);
    assert_eq!(platform_revoke.group_version, 9);

    assert_eq!(group_snapshot(&db, tenant_id, fixture.group_id).await, (9, 6));
    assert_eq!(
        membership_snapshot(&db, tenant_id, fixture.group_id, fixture.admin_id)
            .await,
        ("admin".to_string(), "active".to_string(), 3)
    );
    assert_eq!(
        membership_snapshot(&db, tenant_id, fixture.group_id, fixture.moderator_id)
            .await,
        ("moderator".to_string(), "active".to_string(), 3)
    );
    for user_id in [fixture.member_a_id, fixture.member_b_id] {
        assert_eq!(
            membership_snapshot(&db, tenant_id, fixture.group_id, user_id)
                .await,
            ("member".to_string(), "active".to_string(), 3)
        );
    }
    assert_eq!(
        membership_snapshot(&db, tenant_id, fixture.group_id, fixture.member_c_id)
            .await,
        ("member".to_string(), "active".to_string(), 1)
    );
    assert_eq!(ledger_counts(&db, tenant_id, fixture.group_id).await, (8, 8, 8));

    drop(service);
    drop(db);
    drop(temp);
}

#[tokio::test]
async fn direct_enforcement_receipt_audit_and_event_lifecycle_is_atomic_sqlite() {
    let temp = tempfile::tempdir().expect("temporary atomicity directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    install_groups_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let fixture = fresh_fixture();
    seed_group(&db, tenant_id, fixture, "enforcement-runtime-atomicity").await;
    let service = GroupMembershipEnforcementCommandService::new(db.clone());

    let suspend_key = "atomic-suspend";
    let suspended = suspend(
        &service,
        write_context(tenant_id, fixture.owner_id, suspend_key),
        fixture.group_id,
        fixture.member_c_id,
        1,
        "atomic_runtime",
    )
    .await
    .expect("owner suspension should commit atomically");
    assert_eq!(suspended.membership_id, fixture.member_c_membership_id);
    assert_eq!(suspended.membership_revision, 2);
    assert_eq!(suspended.group_version, 2);
    assert_eq!(suspended.member_count, 6);
    assert!(!suspended.replayed);
    assert_eq!(ledger_counts(&db, tenant_id, fixture.group_id).await, (1, 1, 1));
    assert_ledger_fact(
        &db,
        tenant_id,
        fixture,
        fixture.member_c_membership_id,
        fixture.owner_id,
        suspend_key,
        "group.membership_suspended",
        "groups.membership.suspended",
        "groups.membership.suspend.v1",
    )
    .await;

    let replay = suspend(
        &service,
        write_context(tenant_id, fixture.owner_id, suspend_key),
        fixture.group_id,
        fixture.member_c_id,
        1,
        "atomic_runtime",
    )
    .await
    .expect("exact suspension receipt should replay");
    assert!(replay.replayed);
    assert_eq!(replay.group_version, suspended.group_version);
    assert_eq!(ledger_counts(&db, tenant_id, fixture.group_id).await, (1, 1, 1));

    let changed_key = suspend(
        &service,
        write_context(tenant_id, fixture.owner_id, suspend_key),
        fixture.group_id,
        fixture.member_c_id,
        1,
        "changed_atomic_runtime",
    )
    .await
    .expect_err("same idempotency key with changed request must conflict");
    assert_eq!(changed_key.kind, PortErrorKind::Conflict);
    assert_eq!(changed_key.code, "groups.conflict");
    assert!(!changed_key.retryable);
    assert_eq!(group_snapshot(&db, tenant_id, fixture.group_id).await, (2, 6));
    assert_eq!(
        membership_snapshot(&db, tenant_id, fixture.group_id, fixture.member_c_id)
            .await
            .2,
        2
    );
    assert_eq!(ledger_counts(&db, tenant_id, fixture.group_id).await, (1, 1, 1));

    let revoke_key = "atomic-revoke";
    let revoked = revoke(
        &service,
        write_context(tenant_id, fixture.owner_id, revoke_key),
        fixture.group_id,
        fixture.member_c_id,
        2,
        "atomic_runtime_release",
    )
    .await
    .expect("owner revoke should commit atomically");
    assert_eq!(revoked.membership_revision, 3);
    assert_eq!(revoked.group_version, 3);
    assert_eq!(revoked.member_count, 6);
    assert!(revoked.revoked_at.is_some());
    assert!(!revoked.replayed);
    assert_eq!(ledger_counts(&db, tenant_id, fixture.group_id).await, (2, 2, 2));
    assert_ledger_fact(
        &db,
        tenant_id,
        fixture,
        fixture.member_c_membership_id,
        fixture.owner_id,
        revoke_key,
        "group.membership_suspension_revoked",
        "groups.membership.suspension_revoked",
        "groups.membership.suspension_revoke.v1",
    )
    .await;

    let revoke_replay = revoke(
        &service,
        write_context(tenant_id, fixture.owner_id, revoke_key),
        fixture.group_id,
        fixture.member_c_id,
        2,
        "atomic_runtime_release",
    )
    .await
    .expect("exact revoke receipt should replay after current enforcement became inactive");
    assert!(revoke_replay.replayed);
    assert_eq!(revoke_replay.group_version, revoked.group_version);
    assert_eq!(ledger_counts(&db, tenant_id, fixture.group_id).await, (2, 2, 2));

    let enforcement = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT source_kind, actor_kind, actor_id, revision, CASE WHEN revoked_at IS NULL THEN 0 ELSE 1 END AS revoked FROM group_membership_enforcements WHERE tenant_id = '{tenant_id}' AND group_id = '{}' AND user_id = '{}'",
                fixture.group_id, fixture.member_c_id
            ),
        ))
        .await
        .expect("final enforcement query should succeed")
        .expect("final enforcement row should exist");
    let source_kind: String = enforcement
        .try_get("", "source_kind")
        .expect("source kind should decode");
    let actor_kind: String = enforcement
        .try_get("", "actor_kind")
        .expect("actor kind should decode");
    let actor_id: String = enforcement
        .try_get("", "actor_id")
        .expect("actor id should decode");
    let enforcement_revision: i64 = enforcement
        .try_get("", "revision")
        .expect("enforcement revision should decode");
    let revoked_marker: i64 = enforcement
        .try_get("", "revoked")
        .expect("revoked marker should decode");
    assert_eq!(source_kind, "direct_local");
    assert_eq!(actor_kind, "user");
    assert_eq!(actor_id, fixture.owner_id.to_string());
    assert_eq!(enforcement_revision, revoked.enforcement_revision);
    assert_eq!(revoked_marker, 1);
    assert_eq!(group_snapshot(&db, tenant_id, fixture.group_id).await, (3, 6));
    assert_eq!(
        membership_snapshot(&db, tenant_id, fixture.group_id, fixture.member_c_id)
            .await,
        ("member".to_string(), "active".to_string(), 3)
    );

    drop(service);
    drop(db);
    drop(temp);
}
