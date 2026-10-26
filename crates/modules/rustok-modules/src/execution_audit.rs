use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, Statement, TransactionTrait, Value as SqlValue,
};
use thiserror::Error;
use uuid::Uuid;

use rustok_api::ArtifactBindingExecutionAuditEntry;
use rustok_sandbox::{
    ExecutionObserver, ExecutionRecord, ExecutionStatus, SandboxError, SandboxResult,
    SandboxSubject,
};

use crate::backend::SupportedBackend;
use crate::data::configure_tenant_scope;
use crate::installation::valid_digest;

/// Durable, redacted execution audit adapter for installed module artifacts.
///
/// The neutral sandbox passes only immutable artifact identity, request context,
/// status, bounded metrics, and a stable error code. Payload, input, output,
/// capability grants, credentials, and error text never reach this adapter.
#[derive(Clone)]
pub struct SeaOrmArtifactExecutionObserver {
    db: DatabaseConnection,
}

#[derive(Debug, Error)]
pub enum ArtifactBindingExecutionAuditError {
    #[error("artifact binding audit query is invalid")]
    InvalidRequest,
    #[error("artifact binding audit storage failed: {0}")]
    Storage(String),
}

/// Tenant-scoped reader for host-rendered action and form audit presentation.
/// It accepts exact owner-selected installation and binding identities; callers
/// must authorize the binding before using the returned redacted evidence.
#[derive(Clone)]
pub struct SeaOrmArtifactBindingExecutionAuditReader {
    db: DatabaseConnection,
}

