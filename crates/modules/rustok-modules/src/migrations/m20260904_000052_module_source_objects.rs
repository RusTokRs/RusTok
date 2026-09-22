use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists immutable source object receipts and retention holds.
///
/// Under the canonical release-safety contract, `SourceObjectStore` owns
/// globally deduplicated create-only `source_digest` blobs, with owner/RLS-scoped
/// `source_receipt_id` records over preparation domain, source digest, media type,
/// length, and manifest.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_source_object_receipts (\
                    source_receipt_id UUID PRIMARY KEY,\
                    preparation_id UUID NOT NULL,\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    source_digest TEXT NOT NULL CHECK (source_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    media_type TEXT NOT NULL CHECK (length(trim(media_type)) > 0),\
                    byte_length BIGINT NOT NULL CHECK (byte_length >= 0),\
                    manifest_digest TEXT CHECK (manifest_digest IS NULL OR manifest_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    CONSTRAINT uq_source_receipt_preparation_digest UNIQUE (preparation_id, source_digest)\
                )",
                "CREATE INDEX idx_source_object_receipts_digest \
                 ON module_source_object_receipts (source_digest)",
                "ALTER TABLE module_source_object_receipts ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_source_object_receipts_scope \
                 ON module_source_object_receipts \
                 USING (scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_source_object_retention_holds (\
                    hold_id UUID PRIMARY KEY,\
                    source_digest TEXT NOT NULL CHECK (source_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    held_by TEXT NOT NULL CHECK (length(trim(held_by)) > 0),\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    expires_at TIMESTAMPTZ,\
                    created_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX idx_source_object_retention_holds_digest \
                 ON module_source_object_retention_holds (source_digest)",
            ],
            _ => &[
                "CREATE TABLE module_source_object_receipts (\
                    source_receipt_id TEXT PRIMARY KEY,\
                    preparation_id TEXT NOT NULL,\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71 AND substr(source_digest, 1, 7) = 'sha256:' AND substr(source_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    media_type TEXT NOT NULL CHECK (length(trim(media_type)) > 0),\
                    byte_length INTEGER NOT NULL CHECK (byte_length >= 0),\
                    manifest_digest TEXT CHECK (manifest_digest IS NULL OR (length(manifest_digest) = 71 AND substr(manifest_digest, 1, 7) = 'sha256:' AND substr(manifest_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    created_at TEXT NOT NULL,\
                    CONSTRAINT uq_source_receipt_preparation_digest UNIQUE (preparation_id, source_digest)\
                )",
                "CREATE INDEX idx_source_object_receipts_digest \
                 ON module_source_object_receipts (source_digest)",
                "CREATE TABLE module_source_object_retention_holds (\
                    hold_id TEXT PRIMARY KEY,\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71 AND substr(source_digest, 1, 7) = 'sha256:' AND substr(source_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    held_by TEXT NOT NULL CHECK (length(trim(held_by)) > 0),\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    expires_at TEXT,\
                    created_at TEXT NOT NULL\
                )",
                "CREATE INDEX idx_source_object_retention_holds_digest \
                 ON module_source_object_retention_holds (source_digest)",
            ],
        };

        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = &[
            "DROP TABLE IF EXISTS module_source_object_retention_holds",
            "DROP TABLE IF EXISTS module_source_object_receipts",
        ];

        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }

        Ok(())
    }
}
