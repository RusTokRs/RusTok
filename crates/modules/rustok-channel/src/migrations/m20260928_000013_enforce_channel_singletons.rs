use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        UPDATE channels
                        SET is_default = false
                        WHERE id IN (
                            SELECT id
                            FROM (
                                SELECT
                                    id,
                                    ROW_NUMBER() OVER (
                                        PARTITION BY tenant_id
                                        ORDER BY created_at ASC, id ASC
                                    ) AS row_number
                                FROM channels
                                WHERE is_default = true AND is_active = true
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
                        UPDATE channel_targets
                        SET is_primary = false
                        WHERE id IN (
                            SELECT id
                            FROM (
                                SELECT
                                    id,
                                    ROW_NUMBER() OVER (
                                        PARTITION BY channel_id
                                        ORDER BY created_at ASC, id ASC
                                    ) AS row_number
                                FROM channel_targets
                                WHERE is_primary = true
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
                        CREATE UNIQUE INDEX IF NOT EXISTS ux_channels_one_active_default_per_tenant
                        ON channels (tenant_id)
                        WHERE is_default = true AND is_active = true
                        "#,
                    )
                    .await?;
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE UNIQUE INDEX IF NOT EXISTS ux_channel_targets_one_primary_per_channel
                        ON channel_targets (channel_id)
                        WHERE is_primary = true
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if matches!(manager.get_database_backend(), DatabaseBackend::Postgres | DatabaseBackend::Sqlite) {
            manager
                .get_connection()
                .execute_unprepared("DROP INDEX IF EXISTS ux_channel_targets_one_primary_per_channel")
                .await?;
            manager
                .get_connection()
                .execute_unprepared("DROP INDEX IF EXISTS ux_channels_one_active_default_per_tenant")
                .await?;
        }
        Ok(())
    }
}
