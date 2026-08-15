use std::time::Duration;

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use uuid::Uuid;

use crate::{IndexSchema, LocaleKey, LocaleMode, SchemaRef};

const REPLAY_JOB_REQUEST_CONTRACT_V1: &str = "index_replay_job_v1";
const REPLAY_JOB_REQUEST_CONTRACT_V2: &str = "index_replay_job_v2";
const MAX_SOURCE_NAME_BYTES: usize = 128;
const MAX_WORKER_ID_BYTES: usize = 191;
const MAX_ERROR_CODE_BYTES: usize = 128;
const MAX_LEASE_SECONDS: u64 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayJobLeaseRequest {
    tenant_id: Uuid,
    schema: SchemaRef,
    locale: Option<LocaleKey>,
    source_name: String,
    worker_id: String,
    lease_seconds: u64,
}

impl IndexReplayJobLeaseRequest {
    pub fn new(
        tenant_id: Uuid,
        schema: SchemaRef,
        source_name: impl Into<String>,
        worker_id: impl Into<String>,
        lease_duration: Duration,
    ) -> Result<Self, IndexReplayJobError> {
        Self::new_scoped(
            tenant_id,
            schema,
            None,
            source_name,
            worker_id,
            lease_duration,
        )
    }

    pub(crate) fn for_locale(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: LocaleKey,
        source_name: impl Into<String>,
        worker_id: impl Into<String>,
        lease_duration: Duration,
    ) -> Result<Self, IndexReplayJobError> {
        Self::new_scoped(
            tenant_id,
            schema,
            Some(locale),
            source_name,
            worker_id,
            lease_duration,
        )
    }

