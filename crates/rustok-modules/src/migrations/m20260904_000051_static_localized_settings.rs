use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Adds exact-locale storage for owner-declared localized leaves of static
/// module settings. Language-neutral settings remain in `tenant_modules`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_static_localized_settings (\
                    tenant_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) BETWEEN 1 AND 191),\
                    field_id TEXT NOT NULL CHECK (length(trim(field_id)) BETWEEN 1 AND 128),\
                    locale VARCHAR(32) NOT NULL CHECK (length(trim(locale)) BETWEEN 1 AND 32),\
                    value TEXT NOT NULL,\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    owner_revision BIGINT NOT NULL CHECK (owner_revision > 0),\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\
                    PRIMARY KEY (tenant_id, module_slug, field_id, locale)\
                )",
                "CREATE INDEX idx_module_static_localized_settings_locale \
                 ON module_static_localized_settings (tenant_id, module_slug, locale, field_id)",
                "ALTER TABLE module_static_localized_settings ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_static_localized_settings_scope \
                 ON module_static_localized_settings \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_static_localized_settings (\
                    tenant_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) BETWEEN 1 AND 191),\
                    field_id TEXT NOT NULL CHECK (length(trim(field_id)) BETWEEN 1 AND 128),\
                    locale TEXT NOT NULL CHECK (length(trim(locale)) BETWEEN 1 AND 32),\
                    value TEXT NOT NULL,\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    owner_revision INTEGER NOT NULL CHECK (owner_revision > 0),\
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    PRIMARY KEY (tenant_id, module_slug, field_id, locale)\
                )",
                "CREATE INDEX idx_module_static_localized_settings_locale \
                 ON module_static_localized_settings (tenant_id, module_slug, locale, field_id)",
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
            .execute_raw(Statement::from_string(
                manager.get_database_backend(),
                "DROP TABLE IF EXISTS module_static_localized_settings".to_string(),
            ))
            .await?;
        Ok(())
    }
}