impl SeaOrmArtifactBindingExecutionAuditReader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        installation_id: Uuid,
        binding_id: &str,
        limit: u32,
    ) -> Result<Vec<ArtifactBindingExecutionAuditEntry>, ArtifactBindingExecutionAuditError> {
        if tenant_id.is_nil()
            || installation_id.is_nil()
            || binding_id.trim().is_empty()
            || binding_id != binding_id.trim()
            || binding_id.len() > 256
            || !(1..=100).contains(&limit)
        {
            return Err(ArtifactBindingExecutionAuditError::InvalidRequest);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT CAST(execution_id AS TEXT) AS execution_id, status, \
                     started_at, finished_at, duration_ms, error_code \
                     FROM module_artifact_execution_audit \
                     WHERE tenant_id = {} AND installation_id = {} AND binding_id = {} \
                     ORDER BY started_at DESC, execution_id DESC LIMIT {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                ),
                vec![
                    backend.uuid_value(tenant_id),
                    backend.uuid_value(installation_id),
                    binding_id.to_string().into(),
                    i64::from(limit).into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        rows.into_iter()
            .map(|row| audit_entry_from_row(row, backend))
            .collect()
    }
}

impl SeaOrmArtifactExecutionObserver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn persist(&self, record: &ExecutionRecord) -> SandboxResult<()> {
        if record.execution_id.is_nil() || record.context.execution_id != record.execution_id {
            return Err(audit_error("execution id is nil or inconsistent"));
        }
        let SandboxSubject::ModuleArtifact {
            installation_id,
            slug,
            version,
            digest,
        } = &record.subject
        else {
            return Err(audit_error("execution subject is not a module artifact"));
        };
        if installation_id.is_nil()
            || slug.trim().is_empty()
            || version.trim().is_empty()
            || !valid_digest(digest)
            || record
                .context
                .actor_id
                .as_ref()
                .is_some_and(|actor| actor.len() > 256)
            || record
                .context
                .trace_id
                .as_ref()
                .is_some_and(|trace| trace.len() > 256)
            || record.context.audit_label.as_ref().is_some_and(|label| {
                label.trim().is_empty() || label != label.trim() || label.len() > 256
            })
            || record
                .error_code
                .as_ref()
                .is_some_and(|code| code.len() > 96)
        {
            return Err(audit_error("execution record identity or labels are invalid"));
        }

        let transaction = self.db.begin().await.map_err(audit_error)?;
        match record.context.tenant_id {
            Some(tenant_id) => {
                configure_tenant_scope(&transaction, tenant_id)
                    .await
                    .map_err(audit_error)?;
            }
            None => {
                crate::backend::set_platform_scope(&transaction)
                    .await
                    .map_err(audit_error)?;
            }
        }
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(audit_error)?;
        match record.status {
            ExecutionStatus::Started => {
                let columns = "execution_id, tenant_id, installation_id, module_slug, module_version, artifact_digest, executor, phase, actor_id, trace_id, binding_id, status, started_at";
                let values = (1..=13)
                    .map(|index| backend.placeholder(index))
                    .collect::<Vec<_>>()
                    .join(", ");
                transaction
                    .execute_raw(Statement::from_sql_and_values(
                        backend.db_backend(),
                        format!(
                            "INSERT INTO module_artifact_execution_audit ({columns}) \
                             VALUES ({values}) ON CONFLICT (execution_id) DO NOTHING"
                        ),
                        vec![
                            backend.uuid_value(record.execution_id),
                            backend.optional_uuid_value(record.context.tenant_id),
                            backend.uuid_value(*installation_id),
                            slug.clone().into(),
                            version.clone().into(),
                            digest.clone().into(),
                            executor_name(record).into(),
                            phase_name(record).into(),
                            optional_string_value(record.context.actor_id.clone()),
                            optional_string_value(record.context.trace_id.clone()),
                            optional_string_value(record.context.audit_label.clone()),
                            "started".into(),
                            backend.datetime_value(record.started_at),
                        ],
                    ))
                    .await
                    .map_err(audit_error)?;
            }
            ExecutionStatus::Succeeded | ExecutionStatus::Failed => {
                let updated = transaction
                    .execute_raw(Statement::from_sql_and_values(
                        backend.db_backend(),
                        format!(
                            "UPDATE module_artifact_execution_audit \
                             SET status = {}, finished_at = {}, queue_time_ms = {}, \
                                 duration_ms = {}, instructions_consumed = {}, \
                                 peak_memory_bytes = {}, output_bytes = {}, \
                                 capability_calls = {}, error_code = {} \
                             WHERE execution_id = {} AND status = 'started'",
                            backend.placeholder(1),
                            backend.placeholder(2),
                            backend.placeholder(3),
                            backend.placeholder(4),
                            backend.placeholder(5),
                            backend.placeholder(6),
                            backend.placeholder(7),
                            backend.placeholder(8),
                            backend.placeholder(9),
                            backend.placeholder(10),
                        ),
                        vec![
                            status_name(record).into(),
                            backend.optional_datetime_value(record.finished_at),
                            optional_metric_value(
                                record.metrics.as_ref().map(|value| value.queue_time_ms),
                            )?,
                            optional_metric_value(
                                record.metrics.as_ref().map(|value| value.duration_ms),
                            )?,
                            optional_metric_value(
                                record
                                    .metrics
                                    .as_ref()
                                    .and_then(|value| value.instructions_consumed),
                            )?,
                            optional_metric_value(
                                record
                                    .metrics
                                    .as_ref()
                                    .and_then(|value| value.peak_memory_bytes),
                            )?,
                            optional_metric_value(
                                record.metrics.as_ref().and_then(|value| value.output_bytes),
                            )?,
                            optional_metric_value(
                                record
                                    .metrics
                                    .as_ref()
                                    .map(|value| u64::from(value.capability_calls)),
                            )?,
                            optional_string_value(record.error_code.clone()),
                            backend.uuid_value(record.execution_id),
                        ],
                    ))
                    .await
                    .map_err(audit_error)?;
                if updated.rows_affected() != 1 {
                    return Err(audit_error(
                        "execution finish did not match a started audit row",
                    ));
                }
            }
        }
        transaction.commit().await.map_err(audit_error)?;
        Ok(())
    }
}

