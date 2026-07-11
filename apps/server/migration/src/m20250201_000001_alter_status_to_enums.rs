use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        // Create Content Status Enum
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TYPE content_status_enum AS ENUM ('draft', 'published', 'archived');",
            )
            .await?;

        // Alter Nodes table
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE nodes 
                 ALTER COLUMN status DROP DEFAULT,
                 ALTER COLUMN status TYPE content_status_enum USING status::content_status_enum,
                 ALTER COLUMN status SET DEFAULT 'draft'::content_status_enum;",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        // Revert Nodes table
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE nodes 
                 ALTER COLUMN status DROP DEFAULT,
                 ALTER COLUMN status TYPE varchar(32) USING status::varchar,
                 ALTER COLUMN status SET DEFAULT 'draft';",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP TYPE content_status_enum;")
            .await?;

        Ok(())
    }
}
