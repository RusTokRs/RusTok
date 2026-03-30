use rustok_core::MigrationSource;
use rustok_profiles::ProfilesModule;
use rustok_taxonomy::TaxonomyModule;
use rustok_test_utils::db::setup_test_db;
use sea_orm::DatabaseConnection;
use sea_orm_migration::prelude::SchemaManager;

pub async fn setup_profiles_test_db() -> DatabaseConnection {
    let db = setup_test_db().await;
    let schema_manager = SchemaManager::new(&db);

    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("failed to run taxonomy migration");
    }

    for migration in ProfilesModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("failed to run profiles migration");
    }

    db
}
