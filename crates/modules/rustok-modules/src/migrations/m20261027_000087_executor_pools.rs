use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Durable executor-pool identity registry.
///
/// One row per composed executor pool. `pool_generation` is the monotonic
/// counter smoke receipts bind to: it starts at 1 and advances by one
/// whenever the pool's runtime fingerprint, placement, or placement
/// attestation changes, which is enforced by the registry owner inside one
/// transaction. Rows are platform deployment facts and carry no tenant
/// column, so — like `module_transition_checkpoints` — the table
/// intentionally has no row-level security policy: every read and write is
/// performed by the control-plane owner, never by tenant-scoped callers.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &["CREATE TABLE module_executor_pools (\
                    pool_id TEXT PRIMARY KEY \
                        CHECK (length(trim(pool_id)) BETWEEN 1 AND 128),\
                    placement TEXT NOT NULL \
                        CHECK (placement IN ('in_process', 'isolated_worker')),\
                    fingerprint JSONB NOT NULL,\
                    fingerprint_digest TEXT NOT NULL \
                        CHECK (fingerprint_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    placement_attestation TEXT NULL \
                        CHECK (placement_attestation IS NULL \
                            OR length(trim(placement_attestation)) > 0),\
                    pool_generation BIGINT NOT NULL CHECK (pool_generation > 0),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    CHECK (placement <> 'isolated_worker' \
                        OR placement_attestation IS NOT NULL),\
                    CHECK (updated_at >= created_at)\
                )"],
            SupportedBackend::Sqlite => &["CREATE TABLE module_executor_pools (\
                    pool_id TEXT PRIMARY KEY \
                        CHECK (length(trim(pool_id)) BETWEEN 1 AND 128),\
                    placement TEXT NOT NULL \
                        CHECK (placement IN ('in_process', 'isolated_worker')),\
                    fingerprint JSON NOT NULL,\
                    fingerprint_digest TEXT NOT NULL \
                        CHECK (length(fingerprint_digest) = 71 \
                            AND substr(fingerprint_digest, 1, 7) = 'sha256:' \
                            AND substr(fingerprint_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    placement_attestation TEXT NULL \
                        CHECK (placement_attestation IS NULL \
                            OR length(trim(placement_attestation)) > 0),\
                    pool_generation INTEGER NOT NULL CHECK (pool_generation > 0),\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    CHECK (placement <> 'isolated_worker' \
                        OR placement_attestation IS NOT NULL),\
                    CHECK (updated_at >= created_at)\
                )"],
        };
        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    backend.db_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                "DROP TABLE module_executor_pools".to_string(),
            ))
            .await?;
        Ok(())
    }
}
