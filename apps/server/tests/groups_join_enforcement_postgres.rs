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
use sea_orm_migration::SchemaManager;
use tokio::sync::Barrier;
use uuid::Uuid;

const POSTGRES_URL_ENV: &str = "RUSTOK_GROUPS_TEST_POSTGRES_URL";
const ROUNDS: usize = 8;
const PAIR_TIMEOUT: Duration = Duration::from_secs(30);

fn schema_url(base: &str, schema: &str) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}options=-csearch_path%3D{schema}%2Cpublic")
}

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("Groups join enforcement PostgreSQL connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for PostgreSQL join evidence");
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
    .expect("Groups join enforcement PostgreSQL fixture should seed");
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, operation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("groups-join-postgres-{operation}-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(20))
    .with_idempotency_key(format!("postgres-{operation}-{}", Uuid::new_v4()))
}

fn enforcement_read_context(tenant_id: Uuid, owner_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(owner_id.to_string()),
        "en",
        format!("groups-join-postgres-read-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_claim("groups:access:read")
}

async fn group_snapshot(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> (i64, i64) {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT version, member_count FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("PostgreSQL group snapshot query should succeed")
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
            DatabaseBackend::Postgres,
            format!(
                "SELECT status, revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("PostgreSQL membership snapshot query should succeed")
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
            DatabaseBackend::Postgres,
            format!(
                "SELECT COUNT(*) AS count FROM group_membership_enforcements WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}' AND revoked_at IS NULL"
            ),
        ))
        .await
        .expect("PostgreSQL enforcement count query should succeed")
        .expect("enforcement count should exist");
    row.try_get("", "count")
        .expect("enforcement count should decode")
}

#[tokio::test]
#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]
async fn join_suspension_expiry_and_contention_share_owner_serialization_postgres() {
    let base_url = std::env::var(POSTGRES_URL_ENV)
        .expect("RUSTOK_GROUPS_TEST_POSTGRES_URL must be configured");
    let schema_name = format!("groups_join_enforcement_{}", Uuid::new_v4().simple());
    let admin_db = connect(&base_url).await;
    admin_db
        .execute_unprepared(&format!("CREATE SCHEMA {schema_name}"))
        .await
        .expect("isolated Groups join enforcement schema should create");
    let scoped_url = schema_url(&base_url, &schema_name);
    let db = connect(&scoped_url).await;
    install_groups_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    seed_left_member(
        &db,
        tenant_id,
        group_id,
        owner_id,
        user_id,
        "join-postgres-expiry",
    )
    .await;

    let (base_version, base_member_count) = group_snapshot(&db, tenant_id, group_id).await;
    assert_eq!(base_member_count, 1);
    assert_eq!(
        membership_snapshot(&db, tenant_id, group_id, user_id).await,
        ("left".to_string(), 1)
    );

    let expires_at = Utc::now() + ChronoDuration::seconds(2);
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
    .expect("owner should suspend the PostgreSQL left membership before re-entry");
    assert_eq!(suspended.group_version, base_version + 1);
    assert_eq!(suspended.member_count, 1);
    assert_eq!(suspended.membership_revision, 2);
    assert_eq!(
        membership_snapshot(&db, tenant_id, group_id, user_id).await,
        ("left".to_string(), 2)
    );

    let groups = GroupsService::new(db.clone());
    let blocked = GroupCommandPort::join_group(
        &groups,
        write_context(tenant_id, user_id, "blocked-join"),
        JoinGroupRequest { group_id },
    )
    .await
    .expect_err("effective PostgreSQL suspension must deny re-entry");
    assert_eq!(blocked.code, "groups.membership_suspended");
    assert!(!blocked.retryable);
    assert_eq!(
        group_snapshot(&db, tenant_id, group_id).await,
        (suspended.group_version, 1)
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
    .expect("owner should read suspended PostgreSQL re-entry state");
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
    .expect("owner-clock expiry should restore PostgreSQL left lifecycle state");
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
    .expect("expired PostgreSQL suspension should allow re-entry without cleanup");
    assert_eq!(joined.status, GroupMembershipStatus::Active);
    assert_eq!(
        membership_snapshot(&db, tenant_id, group_id, user_id).await,
        ("active".to_string(), 3)
    );
    assert_eq!(
        group_snapshot(&db, tenant_id, group_id).await,
        (suspended.group_version + 1, 2)
    );

    drop(reader);
    drop(groups);
    drop(enforcement);

    for round in 0..ROUNDS {
        let tenant_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        seed_left_member(
            &db,
            tenant_id,
            group_id,
            owner_id,
            user_id,
            &format!("join-postgres-race-{round}"),
        )
        .await;
        let (race_base_version, race_base_member_count) =
            group_snapshot(&db, tenant_id, group_id).await;
        assert_eq!(race_base_member_count, 1);

        let join_db = connect(&scoped_url).await;
        let enforcement_db = connect(&scoped_url).await;
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
        let (join_joined, enforcement_joined) = tokio::time::timeout(PAIR_TIMEOUT, async {
            tokio::join!(join_task, enforcement_task)
        })
        .await
        .expect("PostgreSQL join/enforcement race must not deadlock or time out");
        let join_result = join_joined.expect("PostgreSQL join race task should join without panic");
        let suspension_result =
            enforcement_joined.expect("PostgreSQL enforcement race task should join without panic");

        match (join_result, suspension_result) {
            (Ok(joined), Err(error)) => {
                assert_eq!(joined.status, GroupMembershipStatus::Active);
                assert_eq!(
                    error.code,
                    "groups.membership_enforcement_revision_conflict"
                );
                assert!(!error.retryable);
                assert_eq!(
                    membership_snapshot(&db, tenant_id, group_id, user_id).await,
                    ("active".to_string(), 2)
                );
                assert_eq!(
                    active_enforcement_count(&db, tenant_id, group_id, user_id).await,
                    0
                );
                assert_eq!(
                    group_snapshot(&db, tenant_id, group_id).await,
                    (race_base_version + 1, 2)
                );
            }
            (Err(error), Ok(suspension)) => {
                assert_eq!(error.code, "groups.membership_suspended");
                assert!(!error.retryable);
                assert_eq!(suspension.membership_revision, 2);
                assert_eq!(suspension.member_count, 1);
                assert_eq!(
                    membership_snapshot(&db, tenant_id, group_id, user_id).await,
                    ("left".to_string(), 2)
                );
                assert_eq!(
                    active_enforcement_count(&db, tenant_id, group_id, user_id).await,
                    1
                );
                assert_eq!(
                    group_snapshot(&db, tenant_id, group_id).await,
                    (race_base_version + 1, 1)
                );
            }
            (join_result, suspension_result) => panic!(
                "PostgreSQL join/enforcement race must have exactly one material winner: join={join_result:?}, suspension={suspension_result:?}"
            ),
        }

        assert_eq!(
            membership_snapshot(&db, tenant_id, group_id, user_id)
                .await
                .1,
            2,
            "each PostgreSQL race target must have exactly one material membership revision advance"
        );
        assert_eq!(
            group_snapshot(&db, tenant_id, group_id).await.0,
            race_base_version + 1,
            "exactly one material owner mutation must advance the PostgreSQL group aggregate"
        );
    }

    drop(db);
    admin_db
        .execute_unprepared(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await
        .expect("isolated Groups join enforcement schema should drop");
}
