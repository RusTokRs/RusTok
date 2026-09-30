use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // PostgreSQL is the production database and supports the partial uniqueness required by
        // the queue contract. Completed/failed history remains unbounded by this constraint.
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE UNIQUE INDEX IF NOT EXISTS idx_seo_index_repair_jobs_one_active_per_tenant ON seo_index_repair_jobs (tenant_id) WHERE status IN ('queued', 'running')",
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "DROP INDEX IF EXISTS idx_seo_index_repair_jobs_one_active_per_tenant",
                )
                .await?;
        }
        Ok(())
    }
}
