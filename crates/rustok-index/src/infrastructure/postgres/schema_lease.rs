use std::time::Duration;

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use uuid::Uuid;

use crate::{IndexSchema, SchemaFingerprint, SchemaRef};

const MAX_WORKER_ID_BYTES: usize = 191;
const MAX_ERROR_CODE_BYTES: usize = 128;
const MAX_LEASE_SECONDS: u64 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaApplicationLeaseRequest {
    tenant_id: Uuid,
    schema: SchemaRef,
    schema_fingerprint: SchemaFingerprint,
    worker_id: String,
    lease_seconds: u64,
}

impl SchemaApplicationLeaseRequest {
    pub fn new(
        tenant_id: Uuid,
        schema: &IndexSchema,
        worker_id: impl Into<String>,
        lease_duration: Duration,
    ) -> Result<Self, SchemaLeaseError> {
        if tenant_id.is_nil() {
            return Err(SchemaLeaseError::NilTenantId);
        }
        let worker_id = worker_id.into();
        validate_worker_id(&worker_id)?;
        let lease_seconds = validate_lease_duration(lease_duration)?;
        let schema_fingerprint = schema
            .fingerprint()
            .map_err(|error| SchemaLeaseError::InvalidSchema(error.to_string()))?;
        Ok(Self {
            tenant_id,
            schema: schema.reference.clone(),
            schema_fingerprint,
            worker_id,
            lease_seconds,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn schema_fingerprint(&self) -> SchemaFingerprint {
        self.schema_fingerprint
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn lease_duration(&self) -> Duration {
        Duration::from_secs(self.lease_seconds)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaApplicationLease {
    tenant_id: Uuid,
    job_id: Uuid,
    schema: SchemaRef,
    schema_fingerprint: SchemaFingerprint,
    worker_id: String,
    attempt_count: u32,
}

impl SchemaApplicationLease {
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn schema_fingerprint(&self) -> SchemaFingerprint {
        self.schema_fingerprint
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaLeaseAcquireOutcome {
    Acquired(SchemaApplicationLease),
    Busy,
    AlreadyApplied { job_id: Uuid },
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum SchemaLeaseError {
    #[error("schema application tenant id must not be nil")]
    NilTenantId,
    #[error("invalid schema application worker id: {reason}")]
    InvalidWorkerId { reason: &'static str },
    #[error(
        "schema application lease duration must be a whole number of seconds between 1 and 86400"
    )]
    InvalidLeaseDuration,
    #[error("invalid schema application error code: {reason}")]
    InvalidErrorCode { reason: &'static str },
    #[error("invalid schema application contract: {0}")]
    InvalidSchema(String),
    #[error("schema is not persisted for this tenant: {0}")]
    SchemaNotRegistered(SchemaRef),
    #[error("schema is retired and cannot be applied: {0}")]
    SchemaRetired(SchemaRef),
    #[error("persisted schema fingerprint does not match the requested schema")]
    SchemaFingerprintConflict,
    #[error("stored schema application job is invalid: {0}")]
    InvalidStoredJob(String),
    #[error("schema application lease ownership was lost")]
    LeaseLost,
    #[error("schema application storage operation failed")]
    Storage(String),
}

#[derive(Clone)]
pub struct PostgresSchemaLeaseStore {
    db: DatabaseConnection,
}

impl PostgresSchemaLeaseStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn acquire(
        &self,
        request: &SchemaApplicationLeaseRequest,
    ) -> Result<SchemaLeaseAcquireOutcome, SchemaLeaseError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let result = self.acquire_in_transaction(&transaction, request).await;
        match result {
            Ok(outcome) => {
                transaction.commit().await.map_err(storage_error)?;
                Ok(outcome)
            }
            Err(error) => {
                transaction.rollback().await.map_err(storage_error)?;
                Err(error)
            }
        }
    }

    pub async fn heartbeat(
        &self,
        lease: &SchemaApplicationLease,
        lease_duration: Duration,
    ) -> Result<(), SchemaLeaseError> {
        let lease_seconds = validate_lease_duration(lease_duration)?;
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let updated = self
            .db
            .execute(Statement::from_sql_and_values(
                backend,
                heartbeat_sql(backend),
                vec![
                    uuid_value(lease.tenant_id, backend),
                    uuid_value(lease.job_id, backend),
                    lease.worker_id.clone().into(),
                    i64::from(lease.attempt_count).into(),
                    i64::try_from(lease_seconds)
                        .map_err(|_| SchemaLeaseError::InvalidLeaseDuration)?
                        .into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(SchemaLeaseError::LeaseLost);
        }
        Ok(())
    }

    pub async fn succeed(&self, lease: &SchemaApplicationLease) -> Result<(), SchemaLeaseError> {
        self.finish(lease, "succeeded", None, None).await
    }

    pub async fn fail(
        &self,
        lease: &SchemaApplicationLease,
        error_code: impl Into<String>,
        error_details: JsonValue,
    ) -> Result<(), SchemaLeaseError> {
        let error_code = error_code.into();
        validate_error_code(&error_code)?;
        self.finish(lease, "failed", Some(error_code), Some(error_details))
            .await
    }

    async fn acquire_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        request: &SchemaApplicationLeaseRequest,
    ) -> Result<SchemaLeaseAcquireOutcome, SchemaLeaseError> {
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        self.lock_schema(transaction, request, backend).await?;
        self.verify_schema_registration(transaction, request, backend)
            .await?;

        let rows = transaction
            .query_all(Statement::from_sql_and_values(
                backend,
                select_schema_jobs_sql(backend),
                schema_scope_values(request, backend),
            ))
            .await
            .map_err(storage_error)?;

        let mut claimable = None;
        if let Some(row) = rows.into_iter().next() {
            let stored = stored_job(&row, backend)?;
            if stored.schema_fingerprint != request.schema_fingerprint.to_string() {
                return Err(SchemaLeaseError::InvalidStoredJob(
                    "job fingerprint does not match the persisted schema version".to_owned(),
                ));
            }
            match stored.state.as_str() {
                "succeeded" => {
                    return Ok(SchemaLeaseAcquireOutcome::AlreadyApplied {
                        job_id: stored.job_id,
                    });
                }
                "running" if !stored.claimable => {
                    return Ok(SchemaLeaseAcquireOutcome::Busy);
                }
                "pending" if !stored.claimable => {
                    return Ok(SchemaLeaseAcquireOutcome::Busy);
                }
                "pending" | "running" => {
                    claimable = Some(stored);
                }
                state => {
                    return Err(SchemaLeaseError::InvalidStoredJob(format!(
                        "unexpected active state {state}"
                    )));
                }
            }
        }

        let job_id;
        let attempt_count;
        if let Some(stored) = claimable {
            job_id = stored.job_id;
            attempt_count = stored.attempt_count.checked_add(1).ok_or_else(|| {
                SchemaLeaseError::InvalidStoredJob("attempt count overflow".to_owned())
            })?;
            let claimed = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    claim_job_sql(backend),
                    vec![
                        uuid_value(request.tenant_id, backend),
                        uuid_value(job_id, backend),
                        request.worker_id.clone().into(),
                        i64::from(attempt_count).into(),
                        i64::try_from(request.lease_seconds)
                            .map_err(|_| SchemaLeaseError::InvalidLeaseDuration)?
                            .into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if claimed.rows_affected() != 1 {
                return Err(SchemaLeaseError::LeaseLost);
            }
        } else {
            job_id = Uuid::new_v4();
            attempt_count = 1;
            let job_request = json!({
                "schema_fingerprint": request.schema_fingerprint.to_string(),
            });
            transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    insert_job_sql(backend),
                    vec![
                        uuid_value(request.tenant_id, backend),
                        uuid_value(job_id, backend),
                        request.schema.module.as_str().to_owned().into(),
                        request.schema.entity.as_str().to_owned().into(),
                        i64::from(request.schema.version.get()).into(),
                        SqlValue::Json(Some(Box::new(job_request))),
                        request.worker_id.clone().into(),
                        i64::try_from(request.lease_seconds)
                            .map_err(|_| SchemaLeaseError::InvalidLeaseDuration)?
                            .into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
        }

        Ok(SchemaLeaseAcquireOutcome::Acquired(
            SchemaApplicationLease {
                tenant_id: request.tenant_id,
                job_id,
                schema: request.schema.clone(),
                schema_fingerprint: request.schema_fingerprint,
                worker_id: request.worker_id.clone(),
                attempt_count,
            },
        ))
    }

    async fn lock_schema(
        &self,
        transaction: &DatabaseTransaction,
        request: &SchemaApplicationLeaseRequest,
        backend: DbBackend,
    ) -> Result<(), SchemaLeaseError> {
        if backend == DbBackend::Sqlite {
            return Ok(());
        }
        let lock_key = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            request.tenant_id,
            request.schema.module.as_str(),
            request.schema.entity.as_str(),
            request.schema.version.get(),
        );
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                vec![lock_key.into()],
            ))
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    async fn verify_schema_registration(
        &self,
        transaction: &DatabaseTransaction,
        request: &SchemaApplicationLeaseRequest,
        backend: DbBackend,
    ) -> Result<(), SchemaLeaseError> {
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                select_schema_sql(backend),
                schema_scope_values(request, backend),
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| SchemaLeaseError::SchemaNotRegistered(request.schema.clone()))?;
        let fingerprint: String = row
            .try_get("", "schema_fingerprint")
            .map_err(storage_error)?;
        let status: String = row.try_get("", "status").map_err(storage_error)?;
        if fingerprint != request.schema_fingerprint.to_string() {
            return Err(SchemaLeaseError::SchemaFingerprintConflict);
        }
        if status != "active" {
            return Err(SchemaLeaseError::SchemaRetired(request.schema.clone()));
        }
        Ok(())
    }

    async fn finish(
        &self,
        lease: &SchemaApplicationLease,
        state: &'static str,
        error_code: Option<String>,
        error_details: Option<JsonValue>,
    ) -> Result<(), SchemaLeaseError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let updated = self
            .db
            .execute(Statement::from_sql_and_values(
                backend,
                finish_job_sql(backend),
                vec![
                    state.into(),
                    error_code.into(),
                    SqlValue::Json(error_details.map(Box::new)),
                    uuid_value(lease.tenant_id, backend),
                    uuid_value(lease.job_id, backend),
                    lease.worker_id.clone().into(),
                    i64::from(lease.attempt_count).into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(SchemaLeaseError::LeaseLost);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct StoredJob {
    job_id: Uuid,
    state: String,
    schema_fingerprint: String,
    attempt_count: u32,
    claimable: bool,
}

fn stored_job(row: &QueryResult, backend: DbBackend) -> Result<StoredJob, SchemaLeaseError> {
    let request: JsonValue = row.try_get("", "request").map_err(storage_error)?;
    let schema_fingerprint = request
        .get("schema_fingerprint")
        .and_then(JsonValue::as_str)
        .filter(|value| value.len() == 64)
        .ok_or_else(|| {
            SchemaLeaseError::InvalidStoredJob(
                "request.schema_fingerprint must be a 64-character string".to_owned(),
            )
        })?
        .to_owned();
    let attempt_count: i64 = row
        .try_get("", "attempt_count_value")
        .map_err(storage_error)?;
    let attempt_count = u32::try_from(attempt_count).map_err(|_| {
        SchemaLeaseError::InvalidStoredJob("attempt count is outside the u32 range".to_owned())
    })?;
    let claimable: bool = row.try_get("", "claimable").map_err(storage_error)?;
    Ok(StoredJob {
        job_id: stored_uuid(row, "job_id", backend)?,
        state: row.try_get("", "state").map_err(storage_error)?,
        schema_fingerprint,
        attempt_count,
        claimable,
    })
}

fn validate_worker_id(worker_id: &str) -> Result<(), SchemaLeaseError> {
    if worker_id.is_empty() {
        return Err(SchemaLeaseError::InvalidWorkerId {
            reason: "must not be empty",
        });
    }
    if worker_id.trim() != worker_id {
        return Err(SchemaLeaseError::InvalidWorkerId {
            reason: "must not contain leading or trailing whitespace",
        });
    }
    if worker_id.len() > MAX_WORKER_ID_BYTES {
        return Err(SchemaLeaseError::InvalidWorkerId {
            reason: "exceeds the storage limit",
        });
    }
    if worker_id.chars().any(char::is_control) {
        return Err(SchemaLeaseError::InvalidWorkerId {
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

fn validate_error_code(error_code: &str) -> Result<(), SchemaLeaseError> {
    if error_code.is_empty() {
        return Err(SchemaLeaseError::InvalidErrorCode {
            reason: "must not be empty",
        });
    }
    if error_code.trim() != error_code {
        return Err(SchemaLeaseError::InvalidErrorCode {
            reason: "must not contain leading or trailing whitespace",
        });
    }
    if error_code.len() > MAX_ERROR_CODE_BYTES {
        return Err(SchemaLeaseError::InvalidErrorCode {
            reason: "exceeds the storage limit",
        });
    }
    if error_code.chars().any(char::is_control) {
        return Err(SchemaLeaseError::InvalidErrorCode {
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

fn validate_lease_duration(lease_duration: Duration) -> Result<u64, SchemaLeaseError> {
    if lease_duration.subsec_nanos() != 0 {
        return Err(SchemaLeaseError::InvalidLeaseDuration);
    }
    let seconds = lease_duration.as_secs();
    if seconds == 0 || seconds > MAX_LEASE_SECONDS {
        return Err(SchemaLeaseError::InvalidLeaseDuration);
    }
    Ok(seconds)
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), SchemaLeaseError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        backend => Err(SchemaLeaseError::Storage(format!(
            "Index schema leases do not support {backend:?}"
        ))),
    }
}

fn storage_error(error: impl std::fmt::Display) -> SchemaLeaseError {
    SchemaLeaseError::Storage(error.to_string())
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

fn stored_uuid(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, SchemaLeaseError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(storage_error)?;
            Uuid::parse_str(&value).map_err(storage_error)
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn schema_scope_values(
    request: &SchemaApplicationLeaseRequest,
    backend: DbBackend,
) -> Vec<SqlValue> {
    vec![
        uuid_value(request.tenant_id, backend),
        request.schema.module.as_str().to_owned().into(),
        request.schema.entity.as_str().to_owned().into(),
        i64::from(request.schema.version.get()).into(),
    ]
}

fn select_schema_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "SELECT schema_fingerprint, status FROM index_schemas WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 LIMIT 1"
    )
}

fn select_schema_jobs_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let (attempt_count, claimable) = match backend {
        DbBackend::Postgres => (
            "CAST(attempt_count AS BIGINT)",
            "((state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP))",
        ),
        DbBackend::Sqlite => (
            "CAST(attempt_count AS INTEGER)",
            "CASE WHEN (state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP) THEN TRUE ELSE FALSE END",
        ),
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "SELECT job_id, state, request, {attempt_count} AS attempt_count_value, {claimable} AS claimable FROM index_jobs WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 AND kind = 'schema_apply' AND scope_kind = 'schema' AND state IN ('pending', 'running', 'succeeded') ORDER BY CASE state WHEN 'succeeded' THEN 0 WHEN 'running' THEN 1 ELSE 2 END, created_at DESC"
    )
}

fn insert_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 8);
    format!(
        "INSERT INTO index_jobs (tenant_id, job_id, kind, state, scope_kind, module_name, entity_name, schema_version, request, attempt_count, available_at, lease_owner, lease_expires_at, heartbeat_at) VALUES ({prefix}1, {prefix}2, 'schema_apply', 'running', 'schema', {prefix}3, {prefix}4, {prefix}5, {prefix}6, 1, CURRENT_TIMESTAMP, {prefix}7, {lease_expires}, CURRENT_TIMESTAMP)"
    )
}

fn claim_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 5);
    format!(
        "UPDATE index_jobs SET state = 'running', lease_owner = {prefix}3, attempt_count = {prefix}4, lease_expires_at = {lease_expires}, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, completed_at = NULL, last_error_code = NULL, last_error_details = NULL WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'schema_apply' AND ((state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP))"
    )
}

fn heartbeat_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 5);
    format!(
        "UPDATE index_jobs SET lease_expires_at = {lease_expires}, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'schema_apply' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP"
    )
}

fn finish_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = {prefix}1, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, last_error_code = {prefix}2, last_error_details = {prefix}3, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}4 AND job_id = {prefix}5 AND kind = 'schema_apply' AND state = 'running' AND lease_owner = {prefix}6 AND attempt_count = {prefix}7 AND lease_expires_at > CURRENT_TIMESTAMP"
    )
}

fn lease_expires_expression(backend: DbBackend, parameter: usize) -> String {
    let prefix = placeholder_prefix(backend);
    match backend {
        DbBackend::Postgres => {
            format!("CURRENT_TIMESTAMP + ({prefix}{parameter} * INTERVAL '1 second')")
        }
        DbBackend::Sqlite => {
            format!("datetime('now', '+' || {prefix}{parameter} || ' seconds')")
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}