    fn new_scoped(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: Option<LocaleKey>,
        source_name: impl Into<String>,
        worker_id: impl Into<String>,
        lease_duration: Duration,
    ) -> Result<Self, IndexReplayJobError> {
        if tenant_id.is_nil() {
            return Err(IndexReplayJobError::NilTenantId);
        }
        let source_name = source_name.into();
        validate_source_name(&source_name)?;
        let worker_id = worker_id.into();
        validate_worker_id(&worker_id)?;
        let lease_seconds = validate_lease_duration(lease_duration)?;
        Ok(Self {
            tenant_id,
            schema,
            locale,
            source_name,
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

    pub fn locale(&self) -> Option<&LocaleKey> {
        self.locale.as_ref()
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn lease_duration(&self) -> Duration {
        Duration::from_secs(self.lease_seconds)
    }

    fn scope_kind(&self) -> &'static str {
        if self.locale.is_some() {
            "locale"
        } else {
            "schema"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayJobLease {
    tenant_id: Uuid,
    job_id: Uuid,
    schema: SchemaRef,
    locale: Option<LocaleKey>,
    source_name: String,
    worker_id: String,
    attempt_count: u32,
}

impl IndexReplayJobLease {
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub(crate) fn locale(&self) -> Option<&LocaleKey> {
        self.locale.as_ref()
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexReplayJobAcquireOutcome {
    Acquired(IndexReplayJobLease),
    Busy,
    AlreadyComplete { job_id: Uuid },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexReplayJobError {
    #[error("Index replay job tenant id must not be nil")]
    NilTenantId,
    #[error("invalid Index replay source name: {reason}")]
    InvalidSourceName { reason: &'static str },
    #[error("invalid Index replay worker id: {reason}")]
    InvalidWorkerId { reason: &'static str },
    #[error("Index replay lease duration must be a whole number of seconds between 1 and 86400")]
    InvalidLeaseDuration,
    #[error("invalid Index replay error code: {reason}")]
    InvalidErrorCode { reason: &'static str },
    #[error("Index replay schema is not persisted for this tenant: {0}")]
    SchemaNotRegistered(SchemaRef),
    #[error("Index replay schema is retired: {0}")]
    SchemaRetired(SchemaRef),
    #[error("Index replay schema does not support locale-scoped jobs: {0}")]
    LocaleScopeUnsupported(SchemaRef),
    #[error("Index replay scope is blocked by failed job {job_id} after attempt {attempt_count}")]
    DeadLettered {
        job_id: Uuid,
        attempt_count: u32,
        error_code: Option<String>,
    },
    #[error("stored Index replay job is invalid: {0}")]
    InvalidStoredJob(String),
    #[error("Index replay completion checkpoint is missing")]
    CheckpointMissing,
    #[error("Index replay completion checkpoint still has a continuation cursor")]
    CheckpointIncomplete,
    #[error("Index replay job lease ownership was lost")]
    LeaseLost,
    #[error("Index replay job storage operation failed")]
    Storage(String),
}

#[derive(Clone)]
pub struct PostgresIndexReplayJobStore {
    db: DatabaseConnection,
}

impl PostgresIndexReplayJobStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn acquire(
        &self,
        request: &IndexReplayJobLeaseRequest,
    ) -> Result<IndexReplayJobAcquireOutcome, IndexReplayJobError> {
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
        lease: &IndexReplayJobLease,
        lease_duration: Duration,
    ) -> Result<(), IndexReplayJobError> {
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
                        .map_err(|_| IndexReplayJobError::InvalidLeaseDuration)?
                        .into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(IndexReplayJobError::LeaseLost);
        }
        Ok(())
    }

    pub async fn succeed(&self, lease: &IndexReplayJobLease) -> Result<(), IndexReplayJobError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let result = async {
            let backend = transaction.get_database_backend();
            ensure_supported_backend(backend)?;
            assert_active_replay_job_lease(&transaction, lease, backend).await?;
            require_complete_checkpoint(&transaction, lease, backend).await?;
            finish_job(&transaction, lease, "succeeded", None, None, backend).await
        }
        .await;
        match result {
            Ok(()) => {
                transaction.commit().await.map_err(storage_error)?;
                Ok(())
            }
            Err(error) => {
                transaction.rollback().await.map_err(storage_error)?;
                Err(error)
            }
        }
    }

    pub async fn fail(
        &self,
        lease: &IndexReplayJobLease,
        error_code: impl Into<String>,
        error_details: JsonValue,
    ) -> Result<(), IndexReplayJobError> {
        let error_code = error_code.into();
        validate_error_code(&error_code)?;
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let updated = self
            .db
            .execute(Statement::from_sql_and_values(
                backend,
                finish_job_sql(backend),
                vec![
                    "failed".into(),
                    Some(error_code).into(),
                    SqlValue::Json(Some(Box::new(error_details))),
                    uuid_value(lease.tenant_id, backend),
                    uuid_value(lease.job_id, backend),
                    lease.worker_id.clone().into(),
                    i64::from(lease.attempt_count).into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(IndexReplayJobError::LeaseLost);
        }
        Ok(())
    }

    async fn acquire_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        request: &IndexReplayJobLeaseRequest,
    ) -> Result<IndexReplayJobAcquireOutcome, IndexReplayJobError> {
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        lock_replay_scope(transaction, request, backend).await?;
        verify_schema_registration(transaction, request, backend).await?;

        let rows = transaction
            .query_all(Statement::from_sql_and_values(
                backend,
                select_replay_jobs_sql(backend),
                replay_scope_values(request, backend),
            ))
            .await
            .map_err(storage_error)?;

        let mut claimable = None;
        if let Some(row) = rows.into_iter().next() {
            let stored = stored_job(&row, backend)?;
            if stored.source_name != request.source_name {
                return Err(IndexReplayJobError::InvalidStoredJob(
                    "request.source_name does not match the replay scope owner".to_owned(),
                ));
            }
            if stored.locale != request.locale {
                return Err(IndexReplayJobError::InvalidStoredJob(
                    "stored replay locale does not match the durable scope".to_owned(),
                ));
            }
            match stored.state.as_str() {
                "succeeded" => {
                    return Ok(IndexReplayJobAcquireOutcome::AlreadyComplete {
                        job_id: stored.job_id,
                    });
                }
                "running" if !stored.claimable => {
                    return Ok(IndexReplayJobAcquireOutcome::Busy);
                }
                "pending" if !stored.claimable => {
                    return Ok(IndexReplayJobAcquireOutcome::Busy);
                }
                "pending" | "running" => {
                    claimable = Some(stored);
                }
                "failed" => {
                    return Err(IndexReplayJobError::DeadLettered {
                        job_id: stored.job_id,
                        attempt_count: stored.attempt_count,
                        error_code: stored.last_error_code,
                    });
                }
                state => {
                    return Err(IndexReplayJobError::InvalidStoredJob(format!(
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
                IndexReplayJobError::InvalidStoredJob("attempt count overflow".to_owned())
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
                            .map_err(|_| IndexReplayJobError::InvalidLeaseDuration)?
                            .into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if claimed.rows_affected() != 1 {
                return Err(IndexReplayJobError::LeaseLost);
            }
        } else {
            job_id = Uuid::new_v4();
            attempt_count = 1;
            let job_request = replay_job_request(request);
            transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    insert_job_sql(backend),
                    vec![
                        uuid_value(request.tenant_id, backend),
                        uuid_value(job_id, backend),
                        request.scope_kind().to_owned().into(),
                        request.schema.module.as_str().to_owned().into(),
                        request.schema.entity.as_str().to_owned().into(),
                        i64::from(request.schema.version.get()).into(),
                        request
                            .locale
                            .as_ref()
                            .map(|locale| locale.as_str().to_owned())
                            .into(),
                        SqlValue::Json(Some(Box::new(job_request))),
                        request.worker_id.clone().into(),
                        i64::try_from(request.lease_seconds)
                            .map_err(|_| IndexReplayJobError::InvalidLeaseDuration)?
                            .into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
        }

        Ok(IndexReplayJobAcquireOutcome::Acquired(
            IndexReplayJobLease {
                tenant_id: request.tenant_id,
                job_id,
                schema: request.schema.clone(),
                locale: request.locale.clone(),
                source_name: request.source_name.clone(),
                worker_id: request.worker_id.clone(),
                attempt_count,
            },
        ))
    }
}

#[derive(Debug)]
struct StoredJob {
    job_id: Uuid,
    state: String,
    source_name: String,
    locale: Option<LocaleKey>,
    attempt_count: u32,
    claimable: bool,
    last_error_code: Option<String>,
}

fn replay_job_request(request: &IndexReplayJobLeaseRequest) -> JsonValue {
    match request.locale.as_ref() {
        Some(locale) => json!({
            "contract": REPLAY_JOB_REQUEST_CONTRACT_V2,
            "source_name": request.source_name.clone(),
            "locale": locale.as_str(),
        }),
        None => json!({
            "contract": REPLAY_JOB_REQUEST_CONTRACT_V1,
            "source_name": request.source_name.clone(),
        }),
    }
}

fn stored_job(row: &QueryResult, backend: DbBackend) -> Result<StoredJob, IndexReplayJobError> {
    let scope_kind: String = row.try_get("", "scope_kind").map_err(storage_error)?;
    let locale_key: Option<String> = row.try_get("", "locale_key").map_err(storage_error)?;
    let locale = match (scope_kind.as_str(), locale_key) {
        ("schema", None) => None,
        ("locale", Some(raw)) => {
            let locale = LocaleKey::new(&raw).map_err(|_| {
                IndexReplayJobError::InvalidStoredJob(
                    "locale_key is outside the canonical locale contract".to_owned(),
                )
            })?;
            if locale.as_str() != raw {
                return Err(IndexReplayJobError::InvalidStoredJob(
                    "locale_key is not canonical".to_owned(),
                ));
            }
            Some(locale)
        }
        _ => {
            return Err(IndexReplayJobError::InvalidStoredJob(
                "scope_kind and locale_key do not form a replay schema/locale scope".to_owned(),
            ));
        }
    };

    let request: JsonValue = row.try_get("", "request").map_err(storage_error)?;
    let object = request.as_object().ok_or_else(|| {
        IndexReplayJobError::InvalidStoredJob("request must be a JSON object".to_owned())
    })?;
    let contract = object
        .get("contract")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            IndexReplayJobError::InvalidStoredJob("request.contract must be a string".to_owned())
        })?;
    let source_name = object
        .get("source_name")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            IndexReplayJobError::InvalidStoredJob("request.source_name must be a string".to_owned())
        })?
        .to_owned();
    validate_source_name(&source_name).map_err(|_| {
        IndexReplayJobError::InvalidStoredJob(
            "request.source_name is outside the replay source contract".to_owned(),
        )
    })?;

    match locale.as_ref() {
        None => {
            if object.len() != 2 || contract != REPLAY_JOB_REQUEST_CONTRACT_V1 {
                return Err(IndexReplayJobError::InvalidStoredJob(
                    "schema replay request must use the exact index_replay_job_v1 contract"
                        .to_owned(),
                ));
            }
        }
        Some(locale) => {
            if object.len() != 3 || contract != REPLAY_JOB_REQUEST_CONTRACT_V2 {
                return Err(IndexReplayJobError::InvalidStoredJob(
                    "locale replay request must use the exact index_replay_job_v2 contract"
                        .to_owned(),
                ));
            }
            if object.get("locale").and_then(JsonValue::as_str) != Some(locale.as_str()) {
                return Err(IndexReplayJobError::InvalidStoredJob(
                    "request.locale does not match locale_key".to_owned(),
                ));
            }
        }
    }

    let attempt_count: i64 = row
        .try_get("", "attempt_count_value")
        .map_err(storage_error)?;
    let attempt_count = u32::try_from(attempt_count).map_err(|_| {
        IndexReplayJobError::InvalidStoredJob("attempt count is outside the u32 range".to_owned())
    })?;
    let last_error_code: Option<String> =
        row.try_get("", "last_error_code").map_err(storage_error)?;
    if let Some(code) = &last_error_code {
        validate_error_code(code).map_err(|_| {
            IndexReplayJobError::InvalidStoredJob(
                "last_error_code is outside the replay error contract".to_owned(),
            )
        })?;
    }
    Ok(StoredJob {
        job_id: stored_uuid(row, "job_id", backend)?,
        state: row.try_get("", "state").map_err(storage_error)?,
        source_name,
        locale,
        attempt_count,
        claimable: row.try_get("", "claimable").map_err(storage_error)?,
        last_error_code,
    })
}

pub(super) async fn assert_active_replay_job_lease(
    transaction: &DatabaseTransaction,
    lease: &IndexReplayJobLease,
    backend: DbBackend,
) -> Result<(), IndexReplayJobError> {
    ensure_supported_backend(backend)?;
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            active_lease_sql(backend),
            vec![
                uuid_value(lease.tenant_id, backend),
                uuid_value(lease.job_id, backend),
                lease.worker_id.clone().into(),
                i64::from(lease.attempt_count).into(),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else {
        return Err(IndexReplayJobError::LeaseLost);
    };
    let active: bool = row.try_get("", "active").map_err(storage_error)?;
    if !active {
        return Err(IndexReplayJobError::LeaseLost);
    }
    Ok(())
}

async fn require_complete_checkpoint(
    transaction: &DatabaseTransaction,
    lease: &IndexReplayJobLease,
    backend: DbBackend,
) -> Result<(), IndexReplayJobError> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            complete_checkpoint_sql(backend),
            vec![
                uuid_value(lease.tenant_id, backend),
                lease.source_name.clone().into(),
                lease.schema.module.as_str().to_owned().into(),
                lease.schema.entity.as_str().to_owned().into(),
                i64::from(lease.schema.version.get()).into(),
                lease
                    .locale
                    .as_ref()
                    .map(|locale| locale.as_str().to_owned())
                    .unwrap_or_default()
                    .into(),
            ],
        ))
        .await
        .map_err(storage_error)?
        .ok_or(IndexReplayJobError::CheckpointMissing)?;
    let cursor: JsonValue = row.try_get("", "cursor").map_err(storage_error)?;
    if !cursor.is_null() {
        return Err(IndexReplayJobError::CheckpointIncomplete);
    }
    Ok(())
}

async fn finish_job(
    transaction: &DatabaseTransaction,
    lease: &IndexReplayJobLease,
    state: &'static str,
    error_code: Option<String>,
    error_details: Option<JsonValue>,
    backend: DbBackend,
) -> Result<(), IndexReplayJobError> {
    let updated = transaction
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
        return Err(IndexReplayJobError::LeaseLost);
    }
    Ok(())
}

async fn lock_replay_scope(
    transaction: &DatabaseTransaction,
    request: &IndexReplayJobLeaseRequest,
    backend: DbBackend,
) -> Result<(), IndexReplayJobError> {
    if backend == DbBackend::Sqlite {
        return Ok(());
    }
    let locale = request.locale.as_ref().map(LocaleKey::as_str).unwrap_or("");
    let lock_key = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        request.tenant_id,
        request.schema.module.as_str(),
        request.schema.entity.as_str(),
        request.schema.version.get(),
        request.scope_kind(),
        locale,
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
    transaction: &DatabaseTransaction,
    request: &IndexReplayJobLeaseRequest,
    backend: DbBackend,
) -> Result<(), IndexReplayJobError> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            select_schema_sql(backend),
            schema_scope_values(request, backend),
        ))
        .await
        .map_err(storage_error)?
        .ok_or_else(|| IndexReplayJobError::SchemaNotRegistered(request.schema.clone()))?;
    let status: String = row.try_get("", "status").map_err(storage_error)?;
    if status != "active" {
        return Err(IndexReplayJobError::SchemaRetired(request.schema.clone()));
    }
    if request.locale.is_some() {
        let schema_json: JsonValue = row.try_get("", "schema_json").map_err(storage_error)?;
        let schema: IndexSchema = serde_json::from_value(schema_json).map_err(storage_error)?;
        if schema.reference != request.schema {
            return Err(IndexReplayJobError::Storage(
                "persisted Index schema identity does not match replay scope".to_owned(),
            ));
        }
        if schema.locale_mode == LocaleMode::None {
            return Err(IndexReplayJobError::LocaleScopeUnsupported(
                request.schema.clone(),
            ));
        }
    }
    Ok(())
}

fn validate_source_name(source_name: &str) -> Result<(), IndexReplayJobError> {
    if source_name.is_empty() {
        return Err(IndexReplayJobError::InvalidSourceName {
            reason: "must not be empty",
        });
    }
    if source_name.len() > MAX_SOURCE_NAME_BYTES {
        return Err(IndexReplayJobError::InvalidSourceName {
            reason: "exceeds the storage limit",
        });
    }
    if !source_name.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
    }) {
        return Err(IndexReplayJobError::InvalidSourceName {
            reason: "must be a lowercase machine name",
        });
    }
    Ok(())
}

fn validate_worker_id(worker_id: &str) -> Result<(), IndexReplayJobError> {
    validate_storage_text(worker_id, MAX_WORKER_ID_BYTES)
        .map_err(|reason| IndexReplayJobError::InvalidWorkerId { reason })
}

fn validate_error_code(error_code: &str) -> Result<(), IndexReplayJobError> {
    validate_storage_text(error_code, MAX_ERROR_CODE_BYTES)
        .map_err(|reason| IndexReplayJobError::InvalidErrorCode { reason })
}

fn validate_storage_text(value: &str, max_bytes: usize) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("must not be empty");
    }
    if value.trim() != value {
        return Err("must not contain leading or trailing whitespace");
    }
    if value.len() > max_bytes {
        return Err("exceeds the storage limit");
    }
    if value.chars().any(char::is_control) {
        return Err("must not contain control characters");
    }
    Ok(())
}

fn validate_lease_duration(lease_duration: Duration) -> Result<u64, IndexReplayJobError> {
    if lease_duration.subsec_nanos() != 0 {
        return Err(IndexReplayJobError::InvalidLeaseDuration);
    }
    let seconds = lease_duration.as_secs();
    if seconds == 0 || seconds > MAX_LEASE_SECONDS {
        return Err(IndexReplayJobError::InvalidLeaseDuration);
    }
    Ok(seconds)
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexReplayJobError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        backend => Err(IndexReplayJobError::Storage(format!(
            "Index replay jobs do not support {backend:?}"
        ))),
    }
}

fn storage_error(error: impl std::fmt::Display) -> IndexReplayJobError {
    IndexReplayJobError::Storage(error.to_string())
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
) -> Result<Uuid, IndexReplayJobError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(storage_error)?;
            Uuid::parse_str(&value).map_err(storage_error)
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn schema_scope_values(request: &IndexReplayJobLeaseRequest, backend: DbBackend) -> Vec<SqlValue> {
    vec![
        uuid_value(request.tenant_id, backend),
        request.schema.module.as_str().to_owned().into(),
        request.schema.entity.as_str().to_owned().into(),
        i64::from(request.schema.version.get()).into(),
    ]
}

fn replay_scope_values(request: &IndexReplayJobLeaseRequest, backend: DbBackend) -> Vec<SqlValue> {
    let mut values = schema_scope_values(request, backend);
    values.push(request.scope_kind().to_owned().into());
    values.push(
        request
            .locale
            .as_ref()
            .map(|locale| locale.as_str().to_owned())
            .into(),
    );
    values
}

fn select_schema_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "SELECT status, schema_json FROM index_schemas WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 LIMIT 1"
    )
}

