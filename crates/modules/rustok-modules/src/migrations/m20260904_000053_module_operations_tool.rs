use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists separately signed operations-tool releases, operations-tool maintenance operations,
/// and host component desired/observed assignments.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_operations_tool_releases (\
                    release_id UUID PRIMARY KEY,\
                    version TEXT NOT NULL UNIQUE CHECK (length(trim(version)) > 0),\
                    protocol_revision INTEGER NOT NULL CHECK (protocol_revision > 0),\
                    package_digest TEXT NOT NULL CHECK (package_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    controller_digest TEXT NOT NULL CHECK (controller_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    reconciler_digest TEXT NOT NULL CHECK (reconciler_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    agent_digest TEXT NOT NULL CHECK (agent_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    signer_key_digest TEXT NOT NULL CHECK (signer_key_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    signature TEXT NOT NULL CHECK (length(trim(signature)) > 0),\
                    issued_at TIMESTAMPTZ NOT NULL,\
                    expires_at TIMESTAMPTZ NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE TABLE module_operations_tool_maintenance_operations (\
                    operation_id UUID PRIMARY KEY,\
                    target_release_id UUID NOT NULL REFERENCES module_operations_tool_releases(release_id),\
                    predecessor_release_id UUID NULL REFERENCES module_operations_tool_releases(release_id),\
                    status TEXT NOT NULL CHECK (status IN ('in_progress', 'rolling_back', 'recovery_required', 'converged', 'rolled_back')),\
                    recovery_attempts INTEGER NOT NULL DEFAULT 0 CHECK (recovery_attempts BETWEEN 0 AND 1),\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL UNIQUE,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id UUID NOT NULL,\
                    recovery_request_digest TEXT NULL CHECK (recovery_request_digest IS NULL OR recovery_request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    recovery_actor_id UUID NULL,\
                    recovery_idempotency_key UUID NULL UNIQUE,\
                    recovery_trace_id TEXT NULL CHECK (recovery_trace_id IS NULL OR length(trim(recovery_trace_id)) > 0),\
                    recovery_correlation_id UUID NULL,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    CHECK ((recovery_request_digest IS NULL AND recovery_actor_id IS NULL AND recovery_idempotency_key IS NULL AND recovery_trace_id IS NULL AND recovery_correlation_id IS NULL) OR (recovery_request_digest IS NOT NULL AND recovery_actor_id IS NOT NULL AND recovery_idempotency_key IS NOT NULL AND recovery_trace_id IS NOT NULL AND recovery_correlation_id IS NOT NULL))\
                )",
                "CREATE UNIQUE INDEX uq_operations_tool_active_fleet_maintenance ON module_operations_tool_maintenance_operations ((1)) WHERE status IN ('in_progress', 'rolling_back', 'recovery_required')",
                "CREATE TABLE module_operations_tool_assignments (\
                    assignment_id UUID PRIMARY KEY,\
                    operation_id UUID NOT NULL REFERENCES module_operations_tool_maintenance_operations(operation_id) ON DELETE CASCADE,\
                    host_id TEXT NOT NULL CHECK (length(trim(host_id)) > 0),\
                    component TEXT NOT NULL CHECK (component IN ('controller', 'reconciler', 'agent')),\
                    desired_digest TEXT NOT NULL CHECK (desired_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    observed_digest TEXT NULL CHECK (observed_digest IS NULL OR observed_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    status TEXT NOT NULL CHECK (status IN ('staged', 'converged', 'failed')),\
                    reported_at TIMESTAMPTZ NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (operation_id, host_id, component)\
                )",
                "CREATE INDEX idx_operations_tool_assignments_op ON module_operations_tool_assignments (operation_id)",
            ],
            _ => &[
                "CREATE TABLE module_operations_tool_releases (\
                    release_id TEXT PRIMARY KEY,\
                    version TEXT NOT NULL UNIQUE CHECK (length(trim(version)) > 0),\
                    protocol_revision INTEGER NOT NULL CHECK (protocol_revision > 0),\
                    package_digest TEXT NOT NULL CHECK (length(package_digest) = 71 AND substr(package_digest, 1, 7) = 'sha256:' AND substr(package_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    controller_digest TEXT NOT NULL CHECK (length(controller_digest) = 71 AND substr(controller_digest, 1, 7) = 'sha256:' AND substr(controller_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    reconciler_digest TEXT NOT NULL CHECK (length(reconciler_digest) = 71 AND substr(reconciler_digest, 1, 7) = 'sha256:' AND substr(reconciler_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    agent_digest TEXT NOT NULL CHECK (length(agent_digest) = 71 AND substr(agent_digest, 1, 7) = 'sha256:' AND substr(agent_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    signer_key_digest TEXT NOT NULL CHECK (length(signer_key_digest) = 71 AND substr(signer_key_digest, 1, 7) = 'sha256:' AND substr(signer_key_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    signature TEXT NOT NULL CHECK (length(trim(signature)) > 0),\
                    issued_at TEXT NOT NULL,\
                    expires_at TEXT NOT NULL,\
                    created_at TEXT NOT NULL\
                )",
                "CREATE TABLE module_operations_tool_maintenance_operations (\
                    operation_id TEXT PRIMARY KEY,\
                    target_release_id TEXT NOT NULL REFERENCES module_operations_tool_releases(release_id),\
                    predecessor_release_id TEXT NULL REFERENCES module_operations_tool_releases(release_id),\
                    status TEXT NOT NULL CHECK (status IN ('in_progress', 'rolling_back', 'recovery_required', 'converged', 'rolled_back')),\
                    recovery_attempts INTEGER NOT NULL DEFAULT 0 CHECK (recovery_attempts BETWEEN 0 AND 1),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    actor_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL UNIQUE,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id TEXT NOT NULL,\
                    recovery_request_digest TEXT NULL CHECK (recovery_request_digest IS NULL OR (length(recovery_request_digest) = 71 AND substr(recovery_request_digest, 1, 7) = 'sha256:' AND substr(recovery_request_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    recovery_actor_id TEXT NULL,\
                    recovery_idempotency_key TEXT NULL UNIQUE,\
                    recovery_trace_id TEXT NULL CHECK (recovery_trace_id IS NULL OR length(trim(recovery_trace_id)) > 0),\
                    recovery_correlation_id TEXT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    CHECK ((recovery_request_digest IS NULL AND recovery_actor_id IS NULL AND recovery_idempotency_key IS NULL AND recovery_trace_id IS NULL AND recovery_correlation_id IS NULL) OR (recovery_request_digest IS NOT NULL AND recovery_actor_id IS NOT NULL AND recovery_idempotency_key IS NOT NULL AND recovery_trace_id IS NOT NULL AND recovery_correlation_id IS NOT NULL))\
                )",
                "CREATE UNIQUE INDEX uq_operations_tool_active_fleet_maintenance ON module_operations_tool_maintenance_operations ((1)) WHERE status IN ('in_progress', 'rolling_back', 'recovery_required')",
                "CREATE TABLE module_operations_tool_assignments (\
                    assignment_id TEXT PRIMARY KEY,\
                    operation_id TEXT NOT NULL REFERENCES module_operations_tool_maintenance_operations(operation_id) ON DELETE CASCADE,\
                    host_id TEXT NOT NULL CHECK (length(trim(host_id)) > 0),\
                    component TEXT NOT NULL CHECK (component IN ('controller', 'reconciler', 'agent')),\
                    desired_digest TEXT NOT NULL CHECK (length(desired_digest) = 71 AND substr(desired_digest, 1, 7) = 'sha256:' AND substr(desired_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    observed_digest TEXT NULL CHECK (observed_digest IS NULL OR (length(observed_digest) = 71 AND substr(observed_digest, 1, 7) = 'sha256:' AND substr(observed_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    status TEXT NOT NULL CHECK (status IN ('staged', 'converged', 'failed')),\
                    reported_at TEXT NULL,\
                    updated_at TEXT NOT NULL,\
                    UNIQUE (operation_id, host_id, component)\
                )",
                "CREATE INDEX idx_operations_tool_assignments_op ON module_operations_tool_assignments (operation_id)",
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
            "DROP TABLE IF EXISTS module_operations_tool_assignments",
            "DROP TABLE IF EXISTS module_operations_tool_maintenance_operations",
            "DROP TABLE IF EXISTS module_operations_tool_releases",
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
