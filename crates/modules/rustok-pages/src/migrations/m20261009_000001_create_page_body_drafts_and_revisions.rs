use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "page body draft and revision storage migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PageBodyRevisions::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(PageBodyDrafts::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE UNIQUE INDEX IF NOT EXISTS uq_pages_tenant_id
    ON pages (tenant_id, id);

CREATE TABLE IF NOT EXISTS page_body_drafts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    page_id UUID NOT NULL,
    locale VARCHAR(16) NOT NULL,
    content TEXT NOT NULL,
    format VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by UUID NULL,
    CONSTRAINT uq_page_body_drafts_page_locale UNIQUE (page_id, locale),
    CONSTRAINT fk_page_body_drafts_page_tenant
        FOREIGN KEY (tenant_id, page_id)
        REFERENCES pages (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_page_body_drafts_page_updated
    ON page_body_drafts (tenant_id, page_id, updated_at);

CREATE TABLE IF NOT EXISTS page_body_revisions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    page_id UUID NOT NULL,
    locale VARCHAR(16) NOT NULL,
    content TEXT NOT NULL,
    format VARCHAR(32) NOT NULL,
    source VARCHAR(32) NOT NULL,
    body_revision VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by UUID NULL,
    CONSTRAINT fk_page_body_revisions_page_tenant
        FOREIGN KEY (tenant_id, page_id)
        REFERENCES pages (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_page_body_revisions_page_locale_time
    ON page_body_revisions (tenant_id, page_id, locale, created_at);
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_pages_tenant_id
         ON pages (tenant_id, id)",
        r#"CREATE TABLE IF NOT EXISTS page_body_drafts (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            page_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            content TEXT NOT NULL,
            format TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            created_by TEXT NULL,
            CONSTRAINT uq_page_body_drafts_page_locale UNIQUE (page_id, locale),
            CONSTRAINT fk_page_body_drafts_page_tenant
                FOREIGN KEY (tenant_id, page_id)
                REFERENCES pages (tenant_id, id)
                ON UPDATE CASCADE ON DELETE CASCADE
        )"#,
        "CREATE INDEX IF NOT EXISTS idx_page_body_drafts_page_updated
         ON page_body_drafts (tenant_id, page_id, updated_at)",
        r#"CREATE TABLE IF NOT EXISTS page_body_revisions (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            page_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            content TEXT NOT NULL,
            format TEXT NOT NULL,
            source TEXT NOT NULL,
            body_revision TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            created_by TEXT NULL,
            CONSTRAINT fk_page_body_revisions_page_tenant
                FOREIGN KEY (tenant_id, page_id)
                REFERENCES pages (tenant_id, id)
                ON UPDATE CASCADE ON DELETE CASCADE
        )"#,
        "CREATE INDEX IF NOT EXISTS idx_page_body_revisions_page_locale_time
         ON page_body_revisions (tenant_id, page_id, locale, created_at)",
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}

#[derive(DeriveIden)]
enum PageBodyDrafts {
    Table,
}

#[derive(DeriveIden)]
enum PageBodyRevisions {
    Table,
}
