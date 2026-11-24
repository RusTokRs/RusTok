use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

/// Durable receipts for reversible and destructive page lifecycle commands.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                conn.execute_unprepared(
                    r#"
CREATE TABLE IF NOT EXISTS page_lifecycle_operation_receipts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    page_id UUID NOT NULL,
    owner_id UUID NULL,
    operation VARCHAR(32) NOT NULL,
    actor_id UUID NULL,
    idempotency_key VARCHAR(128) NOT NULL,
    request_hash VARCHAR(64) NOT NULL,
    response_json TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT uq_page_lifecycle_operation_receipt_request
        UNIQUE (tenant_id, page_id, operation, idempotency_key)
);
CREATE INDEX IF NOT EXISTS idx_page_lifecycle_operation_receipts_retention
    ON page_lifecycle_operation_receipts (tenant_id, page_id, created_at DESC, id DESC);
"#,
                )
                .await?;
            }
            DatabaseBackend::Sqlite => {
                conn.execute_unprepared(
                    r#"
CREATE TABLE IF NOT EXISTS page_lifecycle_operation_receipts (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    page_id TEXT NOT NULL,
    owner_id TEXT NULL,
    operation TEXT NOT NULL CHECK(length(operation) BETWEEN 1 AND 32),
    actor_id TEXT NULL,
    idempotency_key TEXT NOT NULL CHECK(length(idempotency_key) BETWEEN 1 AND 128),
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    response_json TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    CONSTRAINT uq_page_lifecycle_operation_receipt_request
        UNIQUE (tenant_id, page_id, operation, idempotency_key)
);
CREATE INDEX IF NOT EXISTS idx_page_lifecycle_operation_receipts_retention
    ON page_lifecycle_operation_receipts (tenant_id, page_id, created_at DESC, id DESC);
"#,
                )
                .await?;
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "page lifecycle operation receipts migration does not support {backend:?}"
                )));
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PageLifecycleOperationReceipts::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PageLifecycleOperationReceipts {
    Table,
}
