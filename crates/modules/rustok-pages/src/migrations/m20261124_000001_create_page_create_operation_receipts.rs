use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

/// Durable idempotency receipts for page creation, including the initial document.
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
CREATE TABLE IF NOT EXISTS page_create_operation_receipts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    actor_id UUID NULL,
    idempotency_key VARCHAR(128) NOT NULL,
    request_hash VARCHAR(64) NOT NULL,
    page_id UUID NOT NULL,
    response_json TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT uq_page_create_operation_receipt_request
        UNIQUE (tenant_id, idempotency_key)
);
CREATE INDEX IF NOT EXISTS idx_page_create_operation_receipts_retention
    ON page_create_operation_receipts (tenant_id, created_at DESC, id DESC);
"#,
                )
                .await?;
            }
            DatabaseBackend::Sqlite => {
                conn.execute_unprepared(
                    r#"
CREATE TABLE IF NOT EXISTS page_create_operation_receipts (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    actor_id TEXT NULL,
    idempotency_key TEXT NOT NULL CHECK(length(idempotency_key) BETWEEN 1 AND 128),
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    page_id TEXT NOT NULL,
    response_json TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    CONSTRAINT uq_page_create_operation_receipt_request
        UNIQUE (tenant_id, idempotency_key)
);
CREATE INDEX IF NOT EXISTS idx_page_create_operation_receipts_retention
    ON page_create_operation_receipts (tenant_id, created_at DESC, id DESC);
"#,
                )
                .await?;
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "page create operation receipts migration does not support {backend:?}"
                )));
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PageCreateOperationReceipts::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PageCreateOperationReceipts {
    Table,
}
