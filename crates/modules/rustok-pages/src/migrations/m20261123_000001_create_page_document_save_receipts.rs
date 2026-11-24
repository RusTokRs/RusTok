use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

/// Append-only PageService save IDs. The full response is retained for the
/// most recent 64 saves per page; older key/hash tombstones remain indefinitely.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                conn.execute_unprepared(r#"
CREATE TABLE IF NOT EXISTS page_document_save_receipts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    page_id UUID NOT NULL,
    actor_id UUID NULL,
    idempotency_key VARCHAR(128) NOT NULL,
    request_hash VARCHAR(64) NOT NULL,
    result_revision VARCHAR(128) NOT NULL,
    response_json TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT uq_page_document_save_receipt_request UNIQUE (tenant_id, page_id, idempotency_key),
    CONSTRAINT fk_page_document_save_receipt_page_tenant
        FOREIGN KEY (tenant_id, page_id)
        REFERENCES pages (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_page_document_save_receipts_retention
    ON page_document_save_receipts (tenant_id, page_id, created_at DESC, id DESC);
"#).await?;
            }
            DatabaseBackend::Sqlite => {
                conn.execute_unprepared(r#"
CREATE TABLE IF NOT EXISTS page_document_save_receipts (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    page_id TEXT NOT NULL,
    actor_id TEXT NULL,
    idempotency_key TEXT NOT NULL CHECK(length(idempotency_key) BETWEEN 1 AND 128),
    request_hash TEXT NOT NULL,
    result_revision TEXT NOT NULL,
    response_json TEXT NULL,
    created_at TIMESTAMP NOT NULL,
    CONSTRAINT uq_page_document_save_receipt_request UNIQUE (tenant_id, page_id, idempotency_key),
    CONSTRAINT fk_page_document_save_receipt_page_tenant
        FOREIGN KEY (tenant_id, page_id)
        REFERENCES pages (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
);
"#).await?;
                conn.execute_unprepared(
                    "CREATE INDEX IF NOT EXISTS idx_page_document_save_receipts_retention
                     ON page_document_save_receipts (tenant_id, page_id, created_at DESC, id DESC)"
                ).await?;
            }
            backend => return Err(DbErr::Custom(format!(
                "page document save receipts migration does not support {backend:?}"
            ))),
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(PageDocumentSaveReceipts::Table).if_exists().to_owned()).await
    }
}

#[derive(DeriveIden)]
enum PageDocumentSaveReceipts {
    Table,
}
