use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                for statement in [
                    "ALTER TABLE index_content ALTER COLUMN locale TYPE VARCHAR(32)",
                    "ALTER TABLE index_products ALTER COLUMN locale TYPE VARCHAR(32)",
                ] {
                    manager
                        .get_connection()
                        .execute_unprepared(statement)
                        .await?;
                }
            }
            DatabaseBackend::MySql => {
                for statement in [
                    "ALTER TABLE index_content MODIFY COLUMN locale VARCHAR(32) NOT NULL",
                    "ALTER TABLE index_products MODIFY COLUMN locale VARCHAR(32) NOT NULL",
                ] {
                    manager
                        .get_connection()
                        .execute_unprepared(statement)
                        .await?;
                }
            }
            DatabaseBackend::Sqlite => {
                // SQLite does not enforce declared VARCHAR lengths. Both projection
                // columns already use TEXT affinity, so widening requires no rebuild.
            }
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: narrowing localized projection locale columns can truncate
        // valid normalized BCP47-like tags already indexed by tenants.
        Ok(())
    }
}
