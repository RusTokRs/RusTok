#![cfg(feature = "mod-groups")]

use std::sync::Arc;
use std::time::Duration;

use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementCommandService,
    GroupMembershipEnforcementMutationResult, RevokeGroupMembershipSuspensionRequest,
    SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use tokio::sync::Barrier;
use uuid::Uuid;

const POSTGRES_URL_ENV: &str = "RUSTOK_GROUPS_TEST_POSTGRES_URL";
const PAIR_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
struct Fixture {
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    target_id: Uuid,
    target_membership_id: Uuid,
}

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
        .expect("Groups direct enforcement concurrency PostgreSQL connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for PostgreSQL direct enforcement concurrency evidence");
    }
}

fn fresh_fixture() -> Fixture {
    Fixture {
        tenant_id: Uuid::new_v4(),
        group_id: Uuid::new_v4(),
        owner_id: Uuid::new_v4(),
        target_id: Uuid::new_v4(),
        target_membership_id: Uuid::new_v4(),
    }
}

async fn seed_fixture(db: &DatabaseConnection, fixture: Fixture, handle: &str) {
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
        Uuid::new_v4(),
        fixture.tenant_id,
        fixture.group_id,
        fixture.owner_id,
        fixture.target_membership_id,
        fixture.tenant_id,
        fixture.group_id,
        fixture.target_id,
    ))
    .await
    .expect("Groups direct enforcement PostgreSQL concurrency fixture should seed");
}

fn write_context(fixture: Fixture, idempotency_key: &str) -> PortContext {
    PortContext::new(
        fixture.tenant_id.to_string(),
        PortActor::user(fixture.owner_id.to_string()),
        "en",
        format!("groups-enforcement-postgres-concurrency-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(20))
    .with_idempotency_key(idempotency_key)
}

async fn suspend(
    service: &GroupMembershipEnforcementCommandService,
    fixture: Fixture,
    idempotency_key: &str,
    expected_membership_revision: i64,
    reason_code: &str,
) -> Result<GroupMembershipEnforcementMutationResult, rustok_api::PortError> {
    GroupMembershipEnforcementCommandPort::suspend_membership(
        service,
        write_context(fixture, idempotency_key),
        SuspendGroupMembershipRequest {
            group_id: fixture.group_id,
            target_user_id: fixture.target_id,
            expected_membership_revision,
            reason_code: reason_code.to_string(),
            effective_until: None,
        },
    )
    .await
}

async fn revoke(
    service: &GroupMembershipEnforcementCommandService,
    fixture: Fixture,
    idempotency_key: &str,
    expected_membership_revision: i64,
    reason_code: &str,
) -> Result<GroupMembershipEnforcementMutationResult, rustok_api::PortError> {
    GroupMembershipEnforcementCommandPort::revoke_membership_suspension(
        service,
        write_context(fixture, idempotency_key),
        RevokeGroupMembershipSuspensionRequest {
            group_id: fixture.group_id,
            target_user_id: fixture.target_id,
            expected_membership_revision,
            reason_code: reason_code.to_string(),
        },
    )
    .await
}

async fn scalar_count(db: &DatabaseConnection, sql: String) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .expect("PostgreSQL concurrency count query should succeed")
        .expect("count row should exist");
    row.try_get("", "count").expect("count should decode")
}

async fn group_snapshot(db: &DatabaseConnection, fixture: Fixture) -> (i64, i64) {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT version, member_count FROM groups WHERE tenant_id = '{}' AND id = '{}'",
                fixture.tenant_id, fixture.group_id
            ),
        ))
        .await
        .expect("PostgreSQL concurrency group snapshot should succeed")
        .expect("group should exist");
    (
        row.try_get("", "version")
            .expect("group version should decode"),
        row.try_get("", "member_count")
            .expect("member_count should decode"),
    )
}

