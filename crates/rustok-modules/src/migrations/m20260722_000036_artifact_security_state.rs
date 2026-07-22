use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Adds global quarantine/revocation state for immutable artifact releases.
/// This state is deliberately separate from tenant enablement and registry
/// yanking so emergency enforcement cannot rewrite user intent.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_security_states (\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) BETWEEN 1 AND 128),\
                    module_version TEXT NOT NULL CHECK (length(trim(module_version)) BETWEEN 1 AND 128),\
                    payload_digest TEXT NOT NULL CHECK (payload_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    status TEXT NOT NULL CHECK (status IN ('clear', 'quarantined', 'revoked')),\
                    policy_revision TEXT NOT NULL CHECK (length(trim(policy_revision)) BETWEEN 1 AND 128),\
                    reason_code TEXT NOT NULL CHECK (length(trim(reason_code)) BETWEEN 1 AND 128),\
                    reason_detail TEXT NOT NULL CHECK (length(trim(reason_detail)) BETWEEN 1 AND 2000),\
                    changed_by UUID NOT NULL,\
                    changed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    PRIMARY KEY (module_slug, module_version, payload_digest)\
                )",
                "CREATE INDEX module_artifact_security_states_status_idx ON module_artifact_security_states (status, module_slug, module_version)",
                "CREATE TABLE module_artifact_security_operations (\
                    idempotency_key UUID PRIMARY KEY,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('quarantine', 'clear_quarantine', 'revoke')),\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    principal_id UUID NOT NULL,\
                    receipt_json TEXT NULL,\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TIMESTAMPTZ NULL,\
                    CHECK ((completed_at IS NULL AND receipt_json IS NULL) OR\
                           (completed_at IS NOT NULL AND receipt_json IS NOT NULL))\
                )",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_security_states (\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) BETWEEN 1 AND 128),\
                    module_version TEXT NOT NULL CHECK (length(trim(module_version)) BETWEEN 1 AND 128),\
                    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71 AND substr(payload_digest,1,7) = 'sha256:' AND substr(payload_digest,8) NOT GLOB '*[^0-9a-f]*'),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    status TEXT NOT NULL CHECK (status IN ('clear','quarantined','revoked')),\
                    policy_revision TEXT NOT NULL CHECK (length(trim(policy_revision)) BETWEEN 1 AND 128),\
                    reason_code TEXT NOT NULL CHECK (length(trim(reason_code)) BETWEEN 1 AND 128),\
                    reason_detail TEXT NOT NULL CHECK (length(trim(reason_detail)) BETWEEN 1 AND 2000),\
                    changed_by TEXT NOT NULL, changed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    PRIMARY KEY (module_slug,module_version,payload_digest)\
                )",
                "CREATE INDEX module_artifact_security_states_status_idx ON module_artifact_security_states (status,module_slug,module_version)",
                "CREATE TABLE module_artifact_security_operations (\
                    idempotency_key TEXT PRIMARY KEY,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('quarantine','clear_quarantine','revoke')),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest,1,7) = 'sha256:' AND substr(request_digest,8) NOT GLOB '*[^0-9a-f]*'),\
                    principal_id TEXT NOT NULL, receipt_json TEXT NULL,\
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, completed_at TEXT NULL,\
                    CHECK ((completed_at IS NULL AND receipt_json IS NULL) OR (completed_at IS NOT NULL AND receipt_json IS NOT NULL))\
                )",
            ],
            backend => {
                return Err(DbErr::Custom(format!(
                    "artifact security migration does not support {backend:?}"
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
            "module_artifact_security_operations",
            "module_artifact_security_states",
        ] {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    format!("DROP TABLE IF EXISTS {table}"),
                ))
                .await?;
        }
        Ok(())
    }
}
