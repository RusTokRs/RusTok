use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                "ALTER TABLE search_documents ALTER COLUMN locale TYPE VARCHAR(32)"
            }
            DatabaseBackend::MySql => {
                "ALTER TABLE search_documents MODIFY COLUMN locale VARCHAR(32) NOT NULL"
            }
            DatabaseBackend::Sqlite => return Ok(()),
            _ => {
                return Err(DbErr::Migration(
                    "search document locale migration does not support this database backend"
                        .into(),
                ));
            }
        };
        manager.get_connection().execute_unprepared(sql).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Narrowing can truncate a valid platform locale.
        Ok(())
    }
}
