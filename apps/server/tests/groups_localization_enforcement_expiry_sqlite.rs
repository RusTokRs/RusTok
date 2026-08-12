#![cfg(feature = "mod-groups")]

use std::time::Duration;

use chrono::Utc;
use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    GroupLocalizationCommandPort, GroupLocalizationReadPort, GroupLocalizationService,
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementCommandService,
    ListGroupTranslationsRequest, SuspendGroupMembershipRequest, UpsertGroupTranslationRequest,
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
        .expect("Groups localization expiry SQLite connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for localization expiry evidence");
    }
}

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp
        .path()
        .join("groups-localization-enforcement-expiry.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    admin_id: Uuid,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', 'localization-enforcement-expiry', 2);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{admin_id}', 'admin', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups localization expiry fixture should seed");
}

fn read_context(tenant_id: Uuid, actor_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("groups-localization-expiry-read-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, operation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("groups-localization-expiry-write-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key(format!("{operation}-{}", Uuid::new_v4()))
}

async fn membership_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
) -> (String, i64) {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT status, revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND user_id = '{user_id}'"
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
async fn localization_management_follows_effective_suspension_and_owner_clock_expiry_sqlite() {
    let temp =
        tempfile::tempdir().expect("temporary Groups localization expiry directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    install_groups_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    seed_group_fixture(&db, tenant_id, group_id, owner_id, admin_id).await;

    let localization = GroupLocalizationService::new(db.clone());
    let initial = GroupLocalizationCommandPort::upsert_group_translation(
        &localization,
        write_context(tenant_id, owner_id, "owner-initial-translation"),
        UpsertGroupTranslationRequest {
            group_id,
            locale: "en".to_string(),
            title: "Initial title".to_string(),
            summary: None,
            body: None,
        },
    )
    .await
    .expect("active owner should create the initial translation");
    assert!(initial.created);

    let admin_before = GroupLocalizationReadPort::list_group_translations(
        &localization,
        read_context(tenant_id, admin_id),
        ListGroupTranslationsRequest { group_id },
    )
    .await
    .expect("active administrator should read management translations");
    assert_eq!(admin_before.len(), 1);

    let (_, admin_initial_revision) = membership_snapshot(&db, tenant_id, admin_id).await;
    let enforcement = GroupMembershipEnforcementCommandService::new(db.clone());
    let effective_until = Utc::now() + chrono::Duration::seconds(2);
    let suspended = GroupMembershipEnforcementCommandPort::suspend_membership(
        &enforcement,
        write_context(tenant_id, owner_id, "owner-suspend-admin"),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: admin_id,
            expected_membership_revision: admin_initial_revision,
            reason_code: "temporary_settings_review".to_string(),
            effective_until: Some(effective_until),
        },
    )
    .await
    .expect("owner should temporarily suspend the administrator");
    assert_eq!(suspended.membership_revision, admin_initial_revision + 1);
    assert_eq!(suspended.member_count, 2);
    assert!(suspended.group_version > initial.group_version as i64);

    let (stored_status_during_suspension, stored_revision_during_suspension) =
        membership_snapshot(&db, tenant_id, admin_id).await;
    assert_eq!(stored_status_during_suspension, "active");
    assert_eq!(
        stored_revision_during_suspension,
        suspended.membership_revision
    );
    assert_eq!(group_member_count(&db, tenant_id, group_id).await, 2);

    let read_error = GroupLocalizationReadPort::list_group_translations(
        &localization,
        read_context(tenant_id, admin_id),
        ListGroupTranslationsRequest { group_id },
    )
    .await
    .expect_err("suspended administrator must not read management translations");
    assert_eq!(read_error.code, "groups.membership_suspended");
    assert!(!read_error.retryable);

    let write_error = GroupLocalizationCommandPort::upsert_group_translation(
        &localization,
        write_context(tenant_id, admin_id, "suspended-admin-write"),
        UpsertGroupTranslationRequest {
            group_id,
            locale: "fr".to_string(),
            title: "Blocked title".to_string(),
            summary: None,
            body: None,
        },
    )
    .await
    .expect_err("suspended administrator must not mutate translations");
    assert_eq!(write_error.code, "groups.membership_suspended");
    assert!(!write_error.retryable);

    let owner_during = GroupLocalizationReadPort::list_group_translations(
        &localization,
        read_context(tenant_id, owner_id),
        ListGroupTranslationsRequest { group_id },
    )
    .await
    .expect("owner should still read translations while administrator is suspended");
    assert_eq!(
        owner_during.len(),
        1,
        "failed suspended write must not create French translation"
    );

    tokio::time::sleep(Duration::from_millis(2300)).await;

    let admin_after_expiry = GroupLocalizationReadPort::list_group_translations(
        &localization,
        read_context(tenant_id, admin_id),
        ListGroupTranslationsRequest { group_id },
    )
    .await
    .expect("expired suspension should restore administrator management reads without cleanup");
    assert_eq!(admin_after_expiry.len(), 1);

    let restored = GroupLocalizationCommandPort::upsert_group_translation(
        &localization,
        write_context(tenant_id, admin_id, "restored-admin-write"),
        UpsertGroupTranslationRequest {
            group_id,
            locale: "fr".to_string(),
            title: "Restored title".to_string(),
            summary: Some("Owner-clock access restored".to_string()),
            body: None,
        },
    )
    .await
    .expect("expired suspension should restore administrator management writes without cleanup");
    assert!(restored.created);
    assert_eq!(restored.group_version, suspended.group_version as u64 + 1);

    let final_translations = GroupLocalizationReadPort::list_group_translations(
        &localization,
        read_context(tenant_id, admin_id),
        ListGroupTranslationsRequest { group_id },
    )
    .await
    .expect("restored administrator should read both translations");
    assert_eq!(final_translations.len(), 2);
    assert!(
        final_translations
            .iter()
            .any(|translation| translation.locale == "fr")
    );

    let (stored_status_after_expiry, stored_revision_after_expiry) =
        membership_snapshot(&db, tenant_id, admin_id).await;
    assert_eq!(stored_status_after_expiry, "active");
    assert_eq!(stored_revision_after_expiry, suspended.membership_revision);
    assert_eq!(group_member_count(&db, tenant_id, group_id).await, 2);

    drop(enforcement);
    drop(localization);
    drop(db);
    drop(temp);
}
