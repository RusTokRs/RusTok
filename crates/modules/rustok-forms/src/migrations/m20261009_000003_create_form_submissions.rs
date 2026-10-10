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
                "form submission storage migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(FormSubmissions::Table)
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
CREATE TABLE IF NOT EXISTS form_submissions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    form_id VARCHAR(64) NOT NULL,
    locale VARCHAR(16) NOT NULL,
    page_id UUID NULL,
    payload JSONB NOT NULL,
    state VARCHAR(16) NOT NULL,
    ip_hash CHAR(64) NOT NULL,
    user_agent TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    handled_at TIMESTAMPTZ NULL,
    handled_by UUID NULL,
    CONSTRAINT ck_form_submissions_state
        CHECK (state IN ('new', 'read', 'handled', 'spam'))
);

CREATE INDEX IF NOT EXISTS idx_form_submissions_tenant_form_created
    ON form_submissions (tenant_id, form_id, created_at);

CREATE INDEX IF NOT EXISTS idx_form_submissions_tenant_state_created
    ON form_submissions (tenant_id, state, created_at);

CREATE INDEX IF NOT EXISTS idx_form_submissions_rate_window
    ON form_submissions (tenant_id, form_id, ip_hash, created_at);
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        r#"CREATE TABLE IF NOT EXISTS form_submissions (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            form_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            page_id TEXT NULL,
            payload TEXT NOT NULL,
            state TEXT NOT NULL,
            ip_hash TEXT NOT NULL,
            user_agent TEXT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            handled_at TIMESTAMP NULL,
            handled_by TEXT NULL,
            CONSTRAINT ck_form_submissions_state
                CHECK (state IN ('new', 'read', 'handled', 'spam'))
        )"#,
        r#"CREATE INDEX IF NOT EXISTS idx_form_submissions_tenant_form_created
            ON form_submissions (tenant_id, form_id, created_at)"#,
        r#"CREATE INDEX IF NOT EXISTS idx_form_submissions_tenant_state_created
            ON form_submissions (tenant_id, state, created_at)"#,
        r#"CREATE INDEX IF NOT EXISTS idx_form_submissions_rate_window
            ON form_submissions (tenant_id, form_id, ip_hash, created_at)"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}

#[derive(DeriveIden)]
enum FormSubmissions {
    Table,
}
