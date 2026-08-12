#![cfg(feature = "mod-groups")]

use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    GroupCommandPort, GroupMembershipEffectiveStatus, GroupMembershipEnforcementCommandPort,
    GroupMembershipEnforcementCommandService, GroupMembershipEnforcementReadPort,
    GroupMembershipEnforcementService, GroupMembershipStatus, GroupsService, JoinGroupRequest,
    ReadGroupMembershipEnforcementRequest, SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use tempfile::TempDir;
use tokio::sync::Barrier;
use uuid::Uuid;

const SQLITE_BUSY_TIMEOUT_MS: u64 = 5_000;
const ROUNDS: usize = 8;

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp.path().join("groups-join-enforcement.sqlite");
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
        .expect("Groups join enforcement SQLite connection should open");
    db.execute_unprepared(&format!("PRAGMA busy_timeout = {SQLITE_BUSY_TIMEOUT_MS};"))
        .await
        .expect("Groups join enforcement SQLite connection should configure busy timeout");
    db
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for join enforcement evidence");
    }
}

async fn seed_left_member(
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
    ('{group_id}', '{tenant_id}', '{owner_id}', '{handle}', 'public', 'open', 'active', 1);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at, left_at)
VALUES
    ('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP, NULL),
    ('{}', '{tenant_id}', '{group_id}', '{user_id}', 'member', 'left', NULL, CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups join enforcement SQLite fixture should seed");
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, operation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("groups-join-enforcement-{operation}-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key(format!("{operation}-{}", Uuid::new_v4()))
}

fn enforcement_read_context(tenant_id: Uuid, owner_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(owner_id.to_string()),
        "en",
        format!("groups-join-enforcement-read-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_claim("groups:access:read")
}

async fn group_snapshot(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> (i64, i64) {
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
        .query_one(Statement::from_string(
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
        .query_one(Statement::from_string(
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
async fn join_is_denied_until_suspension_expires_without_cleanup_sqlite() {
    let temp = tempfile::tempdir().expect("temporary Groups join evidence directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    db.execute_unprepared("PRAGMA journal_mode = WAL;")
        .await
        .expect("Groups join enforcement SQLite fixture should enable WAL");
    install_groups_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    seed_left_member(&db, tenant_id, group_id, owner_id, user_id, "join-expiry").await;

    let (base_version, base_member_count) = group_snapshot(&db, tenant_id, group_id).await;
    assert_eq!(base_member_count, 1);
    assert_eq!(
        membership_snapshot(&db, tenant_id, group_id, user_id).await,
        ("left".to_string(), 1)
    );

    let expires_at = Utc::now() + ChronoDuration::milliseconds(500);
    let enforcement = GroupMembershipEnforcementCommandService::new(db.clone());
    let suspended = GroupMembershipEnforcementCommandPort::suspend_membership(
        &enforcement,
        write_context(tenant_id, owner_id, "expiry-suspend"),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: user_id,
            expected_membership_revision: 1,
            reason_code: "harassment".to_string(),
            effective_until: Some(expires_at),
        },
    )
    .await
    .expect("owner should suspend the left membership before re-entry");
    assert_eq!(suspended.group_version, base_version + 1);
    assert_eq!(suspended.member_count, 1);
    assert_eq!(suspended.membership_revision, 2);
    assert_eq!(
        membership_snapshot(&db, tenant_id, group_id, user_id).await,
        ("left".to_string(), 2),
        "temporary suspension must not rewrite stored lifecycle state"
    );

    let groups = GroupsService::new(db.clone());
    let blocked = GroupCommandPort::join_group(
        &groups,
        write_context(tenant_id, user_id, "blocked-join"),
        JoinGroupRequest { group_id },
    )
    .await
    .expect_err("effective suspension must deny re-entry");
    assert_eq!(blocked.code, "groups.membership_suspended");
    assert!(!blocked.retryable);
    assert_eq!(
        group_snapshot(&db, tenant_id, group_id).await,
        (suspended.group_version, 1),
        "denied re-entry must not change aggregate lifecycle state"
    );
    assert_eq!(
        membership_snapshot(&db, tenant_id, group_id, user_id).await,
        ("left".to_string(), 2)
    );

    let reader = GroupMembershipEnforcementService::new(db.clone());
    let during = GroupMembershipEnforcementReadPort::read_membership_enforcement(
        &reader,
        enforcement_read_context(tenant_id, owner_id),
        ReadGroupMembershipEnforcementRequest { group_id, user_id },
    )
    .await
    .expect("owner should read suspended re-entry state");
    assert_eq!(
        during.effective_status,
        GroupMembershipEffectiveStatus::Suspended
    );

    let remaining = expires_at
        .signed_duration_since(Utc::now())
        .to_std()
        .unwrap_or_default();
    tokio::time::sleep(remaining + Duration::from_millis(100)).await;

    let expired = GroupMembershipEnforcementReadPort::read_membership_enforcement(
        &reader,
        enforcement_read_context(tenant_id, owner_id),
        ReadGroupMembershipEnforcementRequest { group_id, user_id },
    )
    .await
    .expect("owner-clock expiry should fall back to stored left lifecycle state");
    assert_eq!(
        expired.effective_status,
        GroupMembershipEffectiveStatus::Inactive
    );
    assert_eq!(expired.membership_revision, Some(2));

    let joined = GroupCommandPort::join_group(
        &groups,
        write_context(tenant_id, user_id, "join-after-expiry"),
        JoinGroupRequest { group_id },
    )
    .await
    .expect("expired suspension should allow re-entry without cleanup or revoke");
    assert_eq!(joined.status, GroupMembershipStatus::Active);
    assert_eq!(
        membership_snapshot(&db, tenant_id, group_id, user_id).await,
        ("active".to_string(), 3),
        "lifecycle re-entry after expiry must own the next membership revision"
    );
    assert_eq!(
        group_snapshot(&db, tenant_id, group_id).await,
        (suspended.group_version + 1, 2)
    );

    drop(reader);
    drop(groups);
    drop(enforcement);
    drop(db);
    drop(temp);
}

#[tokio::test]
async fn join_and_suspension_serialize_on_sqlite_group_writer() {
    let temp = tempfile::tempdir().expect("temporary Groups join race directory should create");
    let url = sqlite_fixture_url(&temp);
    let fixture_db = connect(&url).await;
    fixture_db
        .execute_unprepared("PRAGMA journal_mode = WAL;")
        .await
        .expect("Groups join race SQLite fixture should enable WAL");
    install_groups_schema(&fixture_db).await;

    for round in 0..ROUNDS {
        let tenant_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        seed_left_member(
            &fixture_db,
            tenant_id,
            group_id,
            owner_id,
            user_id,
            &format!("join-race-{round}"),
        )
        .await;
        let (base_version, base_member_count) =
            group_snapshot(&fixture_db, tenant_id, group_id).await;
        assert_eq!(base_member_count, 1);

        let join_db = connect(&url).await;
        let enforcement_db = connect(&url).await;
        let barrier = Arc::new(Barrier::new(3));

        let join_barrier = barrier.clone();
        let join_task = tokio::spawn(async move {
            let service = GroupsService::new(join_db);
            join_barrier.wait().await;
            GroupCommandPort::join_group(
                &service,
                write_context(tenant_id, user_id, "raced-join"),
                JoinGroupRequest { group_id },
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
        let join_result = join_task
            .await
            .expect("SQLite join race task should join without panic");
        let suspension_result = enforcement_task
            .await
            .expect("SQLite enforcement race task should join without panic");

        match (join_result, suspension_result) {
            (Ok(joined), Err(error)) => {
                assert_eq!(joined.status, GroupMembershipStatus::Active);
                assert_eq!(
                    error.code,
                    "groups.membership_enforcement_revision_conflict"
                );
                assert!(!error.retryable);
                assert_eq!(
                    membership_snapshot(&fixture_db, tenant_id, group_id, user_id).await,
                    ("active".to_string(), 2)
                );
                assert_eq!(
                    active_enforcement_count(&fixture_db, tenant_id, group_id, user_id).await,
                    0
                );
                assert_eq!(
                    group_snapshot(&fixture_db, tenant_id, group_id).await,
                    (base_version + 1, 2)
                );
            }
            (Err(error), Ok(suspension)) => {
                assert_eq!(error.code, "groups.membership_suspended");
                assert!(!error.retryable);
                assert_eq!(suspension.membership_revision, 2);
                assert_eq!(suspension.member_count, 1);
                assert_eq!(
                    membership_snapshot(&fixture_db, tenant_id, group_id, user_id).await,
                    ("left".to_string(), 2)
                );
                assert_eq!(
                    active_enforcement_count(&fixture_db, tenant_id, group_id, user_id).await,
                    1
                );
                assert_eq!(
                    group_snapshot(&fixture_db, tenant_id, group_id).await,
                    (base_version + 1, 1)
                );
            }
            (join_result, suspension_result) => panic!(
                "SQLite join/enforcement race must have exactly one material winner: join={join_result:?}, suspension={suspension_result:?}"
            ),
        }

        assert_eq!(
            membership_snapshot(&fixture_db, tenant_id, group_id, user_id)
                .await
                .1,
            2,
            "each race target must have exactly one material membership revision advance"
        );
        assert_eq!(
            group_snapshot(&fixture_db, tenant_id, group_id).await.0,
            base_version + 1,
            "exactly one material owner mutation must advance the group aggregate"
        );
    }

    drop(fixture_db);
    drop(temp);
}
