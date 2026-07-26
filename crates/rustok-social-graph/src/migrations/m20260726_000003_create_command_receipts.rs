use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = match manager.get_database_backend() {
            DatabaseBackend::Postgres => POSTGRES_UP,
            DatabaseBackend::Sqlite => SQLITE_UP,
            backend => {
                return Err(DbErr::Custom(format!(
                    "social graph command receipts do not support database backend {backend:?}"
                )));
            }
        };
        manager
            .get_connection()
            .execute_unprepared(sql)
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS social_graph_command_receipts;")
            .await
            .map(|_| ())
    }
}

const POSTGRES_UP: &str = r#"
CREATE TABLE IF NOT EXISTS social_graph_command_receipts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    idempotency_key VARCHAR(191) NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,
    request_json JSONB NOT NULL,
    status VARCHAR(16) NOT NULL,
    response_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMPTZ,
    CONSTRAINT ck_social_graph_receipt_key CHECK (
        length(idempotency_key) BETWEEN 1 AND 191
    ),
    CONSTRAINT ck_social_graph_receipt_schema_version CHECK (schema_version = 1),
    CONSTRAINT ck_social_graph_receipt_status CHECK (
        status IN ('processing', 'completed')
    ),
    CONSTRAINT ck_social_graph_receipt_completion CHECK (
        (status = 'processing' AND response_json IS NULL AND completed_at IS NULL)
        OR
        (status = 'completed' AND response_json IS NOT NULL AND completed_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_social_graph_command_receipt_identity
    ON social_graph_command_receipts (tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_social_graph_command_receipt_created
    ON social_graph_command_receipts (tenant_id, created_at, id);
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS social_graph_command_receipts (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,
    request_json TEXT NOT NULL,
    status TEXT NOT NULL,
    response_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE,
    CHECK (length(idempotency_key) BETWEEN 1 AND 191),
    CHECK (schema_version = 1),
    CHECK (status IN ('processing', 'completed')),
    CHECK (
        (status = 'processing' AND response_json IS NULL AND completed_at IS NULL)
        OR
        (status = 'completed' AND response_json IS NOT NULL AND completed_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_social_graph_command_receipt_identity
    ON social_graph_command_receipts (tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_social_graph_command_receipt_created
    ON social_graph_command_receipts (tenant_id, created_at, id);
"#;
