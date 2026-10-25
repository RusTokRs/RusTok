use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Replaces caller-attested readiness receipts with execution-bound,
/// short-lived evidence.
///
/// Existing rows are deliberately discarded: they may have been minted from
/// a caller-supplied `smoke_test_passed` boolean, carry no smoke binding or
/// output digest, and have no validity deadline. Backfilling them would
/// preserve precisely the fabricated attestation this migration removes.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "DROP TABLE IF EXISTS module_executor_readiness_receipts",
                "CREATE TABLE module_executor_readiness_receipts (\
                    id UUID PRIMARY KEY,\
                    operation_id UUID NOT NULL,\
                    installation_id UUID NOT NULL,\
                    release_digest TEXT NOT NULL CHECK (release_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    payload_digest TEXT NOT NULL CHECK (payload_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    runtime_fingerprint TEXT NOT NULL CHECK (runtime_fingerprint ~ '^sha256:[0-9a-f]{64}$'),\
                    pool_id TEXT NOT NULL CHECK (length(trim(pool_id)) > 0),\
                    pool_generation BIGINT NOT NULL CHECK (pool_generation > 0),\
                    placement TEXT NOT NULL CHECK (placement IN ('in_process', 'isolated_worker')),\
                    placement_policy_revision BIGINT NOT NULL CHECK (placement_policy_revision > 0),\
                    capability_routes_verified BOOLEAN NOT NULL DEFAULT FALSE,\
                    smoke_passed BOOLEAN NOT NULL DEFAULT FALSE,\
                    smoke_binding_id TEXT NOT NULL CHECK (length(trim(smoke_binding_id)) BETWEEN 1 AND 256),\
                    smoke_output_digest TEXT NOT NULL CHECK (smoke_output_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    evaluated_at TIMESTAMPTZ NOT NULL,\
                    valid_until TIMESTAMPTZ NOT NULL CHECK (valid_until > evaluated_at)\
                )",
                "CREATE INDEX idx_executor_readiness_lookup \
                 ON module_executor_readiness_receipts (release_digest, runtime_fingerprint, pool_generation)",
                "CREATE INDEX idx_executor_readiness_operation \
                 ON module_executor_readiness_receipts (operation_id, installation_id)",
                "ALTER TABLE module_executor_readiness_receipts ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_executor_readiness_receipts_scope \
                 ON module_executor_readiness_receipts \
                 USING (\
                    EXISTS (\
                        SELECT 1 FROM module_admitted_oci_releases AS r \
                        WHERE r.release_digest = module_executor_readiness_receipts.release_digest \
                          AND (r.scope_kind = 'platform' OR r.scope_tenant_key = current_setting('rustok.tenant_id', true))\
                    )\
                 ) \
                 WITH CHECK (\
                    EXISTS (\
                        SELECT 1 FROM module_admitted_oci_releases AS r \
                        WHERE r.release_digest = module_executor_readiness_receipts.release_digest \
                          AND (r.scope_kind = 'platform' OR r.scope_tenant_key = current_setting('rustok.tenant_id', true))\
                    )\
                 )",
            ],
            SupportedBackend::Sqlite => &[
                "DROP TABLE IF EXISTS module_executor_readiness_receipts",
                "CREATE TABLE module_executor_readiness_receipts (\
                    id TEXT PRIMARY KEY NOT NULL,\
                    operation_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL,\
                    release_digest TEXT NOT NULL CHECK (length(release_digest) = 71),\
                    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71),\
                    runtime_fingerprint TEXT NOT NULL CHECK (length(runtime_fingerprint) = 71),\
                    pool_id TEXT NOT NULL CHECK (length(trim(pool_id)) > 0),\
                    pool_generation INTEGER NOT NULL CHECK (pool_generation > 0),\
                    placement TEXT NOT NULL CHECK (placement IN ('in_process', 'isolated_worker')),\
                    placement_policy_revision INTEGER NOT NULL CHECK (placement_policy_revision > 0),\
                    capability_routes_verified INTEGER NOT NULL DEFAULT 0,\
                    smoke_passed INTEGER NOT NULL DEFAULT 0,\
                    smoke_binding_id TEXT NOT NULL CHECK (length(trim(smoke_binding_id)) BETWEEN 1 AND 256),\
                    smoke_output_digest TEXT NOT NULL CHECK (length(smoke_output_digest) = 71),\
                    evaluated_at TEXT NOT NULL,\
                    valid_until TEXT NOT NULL CHECK (valid_until > evaluated_at)\
                )",
                "CREATE INDEX idx_executor_readiness_lookup \
                 ON module_executor_readiness_receipts (release_digest, runtime_fingerprint, pool_generation)",
                "CREATE INDEX idx_executor_readiness_operation \
                 ON module_executor_readiness_receipts (operation_id, installation_id)",
            ],
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
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "DROP TABLE IF EXISTS module_executor_readiness_receipts",
                "CREATE TABLE module_executor_readiness_receipts (\
                    id UUID PRIMARY KEY,\
                    operation_id UUID NOT NULL,\
                    installation_id UUID NOT NULL,\
                    release_digest TEXT NOT NULL CHECK (release_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    payload_digest TEXT NOT NULL CHECK (payload_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    runtime_fingerprint TEXT NOT NULL CHECK (runtime_fingerprint ~ '^sha256:[0-9a-f]{64}$'),\
                    pool_id TEXT NOT NULL CHECK (length(trim(pool_id)) > 0),\
                    pool_generation BIGINT NOT NULL CHECK (pool_generation > 0),\
                    placement TEXT NOT NULL CHECK (placement IN ('in_process', 'isolated_worker')),\
                    placement_policy_revision BIGINT NOT NULL CHECK (placement_policy_revision >= 0),\
                    capability_routes_verified BOOLEAN NOT NULL DEFAULT FALSE,\
                    smoke_passed BOOLEAN NOT NULL DEFAULT FALSE,\
                    evaluated_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX idx_executor_readiness_lookup \
                 ON module_executor_readiness_receipts (release_digest, runtime_fingerprint, pool_generation)",
                "CREATE INDEX idx_executor_readiness_operation \
                 ON module_executor_readiness_receipts (operation_id, installation_id)",
                "ALTER TABLE module_executor_readiness_receipts ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_executor_readiness_receipts_scope \
                 ON module_executor_readiness_receipts \
                 USING (\
                    EXISTS (\
                        SELECT 1 FROM module_admitted_oci_releases AS r \
                        WHERE r.release_digest = module_executor_readiness_receipts.release_digest \
                          AND (r.scope_kind = 'platform' OR r.scope_tenant_key = current_setting('rustok.tenant_id', true))\
                    )\
                 ) \
                 WITH CHECK (\
                    EXISTS (\
                        SELECT 1 FROM module_admitted_oci_releases AS r \
                        WHERE r.release_digest = module_executor_readiness_receipts.release_digest \
                          AND (r.scope_kind = 'platform' OR r.scope_tenant_key = current_setting('rustok.tenant_id', true))\
                    )\
                 )",
            ],
            SupportedBackend::Sqlite => &[
                "DROP TABLE IF EXISTS module_executor_readiness_receipts",
                "CREATE TABLE module_executor_readiness_receipts (\
                    id TEXT PRIMARY KEY NOT NULL,\
                    operation_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL,\
                    release_digest TEXT NOT NULL CHECK (length(release_digest) = 71),\
                    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71),\
                    runtime_fingerprint TEXT NOT NULL CHECK (length(runtime_fingerprint) = 71),\
                    pool_id TEXT NOT NULL CHECK (length(trim(pool_id)) > 0),\
                    pool_generation INTEGER NOT NULL CHECK (pool_generation > 0),\
                    placement TEXT NOT NULL CHECK (placement IN ('in_process', 'isolated_worker')),\
                    placement_policy_revision INTEGER NOT NULL CHECK (placement_policy_revision >= 0),\
                    capability_routes_verified INTEGER NOT NULL DEFAULT 0,\
                    smoke_passed INTEGER NOT NULL DEFAULT 0,\
                    evaluated_at TEXT NOT NULL\
                )",
                "CREATE INDEX idx_executor_readiness_lookup \
                 ON module_executor_readiness_receipts (release_digest, runtime_fingerprint, pool_generation)",
                "CREATE INDEX idx_executor_readiness_operation \
                 ON module_executor_readiness_receipts (operation_id, installation_id)",
            ],
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
}