fn select_replay_jobs_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let (attempt_count, claimable, locale_match) = match backend {
        DbBackend::Postgres => (
            "CAST(attempt_count AS BIGINT)",
            "((state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP))",
            format!("locale_key IS NOT DISTINCT FROM {prefix}6"),
        ),
        DbBackend::Sqlite => (
            "CAST(attempt_count AS INTEGER)",
            "CASE WHEN (state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP) THEN TRUE ELSE FALSE END",
            format!("locale_key IS {prefix}6"),
        ),
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "SELECT job_id, state, scope_kind, locale_key, request, last_error_code, {attempt_count} AS attempt_count_value, {claimable} AS claimable FROM index_jobs WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 AND kind = 'rebuild' AND scope_kind = {prefix}5 AND {locale_match} AND state IN ('pending', 'running', 'succeeded', 'failed') ORDER BY CASE state WHEN 'succeeded' THEN 0 WHEN 'running' THEN 1 WHEN 'pending' THEN 2 ELSE 3 END, created_at DESC"
    )
}

fn insert_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 10);
    format!(
        "INSERT INTO index_jobs (tenant_id, job_id, kind, state, scope_kind, module_name, entity_name, schema_version, locale_key, request, attempt_count, available_at, lease_owner, lease_expires_at, heartbeat_at) VALUES ({prefix}1, {prefix}2, 'rebuild', 'running', {prefix}3, {prefix}4, {prefix}5, {prefix}6, {prefix}7, {prefix}8, 1, CURRENT_TIMESTAMP, {prefix}9, {lease_expires}, CURRENT_TIMESTAMP)"
    )
}

