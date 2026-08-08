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
        .expect("Groups enforcement migration SQLite connection should open")
}

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp
        .path()
        .join("groups-membership-enforcement-migration-revision.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

async fn scalar_count(db: &DatabaseConnection, sql: String) -> i64 {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .expect("SQLite migration evidence count query should succeed")
        .expect("SQLite migration evidence count row should exist");
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
            DatabaseBackend::Sqlite,
            format!(
                "SELECT revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("SQLite membership revision query should succeed")
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
            DatabaseBackend::Sqlite,
            format!(
                "SELECT version, member_count FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("SQLite group snapshot query should succeed")
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
        format!("groups-enforcement-migration-sqlite-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(5))
    .with_idempotency_key(key)
}

#[tokio::test]
async fn enforcement_migration_backfills_and_revision_sources_are_monotonic_sqlite() {
    let temp = tempfile::tempdir()
        .expect("temporary Groups enforcement migration SQLite directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
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
            .expect("pre-enforcement Groups migration should apply");
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
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', 'enforcement-migration-revision', 2);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{owner_membership_id}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{target_membership_id}', '{tenant_id}', '{group_id}', '{target_id}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
    ))
    .await
    .expect("pre-enforcement Groups rows should seed");

    assert_eq!(
        scalar_count(
            &db,
            "SELECT COUNT(*) AS count FROM pragma_table_info('group_memberships') WHERE name = 'revision'"
                .to_string(),
        )
        .await,
        0,
        "revision column must not exist before enforcement migration"
    );
    assert_eq!(
        scalar_count(
            &db,
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'group_membership_enforcements'"
                .to_string(),
        )
        .await,
        0,
        "enforcement projection table must not exist before migration 000008"
    );

    migrations[7]
        .up(&manager)
        .await
        .expect("Groups membership-enforcement migration 000008 should apply");

    assert_eq!(
        scalar_count(
            &db,
            "SELECT COUNT(*) AS count FROM pragma_table_info('group_memberships') WHERE name = 'revision'"
                .to_string(),
        )
        .await,
        1,
        "enforcement migration must add exactly one membership revision column"
    );
    assert_eq!(
        scalar_count(
            &db,
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'group_membership_enforcements'"
                .to_string(),
        )
        .await,
        1,
        "enforcement migration must create the bounded projection table"
    );
    assert_eq!(membership_revision(&db, tenant_id, group_id, owner_id).await, 1);
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 1);

    db.execute_unprepared(&format!(
        "UPDATE group_memberships SET role = 'moderator' WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{target_id}'"
    ))
    .await
    .expect("material role change should succeed after revision migration");
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 2);

    let decrease = db
        .execute_unprepared(&format!(
            "UPDATE group_memberships SET revision = 1 WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{target_id}'"
        ))
        .await;
    assert!(decrease.is_err(), "membership revision decrease must fail closed");
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 2);

    for migration in migrations.iter().skip(8) {
        migration
            .up(&manager)
            .await
            .expect("post-enforcement Groups migration should apply");
    }

    let service = GroupMembershipEnforcementCommandService::new(db.clone());
    let suspended = GroupMembershipEnforcementCommandPort::suspend_membership(
        &service,
        write_context(tenant_id, owner_id, "migration-revision-suspend"),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: target_id,
            expected_membership_revision: 2,
            reason_code: "migration_revision_evidence".to_string(),
            effective_until: None,
        },
    )
    .await
    .expect("production suspension should consume the post-role-change revision");
    assert_eq!(suspended.membership_revision, 3);
    assert_eq!(suspended.group_version, 2);
    assert_eq!(suspended.member_count, 2);
    assert_eq!(suspended.enforcement_revision, 1);
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 3);

    let revoked = GroupMembershipEnforcementCommandPort::revoke_membership_suspension(
        &service,
        write_context(tenant_id, owner_id, "migration-revision-revoke"),
        RevokeGroupMembershipSuspensionRequest {
            group_id,
            target_user_id: target_id,
            expected_membership_revision: 3,
            reason_code: "migration_revision_release".to_string(),
        },
    )
    .await
    .expect("production revoke should consume the enforcement-insert revision");
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
    .expect("material lifecycle status change should succeed");
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 5);

    let second_decrease = db
        .execute_unprepared(&format!(
            "UPDATE group_memberships SET revision = 4 WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{target_id}'"
        ))
        .await;
    assert!(
        second_decrease.is_err(),
        "revision must remain monotonic after enforcement and lifecycle mutations"
    );
    assert_eq!(membership_revision(&db, tenant_id, group_id, target_id).await, 5);

    drop(service);
    drop(db);
    drop(temp);
}