async fn target_revision(db: &DatabaseConnection, fixture: Fixture) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT revision FROM group_memberships WHERE tenant_id = '{}' AND group_id = '{}' AND user_id = '{}'",
                fixture.tenant_id, fixture.group_id, fixture.target_id
            ),
        ))
        .await
        .expect("PostgreSQL concurrency membership snapshot should succeed")
        .expect("target membership should exist");
    row.try_get("", "revision")
        .expect("membership revision should decode")
}

async fn enforcement_snapshot(db: &DatabaseConnection, fixture: Fixture) -> (i64, i64) {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT revision, CASE WHEN revoked_at IS NULL THEN 0::BIGINT ELSE 1::BIGINT END AS revoked FROM group_membership_enforcements WHERE tenant_id = '{}' AND group_id = '{}' AND user_id = '{}'",
                fixture.tenant_id, fixture.group_id, fixture.target_id
            ),
        ))
        .await
        .expect("PostgreSQL concurrency enforcement snapshot should succeed")
        .expect("enforcement row should exist");
    (
        row.try_get("", "revision")
            .expect("enforcement revision should decode"),
        row.try_get("", "revoked")
            .expect("revoked marker should decode"),
    )
}

async fn ledger_counts(db: &DatabaseConnection, fixture: Fixture) -> (i64, i64, i64) {
    let audit = scalar_count(
        db,
        format!(
            "SELECT COUNT(*) AS count FROM group_audit_entries WHERE tenant_id = '{}' AND group_id = '{}'",
            fixture.tenant_id, fixture.group_id
        ),
    )
    .await;
    let events = scalar_count(
        db,
        format!(
            "SELECT COUNT(*) AS count FROM group_domain_events WHERE tenant_id = '{}' AND aggregate_type = 'membership' AND aggregate_id = '{}'",
            fixture.tenant_id, fixture.target_membership_id
        ),
    )
    .await;
    let receipts = scalar_count(
        db,
        format!(
            "SELECT COUNT(*) AS count FROM group_command_receipts WHERE tenant_id = '{}' AND group_id = '{}'",
            fixture.tenant_id, fixture.group_id
        ),
    )
    .await;
    (audit, events, receipts)
}

fn assert_same_material_result(
    left: &GroupMembershipEnforcementMutationResult,
    right: &GroupMembershipEnforcementMutationResult,
) {
    assert_eq!(left.group_id, right.group_id);
    assert_eq!(left.membership_id, right.membership_id);
    assert_eq!(left.user_id, right.user_id);
    assert_eq!(left.membership_revision, right.membership_revision);
    assert_eq!(left.group_version, right.group_version);
    assert_eq!(left.member_count, right.member_count);
    assert_eq!(left.effective_status, right.effective_status);
    assert_eq!(left.enforcement_revision, right.enforcement_revision);
    assert_eq!(left.effective_until, right.effective_until);
    assert_eq!(left.revoked_at, right.revoked_at);
}

