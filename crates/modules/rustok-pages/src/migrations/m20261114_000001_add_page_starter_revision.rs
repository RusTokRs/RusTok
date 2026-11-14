use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

/// Existing rows receive their own ID as an initial revision. New writes always
/// mint a fresh UUID, including after deletion and recreation of the same key.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let ddl = match manager.get_database_backend() {
            DatabaseBackend::Postgres => "ALTER TABLE page_starters ADD COLUMN revision UUID NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000'",
            DatabaseBackend::Sqlite => "ALTER TABLE page_starters ADD COLUMN revision TEXT NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000'",
            backend => return Err(DbErr::Custom(format!("page starter catalog does not support {backend:?}"))),
        };
        manager.get_connection().execute_unprepared(ddl).await?;
        manager.get_connection().execute_unprepared("UPDATE page_starters SET revision = id").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("ALTER TABLE page_starters DROP COLUMN revision").await?;
        Ok(())
    }
}
