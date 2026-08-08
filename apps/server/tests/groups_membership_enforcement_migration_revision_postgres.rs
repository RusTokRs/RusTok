#![cfg(feature = "mod-groups")]

use std::time::Duration;

use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementCommandService,
    RevokeGroupMembershipSuspensionRequest, SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
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
        .expect("Groups enforcement migration PostgreSQL connection should open")
}

async fn scalar_count(db: &DatabaseConnection, sql: String) -> i64 {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .expect("PostgreSQL migration evidence count query should succeed")
        .expect("PostgreSQL migration evidence count row should exist");
    row.try_get("", "count").expect("count should decode")
}

async fn membership_revision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> i64 {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("PostgreSQL membership revision query should succeed")
        .expect("membership should exist");
    row.try_get("", "revision")
        .expect("membership revision should decode")
}

async fn group_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
) -> (i64, i64) {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT version, member_count FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("PostgreSQL group snapshot query should succeed")
        .expect("group should exist");
    (
        row.try_get("", "version").expect("group version should decode"),
        row.try_get("", "member_count")
            .expect("group member_count should decode"),
    )
}

fn write_context(tenant_id: Uuid, owner_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(owner_id.to_string()),
        "en",
        format!("groups-enforcement-migration-postgres-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(5))
    .with_idempotency_key(key)
}

#[tokio::test]
#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]
async fn enforcement_migration_backfills_and_revision_sources_are_monotonic_postgres() {
    let base_url = std::env::var(POSTGRES_URL_ENV)
        .expect("RUSTOK_GROUPS_TEST_POSTGRES_URL must be configured");
    let schema_name = format!("groups_enforcement_migration_{}", Uuid::new_v4().simple());
    let admin_db = connect(&base_url).await;
    admin_db
        .execute_unprepared(&format!("CREATE SCHEMA {schema_name}"))
        .await
        .expect("isolated Groups enforcement migration schema should create");
    let scoped_url = schema_url(&base_url, &schema_name);
    let db = connect(&scoped_url).await;
    let manager = SchemaManager::new(&db);
    let migrations = rustok_groups::migrations::migrations();
    assert!(
        migrations.len() >= 9,
        "Groups migration list must contain enforcement state and event-ledger extension"
    );

    for migration in migrations.iter().take(7) {
        migration
            .up(&manager)
            .await
            .expect("pre-enforcement Groups migration should apply in PostgreSQL schema");
    }

    let tenant_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let owner_membership_id = Uuid::new_v4();
    let target_membership_id = Uuid::new_v4();

    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', 'enforcement-postgres-migration-revision', 2);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{owner_membership_id}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{target_membership_id}', '{tenant_id}', '{group_id}', '{target_id}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
    ))
    .await
    .expect("pre-enforcement PostgreSQL Groups rows should seed");

    assert_eq!(
        scalar_count(
            &db,
            "SELECT COUNT(*) AS count FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = 'group_memberships' AND column_name = 'revision'"
                .to_string(),
        )
        .await,
        0,
        "revision column must not exist before PostgreSQL enforcement migration"
    );
    assert_eq!(
        scalar_count(
            &db,
            "SELECT COUNT(*) AS count FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = 'group_membership_enforcements'"
                .to_string(),
        )
        .await,
        0,
        "enforcement projection table must not exist before PostgreSQL migration 000008"
    );

    migrations[7]
        .up(&manager)
        .await
        .expect("Groups membership-enforcement migration 000008 should apply on PostgreSQL");

    assert_eq!(
        scalar_count(
            &db,
            "SELECT COUNT(*) AS count FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = 'group_memberships' AND column_name = 'revision'"
                .to_string(),
        )
        .await,
        1,
        "PostgreSQL enforcement migration must add one membership revision column"
    );
    assert_eq!(
        scalar_count(
            &db,
            "SELECT COUNT(*) AS count FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = 'group_membership_enforcements'"
                .to_string(),
        )
        .await,
        1,
        "PostgreSQL enforcement migration must create the bounded projection table"
    );
    assert_eq!(membership_revision(&db, tenant_id, group_id, owner_id).await, 1);
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 1);

    db.execute_unprepared(&format!(
        "UPDATE group_memberships SET role = 'moderator' WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{target_id}'"
    ))
    .await
    .expect("material PostgreSQL role change should succeed after revision migration");
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 2);

    let decrease = db
        .execute_unprepared(&format!(
            "UPDATE group_memberships SET revision = 1 WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{target_id}'"
        ))
        .await;
    assert!(
        decrease.is_err(),
        "PostgreSQL membership revision decrease must fail closed"
    );
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 2);

    for migration in migrations.iter().skip(8) {
        migration
            .up(&manager)
            .await
            .expect("post-enforcement Groups migration should apply on PostgreSQL");
    }

    let service = GroupMembershipEnforcementCommandService::new(db.clone());
    let suspended = GroupMembershipEnforcementCommandPort::suspend_membership(
        &service,
        write_context(tenant_id, owner_id, "postgres-migration-revision-suspend"),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: target_id,
            expected_membership_revision: 2,
            reason_code: "migration_revision_evidence".to_string(),
            effective_until: None,
        },
    )
    .await
    .expect("PostgreSQL production suspension should consume post-role-change revision");
    assert_eq!(suspended.membership_revision, 3);
    assert_eq!(suspended.group_version, 2);
    assert_eq!(suspended.member_count, 2);
    assert_eq!(suspended.enforcement_revision, 1);
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 3);

    let revoked = GroupMembershipEnforcementCommandPort::revoke_membership_suspension(
        &service,
        write_context(tenant_id, owner_id, "postgres-migration-revision-revoke"),
        RevokeGroupMembershipSuspensionRequest {
            group_id,
            target_user_id: target_id,
            expected_membership_revision: 3,
            reason_code: "migration_revision_release".to_string(),
        },
    )
    .await
    .expect("PostgreSQL production revoke should consume enforcement-insert revision");
    assert_eq!(revoked.membership_revision, 4);
    assert_eq!(revoked.group_version, 3);
    assert_eq!(revoked.member_count, 2);
    assert_eq!(revoked.enforcement_revision, 2);
    assert!(revoked.revoked_at.is_some());
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 4);
    assert_eq!(group_snapshot(&db, tenant_id, group_id).await, (3, 2));

    db.execute_unprepared(&format!(
        "UPDATE group_memberships SET status = 'left' WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{target_id}'"
    ))
    .await
    .expect("material PostgreSQL lifecycle status change should succeed");
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 5);

    let second_decrease = db
        .execute_unprepared(&format!(
            "UPDATE group_memberships SET revision = 4 WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{target_id}'"
        ))
        .await;
    assert!(
        second_decrease.is_err(),
        "PostgreSQL revision must remain monotonic after enforcement and lifecycle mutations"
    );
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 5);

    drop(service);
    drop(db);
    admin_db
        .execute_unprepared(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await
        .expect("isolated Groups enforcement migration schema should drop");
}
