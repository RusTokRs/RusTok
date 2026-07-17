use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value as SqlValue,
};

use rustok_sandbox::{
    ExecutionObserver, ExecutionRecord, ExecutionStatus, SandboxError, SandboxResult,
    SandboxSubject,
};

use crate::data::{configure_tenant_scope, placeholder, uuid_value};

/// Durable, redacted execution audit adapter for installed module artifacts.
///
/// The neutral sandbox passes only immutable artifact identity, request context,
/// status, bounded metrics, and a stable error code. Payload, input, output,
/// capability grants, credentials, and error text never reach this adapter.
#[derive(Clone)]
pub struct SeaOrmArtifactExecutionObserver {
    db: DatabaseConnection,
}

impl SeaOrmArtifactExecutionObserver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn persist(&self, record: &ExecutionRecord) -> Result<(), ()> {
        if record.execution_id.is_nil() || record.context.execution_id != record.execution_id {
            return Err(());
        }
        let SandboxSubject::ModuleArtifact {
            installation_id,
            slug,
            version,
            digest,
        } = &record.subject
        else {
            return Err(());
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
            || record
                .error_code
                .as_ref()
                .is_some_and(|code| code.len() > 96)
        {
            return Err(());
        }

        let transaction = self.db.begin().await.map_err(|_| ())?;
        if let Some(tenant_id) = record.context.tenant_id {
            configure_tenant_scope(&transaction, tenant_id)
                .await
                .map_err(|_| ())?;
        }
        let backend = transaction.get_database_backend();
        match record.status {
            ExecutionStatus::Started => {
                let columns = "execution_id, tenant_id, installation_id, module_slug, module_version, artifact_digest, executor, phase, actor_id, trace_id, status, started_at";
                let values = (1..=12)
                    .map(|index| placeholder(backend, index))
                    .collect::<Vec<_>>()
                    .join(", ");
                transaction
                    .execute(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "INSERT INTO module_artifact_execution_audit ({columns}) \
                             VALUES ({values}) ON CONFLICT (execution_id) DO NOTHING"
                        ),
                        vec![
                            uuid_value(record.execution_id, backend),
                            optional_uuid_value(record.context.tenant_id, backend),
                            uuid_value(*installation_id, backend),
                            slug.clone().into(),
                            version.clone().into(),
                            digest.clone().into(),
                            executor_name(record).into(),
                            phase_name(record).into(),
                            optional_string_value(record.context.actor_id.clone()),
                            optional_string_value(record.context.trace_id.clone()),
                            "started".into(),
                            record.started_at.to_rfc3339().into(),
                        ],
                    ))
                    .await
                    .map_err(|_| ())?;
            }
            ExecutionStatus::Succeeded | ExecutionStatus::Failed => {
                let updated = transaction
                    .execute(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "UPDATE module_artifact_execution_audit \
                             SET status = {}, finished_at = {}, queue_time_ms = {}, \
                                 duration_ms = {}, instructions_consumed = {}, \
                                 peak_memory_bytes = {}, output_bytes = {}, \
                                 capability_calls = {}, error_code = {} \
                             WHERE execution_id = {} AND status = 'started'",
                            placeholder(backend, 1),
                            placeholder(backend, 2),
                            placeholder(backend, 3),
                            placeholder(backend, 4),
                            placeholder(backend, 5),
                            placeholder(backend, 6),
                            placeholder(backend, 7),
                            placeholder(backend, 8),
                            placeholder(backend, 9),
                            placeholder(backend, 10),
                        ),
                        vec![
                            status_name(record).into(),
                            optional_timestamp_value(record.finished_at),
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
                            uuid_value(record.execution_id, backend),
                        ],
                    ))
                    .await
                    .map_err(|_| ())?;
                if updated.rows_affected() != 1 {
                    return Err(());
                }
            }
        }
        transaction.commit().await.map_err(|_| ())?;
        Ok(())
    }
}

#[async_trait]
impl ExecutionObserver for SeaOrmArtifactExecutionObserver {
    async fn observe(&self, record: &ExecutionRecord) -> SandboxResult<()> {
        self.persist(record).await.map_err(|_| {
            SandboxError::AuditUnavailable(
                "artifact execution audit persistence failed".to_string(),
            )
        })
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
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

fn optional_uuid_value(value: Option<uuid::Uuid>, backend: DbBackend) -> SqlValue {
    match value {
        Some(value) => uuid_value(value, backend),
        None => match backend {
            DbBackend::Postgres => SqlValue::Uuid(None),
            _ => SqlValue::String(None),
        },
    }
}

fn optional_string_value(value: Option<String>) -> SqlValue {
    value.map_or(SqlValue::String(None), Into::into)
}

fn optional_timestamp_value(value: Option<chrono::DateTime<chrono::Utc>>) -> SqlValue {
    value.map_or(SqlValue::String(None), |timestamp| {
        timestamp.to_rfc3339().into()
    })
}

fn optional_metric_value(value: Option<u64>) -> Result<SqlValue, ()> {
    match value {
        Some(value) => i64::try_from(value).map(SqlValue::from).map_err(|_| ()),
        None => Ok(SqlValue::BigInt(None)),
    }
}
