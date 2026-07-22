mod support;

use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use support::TestResult;
use support::category_policy::exercise_category_topic_policy;

#[tokio::test]
async fn sqlite_category_topic_policy_blocks_and_restores_topic_writes() -> TestResult<()> {
    let db = setup_sqlite().await?;
    exercise_category_topic_policy(&db).await
}

async fn setup_sqlite() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_category_policy_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    let manager = SchemaManager::new(&db);

    for migration in OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }

    Ok(db)
}