async fn run_same_key_suspend(scoped_url: &str, fixture_db: &DatabaseConnection) {
    let fixture = fresh_fixture();
    seed_fixture(
        fixture_db,
        fixture,
        "enforcement-postgres-concurrency-same-key",
    )
    .await;

    let left_db = connect(scoped_url).await;
    let right_db = connect(scoped_url).await;
    let barrier = Arc::new(Barrier::new(3));

    let left_barrier = barrier.clone();
    let left = tokio::spawn(async move {
        let service = GroupMembershipEnforcementCommandService::new(left_db);
        left_barrier.wait().await;
        suspend(
            &service,
            fixture,
            "postgres-same-key-suspend",
            1,
            "same_key_concurrency",
        )
        .await
    });
    let right_barrier = barrier.clone();
    let right = tokio::spawn(async move {
        let service = GroupMembershipEnforcementCommandService::new(right_db);
        right_barrier.wait().await;
        suspend(
            &service,
            fixture,
            "postgres-same-key-suspend",
            1,
            "same_key_concurrency",
        )
        .await
    });

    barrier.wait().await;
    let (left_join, right_join) =
        tokio::time::timeout(PAIR_TIMEOUT, async { tokio::join!(left, right) })
            .await
            .expect("PostgreSQL same-key concurrent suspend must not deadlock or exceed timeout");
    let left = left_join
        .expect("left PostgreSQL same-key task should join without panic")
        .expect("left PostgreSQL same-key command should succeed");
    let right = right_join
        .expect("right PostgreSQL same-key task should join without panic")
        .expect("right PostgreSQL same-key command should succeed");

    assert_same_material_result(&left, &right);
    assert_eq!(u8::from(left.replayed) + u8::from(right.replayed), 1);
    assert_eq!(left.membership_revision, 2);
    assert_eq!(left.group_version, 2);
    assert_eq!(left.member_count, 2);
    assert_eq!(left.enforcement_revision, 1);
    assert_eq!(group_snapshot(fixture_db, fixture).await, (2, 2));
    assert_eq!(target_revision(fixture_db, fixture).await, 2);
    assert_eq!(enforcement_snapshot(fixture_db, fixture).await, (1, 0));
    assert_eq!(ledger_counts(fixture_db, fixture).await, (1, 1, 1));
}

async fn run_distinct_key_suspend(scoped_url: &str, fixture_db: &DatabaseConnection) {
    let fixture = fresh_fixture();
    seed_fixture(
        fixture_db,
        fixture,
        "enforcement-postgres-concurrency-distinct-key",
    )
    .await;

    let left_db = connect(scoped_url).await;
    let right_db = connect(scoped_url).await;
    let barrier = Arc::new(Barrier::new(3));

    let left_barrier = barrier.clone();
    let left = tokio::spawn(async move {
        let service = GroupMembershipEnforcementCommandService::new(left_db);
        left_barrier.wait().await;
        suspend(
            &service,
            fixture,
            "postgres-distinct-suspend-left",
            1,
            "distinct_key_concurrency",
        )
        .await
    });
    let right_barrier = barrier.clone();
    let right = tokio::spawn(async move {
        let service = GroupMembershipEnforcementCommandService::new(right_db);
        right_barrier.wait().await;
        suspend(
            &service,
            fixture,
            "postgres-distinct-suspend-right",
            1,
            "distinct_key_concurrency",
        )
        .await
    });

    barrier.wait().await;
    let (left_join, right_join) =
        tokio::time::timeout(PAIR_TIMEOUT, async { tokio::join!(left, right) })
            .await
            .expect(
                "PostgreSQL distinct-key concurrent suspend must not deadlock or exceed timeout",
            );
    let left = left_join.expect("left PostgreSQL distinct-key task should join without panic");
    let right = right_join.expect("right PostgreSQL distinct-key task should join without panic");

    match (left, right) {
        (Ok(success), Err(error)) | (Err(error), Ok(success)) => {
            assert!(!success.replayed);
            assert_eq!(success.membership_revision, 2);
            assert_eq!(success.group_version, 2);
            assert_eq!(success.member_count, 2);
            assert_eq!(success.enforcement_revision, 1);
            assert_eq!(
                error.code,
                "groups.membership_enforcement_revision_conflict"
            );
            assert!(!error.retryable);
        }
        (left, right) => panic!(
            "PostgreSQL distinct-key concurrent suspend must produce exactly one commit and one revision conflict: left={left:?}, right={right:?}"
        ),
    }

    assert_eq!(group_snapshot(fixture_db, fixture).await, (2, 2));
    assert_eq!(target_revision(fixture_db, fixture).await, 2);
    assert_eq!(enforcement_snapshot(fixture_db, fixture).await, (1, 0));
    assert_eq!(ledger_counts(fixture_db, fixture).await, (1, 1, 1));
}

