use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE search_query_logs ALTER COLUMN locale TYPE VARCHAR(32)",
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE search_query_logs MODIFY COLUMN locale VARCHAR(32) NULL",
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                // SQLite does not enforce declared VARCHAR lengths; existing values already
                // use TEXT affinity and require no table rewrite.
            }
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: narrowing an observed locale can truncate a valid normalized tag.
        Ok(())
    }
}
