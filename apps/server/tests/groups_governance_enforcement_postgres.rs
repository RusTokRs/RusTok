#![cfg(feature = "mod-groups")]

use std::time::Duration;

use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    ChangeGroupRoleRequest, GroupGovernanceCommandPort, GroupGovernanceService,
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementCommandService, GroupRole,
    SuspendGroupMembershipRequest, TransferGroupOwnershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
    TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const POSTGRES_URL_ENV: &str = "RUSTOK_GROUPS_TEST_POSTGRES_URL";

fn schema_url(base: &str, schema: &str) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}options=-csearch_path%3D{schema}%2Cpublic")
}

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("PostgreSQL Groups evidence connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply in evidence schema");
    }
}

fn user_write_context(
    tenant_id: Uuid,
    user_id: Uuid,
    idempotency_key: impl Into<String>,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        format!("groups-postgres-evidence-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key(idempotency_key)
}

fn platform_write_context(
    tenant_id: Uuid,
    user_id: Uuid,
    idempotency_key: impl Into<String>,
) -> PortContext {
    user_write_context(tenant_id, user_id, idempotency_key).with_claim("groups:manage")
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    admin_id: Uuid,
    replay_target_id: Uuid,
    race_target_id: Uuid,
    replacement_owner_id: Uuid,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', 'governance-evidence', 5);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{admin_id}', 'admin', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{replay_target_id}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{race_target_id}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{replacement_owner_id}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups governance evidence fixture should seed");
}

async fn membership_revision(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("membership revision query should succeed")
        .expect("membership should exist");
    row.try_get("", "revision")
        .expect("membership revision should decode")
}

async fn membership_role(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) -> String {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT role FROM group_memberships WHERE tenant_id = '{tenant_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("membership role query should succeed")
        .expect("membership should exist");
    row.try_get("", "role")
        .expect("membership role should decode")
}

async fn enforcement_count(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT COUNT(*)::BIGINT AS count FROM group_membership_enforcements WHERE tenant_id = '{tenant_id}' AND user_id = '{user_id}' AND revoked_at IS NULL"
            ),
        ))
        .await
        .expect("membership enforcement count query should succeed")
        .expect("enforcement count should exist");
    row.try_get("", "count")
        .expect("enforcement count should decode")
}

async fn group_owner(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> Uuid {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT owner_user_id FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("group owner query should succeed")
        .expect("group should exist");
    row.try_get("", "owner_user_id")
        .expect("group owner UUID should decode")
}

async fn install_moderation_owned_owner_suspension(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
) {
    let transaction = db
        .begin()
        .await
        .expect("owner recovery fixture transaction should begin");
    let row = transaction
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT id FROM group_memberships WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{owner_id}'"
            ),
        ))
        .await
        .expect("owner membership lookup should succeed")
        .expect("owner membership should exist");
    let membership_id: Uuid = row
        .try_get("", "id")
        .expect("owner membership ID should decode");
    let decision_id = Uuid::new_v4();
    transaction
        .execute_unprepared(&format!(
            r#"
INSERT INTO group_membership_enforcements
    (membership_id, tenant_id, group_id, user_id, state, reason_code, source_kind,
     effective_from, restore_status, moderation_decision_id, moderation_decision_hash,
     actor_kind, actor_id)
VALUES
    ('{membership_id}', '{tenant_id}', '{group_id}', '{owner_id}', 'suspended',
     'harassment', 'moderation_decision', CURRENT_TIMESTAMP - INTERVAL '1 second', 'active',
     '{decision_id}', '{}', 'service', 'rustok-moderation');
UPDATE groups
   SET version = version + 1,
       updated_at = CURRENT_TIMESTAMP
 WHERE tenant_id = '{tenant_id}'
   AND id = '{group_id}';
"#,
            "a".repeat(64),
        ))
        .await
        .expect("moderation-owned owner suspension fixture should install");
    transaction
        .commit()
        .await
        .expect("owner recovery fixture transaction should commit");
}

