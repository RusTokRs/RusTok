use rustok_core::MigrationSource;
use rustok_region::RegionModule;
use sea_orm::DatabaseConnection;
use sea_orm_migration::SchemaManager;

pub async fn apply_region_migrations(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in RegionModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("region migration should apply");
    }
}
