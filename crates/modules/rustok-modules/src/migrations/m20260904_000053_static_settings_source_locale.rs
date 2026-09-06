use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Stores explicit authoring-locale provenance for the latest static Settings
/// base projection. The revision is the owner revision recorded by the
/// content-free `base_projection` change journal, not the current shared owner
/// revision: target-only localization writes therefore do not invalidate a
/// still-current source-locale assignment.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_static_settings_source_locales (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    locale TEXT NOT NULL CHECK (length(locale) BETWEEN 2 AND 32),\
                    base_projection_revision BIGINT NOT NULL CHECK (base_projection_revision > 0),\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\
                    PRIMARY KEY (tenant_id, module_slug)\
                )",
                "ALTER TABLE module_static_settings_source_locales ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_static_settings_source_locales_scope \
                 ON module_static_settings_source_locales \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_static_settings_source_locales (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    locale TEXT NOT NULL CHECK (length(locale) BETWEEN 2 AND 32),\
                    base_projection_revision INTEGER NOT NULL CHECK (base_projection_revision > 0),\
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    PRIMARY KEY (tenant_id, module_slug)\
                )",
            ],
            _ => return Err(DbErr::Custom("Unsupported database backend".to_string())),
        };

        for sql in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    (*sql).to_string(),
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS module_static_settings_source_locales")
            .await
            .map(|_| ())
    }
}
