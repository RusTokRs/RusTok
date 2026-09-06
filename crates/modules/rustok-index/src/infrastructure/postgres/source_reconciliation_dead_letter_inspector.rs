use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value as SqlValue,
};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

const RECONCILIATION_FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1";
const MAX_ERROR_CODE_BYTES: usize = 128;

/// A bounded, read-only view of one failed reconciliation job.
///
/// The inspection intentionally excludes the tenant, schema request, cursor, worker,
/// lease, timestamps, and raw diagnostic JSON. Callers receive only stable machine
/// fields that are safe to expose behind an operator authorization boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReconciliationDeadLetterInspection {
    job_id: Uuid,
    attempt_count: u32,
    error_code: Option<String>,
    dependency_code: String,
    retryable: bool,
}

impl IndexReconciliationDeadLetterInspection {
    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    pub fn error_code(&self) -> Option<&str> {
        self.error_code.as_deref()
    }

    pub fn dependency_code(&self) -> &str {
        &self.dependency_code
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredReconciliationFailure {
    contract: String,
    dependency_code: String,
    retryable: bool,
}

/// PostgreSQL-backed read-only dead-letter inspector.
///
/// Authorization is deliberately not accepted as data by this adapter. Server,
/// GraphQL, HTTP, CLI, and admin callers must authorize the exact tenant and actor
/// before invoking `inspect`.
#[derive(Clone)]
pub struct PostgresIndexReconciliationDeadLetterInspector {
    db: DatabaseConnection,
}

impl PostgresIndexReconciliationDeadLetterInspector {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn inspect(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
    ) -> Result<
        Option<IndexReconciliationDeadLetterInspection>,
        IndexReconciliationDeadLetterInspectionError,
    > {
        if tenant_id.is_nil() {
            return Err(IndexReconciliationDeadLetterInspectionError::NilTenantId);
        }
        if job_id.is_nil() {
            return Err(IndexReconciliationDeadLetterInspectionError::NilJobId);
        }

        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                select_failed_job_sql(backend),
                vec![uuid_value(tenant_id, backend), uuid_value(job_id, backend)],
            ))
            .await
            .map_err(|_| IndexReconciliationDeadLetterInspectionError::Storage)?;
        row.map(|row| decode_failed_job(&row, job_id)).transpose()
    }
}

fn decode_failed_job(
    row: &QueryResult,
    job_id: Uuid,
) -> Result<IndexReconciliationDeadLetterInspection, IndexReconciliationDeadLetterInspectionError> {
    let attempt_count: i64 = row
        .try_get("", "attempt_count_value")
        .map_err(|_| IndexReconciliationDeadLetterInspectionError::Storage)?;
    let attempt_count = u32::try_from(attempt_count).map_err(|_| {
        IndexReconciliationDeadLetterInspectionError::InvalidStoredJob(
            "attempt count is outside the u32 range",
        )
    })?;
    if attempt_count == 0 {
        return Err(
            IndexReconciliationDeadLetterInspectionError::InvalidStoredJob(
                "attempt count must be positive",
            ),
        );
    }

    let error_code: Option<String> = row
        .try_get("", "last_error_code")
        .map_err(|_| IndexReconciliationDeadLetterInspectionError::Storage)?;
    if let Some(code) = &error_code {
        validate_machine_code(code).map_err(|_| {
            IndexReconciliationDeadLetterInspectionError::InvalidStoredJob(
                "last_error_code is outside the bounded machine-code contract",
            )
        })?;
    }

    let details: JsonValue = row
        .try_get("", "last_error_details")
        .map_err(|_| IndexReconciliationDeadLetterInspectionError::Storage)?;
    let details: StoredReconciliationFailure = serde_json::from_value(details).map_err(|_| {
        IndexReconciliationDeadLetterInspectionError::InvalidStoredJob(
            "last_error_details does not match the reconciliation failure contract",
        )
    })?;
    if details.contract != RECONCILIATION_FAILURE_CONTRACT {
        return Err(
            IndexReconciliationDeadLetterInspectionError::InvalidStoredJob(
                "last_error_details contract is unsupported",
            ),
        );
    }
    validate_machine_code(&details.dependency_code).map_err(|_| {
        IndexReconciliationDeadLetterInspectionError::InvalidStoredJob(
            "dependency_code is outside the bounded machine-code contract",
        )
    })?;

    Ok(IndexReconciliationDeadLetterInspection {
        job_id,
        attempt_count,
        error_code,
        dependency_code: details.dependency_code,
        retryable: details.retryable,
    })
}

