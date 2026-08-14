//! Durable owner ledger for artifact/sandbox node reconciliation.

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Creates the revisioned desired/observed aggregate for dynamic artifact
/// materialization. Node agents only report the owner-selected identity; they
/// cannot choose installations or mutate aggregate heads.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_node_reconciliations (\
                    reconciliation_id UUID PRIMARY KEY,\
                    predecessor_reconciliation_id UUID NULL REFERENCES module_artifact_node_reconciliations(reconciliation_id) ON DELETE RESTRICT,\
                    reconciliation_revision BIGINT NOT NULL UNIQUE CHECK (reconciliation_revision > 0),\
                    topology_reference TEXT NOT NULL CHECK (length(trim(topology_reference)) BETWEEN 1 AND 512),\
                    topology_digest TEXT NOT NULL CHECK (topology_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    policy_revision TEXT NOT NULL CHECK (policy_revision ~ '^sha256:[0-9a-f]{64}$'),\
                    target_assignment_count INTEGER NOT NULL CHECK (target_assignment_count BETWEEN 1 AND 1024),\
                    status TEXT NOT NULL CHECK (status IN ('preparing', 'activating', 'converged', 'failed', 'degraded', 'superseded')),\
                    requested_by UUID NOT NULL,\
                    requested_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    status_changed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    converged_at TIMESTAMPTZ NULL,\
                    failed_at TIMESTAMPTZ NULL,\
                    failure_code TEXT NULL CHECK (failure_code IS NULL OR length(trim(failure_code)) BETWEEN 1 AND 128),\
                    failure_detail TEXT NULL CHECK (failure_detail IS NULL OR length(trim(failure_detail)) BETWEEN 1 AND 2000),\
                    CHECK (predecessor_reconciliation_id IS NULL OR predecessor_reconciliation_id <> reconciliation_id),\
                    CHECK ((status IN ('converged', 'superseded') AND converged_at IS NOT NULL AND failed_at IS NULL AND failure_code IS NULL AND failure_detail IS NULL) OR\
                           (status IN ('failed', 'degraded') AND failed_at IS NOT NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL) OR\
                           (status IN ('preparing', 'activating') AND converged_at IS NULL AND failed_at IS NULL AND failure_code IS NULL AND failure_detail IS NULL))\
                )",
                "CREATE INDEX module_artifact_node_reconciliations_topology_idx ON module_artifact_node_reconciliations (topology_digest, reconciliation_revision)",
                "CREATE TABLE module_artifact_node_reconciliation_state (\
                    state_id TEXT PRIMARY KEY CHECK (state_id = 'current'),\
                    revision BIGINT NOT NULL CHECK (revision >= 0),\
                    desired_reconciliation_id UUID NULL REFERENCES module_artifact_node_reconciliations(reconciliation_id) ON DELETE RESTRICT,\
                    observed_reconciliation_id UUID NULL REFERENCES module_artifact_node_reconciliations(reconciliation_id) ON DELETE RESTRICT,\
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
                "INSERT INTO module_artifact_node_reconciliation_state (state_id, revision, desired_reconciliation_id, observed_reconciliation_id) VALUES ('current', 0, NULL, NULL)",
                "CREATE TABLE module_artifact_node_reconciliation_assignments (\
                    reconciliation_id UUID NOT NULL REFERENCES module_artifact_node_reconciliations(reconciliation_id) ON DELETE RESTRICT,\
                    node_id UUID NOT NULL,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id) ON DELETE RESTRICT,\
                    installation_scope TEXT NOT NULL CHECK (installation_scope IN ('tenant', 'platform')),\
                    release_digest TEXT NOT NULL CHECK (release_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    payload_digest TEXT NOT NULL CHECK (payload_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    payload_kind TEXT NOT NULL CHECK (payload_kind IN ('rhai', 'wasm_component', 'static_promoted', 'sidecar')),\
                    payload_media_type TEXT NOT NULL CHECK (length(trim(payload_media_type)) BETWEEN 1 AND 256),\
                    admission_revision BIGINT NOT NULL CHECK (admission_revision > 0),\
                    dependency_graph_revision BIGINT NOT NULL CHECK (dependency_graph_revision > 0),\
                    dependency_graph_digest TEXT NOT NULL CHECK (dependency_graph_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    capability_grant_revision BIGINT NOT NULL CHECK (capability_grant_revision > 0),\
                    executor_abi TEXT NOT NULL CHECK (length(trim(executor_abi)) BETWEEN 1 AND 128),\
                    policy_revision TEXT NOT NULL CHECK (policy_revision ~ '^sha256:[0-9a-f]{64}$'),\
                    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),\
                    observation_revision BIGINT NOT NULL DEFAULT 0 CHECK (observation_revision >= 0),\
                    phase TEXT NOT NULL CHECK (phase IN ('pending', 'prepared', 'healthy', 'active', 'failed')),\
                    observed_installation_scope TEXT NULL CHECK (observed_installation_scope IS NULL OR observed_installation_scope IN ('tenant', 'platform')),\
                    observed_release_digest TEXT NULL CHECK (observed_release_digest IS NULL OR observed_release_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    observed_payload_digest TEXT NULL CHECK (observed_payload_digest IS NULL OR observed_payload_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    observed_payload_kind TEXT NULL CHECK (observed_payload_kind IS NULL OR observed_payload_kind IN ('rhai', 'wasm_component', 'static_promoted', 'sidecar')),\
                    observed_payload_media_type TEXT NULL CHECK (observed_payload_media_type IS NULL OR length(trim(observed_payload_media_type)) BETWEEN 1 AND 256),\
                    observed_admission_revision BIGINT NULL CHECK (observed_admission_revision IS NULL OR observed_admission_revision > 0),\
                    observed_dependency_graph_revision BIGINT NULL CHECK (observed_dependency_graph_revision IS NULL OR observed_dependency_graph_revision > 0),\
                    observed_dependency_graph_digest TEXT NULL CHECK (observed_dependency_graph_digest IS NULL OR observed_dependency_graph_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    observed_capability_grant_revision BIGINT NULL CHECK (observed_capability_grant_revision IS NULL OR observed_capability_grant_revision > 0),\
                    observed_executor_abi TEXT NULL CHECK (observed_executor_abi IS NULL OR length(trim(observed_executor_abi)) BETWEEN 1 AND 128),\
                    observed_policy_revision TEXT NULL CHECK (observed_policy_revision IS NULL OR observed_policy_revision ~ '^sha256:[0-9a-f]{64}$'),\
                    health_evidence_reference TEXT NULL CHECK (health_evidence_reference IS NULL OR length(trim(health_evidence_reference)) BETWEEN 1 AND 512),\
                    health_evidence_digest TEXT NULL CHECK (health_evidence_digest IS NULL OR health_evidence_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    failure_code TEXT NULL CHECK (failure_code IS NULL OR length(trim(failure_code)) BETWEEN 1 AND 128),\
                    failure_detail TEXT NULL CHECK (failure_detail IS NULL OR length(trim(failure_detail)) BETWEEN 1 AND 2000),\
                    reported_by TEXT NULL CHECK (reported_by IS NULL OR length(trim(reported_by)) BETWEEN 1 AND 128),\
                    last_report_digest TEXT NULL CHECK (last_report_digest IS NULL OR last_report_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    active_claim_id UUID NULL,\
                    claimed_by_agent TEXT NULL CHECK (claimed_by_agent IS NULL OR length(trim(claimed_by_agent)) BETWEEN 1 AND 128),\
                    claim_expires_at TIMESTAMPTZ NULL,\
                    first_reported_at TIMESTAMPTZ NULL,\
                    last_reported_at TIMESTAMPTZ NULL,\
                    PRIMARY KEY (reconciliation_id, node_id, installation_id),\
                    UNIQUE (reconciliation_id, ordinal),\
                    CHECK ((phase = 'pending' AND observation_revision = 0 AND observed_installation_scope IS NULL AND observed_release_digest IS NULL AND observed_payload_digest IS NULL AND observed_payload_kind IS NULL AND observed_payload_media_type IS NULL AND observed_admission_revision IS NULL AND observed_dependency_graph_revision IS NULL AND observed_dependency_graph_digest IS NULL AND observed_capability_grant_revision IS NULL AND observed_executor_abi IS NULL AND observed_policy_revision IS NULL AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NULL AND last_report_digest IS NULL AND first_reported_at IS NULL AND last_reported_at IS NULL) OR\
                           (phase = 'prepared' AND observation_revision > 0 AND observed_installation_scope IS NOT NULL AND observed_release_digest IS NOT NULL AND observed_payload_digest IS NOT NULL AND observed_payload_kind IS NOT NULL AND observed_payload_media_type IS NOT NULL AND observed_admission_revision IS NOT NULL AND observed_dependency_graph_revision IS NOT NULL AND observed_dependency_graph_digest IS NOT NULL AND observed_capability_grant_revision IS NOT NULL AND observed_executor_abi IS NOT NULL AND observed_policy_revision IS NOT NULL AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL) OR\
                           (phase IN ('healthy', 'active') AND observation_revision > 0 AND observed_installation_scope IS NOT NULL AND observed_release_digest IS NOT NULL AND observed_payload_digest IS NOT NULL AND observed_payload_kind IS NOT NULL AND observed_payload_media_type IS NOT NULL AND observed_admission_revision IS NOT NULL AND observed_dependency_graph_revision IS NOT NULL AND observed_dependency_graph_digest IS NOT NULL AND observed_capability_grant_revision IS NOT NULL AND observed_executor_abi IS NOT NULL AND observed_policy_revision IS NOT NULL AND health_evidence_reference IS NOT NULL AND health_evidence_digest IS NOT NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL) OR\
                           (phase = 'failed' AND observation_revision > 0 AND observed_installation_scope IS NOT NULL AND observed_release_digest IS NOT NULL AND observed_payload_digest IS NOT NULL AND observed_payload_kind IS NOT NULL AND observed_payload_media_type IS NOT NULL AND observed_admission_revision IS NOT NULL AND observed_dependency_graph_revision IS NOT NULL AND observed_dependency_graph_digest IS NOT NULL AND observed_capability_grant_revision IS NOT NULL AND observed_executor_abi IS NOT NULL AND observed_policy_revision IS NOT NULL AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL)),\
                    CHECK ((active_claim_id IS NULL AND claimed_by_agent IS NULL AND claim_expires_at IS NULL) OR (active_claim_id IS NOT NULL AND claimed_by_agent IS NOT NULL AND claim_expires_at IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_node_reconciliation_assignments_claim_idx ON module_artifact_node_reconciliation_assignments (reconciliation_id, node_id, phase, ordinal)",
                "CREATE TABLE module_artifact_node_reconciliation_operations (\
                    idempotency_key UUID PRIMARY KEY,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('request', 'report')),\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    principal_id TEXT NOT NULL CHECK (length(trim(principal_id)) BETWEEN 1 AND 128),\
                    reconciliation_id UUID NULL REFERENCES module_artifact_node_reconciliations(reconciliation_id) ON DELETE RESTRICT,\
                    reconciliation_revision BIGINT NULL CHECK (reconciliation_revision IS NULL OR reconciliation_revision > 0),\
                    reconciliation_state_revision BIGINT NULL CHECK (reconciliation_state_revision IS NULL OR reconciliation_state_revision > 0),\
                    reconciliation_status TEXT NULL CHECK (reconciliation_status IS NULL OR reconciliation_status IN ('preparing', 'activating', 'converged', 'failed', 'degraded', 'superseded')),\
                    node_id UUID NULL,\
                    installation_id UUID NULL,\
                    observation_revision BIGINT NULL CHECK (observation_revision IS NULL OR observation_revision > 0),\
                    assignment_phase TEXT NULL CHECK (assignment_phase IS NULL OR assignment_phase IN ('prepared', 'healthy', 'failed')),\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TIMESTAMPTZ NULL,\
                    CHECK ((completed_at IS NULL AND reconciliation_id IS NULL AND reconciliation_revision IS NULL AND reconciliation_state_revision IS NULL AND reconciliation_status IS NULL AND node_id IS NULL AND installation_id IS NULL AND observation_revision IS NULL AND assignment_phase IS NULL) OR\
                           (completed_at IS NOT NULL AND operation_kind = 'request' AND reconciliation_id IS NOT NULL AND reconciliation_revision IS NOT NULL AND reconciliation_state_revision IS NOT NULL AND reconciliation_status = 'preparing' AND node_id IS NULL AND installation_id IS NULL AND observation_revision IS NULL AND assignment_phase IS NULL) OR\
                           (completed_at IS NOT NULL AND operation_kind = 'report' AND reconciliation_id IS NOT NULL AND reconciliation_revision IS NOT NULL AND reconciliation_state_revision IS NOT NULL AND reconciliation_status IS NOT NULL AND node_id IS NOT NULL AND installation_id IS NOT NULL AND observation_revision IS NOT NULL AND assignment_phase IS NOT NULL))\
                )",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_node_reconciliations (\
                    reconciliation_id TEXT PRIMARY KEY, predecessor_reconciliation_id TEXT NULL REFERENCES module_artifact_node_reconciliations(reconciliation_id) ON DELETE RESTRICT, reconciliation_revision INTEGER NOT NULL UNIQUE CHECK (reconciliation_revision > 0),\
                    topology_reference TEXT NOT NULL CHECK (length(trim(topology_reference)) BETWEEN 1 AND 512), topology_digest TEXT NOT NULL CHECK (length(topology_digest) = 71 AND substr(topology_digest, 1, 7) = 'sha256:' AND substr(topology_digest, 8) NOT GLOB '*[^0-9a-f]*'), policy_revision TEXT NOT NULL CHECK (length(policy_revision) = 71 AND substr(policy_revision, 1, 7) = 'sha256:' AND substr(policy_revision, 8) NOT GLOB '*[^0-9a-f]*'), target_assignment_count INTEGER NOT NULL CHECK (target_assignment_count BETWEEN 1 AND 1024),\
                    status TEXT NOT NULL CHECK (status IN ('preparing', 'activating', 'converged', 'failed', 'degraded', 'superseded')), requested_by TEXT NOT NULL, requested_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, status_changed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, converged_at TEXT NULL, failed_at TEXT NULL, failure_code TEXT NULL CHECK (failure_code IS NULL OR length(trim(failure_code)) BETWEEN 1 AND 128), failure_detail TEXT NULL CHECK (failure_detail IS NULL OR length(trim(failure_detail)) BETWEEN 1 AND 2000),\
                    CHECK (predecessor_reconciliation_id IS NULL OR predecessor_reconciliation_id <> reconciliation_id),\
                    CHECK ((status IN ('converged', 'superseded') AND converged_at IS NOT NULL AND failed_at IS NULL AND failure_code IS NULL AND failure_detail IS NULL) OR (status IN ('failed', 'degraded') AND failed_at IS NOT NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL) OR (status IN ('preparing', 'activating') AND converged_at IS NULL AND failed_at IS NULL AND failure_code IS NULL AND failure_detail IS NULL))\
                )",
                "CREATE INDEX module_artifact_node_reconciliations_topology_idx ON module_artifact_node_reconciliations (topology_digest, reconciliation_revision)",
                "CREATE TABLE module_artifact_node_reconciliation_state (state_id TEXT PRIMARY KEY CHECK (state_id = 'current'), revision INTEGER NOT NULL CHECK (revision >= 0), desired_reconciliation_id TEXT NULL REFERENCES module_artifact_node_reconciliations(reconciliation_id) ON DELETE RESTRICT, observed_reconciliation_id TEXT NULL REFERENCES module_artifact_node_reconciliations(reconciliation_id) ON DELETE RESTRICT, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
                "INSERT INTO module_artifact_node_reconciliation_state (state_id, revision, desired_reconciliation_id, observed_reconciliation_id) VALUES ('current', 0, NULL, NULL)",
                "CREATE TABLE module_artifact_node_reconciliation_assignments (\
                    reconciliation_id TEXT NOT NULL REFERENCES module_artifact_node_reconciliations(reconciliation_id) ON DELETE RESTRICT,\
                    node_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id) ON DELETE RESTRICT,\
                    installation_scope TEXT NOT NULL CHECK (installation_scope IN ('tenant', 'platform')),\
                    release_digest TEXT NOT NULL CHECK (length(release_digest) = 71 AND substr(release_digest, 1, 7) = 'sha256:' AND substr(release_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71 AND substr(payload_digest, 1, 7) = 'sha256:' AND substr(payload_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    payload_kind TEXT NOT NULL CHECK (payload_kind IN ('rhai', 'wasm_component', 'static_promoted', 'sidecar')),\
                    payload_media_type TEXT NOT NULL CHECK (length(trim(payload_media_type)) BETWEEN 1 AND 256),\
                    admission_revision INTEGER NOT NULL CHECK (admission_revision > 0),\
                    dependency_graph_revision INTEGER NOT NULL CHECK (dependency_graph_revision > 0),\
                    dependency_graph_digest TEXT NOT NULL CHECK (length(dependency_graph_digest) = 71 AND substr(dependency_graph_digest, 1, 7) = 'sha256:' AND substr(dependency_graph_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    capability_grant_revision INTEGER NOT NULL CHECK (capability_grant_revision > 0),\
                    executor_abi TEXT NOT NULL CHECK (length(trim(executor_abi)) BETWEEN 1 AND 128),\
                    policy_revision TEXT NOT NULL CHECK (length(policy_revision) = 71 AND substr(policy_revision, 1, 7) = 'sha256:' AND substr(policy_revision, 8) NOT GLOB '*[^0-9a-f]*'),\
                    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),\
                    observation_revision INTEGER NOT NULL DEFAULT 0 CHECK (observation_revision >= 0),\
                    phase TEXT NOT NULL CHECK (phase IN ('pending', 'prepared', 'healthy', 'active', 'failed')),\
                    observed_installation_scope TEXT NULL CHECK (observed_installation_scope IS NULL OR observed_installation_scope IN ('tenant', 'platform')),\
                    observed_release_digest TEXT NULL CHECK (observed_release_digest IS NULL OR (length(observed_release_digest) = 71 AND substr(observed_release_digest, 1, 7) = 'sha256:' AND substr(observed_release_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    observed_payload_digest TEXT NULL CHECK (observed_payload_digest IS NULL OR (length(observed_payload_digest) = 71 AND substr(observed_payload_digest, 1, 7) = 'sha256:' AND substr(observed_payload_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    observed_payload_kind TEXT NULL CHECK (observed_payload_kind IS NULL OR observed_payload_kind IN ('rhai', 'wasm_component', 'static_promoted', 'sidecar')),\
                    observed_payload_media_type TEXT NULL CHECK (observed_payload_media_type IS NULL OR length(trim(observed_payload_media_type)) BETWEEN 1 AND 256),\
                    observed_admission_revision INTEGER NULL CHECK (observed_admission_revision IS NULL OR observed_admission_revision > 0),\
                    observed_dependency_graph_revision INTEGER NULL CHECK (observed_dependency_graph_revision IS NULL OR observed_dependency_graph_revision > 0),\
                    observed_dependency_graph_digest TEXT NULL CHECK (observed_dependency_graph_digest IS NULL OR (length(observed_dependency_graph_digest) = 71 AND substr(observed_dependency_graph_digest, 1, 7) = 'sha256:' AND substr(observed_dependency_graph_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    observed_capability_grant_revision INTEGER NULL CHECK (observed_capability_grant_revision IS NULL OR observed_capability_grant_revision > 0),\
                    observed_executor_abi TEXT NULL CHECK (observed_executor_abi IS NULL OR length(trim(observed_executor_abi)) BETWEEN 1 AND 128),\
                    observed_policy_revision TEXT NULL CHECK (observed_policy_revision IS NULL OR (length(observed_policy_revision) = 71 AND substr(observed_policy_revision, 1, 7) = 'sha256:' AND substr(observed_policy_revision, 8) NOT GLOB '*[^0-9a-f]*')),\
                    health_evidence_reference TEXT NULL CHECK (health_evidence_reference IS NULL OR length(trim(health_evidence_reference)) BETWEEN 1 AND 512), health_evidence_digest TEXT NULL CHECK (health_evidence_digest IS NULL OR (length(health_evidence_digest) = 71 AND substr(health_evidence_digest, 1, 7) = 'sha256:' AND substr(health_evidence_digest, 8) NOT GLOB '*[^0-9a-f]*')), failure_code TEXT NULL CHECK (failure_code IS NULL OR length(trim(failure_code)) BETWEEN 1 AND 128), failure_detail TEXT NULL CHECK (failure_detail IS NULL OR length(trim(failure_detail)) BETWEEN 1 AND 2000), reported_by TEXT NULL CHECK (reported_by IS NULL OR length(trim(reported_by)) BETWEEN 1 AND 128), last_report_digest TEXT NULL CHECK (last_report_digest IS NULL OR (length(last_report_digest) = 71 AND substr(last_report_digest, 1, 7) = 'sha256:' AND substr(last_report_digest, 8) NOT GLOB '*[^0-9a-f]*')), active_claim_id TEXT NULL, claimed_by_agent TEXT NULL CHECK (claimed_by_agent IS NULL OR length(trim(claimed_by_agent)) BETWEEN 1 AND 128), claim_expires_at TEXT NULL, first_reported_at TEXT NULL, last_reported_at TEXT NULL,\
                    PRIMARY KEY (reconciliation_id, node_id, installation_id), UNIQUE (reconciliation_id, ordinal),\
                    CHECK ((observed_payload_digest IS NULL AND observed_payload_kind IS NULL AND observed_payload_media_type IS NULL) OR (observed_payload_digest IS NOT NULL AND observed_payload_kind IS NOT NULL AND observed_payload_media_type IS NOT NULL)),\
                    CHECK ((phase = 'pending' AND observation_revision = 0 AND observed_installation_scope IS NULL AND observed_release_digest IS NULL AND observed_payload_digest IS NULL AND observed_admission_revision IS NULL AND observed_dependency_graph_revision IS NULL AND observed_dependency_graph_digest IS NULL AND observed_capability_grant_revision IS NULL AND observed_executor_abi IS NULL AND observed_policy_revision IS NULL AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NULL AND last_report_digest IS NULL AND first_reported_at IS NULL AND last_reported_at IS NULL) OR (phase = 'prepared' AND observation_revision > 0 AND observed_installation_scope IS NOT NULL AND observed_release_digest IS NOT NULL AND observed_payload_digest IS NOT NULL AND observed_admission_revision IS NOT NULL AND observed_dependency_graph_revision IS NOT NULL AND observed_dependency_graph_digest IS NOT NULL AND observed_capability_grant_revision IS NOT NULL AND observed_executor_abi IS NOT NULL AND observed_policy_revision IS NOT NULL AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL) OR (phase IN ('healthy', 'active') AND observation_revision > 0 AND observed_installation_scope IS NOT NULL AND observed_release_digest IS NOT NULL AND observed_payload_digest IS NOT NULL AND observed_admission_revision IS NOT NULL AND observed_dependency_graph_revision IS NOT NULL AND observed_dependency_graph_digest IS NOT NULL AND observed_capability_grant_revision IS NOT NULL AND observed_executor_abi IS NOT NULL AND observed_policy_revision IS NOT NULL AND health_evidence_reference IS NOT NULL AND health_evidence_digest IS NOT NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL) OR (phase = 'failed' AND observation_revision > 0 AND observed_installation_scope IS NOT NULL AND observed_release_digest IS NOT NULL AND observed_payload_digest IS NOT NULL AND observed_admission_revision IS NOT NULL AND observed_dependency_graph_revision IS NOT NULL AND observed_dependency_graph_digest IS NOT NULL AND observed_capability_grant_revision IS NOT NULL AND observed_executor_abi IS NOT NULL AND observed_policy_revision IS NOT NULL AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL)),\
                    CHECK ((active_claim_id IS NULL AND claimed_by_agent IS NULL AND claim_expires_at IS NULL) OR (active_claim_id IS NOT NULL AND claimed_by_agent IS NOT NULL AND claim_expires_at IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_node_reconciliation_assignments_claim_idx ON module_artifact_node_reconciliation_assignments (reconciliation_id, node_id, phase, ordinal)",
                "CREATE TABLE module_artifact_node_reconciliation_operations (\
                    idempotency_key TEXT PRIMARY KEY, operation_kind TEXT NOT NULL CHECK (operation_kind IN ('request', 'report')), request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'), principal_id TEXT NOT NULL CHECK (length(trim(principal_id)) BETWEEN 1 AND 128), reconciliation_id TEXT NULL REFERENCES module_artifact_node_reconciliations(reconciliation_id) ON DELETE RESTRICT, reconciliation_revision INTEGER NULL CHECK (reconciliation_revision IS NULL OR reconciliation_revision > 0), reconciliation_state_revision INTEGER NULL CHECK (reconciliation_state_revision IS NULL OR reconciliation_state_revision > 0), reconciliation_status TEXT NULL CHECK (reconciliation_status IS NULL OR reconciliation_status IN ('preparing', 'activating', 'converged', 'failed', 'degraded', 'superseded')), node_id TEXT NULL, installation_id TEXT NULL, observation_revision INTEGER NULL CHECK (observation_revision IS NULL OR observation_revision > 0), assignment_phase TEXT NULL CHECK (assignment_phase IS NULL OR assignment_phase IN ('prepared', 'healthy', 'failed')), created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, completed_at TEXT NULL,\
                    CHECK ((completed_at IS NULL AND reconciliation_id IS NULL AND reconciliation_revision IS NULL AND reconciliation_state_revision IS NULL AND reconciliation_status IS NULL AND node_id IS NULL AND installation_id IS NULL AND observation_revision IS NULL AND assignment_phase IS NULL) OR (completed_at IS NOT NULL AND operation_kind = 'request' AND reconciliation_id IS NOT NULL AND reconciliation_revision IS NOT NULL AND reconciliation_state_revision IS NOT NULL AND reconciliation_status = 'preparing' AND node_id IS NULL AND installation_id IS NULL AND observation_revision IS NULL AND assignment_phase IS NULL) OR (completed_at IS NOT NULL AND operation_kind = 'report' AND reconciliation_id IS NOT NULL AND reconciliation_revision IS NOT NULL AND reconciliation_state_revision IS NOT NULL AND reconciliation_status IS NOT NULL AND node_id IS NOT NULL AND installation_id IS NOT NULL AND observation_revision IS NOT NULL AND assignment_phase IS NOT NULL))\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact node reconciliation migration does not support {backend:?}"
                )));
            }
        };

        for statement in statements {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            "module_artifact_node_reconciliation_operations",
            "module_artifact_node_reconciliation_assignments",
            "module_artifact_node_reconciliation_state",
            "module_artifact_node_reconciliations",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE {table}"))
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    use super::*;

    #[tokio::test]
    async fn sqlite_schema_binds_observations_to_exact_artifact_identity() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("foreign keys");
        db.execute_unprepared(
            "CREATE TABLE module_artifact_installations (installation_id TEXT PRIMARY KEY)",
        )
        .await
        .expect("installation prerequisite");
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("artifact node reconciliation migration");

        let reconciliation_columns =
            column_names(&db, "module_artifact_node_reconciliations").await;
        assert!(reconciliation_columns.contains("predecessor_reconciliation_id"));
        assert!(reconciliation_columns.contains("topology_digest"));
        assert!(reconciliation_columns.contains("target_assignment_count"));

        let assignment_columns =
            column_names(&db, "module_artifact_node_reconciliation_assignments").await;
        for column in [
            "payload_digest",
            "payload_kind",
            "payload_media_type",
            "admission_revision",
            "observed_release_digest",
            "observed_payload_digest",
            "observed_payload_kind",
            "observed_payload_media_type",
            "observed_admission_revision",
            "observed_dependency_graph_digest",
            "observed_capability_grant_revision",
            "observed_executor_abi",
            "last_report_digest",
            "active_claim_id",
        ] {
            assert!(assignment_columns.contains(column), "missing {column}");
        }
        assert_eq!(
            primary_key_columns(&db, "module_artifact_node_reconciliation_assignments").await,
            vec!["reconciliation_id", "node_id", "installation_id"]
        );

        let operation_columns =
            column_names(&db, "module_artifact_node_reconciliation_operations").await;
        assert!(operation_columns.contains("principal_id"));
        assert!(operation_columns.contains("assignment_phase"));
        assert!(!operation_columns.contains("agent_selected_installation_id"));
    }

    async fn column_names(db: &sea_orm::DatabaseConnection, table: &str) -> HashSet<String> {
        db.query_all(Statement::from_string(
            DbBackend::Sqlite,
            format!("PRAGMA table_info({table})"),
        ))
        .await
        .expect("table info")
        .into_iter()
        .map(|row| row.try_get("", "name").expect("column name"))
        .collect()
    }

    async fn primary_key_columns(db: &sea_orm::DatabaseConnection, table: &str) -> Vec<String> {
        let mut columns = db
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                format!("PRAGMA table_info({table})"),
            ))
            .await
            .expect("table info")
            .into_iter()
            .filter_map(|row| {
                let ordinal: i64 = row.try_get("", "pk").expect("primary key ordinal");
                (ordinal > 0).then(|| {
                    (
                        ordinal,
                        row.try_get::<String>("", "name").expect("column name"),
                    )
                })
            })
            .collect::<Vec<_>>();
        columns.sort_by_key(|(ordinal, _)| *ordinal);
        columns.into_iter().map(|(_, name)| name).collect()
    }
}
