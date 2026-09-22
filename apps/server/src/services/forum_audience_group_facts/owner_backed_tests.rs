use std::time::Duration;

use chrono::Utc;
use rustok_api::{PortActor, PortContext};
use rustok_forum::{ForumAudienceFactsPort, ForumAudienceFactsRequest};
use rustok_groups::{
    GroupMembershipEffectiveStatus, GroupMembershipEnforcementCommandPort,
    GroupMembershipEnforcementCommandService, SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use tempfile::TempDir;
use uuid::Uuid;

use super::ServerForumAudienceGroupFactsPort;

const POSTGRES_URL_ENV: &str = "RUSTOK_GROUPS_TEST_POSTGRES_URL";

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("Forum Groups owner-backed evidence connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for Forum audience evidence");
    }
}

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp
        .path()
        .join("forum-groups-owner-backed-audience.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

fn postgres_schema_url(base: &str, schema: &str) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}options=-csearch_path%3D{schema}%2Cpublic")
}

fn member_read_context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        format!("forum-groups-owner-backed-read-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
}

fn owner_write_context(tenant_id: Uuid, owner_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(owner_id.to_string()),
        "en",
        format!("forum-groups-owner-backed-write-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key(format!("forum-groups-owner-backed-{}", Uuid::new_v4()))
}

fn facts_request(tenant_id: Uuid, user_id: Uuid, group_id: Uuid) -> ForumAudienceFactsRequest {
    ForumAudienceFactsRequest {
        tenant_id,
        user_id,
        include_trust_level: false,
        channel_slugs: Vec::new(),
        group_ids: vec![group_id],
    }
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    member_id: Uuid,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', 'forum-owner-backed-audience', 2);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{member_id}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Forum Groups audience evidence fixture should seed");
}

async fn membership_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    member_id: Uuid,
) -> (String, i64) {
    let row = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
            format!(
                "SELECT status, revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND user_id = '{member_id}'"
            ),
        ))
        .await
        .expect("membership snapshot query should succeed")
        .expect("membership snapshot should exist");
    (
        row.try_get("", "status")
            .expect("membership status should decode"),
        row.try_get("", "revision")
            .expect("membership revision should decode"),
    )
}

async fn group_member_count(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
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

async fn exercise_owner_backed_forum_group_facts(db: &DatabaseConnection) {
    let tenant_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let member_id = Uuid::new_v4();
    seed_group_fixture(db, tenant_id, group_id, owner_id, member_id).await;

    let (stored_status, initial_revision) = membership_snapshot(db, tenant_id, member_id).await;
    assert_eq!(stored_status, "active");
    assert_eq!(group_member_count(db, tenant_id, group_id).await, 2);

    let adapter = ServerForumAudienceGroupFactsPort::from_db(db.clone());
    let active_facts = ForumAudienceFactsPort::resolve_forum_audience_facts(
        &adapter,
        member_read_context(tenant_id, member_id),
        facts_request(tenant_id, member_id, group_id),
    )
    .await
    .expect("lifecycle-active member should satisfy the requested Forum group selector");
    assert_eq!(active_facts.group_memberships, vec![group_id]);

    let command = GroupMembershipEnforcementCommandService::new(db.clone());
    let effective_until = Utc::now() + chrono::Duration::seconds(2);
    let suspended = GroupMembershipEnforcementCommandPort::suspend_membership(
        &command,
        owner_write_context(tenant_id, owner_id),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: member_id,
            expected_membership_revision: initial_revision,
            reason_code: "forum_acl_review".to_string(),
            effective_until: Some(effective_until),
        },
    )
    .await
    .expect("Groups owner should create the temporary suspension used by Forum audience evidence");
    assert_eq!(
        suspended.effective_status,
        GroupMembershipEffectiveStatus::Suspended
    );

    let (stored_status_during_suspension, suspended_revision) =
        membership_snapshot(db, tenant_id, member_id).await;
    assert_eq!(stored_status_during_suspension, "active");
    assert_eq!(suspended_revision, suspended.membership_revision);
    assert_eq!(group_member_count(db, tenant_id, group_id).await, 2);

    let facts_after_suspend = ForumAudienceFactsPort::resolve_forum_audience_facts(
        &adapter,
        member_read_context(tenant_id, member_id),
        facts_request(tenant_id, member_id, group_id),
    )
    .await
    .expect("a suspended member should resolve a valid negative Forum group fact");
    assert!(facts_after_suspend.group_memberships.is_empty());

    tokio::time::sleep(Duration::from_millis(2300)).await;

    let facts_after_expiry = ForumAudienceFactsPort::resolve_forum_audience_facts(
        &adapter,
        member_read_context(tenant_id, member_id),
        facts_request(tenant_id, member_id, group_id),
    )
    .await
    .expect("owner-clock expiry should restore the Forum group fact without cleanup");
    assert_eq!(facts_after_expiry.group_memberships, vec![group_id]);

    let (stored_status_after_expiry, revision_after_expiry) =
        membership_snapshot(db, tenant_id, member_id).await;
    assert_eq!(stored_status_after_expiry, "active");
    assert_eq!(revision_after_expiry, suspended_revision);
    assert_eq!(group_member_count(db, tenant_id, group_id).await, 2);
}

#[tokio::test]
async fn forum_group_facts_follow_groups_owner_clock_sqlite() {
    let temp =
        tempfile::tempdir().expect("temporary Forum Groups audience directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    assert_eq!(db.get_database_backend(), DatabaseBackend::Sqlite);
    install_groups_schema(&db).await;
    exercise_owner_backed_forum_group_facts(&db).await;
    drop(db);
    drop(temp);
}

#[tokio::test]
#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]
async fn forum_group_facts_follow_groups_owner_clock_postgres() {
    let base_url = std::env::var(POSTGRES_URL_ENV)
        .expect("RUSTOK_GROUPS_TEST_POSTGRES_URL must be configured");
    let schema = format!("forum_groups_audience_{}", Uuid::new_v4().simple());
    let admin = connect(&base_url).await;
    admin
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .expect("isolated Forum Groups audience schema should create");
    let scoped_url = postgres_schema_url(&base_url, &schema);
    let db = connect(&scoped_url).await;
    assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
    install_groups_schema(&db).await;
    exercise_owner_backed_forum_group_facts(&db).await;
    drop(db);
    admin
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .expect("isolated Forum Groups audience schema should drop");
}
