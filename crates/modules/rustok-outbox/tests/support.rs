//! Shared SQLite setup for outbox integration tests.

use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigrationTrait;
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

use rustok_outbox::SysEventsMigration;

pub async fn setup_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:tx_events_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);

    let db = Database::connect(opts)
        .await
        .expect("Failed to connect test sqlite database");

    let schema_manager = SchemaManager::new(&db);
    SysEventsMigration
        .up(&schema_manager)
        .await
        .expect("Failed to run outbox migration");

    db
}