fn validate_machine_code(value: &str) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > MAX_ERROR_CODE_BYTES
        || value.trim() != value
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(());
    }
    Ok(())
}

fn ensure_supported_backend(
    backend: DbBackend,
) -> Result<(), IndexReconciliationDeadLetterInspectionError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        _ => Err(IndexReconciliationDeadLetterInspectionError::UnsupportedBackend),
    }
}

fn placeholder_prefix(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "$",
        DbBackend::Sqlite => "?",
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.to_string().into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn select_failed_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let attempt_count = match backend {
        DbBackend::Postgres => "CAST(attempt_count AS BIGINT)",
        DbBackend::Sqlite => "CAST(attempt_count AS INTEGER)",
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "SELECT {attempt_count} AS attempt_count_value, last_error_code, last_error_details FROM index_jobs WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'failed' LIMIT 1"
    )
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IndexReconciliationDeadLetterInspectionError {
    #[error("Index reconciliation dead-letter inspection tenant id must not be nil")]
    NilTenantId,
    #[error("Index reconciliation dead-letter inspection job id must not be nil")]
    NilJobId,
    #[error("stored Index reconciliation dead letter is invalid: {0}")]
    InvalidStoredJob(&'static str),
    #[error("Index reconciliation dead-letter inspection does not support this database backend")]
    UnsupportedBackend,
    #[error("Index reconciliation dead-letter inspection storage operation failed")]
    Storage,
}

#[cfg(test)]
mod tests {
    use sea_orm::{Database, DbBackend, Statement};
    use serde_json::json;

    use super::*;

    async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        db.execute_raw(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE index_jobs (tenant_id TEXT NOT NULL, job_id TEXT NOT NULL, kind TEXT NOT NULL, state TEXT NOT NULL, attempt_count INTEGER NOT NULL, last_error_code TEXT NULL, last_error_details JSON NOT NULL)"
                .to_owned(),
        ))
        .await
        .expect("index_jobs fixture");
        db
    }

    async fn insert_failed(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        job_id: Uuid,
        details: JsonValue,
    ) {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO index_jobs (tenant_id, job_id, kind, state, attempt_count, last_error_code, last_error_details) VALUES (?1, ?2, 'reconcile', 'failed', 3, ?3, ?4)",
            vec![
                tenant_id.to_string().into(),
                job_id.to_string().into(),
                "index.reconciliation_page_failed".into(),
                SqlValue::Json(Some(Box::new(details))),
            ],
        ))
        .await
        .expect("failed job fixture");
    }

    #[tokio::test]
    async fn inspection_is_tenant_scoped_and_bounded() {
        let db = database().await;
        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        insert_failed(
            &db,
            tenant_id,
            job_id,
            json!({
                "contract": RECONCILIATION_FAILURE_CONTRACT,
                "dependency_code": "owner_source_retryable",
                "retryable": true
            }),
        )
        .await;
        let inspector = PostgresIndexReconciliationDeadLetterInspector::new(db);

        assert!(
            inspector
                .inspect(Uuid::new_v4(), job_id)
                .await
                .unwrap()
                .is_none()
        );
        let inspection = inspector
            .inspect(tenant_id, job_id)
            .await
            .unwrap()
            .expect("exact tenant failed job");
        assert_eq!(inspection.job_id(), job_id);
        assert_eq!(inspection.attempt_count(), 3);
        assert_eq!(
            inspection.error_code(),
            Some("index.reconciliation_page_failed")
        );
        assert_eq!(inspection.dependency_code(), "owner_source_retryable");
        assert!(inspection.retryable());
    }

    #[tokio::test]
    async fn inspection_fails_closed_on_unbounded_diagnostic_shape() {
        let db = database().await;
        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        insert_failed(
            &db,
            tenant_id,
            job_id,
            json!({
                "contract": RECONCILIATION_FAILURE_CONTRACT,
                "dependency_code": "owner_source_permanent",
                "retryable": false,
                "private_detail": "must-not-pass"
            }),
        )
        .await;
        let inspector = PostgresIndexReconciliationDeadLetterInspector::new(db);

        assert!(matches!(
            inspector.inspect(tenant_id, job_id).await,
            Err(IndexReconciliationDeadLetterInspectionError::InvalidStoredJob(_))
        ));
    }
}
