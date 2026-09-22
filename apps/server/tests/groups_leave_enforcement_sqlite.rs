#![cfg(feature = "mod-groups")]

use std::sync::Arc;
use std::time::Duration;

use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    GroupCommandPort, GroupMembershipEffectiveStatus, GroupMembershipEnforcementCommandPort,
    GroupMembershipEnforcementCommandService, GroupMembershipEnforcementReadPort,
    GroupMembershipEnforcementService, GroupMembershipStatus, GroupsService, LeaveGroupRequest,
    ReadGroupMembershipEnforcementRequest, RevokeGroupMembershipSuspensionRequest,
    SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use tempfile::TempDir;
use tokio::sync::Barrier;
use uuid::Uuid;

const SQLITE_BUSY_TIMEOUT_MS: u64 = 5_000;
const ROUNDS: usize = 8;

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp.path().join("groups-leave-enforcement.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("Groups leave enforcement SQLite connection should open");
    db.execute_unprepared(&format!("PRAGMA busy_timeout = {SQLITE_BUSY_TIMEOUT_MS};"))
        .await
        .expect("Groups leave enforcement SQLite connection should configure busy timeout");
    db
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for leave enforcement evidence");
    }
}

async fn seed_active_member(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    user_id: Uuid,
    handle: &str,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups
    (id, tenant_id, owner_user_id, handle, visibility, join_policy, status, member_count)
VALUES
    ('{group_id}', '{tenant_id}', '{owner_id}', '{handle}', 'public', 'open', 'active', 2);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{user_id}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups leave enforcement active fixture should seed");
}

async fn seed_banned_member(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    user_id: Uuid,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups
    (id, tenant_id, owner_user_id, handle, visibility, join_policy, status, member_count)
VALUES
    ('{group_id}', '{tenant_id}', '{owner_id}', 'leave-banned', 'public', 'open', 'active', 1);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{user_id}', 'member', 'banned', NULL);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups leave enforcement banned fixture should seed");
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, operation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("groups-leave-enforcement-{operation}-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key(format!("{operation}-{}", Uuid::new_v4()))
}

fn read_context(tenant_id: Uuid, owner_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(owner_id.to_string()),
        "en",
        format!("groups-leave-enforcement-read-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_claim("groups:access:read")
}

async fn group_snapshot(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> (i64, i64) {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT version, member_count FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("group snapshot query should succeed")
        .expect("group should exist");
    (
        row.try_get("", "version")
            .expect("group version should decode"),
        row.try_get("", "member_count")
            .expect("group member_count should decode"),
    )
}

async fn membership_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> (String, i64) {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT status, revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("membership snapshot query should succeed")
        .expect("membership should exist");
    (
        row.try_get("", "status")
            .expect("membership status should decode"),
        row.try_get("", "revision")
            .expect("membership revision should decode"),
    )
}

async fn active_enforcement_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT COUNT(*) AS count FROM group_membership_enforcements WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}' AND revoked_at IS NULL"
            ),
        ))
        .await
        .expect("enforcement count query should succeed")
        .expect("enforcement count should exist");
    row.try_get("", "count")
        .expect("enforcement count should decode")
}

