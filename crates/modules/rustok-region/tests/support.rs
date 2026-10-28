use rustok_core::MigrationSource;
use rustok_region::RegionModule;
use sea_orm::{DatabaseConnection, DbBackend};
use sea_orm_migration::SchemaManager;

pub async fn ensure_region_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    let manager = SchemaManager::new(db);
    for migration in RegionModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("region migration should apply");
    }
}
