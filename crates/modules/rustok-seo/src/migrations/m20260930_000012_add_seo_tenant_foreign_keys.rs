use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const TENANT_FK_TABLES: &[(&str, &str)] = &[
    ("seo_redirects", "fk_seo_redirects_tenant"),
    ("seo_revisions", "fk_seo_revisions_tenant"),
    ("seo_sitemap_jobs", "fk_seo_sitemap_jobs_tenant"),
    ("seo_sitemap_files", "fk_seo_sitemap_files_tenant"),
    ("seo_bulk_jobs", "fk_seo_bulk_jobs_tenant"),
    ("seo_bulk_job_items", "fk_seo_bulk_job_items_tenant"),
    ("seo_bulk_job_artifacts", "fk_seo_bulk_job_artifacts_tenant"),
    ("seo_event_deliveries", "fk_seo_event_deliveries_tenant"),
    ("seo_index_deliveries", "fk_seo_index_deliveries_tenant"),
    ("seo_index_cursors", "fk_seo_index_cursors_tenant"),
    ("seo_index_repair_jobs", "fk_seo_index_repair_jobs_tenant"),
    (
        "seo_translation_resource_state",
        "fk_seo_translation_resource_state_tenant",
    ),
    (
        "seo_translation_locale_state",
        "fk_seo_translation_locale_state_tenant",
    ),
    (
        "seo_translation_change_journal",
        "fk_seo_translation_change_journal_tenant",
    ),
    (
        "seo_translation_apply_receipts",
        "fk_seo_translation_apply_receipts_tenant",
    ),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        let statements = TENANT_FK_TABLES
            .iter()
            .map(|(table, constraint)| {
                format!(
                    "DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = '{constraint}') THEN ALTER TABLE {table} ADD CONSTRAINT {constraint} FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE; END IF; END $$;"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        manager
            .get_connection()
            .execute_unprepared(statements.as_str())
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        let statements = TENANT_FK_TABLES
            .iter()
            .map(|(table, constraint)| {
                format!("ALTER TABLE {table} DROP CONSTRAINT IF EXISTS {constraint};")
            })
            .collect::<Vec<_>>()
            .join("\n");
        manager
            .get_connection()
            .execute_unprepared(statements.as_str())
            .await
            .map(|_| ())
    }
}
