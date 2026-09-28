use chrono::Utc;
use rustok_installer_persistence::{InstallerPersistenceService, entities::install_session};
use rustok_migrations::SqliteTestMigrator as Migrator;
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

async fn insert_session(
    db: &sea_orm::DatabaseConnection,
    status: &str,
    created_at: chrono::DateTime<Utc>,
) {
    install_session::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(None),
        status: Set(status.to_string()),
        profile: Set("monolith".to_string()),
        environment: Set("production".to_string()),
        database_engine: Set("postgres".to_string()),
        seed_profile: Set("minimal".to_string()),
        plan_snapshot: Set(serde_json::json!({})),
        lock_owner: Set(None),
        lock_expires_at: Set(None),
        error_message: Set(None),
        created_by: Set(None),
        created_at: Set(created_at),
        updated_at: Set(created_at),
        completed_at: Set((status == "completed").then_some(created_at)),
    }
    .insert(db)
    .await
    .expect("installer session should insert");
}

#[tokio::test]
async fn historical_completed_session_stays_closed_after_later_failure() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let persistence = InstallerPersistenceService::new(db.clone());
    let now = Utc::now();

    insert_session(&db, "completed", now).await;
    insert_session(&db, "failed", now + chrono::Duration::seconds(1)).await;

    assert!(
        persistence
            .has_completed_session()
            .await
            .expect("completed-session query should succeed")
    );
}

#[tokio::test]
async fn setup_remains_open_when_no_completed_session_exists() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let persistence = InstallerPersistenceService::new(db.clone());

    insert_session(&db, "failed", Utc::now()).await;

    assert!(
        !persistence
            .has_completed_session()
            .await
            .expect("completed-session query should succeed")
    );
}
