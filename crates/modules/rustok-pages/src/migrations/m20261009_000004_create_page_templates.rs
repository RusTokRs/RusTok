use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Additive catalog only: existing pages and their free-form labels are not
        // rewritten. `default` continues to denote the empty legacy layout.
        let ddl = match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                r#"
CREATE TABLE IF NOT EXISTS page_templates (
    tenant_id UUID NOT NULL,
    locale VARCHAR(35) NOT NULL,
    template_key VARCHAR(128) NOT NULL,
    header_symbol_ids JSONB NOT NULL,
    footer_symbol_ids JSONB NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, locale, template_key),
    CHECK (template_key <> 'default')
);
"#
            }
            DatabaseBackend::Sqlite => {
                r#"
CREATE TABLE IF NOT EXISTS page_templates (
    tenant_id TEXT NOT NULL,
    locale TEXT NOT NULL,
    template_key TEXT NOT NULL,
    header_symbol_ids TEXT NOT NULL,
    footer_symbol_ids TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, locale, template_key),
    CHECK (template_key <> 'default')
);
"#
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "page template catalog does not support {backend:?}"
                )));
            }
        };
        manager.get_connection().execute_unprepared(ddl).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PageTemplates::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum PageTemplates {
    Table,
}
