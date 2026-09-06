use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

/// Durable state persistence for module transitions and artifact retention holds.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_transition_checkpoints (\
                    operation_id UUID PRIMARY KEY,\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) > 0),\
                    tenant_id UUID NULL,\
                    predecessor_digest TEXT NULL,\
                    candidate_digest TEXT NOT NULL CHECK (length(trim(candidate_digest)) > 0),\
                    state JSONB NOT NULL,\
                    security_epoch BIGINT NOT NULL CHECK (security_epoch >= 0),\
                    fences JSONB NOT NULL,\
                    recovery_attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (recovery_attempt_count >= 0),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX idx_module_transition_checkpoints_slug ON module_transition_checkpoints(module_slug)",
                "CREATE INDEX idx_module_transition_checkpoints_tenant ON module_transition_checkpoints(tenant_id)",
                "CREATE TABLE module_transition_operations (\
                    idempotency_key UUID PRIMARY KEY,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('trigger_recovery', 'finalize')),\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    tenant_id UUID NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    operation_id UUID NOT NULL REFERENCES module_transition_checkpoints(operation_id) ON DELETE RESTRICT,\
                    resulting_revision BIGINT NOT NULL CHECK (resulting_revision > 0),\
                    created_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX idx_module_transition_operations_operation ON module_transition_operations(operation_id)",
                "CREATE TABLE module_retention_holds (\
                    hold_id UUID PRIMARY KEY,\
                    target_type TEXT NOT NULL CHECK (length(trim(target_type)) > 0),\
                    target_identity TEXT NOT NULL CHECK (length(trim(target_identity)) > 0),\
                    target JSONB NOT NULL,\
                    kind JSONB NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX idx_module_retention_holds_target ON module_retention_holds(target_type, target_identity)",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_transition_checkpoints (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) > 0),\
                    tenant_id TEXT NULL,\
                    predecessor_digest TEXT NULL,\
                    candidate_digest TEXT NOT NULL CHECK (length(trim(candidate_digest)) > 0),\
                    state JSON NOT NULL,\
                    security_epoch INTEGER NOT NULL CHECK (security_epoch >= 0),\
                    fences JSON NOT NULL,\
                    recovery_attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (recovery_attempt_count >= 0),\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL\
                )",
                "CREATE INDEX idx_module_transition_checkpoints_slug ON module_transition_checkpoints(module_slug)",
                "CREATE INDEX idx_module_transition_checkpoints_tenant ON module_transition_checkpoints(tenant_id)",
                "CREATE TABLE module_transition_operations (\
                    idempotency_key TEXT PRIMARY KEY NOT NULL,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('trigger_recovery', 'finalize')),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    actor_id TEXT NOT NULL, tenant_id TEXT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    operation_id TEXT NOT NULL REFERENCES module_transition_checkpoints(operation_id) ON DELETE RESTRICT,\
                    resulting_revision INTEGER NOT NULL CHECK (resulting_revision > 0),\
                    created_at TEXT NOT NULL\
                )",
                "CREATE INDEX idx_module_transition_operations_operation ON module_transition_operations(operation_id)",
                "CREATE TABLE module_retention_holds (\
                    hold_id TEXT PRIMARY KEY NOT NULL,\
                    target_type TEXT NOT NULL CHECK (length(trim(target_type)) > 0),\
                    target_identity TEXT NOT NULL CHECK (length(trim(target_identity)) > 0),\
                    target JSON NOT NULL,\
                    kind JSON NOT NULL,\
                    created_at TEXT NOT NULL\
                )",
                "CREATE INDEX idx_module_retention_holds_target ON module_retention_holds(target_type, target_identity)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "module transition and retention migration does not support database backend {backend:?}"
                )));
            }
        };

        for statement in statements {
            manager
                .get_connection()
                .execute_unprepared(statement)
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres | DbBackend::Sqlite => &[
                "DROP TABLE IF EXISTS module_retention_holds",
                "DROP TABLE IF EXISTS module_transition_operations",
                "DROP TABLE IF EXISTS module_transition_checkpoints",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "module transition and retention migration does not support database backend {backend:?}"
                )));
            }
        };

        for statement in statements {
            manager
                .get_connection()
                .execute_unprepared(statement)
                .await?;
        }

        Ok(())
    }
}