fn claim_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 5);
    format!(
        "UPDATE index_jobs SET state = 'running', lease_owner = {prefix}3, attempt_count = {prefix}4, lease_expires_at = {lease_expires}, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, completed_at = NULL, last_error_code = NULL, last_error_details = NULL WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND ((state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP))"
    )
}

fn heartbeat_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 5);
    format!(
        "UPDATE index_jobs SET lease_expires_at = {lease_expires}, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP"
    )
}

fn finish_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = {prefix}1, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, last_error_code = {prefix}2, last_error_details = {prefix}3, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}4 AND job_id = {prefix}5 AND kind = 'rebuild' AND state = 'running' AND lease_owner = {prefix}6 AND attempt_count = {prefix}7 AND lease_expires_at > CURRENT_TIMESTAMP"
    )
}

fn active_lease_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lock = match backend {
        DbBackend::Postgres => " FOR UPDATE",
        DbBackend::Sqlite => "",
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "SELECT CASE WHEN lease_expires_at > CURRENT_TIMESTAMP THEN TRUE ELSE FALSE END AS active FROM index_jobs WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 LIMIT 1{lock}"
    )
}

fn complete_checkpoint_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lock = match backend {
        DbBackend::Postgres => " FOR UPDATE",
        DbBackend::Sqlite => "",
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "SELECT cursor FROM index_checkpoints WHERE tenant_id = {prefix}1 AND checkpoint_kind = 'rebuild' AND source_name = {prefix}2 AND module_name = {prefix}3 AND entity_name = {prefix}4 AND schema_version = {prefix}5 AND locale_key = {prefix}6 AND partition_key = '' LIMIT 1{lock}"
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