#[tokio::test]
#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]
async fn postgres_groups_governance_and_enforcement_are_replay_safe_serialized_and_recoverable() {
    let base_url = std::env::var(POSTGRES_URL_ENV)
        .expect("RUSTOK_GROUPS_TEST_POSTGRES_URL must be configured");
    let schema = format!("groups_governance_evidence_{}", Uuid::new_v4().simple());
    let admin = connect(&base_url).await;
    admin
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .expect("isolated Groups evidence schema should create");
    let scoped_url = schema_url(&base_url, &schema);
    let fixture = connect(&scoped_url).await;
    install_groups_schema(&fixture).await;

    let tenant_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    let replay_target_id = Uuid::new_v4();
    let race_target_id = Uuid::new_v4();
    let replacement_owner_id = Uuid::new_v4();
    let platform_user_id = Uuid::new_v4();
    seed_group_fixture(
        &fixture,
        tenant_id,
        group_id,
        owner_id,
        admin_id,
        replay_target_id,
        race_target_id,
        replacement_owner_id,
    )
    .await;

    // A completed governance receipt must replay before current effective authorization. The admin
    // first changes a member role, is then suspended by the owner, and finally replays the exact
    // lost-response request successfully despite no longer having current authority.
    let replay_key = format!("governance-replay-{}", Uuid::new_v4());
    let replay_request = ChangeGroupRoleRequest {
        group_id,
        target_user_id: replay_target_id,
        role: GroupRole::Moderator,
    };
    let first_governance = GroupGovernanceService::new(connect(&scoped_url).await);
    let first = GroupGovernanceCommandPort::change_group_role(
        &first_governance,
        user_write_context(tenant_id, admin_id, replay_key.clone()),
        replay_request.clone(),
    )
    .await
    .expect("active admin should change member role");
    assert!(!first.replayed);
    assert_eq!(first.previous_role, GroupRole::Member);
    assert_eq!(first.current_role, GroupRole::Moderator);

    let admin_revision = membership_revision(&fixture, tenant_id, admin_id).await;
    let enforcement = GroupMembershipEnforcementCommandService::new(connect(&scoped_url).await);
    GroupMembershipEnforcementCommandPort::suspend_membership(
        &enforcement,
        user_write_context(
            tenant_id,
            owner_id,
            format!("suspend-admin-{}", Uuid::new_v4()),
        ),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: admin_id,
            expected_membership_revision: admin_revision,
            reason_code: "harassment".to_string(),
            effective_until: None,
        },
    )
    .await
    .expect("owner should suspend admin after governance commit");

    let replay_governance = GroupGovernanceService::new(connect(&scoped_url).await);
    let replayed = GroupGovernanceCommandPort::change_group_role(
        &replay_governance,
        user_write_context(tenant_id, admin_id, replay_key.clone()),
        replay_request.clone(),
    )
    .await
    .expect("matching lost-response receipt should replay before suspended admin authorization");
    assert!(replayed.replayed);
    assert_eq!(replayed.previous_role, first.previous_role);
    assert_eq!(replayed.current_role, first.current_role);
    assert_eq!(replayed.group_version, first.group_version);

    let wrong_actor_governance = GroupGovernanceService::new(connect(&scoped_url).await);
    let wrong_actor = GroupGovernanceCommandPort::change_group_role(
        &wrong_actor_governance,
        user_write_context(tenant_id, owner_id, replay_key),
        replay_request.clone(),
    )
    .await
    .expect_err("another actor must not reuse the completed admin receipt");
    assert_eq!(wrong_actor.code, "groups.conflict");

    let suspended_admin_governance = GroupGovernanceService::new(connect(&scoped_url).await);
    let suspended_admin = GroupGovernanceCommandPort::change_group_role(
        &suspended_admin_governance,
        user_write_context(
            tenant_id,
            admin_id,
            format!("fresh-governance-{}", Uuid::new_v4()),
        ),
        ChangeGroupRoleRequest {
            group_id,
            target_user_id: replay_target_id,
            role: GroupRole::Member,
        },
    )
    .await
    .expect_err("fresh governance must re-check the suspended admin effective state");
    assert_eq!(suspended_admin.code, "groups.membership_suspended");

    // Governance role mutation and direct suspension share the group serialization row. Racing the
    // two from independent connections can only serialize in one of two safe ways: the role wins
    // and makes the prepared suspension revision stale, or suspension wins and governance observes
    // the effective denial. They must never both commit against the same reviewed revision.
    let race_revision = membership_revision(&fixture, tenant_id, race_target_id).await;
    let race_governance = GroupGovernanceService::new(connect(&scoped_url).await);
    let race_enforcement =
        GroupMembershipEnforcementCommandService::new(connect(&scoped_url).await);
    let role_future = GroupGovernanceCommandPort::change_group_role(
        &race_governance,
        user_write_context(tenant_id, owner_id, format!("race-role-{}", Uuid::new_v4())),
        ChangeGroupRoleRequest {
            group_id,
            target_user_id: race_target_id,
            role: GroupRole::Moderator,
        },
    );
    let suspension_future = GroupMembershipEnforcementCommandPort::suspend_membership(
        &race_enforcement,
        user_write_context(
            tenant_id,
            owner_id,
            format!("race-suspend-{}", Uuid::new_v4()),
        ),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: race_target_id,
            expected_membership_revision: race_revision,
            reason_code: "harassment".to_string(),
            effective_until: None,
        },
    );
    let (role_result, suspension_result) = tokio::join!(role_future, suspension_future);
    match (role_result, suspension_result) {
        (Ok(role), Err(suspension_error)) => {
            assert_eq!(role.current_role, GroupRole::Moderator);
            assert_eq!(
                suspension_error.code,
                "groups.membership_enforcement_revision_conflict"
            );
            assert_eq!(
                membership_role(&fixture, tenant_id, race_target_id).await,
                "moderator"
            );
            assert_eq!(
                enforcement_count(&fixture, tenant_id, race_target_id).await,
                0
            );
        }
        (Err(role_error), Ok(suspension)) => {
            assert_eq!(role_error.code, "groups.membership_suspended");
            assert_eq!(suspension.user_id, race_target_id);
            assert_eq!(
                membership_role(&fixture, tenant_id, race_target_id).await,
                "member"
            );
            assert_eq!(
                enforcement_count(&fixture, tenant_id, race_target_id).await,
                1
            );
        }
        (role_result, suspension_result) => panic!(
            "governance/enforcement race must produce exactly one safe commit: role={role_result:?}, suspension={suspension_result:?}"
        ),
    }
    assert_eq!(
        membership_revision(&fixture, tenant_id, race_target_id).await,
        race_revision + 1,
        "exactly one serialized material change should advance the raced membership revision"
    );

    // Platform recovery is allowed to transfer away from a suspended current owner, but the owner
    // enforcement row is still resolved fail-closed and the replacement owner must be effective
    // active. This fixture writes the future moderation-owned projection shape directly because the
    // neutral Groups Moderation adapter is intentionally not part of this evidence slice.
    install_moderation_owned_owner_suspension(&fixture, tenant_id, group_id, owner_id).await;
    let platform_governance = GroupGovernanceService::new(connect(&scoped_url).await);
    let recovered = GroupGovernanceCommandPort::transfer_group_ownership(
        &platform_governance,
        platform_write_context(
            tenant_id,
            platform_user_id,
            format!("platform-recovery-{}", Uuid::new_v4()),
        ),
        TransferGroupOwnershipRequest {
            group_id,
            new_owner_user_id: replacement_owner_id,
        },
    )
    .await
    .expect("platform manager should recover ownership from a valid suspended current owner");
    assert_eq!(recovered.current_role, GroupRole::Owner);
    assert_eq!(recovered.target_user_id, replacement_owner_id);
    assert_eq!(
        group_owner(&fixture, tenant_id, group_id).await,
        replacement_owner_id
    );
    assert_eq!(
        membership_role(&fixture, tenant_id, owner_id).await,
        "admin"
    );
    assert_eq!(
        membership_role(&fixture, tenant_id, replacement_owner_id).await,
        "owner"
    );

    drop(race_governance);
    drop(race_enforcement);
    drop(first_governance);
    drop(enforcement);
    drop(replay_governance);
    drop(wrong_actor_governance);
    drop(suspended_admin_governance);
    drop(platform_governance);
    drop(fixture);
    admin
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .expect("isolated Groups evidence schema should drop");
}
