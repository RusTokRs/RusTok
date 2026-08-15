#![cfg(feature = "mod-groups")]

use std::time::Duration;

use chrono::Utc;
use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    ChangeGroupRoleRequest, GroupGovernanceCommandPort, GroupGovernanceService,
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementCommandService, GroupRole,
    SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use tempfile::TempDir;
use uuid::Uuid;

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("Groups governance expiry SQLite connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for governance expiry evidence");
    }
}

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp
        .path()
        .join("groups-governance-suspension-expiry.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    admin_id: Uuid,
    first_target_id: Uuid,
    second_target_id: Uuid,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', 'governance-suspension-expiry', 4);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{admin_id}', 'admin', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{first_target_id}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{second_target_id}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups governance expiry fixture should seed");
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, operation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("groups-governance-expiry-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key(format!("{operation}-{}", Uuid::new_v4()))
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
        row.try_get("", "role")
            .expect("membership role should decode"),
        row.try_get("", "status")
            .expect("membership status should decode"),
        row.try_get("", "revision")
            .expect("membership revision should decode"),
    )
}

async fn group_member_count(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> i64 {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT member_count FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("group member_count query should succeed")
        .expect("group should exist");
    row.try_get("", "member_count")
        .expect("group member_count should decode")
}

#[tokio::test]
async fn governance_authority_follows_temporary_suspension_and_owner_clock_expiry_sqlite() {
    let temp =
        tempfile::tempdir().expect("temporary Groups governance expiry directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    install_groups_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    let first_target_id = Uuid::new_v4();
    let second_target_id = Uuid::new_v4();
    seed_group_fixture(
        &db,
        tenant_id,
        group_id,
        owner_id,
        admin_id,
        first_target_id,
        second_target_id,
    )
    .await;

    let governance = GroupGovernanceService::new(db.clone());
    let baseline = GroupGovernanceCommandPort::change_group_role(
        &governance,
        write_context(tenant_id, admin_id, "admin-baseline-role-change"),
        ChangeGroupRoleRequest {
            group_id,
            target_user_id: first_target_id,
            role: GroupRole::Moderator,
        },
    )
    .await
    .expect("active administrator should change a member role before suspension");
    assert_eq!(baseline.previous_role, GroupRole::Member);
    assert_eq!(baseline.current_role, GroupRole::Moderator);
    assert!(!baseline.replayed);

    let (admin_role_before, admin_status_before, admin_revision_before) =
        membership_snapshot(&db, tenant_id, group_id, admin_id).await;
    assert_eq!(admin_role_before, "admin");
    assert_eq!(admin_status_before, "active");
    assert_eq!(group_member_count(&db, tenant_id, group_id).await, 4);

    let enforcement = GroupMembershipEnforcementCommandService::new(db.clone());
    let effective_until = Utc::now() + chrono::Duration::seconds(2);
    let suspended = GroupMembershipEnforcementCommandPort::suspend_membership(
        &enforcement,
        write_context(tenant_id, owner_id, "owner-suspend-admin"),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: admin_id,
            expected_membership_revision: admin_revision_before,
            reason_code: "temporary_governance_review".to_string(),
            effective_until: Some(effective_until),
        },
    )
    .await
    .expect("owner should temporarily suspend the administrator");
    assert_eq!(suspended.membership_revision, admin_revision_before + 1);
    assert_eq!(suspended.member_count, 4);
    assert!(suspended.group_version > baseline.group_version as i64);

    let (admin_role_during, admin_status_during, admin_revision_during) =
        membership_snapshot(&db, tenant_id, group_id, admin_id).await;
    assert_eq!(admin_role_during, "admin");
    assert_eq!(admin_status_during, "active");
    assert_eq!(admin_revision_during, suspended.membership_revision);

    let denied = GroupGovernanceCommandPort::change_group_role(
        &governance,
        write_context(tenant_id, admin_id, "suspended-admin-role-change"),
        ChangeGroupRoleRequest {
            group_id,
            target_user_id: second_target_id,
            role: GroupRole::Moderator,
        },
    )
    .await
    .expect_err("suspended administrator must not retain governance authority");
    assert_eq!(denied.code, "groups.membership_suspended");
    assert!(!denied.retryable);

    let (blocked_target_role, blocked_target_status, blocked_target_revision) =
        membership_snapshot(&db, tenant_id, group_id, second_target_id).await;
    assert_eq!(blocked_target_role, "member");
    assert_eq!(blocked_target_status, "active");
    assert_eq!(blocked_target_revision, 1);
    assert_eq!(group_member_count(&db, tenant_id, group_id).await, 4);

    tokio::time::sleep(Duration::from_millis(2300)).await;

    let restored = GroupGovernanceCommandPort::change_group_role(
        &governance,
        write_context(tenant_id, admin_id, "restored-admin-role-change"),
        ChangeGroupRoleRequest {
            group_id,
            target_user_id: second_target_id,
            role: GroupRole::Moderator,
        },
    )
    .await
    .expect("expired suspension should restore administrator governance authority without cleanup");
    assert_eq!(restored.previous_role, GroupRole::Member);
    assert_eq!(restored.current_role, GroupRole::Moderator);
    assert!(!restored.replayed);
    assert_eq!(restored.group_version, suspended.group_version as u64 + 1);

    let (admin_role_after, admin_status_after, admin_revision_after) =
        membership_snapshot(&db, tenant_id, group_id, admin_id).await;
    assert_eq!(admin_role_after, "admin");
    assert_eq!(admin_status_after, "active");
    assert_eq!(admin_revision_after, suspended.membership_revision);

    let (restored_target_role, restored_target_status, restored_target_revision) =
        membership_snapshot(&db, tenant_id, group_id, second_target_id).await;
    assert_eq!(restored_target_role, "moderator");
    assert_eq!(restored_target_status, "active");
    assert_eq!(restored_target_revision, blocked_target_revision + 1);
    assert_eq!(group_member_count(&db, tenant_id, group_id).await, 4);

    drop(enforcement);
    drop(governance);
    drop(db);
    drop(temp);
}
