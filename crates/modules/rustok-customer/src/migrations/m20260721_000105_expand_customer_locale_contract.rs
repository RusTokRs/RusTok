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
                        "ALTER TABLE customers ALTER COLUMN locale TYPE VARCHAR(32)",
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE customers MODIFY COLUMN locale VARCHAR(32) NULL",
                    )
                    .await?;
            }
            // SQLite does not enforce VARCHAR length, so widening the declared
            // type would be a no-op (rebuild_sqlite_customers is omitted). Rebuilding
            // the table here is unsafe because SQLite rewrites dependent trigger
            // references during RENAME TABLE.
            DatabaseBackend::Sqlite => {}
            _ => {
                return Err(DbErr::Migration(
                    "customer locale migration does not support this database backend".to_string(),
                ));
            }
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: narrowing the locale column risks truncating valid normalized tags.
        Ok(())
    }
}