async fn run_distinct_key_revoke(scoped_url: &str, fixture_db: &DatabaseConnection) {
    let fixture = fresh_fixture();
    seed_fixture(
        fixture_db,
        fixture,
        "enforcement-postgres-concurrency-revoke",
    )
    .await;
    let baseline_service = GroupMembershipEnforcementCommandService::new(fixture_db.clone());
    let baseline = suspend(
        &baseline_service,
        fixture,
        "postgres-baseline-suspend",
        1,
        "revoke_concurrency_baseline",
    )
    .await
    .expect("PostgreSQL baseline suspension should succeed before revoke contention");
    assert_eq!(baseline.membership_revision, 2);
    assert_eq!(baseline.group_version, 2);
    assert_eq!(ledger_counts(fixture_db, fixture).await, (1, 1, 1));

    let left_db = connect(scoped_url).await;
    let right_db = connect(scoped_url).await;
    let barrier = Arc::new(Barrier::new(3));

    let left_barrier = barrier.clone();
    let left = tokio::spawn(async move {
        let service = GroupMembershipEnforcementCommandService::new(left_db);
        left_barrier.wait().await;
        revoke(
            &service,
            fixture,
            "postgres-revoke-left",
            2,
            "revoke_concurrency",
        )
        .await
    });
    let right_barrier = barrier.clone();
    let right = tokio::spawn(async move {
        let service = GroupMembershipEnforcementCommandService::new(right_db);
        right_barrier.wait().await;
        revoke(
            &service,
            fixture,
            "postgres-revoke-right",
            2,
            "revoke_concurrency",
        )
        .await
    });

    barrier.wait().await;
    let (left_join, right_join) =
        tokio::time::timeout(PAIR_TIMEOUT, async { tokio::join!(left, right) })
            .await
            .expect(
                "PostgreSQL distinct-key concurrent revoke must not deadlock or exceed timeout",
            );
    let left = left_join.expect("left PostgreSQL revoke task should join without panic");
    let right = right_join.expect("right PostgreSQL revoke task should join without panic");

    match (left, right) {
        (Ok(success), Err(error)) | (Err(error), Ok(success)) => {
            assert!(!success.replayed);
            assert_eq!(success.membership_revision, 3);
            assert_eq!(success.group_version, 3);
            assert_eq!(success.member_count, 2);
            assert_eq!(success.enforcement_revision, 2);
            assert!(success.revoked_at.is_some());
            assert_eq!(
                error.code,
                "groups.membership_enforcement_revision_conflict"
            );
            assert!(!error.retryable);
        }
        (left, right) => panic!(
            "PostgreSQL distinct-key concurrent revoke must produce exactly one commit and one revision conflict: left={left:?}, right={right:?}"
        ),
    }

    assert_eq!(group_snapshot(fixture_db, fixture).await, (3, 2));
    assert_eq!(target_revision(fixture_db, fixture).await, 3);
    assert_eq!(enforcement_snapshot(fixture_db, fixture).await, (2, 1));
    assert_eq!(ledger_counts(fixture_db, fixture).await, (2, 2, 2));
}

#[tokio::test]
#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]
async fn direct_enforcement_receipt_and_revision_contention_postgres() {
    let base_url = std::env::var(POSTGRES_URL_ENV)
        .expect("RUSTOK_GROUPS_TEST_POSTGRES_URL must be configured");
    let schema_name = format!("groups_enforcement_concurrency_{}", Uuid::new_v4().simple());
    let admin_db = connect(&base_url).await;
    admin_db
        .execute_unprepared(&format!("CREATE SCHEMA {schema_name}"))
        .await
        .expect("isolated Groups enforcement concurrency schema should create");
    let scoped_url = schema_url(&base_url, &schema_name);
    let fixture_db = connect(&scoped_url).await;
    install_groups_schema(&fixture_db).await;

    run_same_key_suspend(&scoped_url, &fixture_db).await;
    run_distinct_key_suspend(&scoped_url, &fixture_db).await;
    run_distinct_key_revoke(&scoped_url, &fixture_db).await;

    drop(fixture_db);
    admin_db
        .execute_unprepared(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await
        .expect("isolated Groups enforcement concurrency schema should drop");
}
