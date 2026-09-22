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
                DELETE FROM search_settings
                WHERE id IN (
                    SELECT id
                    FROM (
                        SELECT
                            id,
                            ROW_NUMBER() OVER (
                                PARTITION BY tenant_id
                                ORDER BY updated_at DESC, id DESC
                            ) AS row_number
                        FROM search_settings
                    ) ranked
                    WHERE ranked.row_number > 1
                )
                "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE UNIQUE INDEX IF NOT EXISTS uq_search_settings_tenant
                    ON search_settings (tenant_id)
                    WHERE tenant_id IS NOT NULL
                "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE UNIQUE INDEX IF NOT EXISTS uq_search_settings_global
                    ON search_settings ((1))
                    WHERE tenant_id IS NULL
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
            .execute_unprepared("DROP INDEX IF EXISTS uq_search_settings_global")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS uq_search_settings_tenant")
            .await?;

        Ok(())
    }
}
