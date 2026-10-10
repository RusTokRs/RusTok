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
                "page publish job storage migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PagePublishJobs::Table)
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
CREATE TABLE IF NOT EXISTS page_publish_jobs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    page_id UUID NOT NULL,
    publish_at TIMESTAMPTZ NOT NULL,
    state VARCHAR(16) NOT NULL,
    command JSONB NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error_code VARCHAR(64) NULL,
    last_error_message TEXT NULL,
    publish_operation_id UUID NULL,
    created_by UUID NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT ck_page_publish_jobs_state
        CHECK (state IN ('scheduled', 'executing', 'published', 'canceled', 'failed')),
    CONSTRAINT fk_page_publish_jobs_page_tenant
        FOREIGN KEY (tenant_id, page_id)
        REFERENCES pages (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_page_publish_jobs_active
    ON page_publish_jobs (tenant_id, page_id)
    WHERE state = 'scheduled';

CREATE INDEX IF NOT EXISTS idx_page_publish_jobs_due
    ON page_publish_jobs (tenant_id, publish_at)
    WHERE state = 'scheduled';

CREATE INDEX IF NOT EXISTS idx_page_publish_jobs_page_created
    ON page_publish_jobs (tenant_id, page_id, created_at);
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        r#"CREATE TABLE IF NOT EXISTS page_publish_jobs (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            page_id TEXT NOT NULL,
            publish_at TIMESTAMP NOT NULL,
            state TEXT NOT NULL,
            command TEXT NOT NULL,
            attempts INTEGER NOT NULL DEFAULT 0,
            last_error_code TEXT NULL,
            last_error_message TEXT NULL,
            publish_operation_id TEXT NULL,
            created_by TEXT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            CONSTRAINT ck_page_publish_jobs_state
                CHECK (state IN ('scheduled', 'executing', 'published', 'canceled', 'failed')),
            CONSTRAINT fk_page_publish_jobs_page_tenant
                FOREIGN KEY (tenant_id, page_id)
                REFERENCES pages (tenant_id, id)
                ON UPDATE CASCADE ON DELETE CASCADE
        )"#,
        r#"CREATE UNIQUE INDEX IF NOT EXISTS uq_page_publish_jobs_active
            ON page_publish_jobs (tenant_id, page_id)
            WHERE state = 'scheduled'"#,
        r#"CREATE INDEX IF NOT EXISTS idx_page_publish_jobs_due
            ON page_publish_jobs (tenant_id, publish_at)
            WHERE state = 'scheduled'"#,
        r#"CREATE INDEX IF NOT EXISTS idx_page_publish_jobs_page_created
            ON page_publish_jobs (tenant_id, page_id, created_at)"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}

#[derive(DeriveIden)]
enum PagePublishJobs {
    Table,
}
