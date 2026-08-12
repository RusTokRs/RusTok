use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Adds the durable desired/observed native-distribution rollout aggregate.
/// Deployment agents report evidence; the control plane never loads or starts
/// the native artifact itself.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_static_distribution_rollouts (\
                    rollout_id UUID PRIMARY KEY,\
                    predecessor_rollout_id UUID NULL REFERENCES module_static_distribution_rollouts(rollout_id) ON DELETE RESTRICT,\
                    distribution_release_id UUID NOT NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    transition_kind TEXT NOT NULL CHECK (transition_kind IN ('update', 'recovery')),\
                    rollout_revision BIGINT NOT NULL UNIQUE CHECK (rollout_revision > 0),\
                    distribution_release_revision BIGINT NOT NULL CHECK (distribution_release_revision > 0),\
                    release_state_revision_at_request BIGINT NOT NULL CHECK (release_state_revision_at_request > 0),\
                    composition_revision BIGINT NOT NULL CHECK (composition_revision > 0),\
                    composition_digest TEXT NOT NULL CHECK (composition_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    bundle_reference TEXT NOT NULL CHECK (length(trim(bundle_reference)) BETWEEN 1 AND 512),\
                    bundle_root_digest TEXT NOT NULL CHECK (bundle_root_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    role_set_digest TEXT NOT NULL CHECK (role_set_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    executor_mode TEXT NOT NULL CHECK (executor_mode = 'static_native'),\
                    topology_reference TEXT NOT NULL CHECK (length(trim(topology_reference)) BETWEEN 1 AND 512),\
                    topology_digest TEXT NOT NULL CHECK (topology_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    policy_revision TEXT NOT NULL CHECK (length(trim(policy_revision)) BETWEEN 1 AND 128),\
                    target_assignment_count INTEGER NOT NULL CHECK (target_assignment_count BETWEEN 1 AND 1024),\
                    status TEXT NOT NULL CHECK (status IN ('preparing', 'activating', 'converged', 'failed', 'degraded', 'superseded')),\
                    requested_by UUID NOT NULL,\
                    requested_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    status_changed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    converged_at TIMESTAMPTZ NULL,\
                    failed_at TIMESTAMPTZ NULL,\
                    failure_code TEXT NULL CHECK (failure_code IS NULL OR length(trim(failure_code)) BETWEEN 1 AND 128),\
                    failure_detail TEXT NULL CHECK (failure_detail IS NULL OR length(trim(failure_detail)) BETWEEN 1 AND 2000),\
                    CHECK (predecessor_rollout_id IS NULL OR predecessor_rollout_id <> rollout_id),\
                    CHECK ((status IN ('converged', 'superseded') AND converged_at IS NOT NULL AND failed_at IS NULL AND failure_code IS NULL AND failure_detail IS NULL) OR\
                           (status IN ('failed', 'degraded') AND failed_at IS NOT NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL) OR\
                           (status IN ('preparing', 'activating') AND converged_at IS NULL AND failed_at IS NULL AND failure_code IS NULL AND failure_detail IS NULL))\
                )",
                "CREATE INDEX module_static_distribution_rollouts_release_idx ON module_static_distribution_rollouts (distribution_release_id, rollout_revision)",
                "CREATE TABLE module_static_distribution_rollout_assignments (\
                    rollout_id UUID NOT NULL REFERENCES module_static_distribution_rollouts(rollout_id) ON DELETE RESTRICT,\
                    node_id TEXT NOT NULL CHECK (length(trim(node_id)) BETWEEN 1 AND 128),\
                    role TEXT NOT NULL CHECK (role IN ('monolith', 'api', 'admin_ssr', 'storefront_ssr', 'worker', 'registry')),\
                    candidate_artifact_digest TEXT NOT NULL CHECK (candidate_artifact_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    predecessor_artifact_digest TEXT NULL CHECK (predecessor_artifact_digest IS NULL OR predecessor_artifact_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),\
                    observation_revision BIGINT NOT NULL DEFAULT 0 CHECK (observation_revision >= 0),\
                    phase TEXT NOT NULL CHECK (phase IN ('pending', 'prepared', 'healthy', 'active', 'failed')),\
                    observed_release_id UUID NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    observed_release_revision BIGINT NULL CHECK (observed_release_revision IS NULL OR observed_release_revision > 0),\
                    observed_composition_revision BIGINT NULL CHECK (observed_composition_revision IS NULL OR observed_composition_revision > 0),\
                    observed_composition_digest TEXT NULL CHECK (observed_composition_digest IS NULL OR observed_composition_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    observed_bundle_root_digest TEXT NULL CHECK (observed_bundle_root_digest IS NULL OR observed_bundle_root_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    observed_role_set_digest TEXT NULL CHECK (observed_role_set_digest IS NULL OR observed_role_set_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    observed_policy_revision TEXT NULL CHECK (observed_policy_revision IS NULL OR length(trim(observed_policy_revision)) BETWEEN 1 AND 128),\
                    observed_executor_mode TEXT NULL CHECK (observed_executor_mode IS NULL OR observed_executor_mode = 'static_native'),\
                    health_evidence_reference TEXT NULL CHECK (health_evidence_reference IS NULL OR length(trim(health_evidence_reference)) BETWEEN 1 AND 512),\
                    health_evidence_digest TEXT NULL CHECK (health_evidence_digest IS NULL OR health_evidence_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    failure_code TEXT NULL CHECK (failure_code IS NULL OR length(trim(failure_code)) BETWEEN 1 AND 128),\
                    failure_detail TEXT NULL CHECK (failure_detail IS NULL OR length(trim(failure_detail)) BETWEEN 1 AND 2000),\
                    reported_by TEXT NULL CHECK (reported_by IS NULL OR length(trim(reported_by)) BETWEEN 1 AND 128),\
                    last_report_digest TEXT NULL CHECK (last_report_digest IS NULL OR last_report_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    first_reported_at TIMESTAMPTZ NULL,\
                    last_reported_at TIMESTAMPTZ NULL,\
                    PRIMARY KEY (rollout_id, node_id, role),\
                    UNIQUE (rollout_id, ordinal),\
                    CHECK ((phase = 'pending' AND observation_revision = 0 AND observed_release_id IS NULL AND observed_release_revision IS NULL AND observed_composition_revision IS NULL AND observed_composition_digest IS NULL AND observed_bundle_root_digest IS NULL AND observed_role_set_digest IS NULL AND observed_policy_revision IS NULL AND observed_executor_mode IS NULL AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NULL AND last_report_digest IS NULL AND first_reported_at IS NULL AND last_reported_at IS NULL) OR\
                           (phase = 'prepared' AND observation_revision > 0 AND observed_release_id IS NOT NULL AND observed_release_revision IS NOT NULL AND observed_composition_revision IS NOT NULL AND observed_composition_digest IS NOT NULL AND observed_bundle_root_digest IS NOT NULL AND observed_role_set_digest IS NOT NULL AND observed_policy_revision IS NOT NULL AND observed_executor_mode = 'static_native' AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL) OR\
                           (phase IN ('healthy', 'active') AND observation_revision > 0 AND observed_release_id IS NOT NULL AND observed_release_revision IS NOT NULL AND observed_composition_revision IS NOT NULL AND observed_composition_digest IS NOT NULL AND observed_bundle_root_digest IS NOT NULL AND observed_role_set_digest IS NOT NULL AND observed_policy_revision IS NOT NULL AND observed_executor_mode = 'static_native' AND health_evidence_reference IS NOT NULL AND health_evidence_digest IS NOT NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL) OR\
                           (phase = 'failed' AND observation_revision > 0 AND observed_release_id IS NOT NULL AND observed_release_revision IS NOT NULL AND observed_composition_revision IS NOT NULL AND observed_composition_digest IS NOT NULL AND observed_bundle_root_digest IS NOT NULL AND observed_role_set_digest IS NOT NULL AND observed_policy_revision IS NOT NULL AND observed_executor_mode = 'static_native' AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL))\
                )",
                "CREATE INDEX module_static_distribution_rollout_assignments_phase_idx ON module_static_distribution_rollout_assignments (rollout_id, phase, ordinal)",
                "CREATE TABLE module_static_distribution_rollout_state (\
                    state_id TEXT PRIMARY KEY CHECK (state_id = 'current'),\
                    revision BIGINT NOT NULL CHECK (revision >= 0),\
                    desired_rollout_id UUID NULL REFERENCES module_static_distribution_rollouts(rollout_id) ON DELETE RESTRICT,\
                    observed_rollout_id UUID NULL REFERENCES module_static_distribution_rollouts(rollout_id) ON DELETE RESTRICT,\
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
                "INSERT INTO module_static_distribution_rollout_state (state_id, revision, desired_rollout_id, observed_rollout_id) VALUES ('current', 0, NULL, NULL)",
                "CREATE TABLE module_static_distribution_rollout_operations (\
                    idempotency_key UUID PRIMARY KEY,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('request', 'recovery', 'report')),\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    principal_id TEXT NOT NULL CHECK (length(trim(principal_id)) BETWEEN 1 AND 128),\
                    rollout_id UUID NULL REFERENCES module_static_distribution_rollouts(rollout_id) ON DELETE RESTRICT,\
                    rollout_revision BIGINT NULL CHECK (rollout_revision IS NULL OR rollout_revision > 0),\
                    rollout_state_revision BIGINT NULL CHECK (rollout_state_revision IS NULL OR rollout_state_revision > 0),\
                    rollout_status TEXT NULL CHECK (rollout_status IS NULL OR rollout_status IN ('preparing', 'activating', 'converged', 'failed', 'degraded', 'superseded')),\
                    node_id TEXT NULL CHECK (node_id IS NULL OR length(trim(node_id)) BETWEEN 1 AND 128),\
                    role TEXT NULL CHECK (role IS NULL OR role IN ('monolith', 'api', 'admin_ssr', 'storefront_ssr', 'worker', 'registry')),\
                    observation_revision BIGINT NULL CHECK (observation_revision IS NULL OR observation_revision > 0),\
                    assignment_phase TEXT NULL CHECK (assignment_phase IS NULL OR assignment_phase IN ('prepared', 'healthy', 'active', 'failed')),\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TIMESTAMPTZ NULL,\
                    CHECK ((completed_at IS NULL AND rollout_id IS NULL AND rollout_revision IS NULL AND rollout_state_revision IS NULL AND rollout_status IS NULL AND node_id IS NULL AND role IS NULL AND observation_revision IS NULL AND assignment_phase IS NULL) OR\
                           (completed_at IS NOT NULL AND operation_kind IN ('request', 'recovery') AND rollout_id IS NOT NULL AND rollout_revision IS NOT NULL AND rollout_state_revision IS NOT NULL AND rollout_status = 'preparing' AND node_id IS NULL AND role IS NULL AND observation_revision IS NULL AND assignment_phase IS NULL) OR\
                           (completed_at IS NOT NULL AND operation_kind = 'report' AND rollout_id IS NOT NULL AND rollout_revision IS NOT NULL AND rollout_state_revision IS NOT NULL AND rollout_status IS NOT NULL AND node_id IS NOT NULL AND role IS NOT NULL AND observation_revision IS NOT NULL AND assignment_phase IS NOT NULL))\
                )",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_static_distribution_rollouts (\
                    rollout_id TEXT PRIMARY KEY, predecessor_rollout_id TEXT NULL REFERENCES module_static_distribution_rollouts(rollout_id) ON DELETE RESTRICT, distribution_release_id TEXT NOT NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT, transition_kind TEXT NOT NULL CHECK (transition_kind IN ('update','recovery')),\
                    rollout_revision INTEGER NOT NULL UNIQUE CHECK (rollout_revision > 0), distribution_release_revision INTEGER NOT NULL CHECK (distribution_release_revision > 0), release_state_revision_at_request INTEGER NOT NULL CHECK (release_state_revision_at_request > 0), composition_revision INTEGER NOT NULL CHECK (composition_revision > 0),\
                    composition_digest TEXT NOT NULL CHECK (length(composition_digest) = 71 AND substr(composition_digest,1,7) = 'sha256:' AND substr(composition_digest,8) NOT GLOB '*[^0-9a-f]*'), bundle_reference TEXT NOT NULL CHECK (length(trim(bundle_reference)) BETWEEN 1 AND 512), bundle_root_digest TEXT NOT NULL CHECK (length(bundle_root_digest) = 71 AND substr(bundle_root_digest,1,7) = 'sha256:' AND substr(bundle_root_digest,8) NOT GLOB '*[^0-9a-f]*'), role_set_digest TEXT NOT NULL CHECK (length(role_set_digest) = 71 AND substr(role_set_digest,1,7) = 'sha256:' AND substr(role_set_digest,8) NOT GLOB '*[^0-9a-f]*'), executor_mode TEXT NOT NULL CHECK (executor_mode = 'static_native'),\
                    topology_reference TEXT NOT NULL CHECK (length(trim(topology_reference)) BETWEEN 1 AND 512), topology_digest TEXT NOT NULL CHECK (length(topology_digest) = 71 AND substr(topology_digest,1,7) = 'sha256:' AND substr(topology_digest,8) NOT GLOB '*[^0-9a-f]*'), policy_revision TEXT NOT NULL CHECK (length(trim(policy_revision)) BETWEEN 1 AND 128), target_assignment_count INTEGER NOT NULL CHECK (target_assignment_count BETWEEN 1 AND 1024),\
                    status TEXT NOT NULL CHECK (status IN ('preparing','activating','converged','failed','degraded','superseded')), requested_by TEXT NOT NULL, requested_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, status_changed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, converged_at TEXT NULL, failed_at TEXT NULL, failure_code TEXT NULL, failure_detail TEXT NULL,\
                    CHECK (predecessor_rollout_id IS NULL OR predecessor_rollout_id <> rollout_id), CHECK ((status IN ('converged','superseded') AND converged_at IS NOT NULL AND failed_at IS NULL AND failure_code IS NULL AND failure_detail IS NULL) OR (status IN ('failed','degraded') AND failed_at IS NOT NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL) OR (status IN ('preparing','activating') AND converged_at IS NULL AND failed_at IS NULL AND failure_code IS NULL AND failure_detail IS NULL))\
                )",
                "CREATE INDEX module_static_distribution_rollouts_release_idx ON module_static_distribution_rollouts (distribution_release_id, rollout_revision)",
                "CREATE TABLE module_static_distribution_rollout_assignments (\
                    rollout_id TEXT NOT NULL REFERENCES module_static_distribution_rollouts(rollout_id) ON DELETE RESTRICT, node_id TEXT NOT NULL CHECK (length(trim(node_id)) BETWEEN 1 AND 128), role TEXT NOT NULL CHECK (role IN ('monolith','api','admin_ssr','storefront_ssr','worker','registry')), candidate_artifact_digest TEXT NOT NULL CHECK (length(candidate_artifact_digest) = 71 AND substr(candidate_artifact_digest,1,7) = 'sha256:' AND substr(candidate_artifact_digest,8) NOT GLOB '*[^0-9a-f]*'), predecessor_artifact_digest TEXT NULL CHECK (predecessor_artifact_digest IS NULL OR (length(predecessor_artifact_digest) = 71 AND substr(predecessor_artifact_digest,1,7) = 'sha256:' AND substr(predecessor_artifact_digest,8) NOT GLOB '*[^0-9a-f]*')), ordinal INTEGER NOT NULL CHECK (ordinal >= 0), observation_revision INTEGER NOT NULL DEFAULT 0 CHECK (observation_revision >= 0), phase TEXT NOT NULL CHECK (phase IN ('pending','prepared','healthy','active','failed')),\
                    observed_release_id TEXT NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT, observed_release_revision INTEGER NULL, observed_composition_revision INTEGER NULL, observed_composition_digest TEXT NULL, observed_bundle_root_digest TEXT NULL, observed_role_set_digest TEXT NULL, observed_policy_revision TEXT NULL, observed_executor_mode TEXT NULL CHECK (observed_executor_mode IS NULL OR observed_executor_mode = 'static_native'), health_evidence_reference TEXT NULL, health_evidence_digest TEXT NULL, failure_code TEXT NULL, failure_detail TEXT NULL, reported_by TEXT NULL, last_report_digest TEXT NULL, first_reported_at TEXT NULL, last_reported_at TEXT NULL,\
                    PRIMARY KEY (rollout_id,node_id,role), UNIQUE (rollout_id,ordinal),\
                    CHECK ((phase = 'pending' AND observation_revision = 0 AND observed_release_id IS NULL AND observed_release_revision IS NULL AND observed_composition_revision IS NULL AND observed_composition_digest IS NULL AND observed_bundle_root_digest IS NULL AND observed_role_set_digest IS NULL AND observed_policy_revision IS NULL AND observed_executor_mode IS NULL AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NULL AND last_report_digest IS NULL AND first_reported_at IS NULL AND last_reported_at IS NULL) OR (phase = 'prepared' AND observation_revision > 0 AND observed_release_id IS NOT NULL AND observed_release_revision IS NOT NULL AND observed_composition_revision IS NOT NULL AND observed_composition_digest IS NOT NULL AND observed_bundle_root_digest IS NOT NULL AND observed_role_set_digest IS NOT NULL AND observed_policy_revision IS NOT NULL AND observed_executor_mode = 'static_native' AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL) OR (phase IN ('healthy','active') AND observation_revision > 0 AND observed_release_id IS NOT NULL AND observed_release_revision IS NOT NULL AND observed_composition_revision IS NOT NULL AND observed_composition_digest IS NOT NULL AND observed_bundle_root_digest IS NOT NULL AND observed_role_set_digest IS NOT NULL AND observed_policy_revision IS NOT NULL AND observed_executor_mode = 'static_native' AND health_evidence_reference IS NOT NULL AND health_evidence_digest IS NOT NULL AND failure_code IS NULL AND failure_detail IS NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL) OR (phase = 'failed' AND observation_revision > 0 AND observed_release_id IS NOT NULL AND observed_release_revision IS NOT NULL AND observed_composition_revision IS NOT NULL AND observed_composition_digest IS NOT NULL AND observed_bundle_root_digest IS NOT NULL AND observed_role_set_digest IS NOT NULL AND observed_policy_revision IS NOT NULL AND observed_executor_mode = 'static_native' AND health_evidence_reference IS NULL AND health_evidence_digest IS NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL AND reported_by IS NOT NULL AND last_report_digest IS NOT NULL AND first_reported_at IS NOT NULL AND last_reported_at IS NOT NULL))\
                )",
                "CREATE INDEX module_static_distribution_rollout_assignments_phase_idx ON module_static_distribution_rollout_assignments (rollout_id, phase, ordinal)",
                "CREATE TABLE module_static_distribution_rollout_state (state_id TEXT PRIMARY KEY CHECK (state_id = 'current'), revision INTEGER NOT NULL CHECK (revision >= 0), desired_rollout_id TEXT NULL REFERENCES module_static_distribution_rollouts(rollout_id) ON DELETE RESTRICT, observed_rollout_id TEXT NULL REFERENCES module_static_distribution_rollouts(rollout_id) ON DELETE RESTRICT, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
                "INSERT INTO module_static_distribution_rollout_state (state_id,revision,desired_rollout_id,observed_rollout_id) VALUES ('current',0,NULL,NULL)",
                "CREATE TABLE module_static_distribution_rollout_operations (\
                    idempotency_key TEXT PRIMARY KEY, operation_kind TEXT NOT NULL CHECK (operation_kind IN ('request','recovery','report')), request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest,1,7) = 'sha256:' AND substr(request_digest,8) NOT GLOB '*[^0-9a-f]*'), principal_id TEXT NOT NULL CHECK (length(trim(principal_id)) BETWEEN 1 AND 128), rollout_id TEXT NULL REFERENCES module_static_distribution_rollouts(rollout_id) ON DELETE RESTRICT, rollout_revision INTEGER NULL, rollout_state_revision INTEGER NULL, rollout_status TEXT NULL, node_id TEXT NULL, role TEXT NULL CHECK (role IS NULL OR role IN ('monolith','api','admin_ssr','storefront_ssr','worker','registry')), observation_revision INTEGER NULL, assignment_phase TEXT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, completed_at TEXT NULL,\
                    CHECK ((completed_at IS NULL AND rollout_id IS NULL AND rollout_revision IS NULL AND rollout_state_revision IS NULL AND rollout_status IS NULL AND node_id IS NULL AND role IS NULL AND observation_revision IS NULL AND assignment_phase IS NULL) OR (completed_at IS NOT NULL AND operation_kind IN ('request','recovery') AND rollout_id IS NOT NULL AND rollout_revision IS NOT NULL AND rollout_state_revision IS NOT NULL AND rollout_status = 'preparing' AND node_id IS NULL AND role IS NULL AND observation_revision IS NULL AND assignment_phase IS NULL) OR (completed_at IS NOT NULL AND operation_kind = 'report' AND rollout_id IS NOT NULL AND rollout_revision IS NOT NULL AND rollout_state_revision IS NOT NULL AND rollout_status IS NOT NULL AND node_id IS NOT NULL AND role IS NOT NULL AND observation_revision IS NOT NULL AND assignment_phase IS NOT NULL))\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "static distribution rollout migration does not support database backend {backend:?}"
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
            "module_static_distribution_rollout_operations",
            "module_static_distribution_rollout_state",
            "module_static_distribution_rollout_assignments",
            "module_static_distribution_rollouts",
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
    async fn sqlite_schema_uses_complete_bundle_observation_identity() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("foreign keys");
        db.execute_unprepared(
            "CREATE TABLE module_static_distribution_releases (\
                 distribution_release_id TEXT PRIMARY KEY\
             )",
        )
        .await
        .expect("release prerequisite");
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("rollout migration");

        let rollout_columns = column_names(&db, "module_static_distribution_rollouts").await;
        assert!(rollout_columns.contains("bundle_reference"));
        assert!(rollout_columns.contains("transition_kind"));
        assert!(rollout_columns.contains("release_state_revision_at_request"));
        assert!(rollout_columns.contains("bundle_root_digest"));
        assert!(rollout_columns.contains("role_set_digest"));
        assert!(!rollout_columns.contains("artifact_reference"));
        assert!(!rollout_columns.contains("artifact_digest"));

        let assignment_columns =
            column_names(&db, "module_static_distribution_rollout_assignments").await;
        assert!(assignment_columns.contains("role"));
        assert!(assignment_columns.contains("candidate_artifact_digest"));
        assert!(assignment_columns.contains("predecessor_artifact_digest"));
        assert!(assignment_columns.contains("observed_bundle_root_digest"));
        assert!(assignment_columns.contains("observed_role_set_digest"));
        assert!(!assignment_columns.contains("observed_artifact_digest"));

        let operation_columns =
            column_names(&db, "module_static_distribution_rollout_operations").await;
        assert!(operation_columns.contains("role"));
        assert!(operation_columns.contains("assignment_phase"));
        assert!(!operation_columns.contains("node_phase"));

        let assignment_primary_key =
            primary_key_columns(&db, "module_static_distribution_rollout_assignments").await;
        assert_eq!(
            assignment_primary_key,
            vec!["rollout_id", "node_id", "role"]
        );
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
