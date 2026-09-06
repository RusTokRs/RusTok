use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "product media-reference migration requires PostgreSQL".to_string(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE product_images DROP CONSTRAINT IF EXISTS product_images_media_id_fkey",
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Product stores a Media-owned UUID and validates it through the public
        // owner contract. Reintroducing a cross-module storage FK is invalid.
        Ok(())
    }
}