/// Decodes an audit timestamp in its backend-native representation and
/// re-emits canonical RFC 3339, so the wire value does not depend on the
/// backend's text casting of timestamps.
fn audit_timestamp(
    row: &sea_orm::QueryResult,
    backend: SupportedBackend,
    column: &str,
) -> Result<Option<String>, ArtifactBindingExecutionAuditError> {
    match backend {
        SupportedBackend::Postgres => row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", column)
            .map(|value| value.map(|timestamp| timestamp.to_rfc3339()))
            .map_err(storage_error),
        SupportedBackend::Sqlite => row
            .try_get::<Option<String>>("", column)
            .map_err(storage_error)?
            .map(|value| {
                crate::backend::parse_sqlite_datetime(&value)
                    .map(|timestamp| timestamp.to_rfc3339())
                    .map_err(storage_error)
            })
            .transpose(),
    }
}

fn audit_entry_from_row(
    row: sea_orm::QueryResult,
    backend: SupportedBackend,
) -> Result<ArtifactBindingExecutionAuditEntry, ArtifactBindingExecutionAuditError> {
    let execution_id = row
        .try_get::<String>("", "execution_id")
        .map_err(storage_error)
        .and_then(|value| Uuid::parse_str(&value).map_err(storage_error))?;
    let duration_ms = row
        .try_get::<Option<i64>>("", "duration_ms")
        .map_err(storage_error)?
        .map(|value| u64::try_from(value).map_err(storage_error))
        .transpose()?;
    Ok(ArtifactBindingExecutionAuditEntry {
        execution_id,
        status: row.try_get("", "status").map_err(storage_error)?,
        started_at: audit_timestamp(&row, backend, "started_at")?.ok_or_else(|| {
            storage_error("execution audit row is missing its start timestamp")
        })?,
        finished_at: audit_timestamp(&row, backend, "finished_at")?,
        duration_ms,
        error_code: row.try_get("", "error_code").map_err(storage_error)?,
    })
}

#[async_trait]
impl ExecutionObserver for SeaOrmArtifactExecutionObserver {
    async fn observe(&self, record: &ExecutionRecord) -> SandboxResult<()> {
        self.persist(record).await
    }
}

fn executor_name(record: &ExecutionRecord) -> &'static str {
    match record.executor {
        rustok_sandbox::SandboxExecutorKind::Rhai => "rhai",
        rustok_sandbox::SandboxExecutorKind::WasmComponent => "wasm_component",
        rustok_sandbox::SandboxExecutorKind::Sidecar => "sidecar",
    }
}

fn phase_name(record: &ExecutionRecord) -> &'static str {
    match record.context.phase {
        rustok_sandbox::ExecutionPhase::Validate => "validate",
        rustok_sandbox::ExecutionPhase::Test => "test",
        rustok_sandbox::ExecutionPhase::Manual => "manual",
        rustok_sandbox::ExecutionPhase::BeforeHook => "before_hook",
        rustok_sandbox::ExecutionPhase::AfterHook => "after_hook",
        rustok_sandbox::ExecutionPhase::Scheduled => "scheduled",
        rustok_sandbox::ExecutionPhase::Http => "http",
        rustok_sandbox::ExecutionPhase::Event => "event",
        rustok_sandbox::ExecutionPhase::Lifecycle => "lifecycle",
    }
}

fn status_name(record: &ExecutionRecord) -> &'static str {
    match record.status {
        ExecutionStatus::Started => "started",
        ExecutionStatus::Succeeded => "succeeded",
        ExecutionStatus::Failed => "failed",
    }
}

fn optional_string_value(value: Option<String>) -> SqlValue {
    value.map_or(SqlValue::String(None), Into::into)
}

fn optional_metric_value(value: Option<u64>) -> SandboxResult<SqlValue> {
    match value {
        Some(value) => i64::try_from(value).map(SqlValue::from).map_err(audit_error),
        None => Ok(SqlValue::BigInt(None)),
    }
}

fn audit_error<E: std::fmt::Display>(error: E) -> SandboxError {
    SandboxError::AuditUnavailable(error.to_string())
}

