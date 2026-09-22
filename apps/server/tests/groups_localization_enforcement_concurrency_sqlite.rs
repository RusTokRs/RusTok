#![cfg(feature = "mod-groups")]

use std::sync::Arc;
use std::time::Duration;

use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    GroupLocalizationCommandPort, GroupLocalizationReadPort, GroupLocalizationService,
    GroupMembershipEffectiveStatus, GroupMembershipEnforcementCommandPort,
    GroupMembershipEnforcementCommandService, GroupMembershipEnforcementReadPort,
    GroupMembershipEnforcementService, ListGroupTranslationsRequest,
    ReadGroupMembershipEnforcementRequest, SuspendGroupMembershipRequest,
    UpsertGroupTranslationRequest,
};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use tempfile::TempDir;
use tokio::sync::Barrier;
use uuid::Uuid;

const ROUNDS: usize = 12;
const SQLITE_BUSY_TIMEOUT_MS: u64 = 5_000;

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp
        .path()
        .join("groups-localization-enforcement-concurrency.sqlite");
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
        .expect("Groups localization concurrency SQLite connection should open");
    db.execute_unprepared(&format!("PRAGMA busy_timeout = {SQLITE_BUSY_TIMEOUT_MS};"))
        .await
        .expect("SQLite localization concurrency connection should configure busy timeout");
    db
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration.up(&manager).await.expect(
            "production Groups migration should apply for SQLite localization concurrency evidence",
        );
    }
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    admin_id: Uuid,
    round: usize,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', 'localization-sqlite-concurrency-{round}', 2);

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
    .expect("Groups SQLite localization concurrency fixture should seed");
}

fn read_context(tenant_id: Uuid, actor_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!(
            "groups-localization-sqlite-concurrency-read-{}",
            Uuid::new_v4()
        ),
    )
    .with_deadline(Duration::from_secs(10))
}

fn owner_effective_read_context(tenant_id: Uuid, owner_id: Uuid) -> PortContext {
    read_context(tenant_id, owner_id).with_claim("groups:access:read")
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, operation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!(
            "groups-localization-sqlite-concurrency-write-{}",
            Uuid::new_v4()
        ),
    )
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key(format!("{operation}-{}", Uuid::new_v4()))
}

#[tokio::test]
async fn localization_write_and_suspension_serialize_on_sqlite_writer_reservation() {
    let temp = tempfile::tempdir()
        .expect("temporary Groups localization concurrency directory should create");
    let url = sqlite_fixture_url(&temp);
    let fixture_db = connect(&url).await;
    fixture_db
        .execute_unprepared("PRAGMA journal_mode = WAL;")
        .await
        .expect("SQLite localization concurrency fixture should enable WAL");
    install_groups_schema(&fixture_db).await;

    for round in 0..ROUNDS {
        let tenant_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let admin_id = Uuid::new_v4();
        seed_group_fixture(&fixture_db, tenant_id, group_id, owner_id, admin_id, round).await;

        let setup_localization = GroupLocalizationService::new(fixture_db.clone());
        GroupLocalizationCommandPort::upsert_group_translation(
            &setup_localization,
            write_context(tenant_id, owner_id, "owner-initial-translation"),
            UpsertGroupTranslationRequest {
                group_id,
                locale: "en".to_string(),
                title: format!("Round {round} initial"),
                summary: None,
                body: None,
            },
        )
        .await
        .expect("owner should create the initial translation before the SQLite race");

        let localization_db = connect(&url).await;
        let enforcement_db = connect(&url).await;
        let barrier = Arc::new(Barrier::new(3));

        let localization_barrier = barrier.clone();
        let localization_task = tokio::spawn(async move {
            let service = GroupLocalizationService::new(localization_db);
            localization_barrier.wait().await;
            GroupLocalizationCommandPort::upsert_group_translation(
                &service,
                write_context(tenant_id, admin_id, "admin-raced-translation"),
                UpsertGroupTranslationRequest {
                    group_id,
                    locale: "fr".to_string(),
                    title: format!("Round {round} French"),
                    summary: None,
                    body: None,
                },
            )
            .await
        });

        let enforcement_barrier = barrier.clone();
        let enforcement_task = tokio::spawn(async move {
            let service = GroupMembershipEnforcementCommandService::new(enforcement_db);
            enforcement_barrier.wait().await;
            GroupMembershipEnforcementCommandPort::suspend_membership(
                &service,
                write_context(tenant_id, owner_id, "owner-raced-suspension"),
                SuspendGroupMembershipRequest {
                    group_id,
                    target_user_id: admin_id,
                    expected_membership_revision: 1,
                    reason_code: "concurrent_settings_review".to_string(),
                    effective_until: None,
                },
            )
            .await
        });

        barrier.wait().await;
        let localization_result = localization_task
            .await
            .expect("SQLite localization race task should join without panic");
        let suspension = enforcement_task
            .await
            .expect("SQLite enforcement race task should join without panic")
            .expect("owner suspension must serialize to a successful SQLite commit");
        assert_eq!(suspension.membership_revision, 2);
        assert_eq!(suspension.member_count, 2);
        assert_eq!(
            suspension.effective_status,
            GroupMembershipEffectiveStatus::Suspended
        );

        let owner_reader = GroupLocalizationService::new(fixture_db.clone());
        let translations = GroupLocalizationReadPort::list_group_translations(
            &owner_reader,
            read_context(tenant_id, owner_id),
            ListGroupTranslationsRequest { group_id },
        )
        .await
        .expect("owner should inspect translations after the serialized SQLite race");
        let french_present = translations
            .iter()
            .any(|translation| translation.locale == "fr");

        match localization_result {
            Ok(result) => {
                assert!(result.created);
                assert!(
                    french_present,
                    "a SQLite localization commit that won before suspension must remain materialized"
                );
                assert!(
                    result.group_version < suspension.group_version as u64,
                    "when both SQLite commands succeed, localization must serialize before suspension"
                );
            }
            Err(error) => {
                assert_eq!(error.code, "groups.membership_suspended");
                assert!(!error.retryable);
                assert!(
                    !french_present,
                    "a SQLite localization command denied after suspension must not write translation state"
                );
            }
        }

        let effective_reader = GroupMembershipEnforcementService::new(fixture_db.clone());
        let effective = GroupMembershipEnforcementReadPort::read_membership_enforcement(
            &effective_reader,
            owner_effective_read_context(tenant_id, owner_id),
            ReadGroupMembershipEnforcementRequest {
                group_id,
                user_id: admin_id,
            },
        )
        .await
        .expect("owner should observe final effective membership after the SQLite race");
        assert_eq!(
            effective.effective_status,
            GroupMembershipEffectiveStatus::Suspended
        );
        assert!(!effective.active_member);
        assert_eq!(effective.membership_revision, Some(2));
    }

    drop(fixture_db);
    drop(temp);
}
