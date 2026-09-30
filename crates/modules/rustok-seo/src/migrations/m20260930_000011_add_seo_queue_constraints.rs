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
            // Make the invariant installable on databases that already contain duplicates from
            // the old check-then-insert race. Keep the oldest active row and fail the rest.
            manager
                .get_connection()
                .execute_unprepared(
                    "WITH ranked AS (SELECT id, ROW_NUMBER() OVER (PARTITION BY tenant_id ORDER BY created_at ASC, id ASC) AS row_number FROM seo_index_repair_jobs WHERE status IN ('queued', 'running')) UPDATE seo_index_repair_jobs AS jobs SET status = 'failed', last_error = 'superseded duplicate active job during queue constraint migration', completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP FROM ranked WHERE jobs.id = ranked.id AND ranked.row_number > 1",
                )
                .await?;
            manager
                .get_connection()
                .execute_unprepared(
                    "WITH ranked AS (SELECT id, ROW_NUMBER() OVER (PARTITION BY tenant_id ORDER BY created_at ASC, id ASC) AS row_number FROM seo_sitemap_jobs WHERE status IN ('queued', 'running', 'submitting')) UPDATE seo_sitemap_jobs AS jobs SET status = 'failed', last_error = 'superseded duplicate active job during queue constraint migration', completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP FROM ranked WHERE jobs.id = ranked.id AND ranked.row_number > 1",
                )
                .await?;
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE UNIQUE INDEX IF NOT EXISTS idx_seo_index_repair_jobs_one_active_per_tenant ON seo_index_repair_jobs (tenant_id) WHERE status IN ('queued', 'running')",
                )
                .await?;
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE UNIQUE INDEX IF NOT EXISTS idx_seo_sitemap_jobs_one_active_per_tenant ON seo_sitemap_jobs (tenant_id) WHERE status IN ('queued', 'running', 'submitting')",
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
                    "DROP INDEX IF EXISTS idx_seo_sitemap_jobs_one_active_per_tenant",
                )
                .await?;
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