#[tokio::test]
async fn leave_preserves_legacy_ban_and_suspension_projection_sqlite() {
    let temp =
        tempfile::tempdir().expect("temporary Groups leave evidence directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    db.execute_unprepared("PRAGMA journal_mode = WAL;")
        .await
        .expect("Groups leave enforcement SQLite fixture should enable WAL");
    install_groups_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let banned_group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let banned_user_id = Uuid::new_v4();
    seed_banned_member(&db, tenant_id, banned_group_id, owner_id, banned_user_id).await;
    let banned_base = group_snapshot(&db, tenant_id, banned_group_id).await;
    let groups = GroupsService::new(db.clone());
    let banned_error = GroupCommandPort::leave_group(
        &groups,
        write_context(tenant_id, banned_user_id, "banned-leave"),
        LeaveGroupRequest {
            group_id: banned_group_id,
        },
    )
    .await
    .expect_err("legacy banned membership must not be rewritten to left");
    assert_eq!(banned_error.code, "groups.membership_banned");
    assert!(!banned_error.retryable);
    assert_eq!(
        membership_snapshot(&db, tenant_id, banned_group_id, banned_user_id).await,
        ("banned".to_string(), 1)
    );
    assert_eq!(
        group_snapshot(&db, tenant_id, banned_group_id).await,
        banned_base
    );

    let suspended_group_id = Uuid::new_v4();
    let suspended_user_id = Uuid::new_v4();
    seed_active_member(
        &db,
        tenant_id,
        suspended_group_id,
        owner_id,
        suspended_user_id,
        "leave-suspended",
    )
    .await;
    let suspended_base = group_snapshot(&db, tenant_id, suspended_group_id).await;
    let enforcement = GroupMembershipEnforcementCommandService::new(db.clone());
    let suspended = GroupMembershipEnforcementCommandPort::suspend_membership(
        &enforcement,
        write_context(tenant_id, owner_id, "suspend-before-leave"),
        SuspendGroupMembershipRequest {
            group_id: suspended_group_id,
            target_user_id: suspended_user_id,
            expected_membership_revision: 1,
            reason_code: "harassment".to_string(),
            effective_until: None,
        },
    )
    .await
    .expect("owner should suspend active member before leave");
    assert_eq!(suspended.membership_revision, 2);
    assert_eq!(suspended.member_count, 2);
    assert_eq!(suspended.group_version, suspended_base.0 + 1);

    let left = GroupCommandPort::leave_group(
        &groups,
        write_context(tenant_id, suspended_user_id, "leave-while-suspended"),
        LeaveGroupRequest {
            group_id: suspended_group_id,
        },
    )
    .await
    .expect("temporary suspension must not prevent a non-owner from leaving");
    assert_eq!(left.status, GroupMembershipStatus::Left);
    assert_eq!(
        membership_snapshot(&db, tenant_id, suspended_group_id, suspended_user_id).await,
        ("left".to_string(), 3)
    );
    assert_eq!(
        group_snapshot(&db, tenant_id, suspended_group_id).await,
        (suspended.group_version + 1, 1)
    );
    assert_eq!(
        active_enforcement_count(&db, tenant_id, suspended_group_id, suspended_user_id).await,
        1,
        "leaving must preserve the active enforcement projection"
    );

    let reader = GroupMembershipEnforcementService::new(db.clone());
    let while_suspended = GroupMembershipEnforcementReadPort::read_membership_enforcement(
        &reader,
        read_context(tenant_id, owner_id),
        ReadGroupMembershipEnforcementRequest {
            group_id: suspended_group_id,
            user_id: suspended_user_id,
        },
    )
    .await
    .expect("owner should observe preserved suspension after leave");
    assert_eq!(
        while_suspended.effective_status,
        GroupMembershipEffectiveStatus::Suspended
    );
    assert_eq!(
        while_suspended.stored_status,
        Some(GroupMembershipStatus::Left)
    );

    let revoked = GroupMembershipEnforcementCommandPort::revoke_membership_suspension(
        &enforcement,
        write_context(tenant_id, owner_id, "revoke-after-leave"),
        RevokeGroupMembershipSuspensionRequest {
            group_id: suspended_group_id,
            target_user_id: suspended_user_id,
            expected_membership_revision: 3,
            reason_code: "review_complete".to_string(),
        },
    )
    .await
    .expect("owner should revoke preserved direct-local suspension after leave");
    assert_eq!(
        revoked.effective_status,
        GroupMembershipEffectiveStatus::Inactive
    );
    assert_eq!(revoked.membership_revision, 4);
    assert_eq!(revoked.member_count, 1);
    assert_eq!(
        membership_snapshot(&db, tenant_id, suspended_group_id, suspended_user_id).await,
        ("left".to_string(), 4)
    );

    drop(reader);
    drop(enforcement);
    drop(groups);
    drop(db);
    drop(temp);
}

#[tokio::test]
async fn leave_and_suspension_serialize_on_sqlite_group_writer() {
    let temp = tempfile::tempdir().expect("temporary Groups leave race directory should create");
    let url = sqlite_fixture_url(&temp);
    let fixture_db = connect(&url).await;
    fixture_db
        .execute_unprepared("PRAGMA journal_mode = WAL;")
        .await
        .expect("Groups leave race SQLite fixture should enable WAL");
    install_groups_schema(&fixture_db).await;

    for round in 0..ROUNDS {
        let tenant_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        seed_active_member(
            &fixture_db,
            tenant_id,
            group_id,
            owner_id,
            user_id,
            &format!("leave-race-{round}"),
        )
        .await;
        let base = group_snapshot(&fixture_db, tenant_id, group_id).await;
        assert_eq!(base.1, 2);

        let leave_db = connect(&url).await;
        let enforcement_db = connect(&url).await;
        let barrier = Arc::new(Barrier::new(3));

        let leave_barrier = barrier.clone();
        let leave_task = tokio::spawn(async move {
            let service = GroupsService::new(leave_db);
            leave_barrier.wait().await;
            GroupCommandPort::leave_group(
                &service,
                write_context(tenant_id, user_id, "raced-leave"),
                LeaveGroupRequest { group_id },
            )
            .await
        });

        let enforcement_barrier = barrier.clone();
        let enforcement_task = tokio::spawn(async move {
            let service = GroupMembershipEnforcementCommandService::new(enforcement_db);
            enforcement_barrier.wait().await;
            GroupMembershipEnforcementCommandPort::suspend_membership(
                &service,
                write_context(tenant_id, owner_id, "raced-suspension"),
                SuspendGroupMembershipRequest {
                    group_id,
                    target_user_id: user_id,
                    expected_membership_revision: 1,
                    reason_code: "concurrent_settings_review".to_string(),
                    effective_until: None,
                },
            )
            .await
        });

        barrier.wait().await;
        let left = leave_task
            .await
            .expect("SQLite leave race task should join without panic")
            .expect("leave must succeed whether it serializes before or after suspension");
        assert_eq!(left.status, GroupMembershipStatus::Left);
        let suspension_result = enforcement_task
            .await
            .expect("SQLite enforcement race task should join without panic");

        match suspension_result {
            Err(error) => {
                assert_eq!(
                    error.code,
                    "groups.membership_enforcement_revision_conflict"
                );
                assert!(!error.retryable);
                assert_eq!(
                    membership_snapshot(&fixture_db, tenant_id, group_id, user_id).await,
                    ("left".to_string(), 2)
                );
                assert_eq!(
                    active_enforcement_count(&fixture_db, tenant_id, group_id, user_id).await,
                    0
                );
                assert_eq!(
                    group_snapshot(&fixture_db, tenant_id, group_id).await,
                    (base.0 + 1, 1)
                );
            }
            Ok(suspension) => {
                assert_eq!(suspension.membership_revision, 2);
                assert_eq!(suspension.member_count, 2);
                assert_eq!(
                    membership_snapshot(&fixture_db, tenant_id, group_id, user_id).await,
                    ("left".to_string(), 3)
                );
                assert_eq!(
                    active_enforcement_count(&fixture_db, tenant_id, group_id, user_id).await,
                    1
                );
                assert_eq!(
                    group_snapshot(&fixture_db, tenant_id, group_id).await,
                    (base.0 + 2, 1)
                );
            }
        }
    }

    drop(fixture_db);
    drop(temp);
}
