use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let ddl = match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                r#"
CREATE TABLE IF NOT EXISTS page_starters (
    id UUID NOT NULL PRIMARY KEY,
    tenant_id UUID NOT NULL,
    starter_key VARCHAR(128) NOT NULL,
    category VARCHAR(64) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    thumbnail_url VARCHAR(1024),
    default_template_key VARCHAR(128) NOT NULL DEFAULT 'default',
    content JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_page_starters_tenant_key UNIQUE (tenant_id, starter_key)
);
CREATE INDEX IF NOT EXISTS idx_page_starters_tenant_category ON page_starters (tenant_id, category);
"#
            }
            DatabaseBackend::Sqlite => {
                r#"
CREATE TABLE IF NOT EXISTS page_starters (
    id TEXT NOT NULL PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    starter_key TEXT NOT NULL,
    category TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    thumbnail_url TEXT,
    default_template_key TEXT NOT NULL DEFAULT 'default',
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (tenant_id, starter_key)
);
CREATE INDEX IF NOT EXISTS idx_page_starters_tenant_category ON page_starters (tenant_id, category);
"#
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "page starter catalog does not support {backend:?}"
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
                    .table(PageStarters::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum PageStarters {
    Table,
}
