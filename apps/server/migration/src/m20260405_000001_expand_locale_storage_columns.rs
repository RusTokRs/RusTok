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

        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE tenants
    ALTER COLUMN default_locale TYPE VARCHAR(32);

ALTER TABLE tenant_locales
    ALTER COLUMN locale TYPE VARCHAR(32),
    ALTER COLUMN fallback_locale TYPE VARCHAR(32);
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE tenant_locales
    ALTER COLUMN fallback_locale TYPE VARCHAR(5),
    ALTER COLUMN locale TYPE VARCHAR(5);

ALTER TABLE tenants
    ALTER COLUMN default_locale TYPE VARCHAR(5);
"#,
            )
            .await?;

        Ok(())
    }
}