fn storage_error(error: impl std::fmt::Display) -> ArtifactBindingExecutionAuditError {
    ArtifactBindingExecutionAuditError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use sea_orm::{ConnectionTrait, Database};

    use super::*;
    use rustok_sandbox::{ExecutionPhase, SandboxContext, SandboxExecutorKind};

    async fn database() -> DatabaseConnection {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        database
            .execute_unprepared(
                "CREATE TABLE module_artifact_execution_audit (\
                 execution_id TEXT PRIMARY KEY, tenant_id TEXT NULL, installation_id TEXT NOT NULL, \
                 module_slug TEXT NOT NULL, module_version TEXT NOT NULL, artifact_digest TEXT NOT NULL, \
                 executor TEXT NOT NULL, phase TEXT NOT NULL, actor_id TEXT NULL, trace_id TEXT NULL, \
                 binding_id TEXT NULL, status TEXT NOT NULL, started_at TEXT NOT NULL, \
                 finished_at TEXT NULL, duration_ms INTEGER NULL, error_code TEXT NULL)",
            )
            .await
            .expect("audit table");
        database
    }

    fn started_record(tenant_id: Uuid, installation_id: Uuid, binding_id: &str) -> ExecutionRecord {
        let mut context = SandboxContext::new(ExecutionPhase::Http);
        context.tenant_id = Some(tenant_id);
        context.audit_label = Some(binding_id.to_string());
        ExecutionRecord {
            execution_id: context.execution_id,
            subject: SandboxSubject::ModuleArtifact {
                installation_id,
                slug: "payments".to_string(),
                version: "1.0.0".to_string(),
                digest: format!("sha256:{}", "a".repeat(64)),
            },
            context,
            executor: SandboxExecutorKind::WasmComponent,
            status: ExecutionStatus::Started,
            // Audit timestamps persist at the crate's fixed-millisecond
            // text precision; align the fixture so the stored evidence
            // string can be asserted exactly.
            started_at: chrono::DateTime::from_timestamp_millis(Utc::now().timestamp_millis())
                .expect("millis-aligned timestamp"),
            finished_at: None,
            metrics: None,
            error_code: None,
        }
    }

    #[tokio::test]
    async fn reader_exposes_only_redacted_evidence_for_the_exact_binding() {
        let database = database().await;
        let observer = SeaOrmArtifactExecutionObserver::new(database.clone());
        let reader = SeaOrmArtifactBindingExecutionAuditReader::new(database);
        let tenant_id = Uuid::new_v4();
        let installation_id = Uuid::new_v4();
        let expected = started_record(tenant_id, installation_id, "admin_actions.reconcile");
        let other_binding = started_record(tenant_id, installation_id, "admin_actions.rotate");
        let other_tenant =
            started_record(Uuid::new_v4(), installation_id, "admin_actions.reconcile");

        observer.observe(&expected).await.expect("expected audit");
        observer
            .observe(&other_binding)
            .await
            .expect("other binding audit");
        observer
            .observe(&other_tenant)
            .await
            .expect("other tenant audit");

        let entries = reader
            .list(tenant_id, installation_id, "admin_actions.reconcile", 50)
            .await
            .expect("binding audit evidence");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].execution_id, expected.execution_id);
        assert_eq!(entries[0].status, "started");
        assert_eq!(entries[0].started_at, expected.started_at.to_rfc3339());
        assert_eq!(entries[0].finished_at, None);
        assert_eq!(entries[0].duration_ms, None);
        assert_eq!(entries[0].error_code, None);
        assert_eq!(
            serde_json::to_value(&entries[0]).expect("redacted audit serialization"),
            serde_json::json!({
                "execution_id": expected.execution_id,
                "status": "started",
                "started_at": expected.started_at.to_rfc3339(),
                "finished_at": null,
                "duration_ms": null,
                "error_code": null,
            })
        );
        assert!(matches!(
            reader.list(tenant_id, installation_id, " ", 50).await,
            Err(ArtifactBindingExecutionAuditError::InvalidRequest)
        ));
    }
}
