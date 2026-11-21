use std::time::Duration;

use sea_orm::sqlx::{self, PgConnection};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::application::postgres_value_sql::{
    list_expression, scalar_expression, schema_scope_sql,
};
use crate::{
    FieldCardinality, FieldName, IndexSchema, IndexValueType, SchemaFingerprint, SchemaRef,
};

use super::source_job_lease::{
    deadline_after_seconds, due_job_where, job_is_due, live_lease_where, placeholder_prefix,
};

const MAX_WORKER_ID_BYTES: usize = 191;
const MAX_ERROR_CODE_BYTES: usize = 128;
const MAX_LEASE_SECONDS: u64 = 86_400;
const OWNED_INDEX_COMMENT_PREFIX: &str = "rustok-index:";
const STORAGE_VALUE_CONTRACT: &str = "tagged_index_expression_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryIndexKind {
    Scalar,
    JsonContainment,
}

impl SecondaryIndexKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
            Self::JsonContainment => "json_containment",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryIndexSpec {
    tenant_id: Uuid,
    schema: SchemaRef,
    schema_fingerprint: SchemaFingerprint,
    field_name: FieldName,
    value_type: IndexValueType,
    cardinality: FieldCardinality,
    kind: SecondaryIndexKind,
    index_name: String,
    definition_hash: String,
}

impl SecondaryIndexSpec {
    fn new(
        tenant_id: Uuid,
        schema: &IndexSchema,
        schema_fingerprint: SchemaFingerprint,
        field_name: FieldName,
        value_type: IndexValueType,
        cardinality: FieldCardinality,
        kind: SecondaryIndexKind,
    ) -> Self {
        let definition = format!(
            "rustok-index-secondary\u{1f}{STORAGE_VALUE_CONTRACT}\u{1f}{tenant_id}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            schema.reference.module.as_str(),
            schema.reference.entity.as_str(),
            schema.reference.version.get(),
            schema_fingerprint,
            field_name.as_str(),
            value_type_tag(value_type),
            cardinality_tag(cardinality),
            kind.as_str(),
        );
        let definition_hash = hex::encode(Sha256::digest(definition.as_bytes()));
        let index_name = format!("idx_index_entities_{}", &definition_hash[..24]);
        Self {
            tenant_id,
            schema: schema.reference.clone(),
            schema_fingerprint,
            field_name,
            value_type,
            cardinality,
            kind,
            index_name,
            definition_hash,
        }
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

    pub fn field_name(&self) -> &FieldName {
        &self.field_name
    }

    pub fn value_type(&self) -> IndexValueType {
        self.value_type
    }

    pub fn cardinality(&self) -> FieldCardinality {
        self.cardinality
    }

    pub fn kind(&self) -> SecondaryIndexKind {
        self.kind
    }

    pub fn index_name(&self) -> &str {
        &self.index_name
    }

    pub fn definition_hash(&self) -> &str {
        &self.definition_hash
    }

    fn owner_comment(&self) -> String {
        format!("{OWNED_INDEX_COMMENT_PREFIX}{}", self.definition_hash)
    }

    pub(crate) fn create_statement(
        &self,
        backend: DbBackend,
    ) -> Result<String, SecondaryIndexError> {
        let name = quote_identifier(&self.index_name);
        let predicate = self.predicate(backend);
        match (backend, self.kind) {
            (DbBackend::Postgres, SecondaryIndexKind::Scalar) => Ok(format!(
                "CREATE INDEX CONCURRENTLY {name} ON index_entities (locale_key, {}, entity_id) WHERE {predicate}",
                scalar_expression("payload", &self.field_name, self.value_type),
            )),
            (DbBackend::Postgres, SecondaryIndexKind::JsonContainment) => Ok(format!(
                "CREATE INDEX CONCURRENTLY {name} ON index_entities USING gin (({}) jsonb_path_ops) WHERE {predicate}",
                list_expression("payload", &self.field_name),
            )),
            (DbBackend::Sqlite, _) if cfg!(test) => {
                let path = quote_literal(&format!("$.{}.value", self.field_name.as_str()));
                Ok(format!(
                    "CREATE INDEX {name} ON index_entities (locale_key, json_extract(payload, {path}), entity_id) WHERE {predicate}",
                ))
            }
            (backend, _) => Err(SecondaryIndexError::UnsupportedBackend(format!(
                "{backend:?}"
            ))),
        }
    }

    fn reindex_statement(&self, backend: DbBackend) -> Result<String, SecondaryIndexError> {
        let name = quote_identifier(&self.index_name);
        match backend {
            DbBackend::Postgres => Ok(format!("REINDEX INDEX CONCURRENTLY {name}")),
            DbBackend::Sqlite if cfg!(test) => Ok(format!("REINDEX {name}")),
            backend => Err(SecondaryIndexError::UnsupportedBackend(format!(
                "{backend:?}"
            ))),
        }
    }

    fn drop_statement(&self, backend: DbBackend) -> Result<String, SecondaryIndexError> {
        let name = quote_identifier(&self.index_name);
        match backend {
            DbBackend::Postgres => Ok(format!("DROP INDEX CONCURRENTLY IF EXISTS {name}")),
            DbBackend::Sqlite if cfg!(test) => Ok(format!("DROP INDEX IF EXISTS {name}")),
            backend => Err(SecondaryIndexError::UnsupportedBackend(format!(
                "{backend:?}"
            ))),
        }
    }

    fn predicate(&self, backend: DbBackend) -> String {
        if backend == DbBackend::Postgres {
            return schema_scope_sql("", self.tenant_id, &self.schema, self.schema_fingerprint);
        }
        let tenant = quote_literal(&self.tenant_id.to_string());
        format!(
            "tenant_id = {tenant} AND module_name = {} AND entity_name = {} AND schema_version = {} AND schema_fingerprint = {} AND is_deleted = FALSE",
            quote_literal(self.schema.module.as_str()),
            quote_literal(self.schema.entity.as_str()),
            self.schema.version.get(),
            quote_literal(&self.schema_fingerprint.to_string()),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryIndexPlan {
    schema: SchemaRef,
    schema_fingerprint: SchemaFingerprint,
    indexes: Vec<SecondaryIndexSpec>,
}

impl SecondaryIndexPlan {
    pub fn from_schema(tenant_id: Uuid, schema: &IndexSchema) -> Result<Self, SecondaryIndexError> {
        if tenant_id.is_nil() {
            return Err(SecondaryIndexError::NilTenantId);
        }
        let schema_fingerprint = schema
            .fingerprint()
            .map_err(|error| SecondaryIndexError::InvalidSchema(error.to_string()))?;
        let mut indexes = schema
            .fields
            .iter()
            .filter(|field| field.filterable || field.sortable)
            // Decimal-list membership uses numeric equality, not JSONB string containment.
            .filter(|field| {
                !(field.cardinality == FieldCardinality::Many
                    && field.value_type == IndexValueType::Decimal)
            })
            .map(|field| {
                let kind = match field.cardinality {
                    FieldCardinality::One => SecondaryIndexKind::Scalar,
                    FieldCardinality::Many => SecondaryIndexKind::JsonContainment,
                };
                SecondaryIndexSpec::new(
                    tenant_id,
                    schema,
                    schema_fingerprint,
                    field.name.clone(),
                    field.value_type,
                    field.cardinality,
                    kind,
                )
            })
            .collect::<Vec<_>>();
        indexes.sort_by(|left, right| left.field_name.cmp(&right.field_name));
        Ok(Self {
            schema: schema.reference.clone(),
            schema_fingerprint,
            indexes,
        })
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn schema_fingerprint(&self) -> SchemaFingerprint {
        self.schema_fingerprint
    }

    pub fn indexes(&self) -> &[SecondaryIndexSpec] {
        &self.indexes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryIndexOperation {
    Ensure,
    Reindex,
    Retire,
}

impl SecondaryIndexOperation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ensure => "ensure",
            Self::Reindex => "reindex",
            Self::Retire => "retire",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryIndexRequest {
    spec: SecondaryIndexSpec,
    operation: SecondaryIndexOperation,
    worker_id: String,
    lease_seconds: u64,
}

impl SecondaryIndexRequest {
    pub fn new(
        spec: SecondaryIndexSpec,
        operation: SecondaryIndexOperation,
        worker_id: impl Into<String>,
        lease_duration: Duration,
    ) -> Result<Self, SecondaryIndexError> {
        let worker_id = worker_id.into();
        validate_worker_id(&worker_id)?;
        let lease_seconds = validate_lease_duration(lease_duration)?;
        Ok(Self {
            spec,
            operation,
            worker_id,
            lease_seconds,
        })
    }

    pub fn spec(&self) -> &SecondaryIndexSpec {
        &self.spec
    }

    pub fn operation(&self) -> SecondaryIndexOperation {
        self.operation
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn lease_duration(&self) -> Duration {
        Duration::from_secs(self.lease_seconds)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryIndexLease {
    tenant_id: Uuid,
    job_id: Uuid,
    spec: SecondaryIndexSpec,
    operation: SecondaryIndexOperation,
    worker_id: String,
    attempt_count: u32,
    lease_seconds: u64,
}

impl SecondaryIndexLease {
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub fn spec(&self) -> &SecondaryIndexSpec {
        &self.spec
    }

    pub fn operation(&self) -> SecondaryIndexOperation {
        self.operation
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    pub fn lease_duration(&self) -> Duration {
        Duration::from_secs(self.lease_seconds)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecondaryIndexClaimOutcome {
    Acquired(SecondaryIndexLease),
    Busy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecondaryIndexExecutionOutcome {
    Ready { index_name: String, created: bool },
    Reindexed { index_name: String },
    Retired { index_name: String, dropped: bool },
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum SecondaryIndexError {
    #[error("secondary-index tenant id must not be nil")]
    NilTenantId,
    #[error("invalid secondary-index schema: {0}")]
    InvalidSchema(String),
    #[error("invalid secondary-index worker id: {reason}")]
    InvalidWorkerId { reason: &'static str },
    #[error("secondary-index lease duration must be a whole number of seconds between 1 and 86400")]
    InvalidLeaseDuration,
    #[error("invalid secondary-index error code: {reason}")]
    InvalidErrorCode { reason: &'static str },
    #[error("secondary-index schema is not selected in this runtime: {0}")]
    SchemaNotSelected(SchemaRef),
    #[error("field does not support a schema-derived secondary index")]
    UnsupportedField,
    #[error("secondary-index schema is not persisted for this tenant: {0}")]
    SchemaNotRegistered(SchemaRef),
    #[error("secondary-index schema is retired: {0}")]
    SchemaRetired(SchemaRef),
    #[error("persisted schema fingerprint does not match the secondary-index plan")]
    SchemaFingerprintConflict,
    #[error("stored secondary-index job is invalid: {0}")]
    InvalidStoredJob(String),
    #[error("another Index worker holds the physical secondary-index DDL lock")]
    DdlBusy,
    #[error("secondary-index lease ownership was lost")]
    LeaseLost,
    #[error("secondary index {index_name} is owned by a different definition")]
    IndexOwnershipConflict { index_name: String },
    #[error("secondary index does not exist: {0}")]
    IndexMissing(String),
    #[error("secondary index is not ready and valid: {0}")]
    IndexNotReady(String),
    #[error("Index secondary indexes do not support database backend {0}")]
    UnsupportedBackend(String),
    #[error("secondary-index storage operation failed")]
    Storage(String),
}

#[derive(Clone)]
pub struct PostgresSecondaryIndexManager {
    db: DatabaseConnection,
}

impl PostgresSecondaryIndexManager {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn claim(
        &self,
        request: &SecondaryIndexRequest,
    ) -> Result<SecondaryIndexClaimOutcome, SecondaryIndexError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let result = self.claim_in_transaction(&transaction, request).await;
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

    pub async fn execute(
        &self,
        lease: &SecondaryIndexLease,
    ) -> Result<SecondaryIndexExecutionOutcome, SecondaryIndexError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        if backend == DbBackend::Postgres {
            return self.execute_postgres(lease).await;
        }
        self.assert_current(lease, backend).await?;
        self.execute_sqlite(lease, backend).await
    }

    pub(crate) async fn operate(
        &self,
        request: &SecondaryIndexRequest,
    ) -> Result<Option<SecondaryIndexExecutionOutcome>, SecondaryIndexError> {
        let SecondaryIndexClaimOutcome::Acquired(lease) = self.claim(request).await? else {
            return Ok(None);
        };
        let outcome = match self.execute(&lease).await {
            Ok(outcome) => outcome,
            Err(error) => {
                self.defer_interrupted_operation(&lease).await?;
                return Err(error);
            }
        };
        self.succeed(&lease).await?;
        Ok(Some(outcome))
    }

    async fn defer_interrupted_operation(
        &self,
        lease: &SecondaryIndexLease,
    ) -> Result<(), SecondaryIndexError> {
        let backend = self.db.get_database_backend();
        // This orchestrator is composed only for the PostgreSQL operator. An expired/lost
        // generation already requires normal reacquisition; never modify its replacement.
        let fence = live_lease_where(backend, "secondary_index", 1, false);
        let updated = self.db.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("UPDATE index_jobs SET state = 'pending', lease_owner = NULL, lease_expires_at = NULL,
                available_at = clock_timestamp() + INTERVAL '5 seconds', updated_at = clock_timestamp(),
                last_error_code = 'index.secondary_index_execution_failed', last_error_details = NULL
             WHERE {fence}"),
            vec![lease.tenant_id.into(), lease.job_id.into(), lease.worker_id.clone().into(), i64::from(lease.attempt_count).into()],
        )).await.map_err(storage_error)?;
        if updated.rows_affected() == 0 {
            tracing::debug!(job_id = %lease.job_id, "Secondary Index operation lost its lease before deferral");
        }
        Ok(())
    }

    pub async fn heartbeat(
        &self,
        lease: &SecondaryIndexLease,
        lease_duration: Duration,
    ) -> Result<(), SecondaryIndexError> {
        let lease_seconds = validate_lease_duration(lease_duration)?;
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let updated = self
            .db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                heartbeat_sql(backend),
                vec![
                    uuid_value(lease.tenant_id, backend),
                    uuid_value(lease.job_id, backend),
                    lease.worker_id.clone().into(),
                    i64::from(lease.attempt_count).into(),
                    i64::try_from(lease_seconds)
                        .map_err(|_| SecondaryIndexError::InvalidLeaseDuration)?
                        .into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(SecondaryIndexError::LeaseLost);
        }
        Ok(())
    }

    pub async fn succeed(&self, lease: &SecondaryIndexLease) -> Result<(), SecondaryIndexError> {
        self.finish(lease, "succeeded", None, None).await
    }

    pub async fn fail(
        &self,
        lease: &SecondaryIndexLease,
        error_code: impl Into<String>,
        error_details: JsonValue,
    ) -> Result<(), SecondaryIndexError> {
        let error_code = error_code.into();
        validate_error_code(&error_code)?;
        self.finish(lease, "failed", Some(error_code), Some(error_details))
            .await
    }

    async fn claim_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        request: &SecondaryIndexRequest,
    ) -> Result<SecondaryIndexClaimOutcome, SecondaryIndexError> {
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        if !self
            .lock_index(transaction, request.spec(), backend)
            .await?
        {
            return Ok(SecondaryIndexClaimOutcome::Busy);
        }
        self.verify_schema_registration(transaction, request.spec(), request.operation(), backend)
            .await?;

        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                select_active_jobs_sql(backend),
                schema_scope_values(request.spec(), backend),
            ))
            .await
            .map_err(storage_error)?;
        let mut reclaim = None;
        for row in rows {
            let stored = stored_job(&row, backend)?;
            if stored.index_name != request.spec.index_name {
                continue;
            }
            if !stored.claimable {
                return Ok(SecondaryIndexClaimOutcome::Busy);
            }
            if stored.operation == request.operation.as_str()
                && stored.definition_hash == request.spec.definition_hash
                && stored.schema_fingerprint == request.spec.schema_fingerprint.to_string()
            {
                reclaim = Some(stored);
                break;
            }
            // A different operation cannot supersede a possibly interrupted CREATE. Its
            // original Ensure must recover (or report conflict) before any other physical DDL.
            if stored.creation_reserved {
                return Ok(SecondaryIndexClaimOutcome::Busy);
            }
            self.supersede_expired_job(transaction, &stored, request, backend)
                .await?;
        }

        let job_id;
        let attempt_count;
        if let Some(stored) = reclaim {
            job_id = stored.job_id;
            attempt_count = stored
                .attempt_count
                .checked_add(1)
                .filter(|attempt| i32::try_from(*attempt).is_ok())
                .ok_or_else(|| {
                    SecondaryIndexError::InvalidStoredJob("attempt count overflow".to_owned())
                })?;
            let claimed = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    claim_job_sql(backend),
                    vec![
                        uuid_value(request.spec.tenant_id, backend),
                        uuid_value(job_id, backend),
                        request.worker_id.clone().into(),
                        i64::from(attempt_count).into(),
                        i64::try_from(request.lease_seconds)
                            .map_err(|_| SecondaryIndexError::InvalidLeaseDuration)?
                            .into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if claimed.rows_affected() != 1 {
                return Err(SecondaryIndexError::LeaseLost);
            }
        } else {
            job_id = Uuid::new_v4();
            attempt_count = 1;
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    insert_job_sql(backend),
                    vec![
                        uuid_value(request.spec.tenant_id, backend),
                        uuid_value(job_id, backend),
                        request.spec.schema.module.as_str().to_owned().into(),
                        request.spec.schema.entity.as_str().to_owned().into(),
                        i64::from(request.spec.schema.version.get()).into(),
                        SqlValue::Json(Some(Box::new(request_json(request)))),
                        request.worker_id.clone().into(),
                        i64::try_from(request.lease_seconds)
                            .map_err(|_| SecondaryIndexError::InvalidLeaseDuration)?
                            .into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
        }

        Ok(SecondaryIndexClaimOutcome::Acquired(SecondaryIndexLease {
            tenant_id: request.spec.tenant_id,
            job_id,
            spec: request.spec.clone(),
            operation: request.operation,
            worker_id: request.worker_id.clone(),
            attempt_count,
            lease_seconds: request.lease_seconds,
        }))
    }

    async fn lock_index(
        &self,
        transaction: &DatabaseTransaction,
        spec: &SecondaryIndexSpec,
        backend: DbBackend,
    ) -> Result<bool, SecondaryIndexError> {
        if backend == DbBackend::Sqlite {
            return Ok(true);
        }
        transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                "SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0)) AS locked",
                vec![ddl_lock_key(spec).into()],
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                SecondaryIndexError::Storage("missing secondary-index lock result".to_owned())
            })?
            .try_get("", "locked")
            .map_err(storage_error)
    }

    async fn verify_schema_registration(
        &self,
        transaction: &DatabaseTransaction,
        spec: &SecondaryIndexSpec,
        operation: SecondaryIndexOperation,
        backend: DbBackend,
    ) -> Result<(), SecondaryIndexError> {
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                select_schema_sql(backend),
                schema_scope_values(spec, backend),
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| SecondaryIndexError::SchemaNotRegistered(spec.schema.clone()))?;
        let fingerprint: String = row
            .try_get("", "schema_fingerprint")
            .map_err(storage_error)?;
        let status: String = row.try_get("", "status").map_err(storage_error)?;
        let schema_json: JsonValue = row.try_get("", "schema_json").map_err(storage_error)?;
        if !matches_schema_contract(spec, &fingerprint, schema_json) {
            return Err(SecondaryIndexError::SchemaFingerprintConflict);
        }
        if status != "active" && operation != SecondaryIndexOperation::Retire {
            return Err(SecondaryIndexError::SchemaRetired(spec.schema.clone()));
        }
        Ok(())
    }

    async fn supersede_expired_job(
        &self,
        transaction: &DatabaseTransaction,
        stored: &StoredJob,
        request: &SecondaryIndexRequest,
        backend: DbBackend,
    ) -> Result<(), SecondaryIndexError> {
        let details = json!({
            "replacement_action": request.operation.as_str(),
            "replacement_definition_hash": request.spec.definition_hash.as_str(),
        });
        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                supersede_job_sql(backend),
                vec![
                    uuid_value(request.spec.tenant_id, backend),
                    uuid_value(stored.job_id, backend),
                    SqlValue::Json(Some(Box::new(details))),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(SecondaryIndexError::LeaseLost);
        }
        Ok(())
    }

    async fn execute_sqlite(
        &self,
        lease: &SecondaryIndexLease,
        backend: DbBackend,
    ) -> Result<SecondaryIndexExecutionOutcome, SecondaryIndexError> {
        let spec = &lease.spec;
        let index_name = spec.index_name.clone();
        let exists = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                "SELECT name FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [index_name.clone().into()],
            ))
            .await
            .map_err(storage_error)?
            .is_some();
        match lease.operation {
            SecondaryIndexOperation::Ensure => {
                if !exists {
                    self.db
                        .execute_unprepared(&spec.create_statement(backend)?)
                        .await
                        .map_err(storage_error)?;
                }
                Ok(SecondaryIndexExecutionOutcome::Ready {
                    index_name,
                    created: !exists,
                })
            }
            SecondaryIndexOperation::Reindex => {
                if !exists {
                    return Err(SecondaryIndexError::IndexMissing(index_name));
                }
                self.db
                    .execute_unprepared(&spec.reindex_statement(backend)?)
                    .await
                    .map_err(storage_error)?;
                Ok(SecondaryIndexExecutionOutcome::Reindexed { index_name })
            }
            SecondaryIndexOperation::Retire => {
                if exists {
                    self.db
                        .execute_unprepared(&spec.drop_statement(backend)?)
                        .await
                        .map_err(storage_error)?;
                }
                Ok(SecondaryIndexExecutionOutcome::Retired {
                    index_name,
                    dropped: exists,
                })
            }
        }
    }

    async fn execute_postgres(
        &self,
        lease: &SecondaryIndexLease,
    ) -> Result<SecondaryIndexExecutionOutcome, SecondaryIndexError> {
        let mut connection = self
            .db
            .get_postgres_connection_pool()
            .acquire()
            .await
            .map_err(storage_error)?;
        // Concurrent DDL cannot run in a transaction. Pin a session lock spanning the lease
        // checks, catalogue inspection and DDL. Never return it to the pool on cancellation.
        connection.close_on_drop();
        let locked: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtextextended($1, 0))")
                .bind(ddl_lock_key(&lease.spec))
                .fetch_one(&mut *connection)
                .await
                .map_err(storage_error)?;
        if !locked {
            return Err(SecondaryIndexError::DdlBusy);
        }
        execute_postgres_locked(&mut connection, lease).await
    }

    async fn assert_current(
        &self,
        lease: &SecondaryIndexLease,
        backend: DbBackend,
    ) -> Result<(), SecondaryIndexError> {
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                assert_current_sql(backend),
                vec![
                    uuid_value(lease.tenant_id, backend),
                    uuid_value(lease.job_id, backend),
                    lease.worker_id.clone().into(),
                    i64::from(lease.attempt_count).into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if row.is_none() {
            return Err(SecondaryIndexError::LeaseLost);
        }
        Ok(())
    }

    async fn finish(
        &self,
        lease: &SecondaryIndexLease,
        state: &'static str,
        error_code: Option<String>,
        error_details: Option<JsonValue>,
    ) -> Result<(), SecondaryIndexError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let updated = self
            .db
            .execute_raw(Statement::from_sql_and_values(
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
            return Err(SecondaryIndexError::LeaseLost);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct StoredJob {
    job_id: Uuid,
    creation_reserved: bool,
    operation: String,
    index_name: String,
    definition_hash: String,
    schema_fingerprint: String,
    attempt_count: u32,
    claimable: bool,
}

fn stored_job(row: &QueryResult, backend: DbBackend) -> Result<StoredJob, SecondaryIndexError> {
    let request: JsonValue = row.try_get("", "request").map_err(storage_error)?;
    let string_field = |name: &str| {
        request
            .get(name)
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                SecondaryIndexError::InvalidStoredJob(format!(
                    "request.{name} must be a non-empty string"
                ))
            })
    };
    let attempt_count: i64 = row
        .try_get("", "attempt_count_value")
        .map_err(storage_error)?;
    Ok(StoredJob {
        creation_reserved: row
            .try_get("", "creation_reserved")
            .map_err(storage_error)?,
        job_id: stored_uuid(row, "job_id", backend)?,
        operation: string_field("operation")?,
        index_name: string_field("index_name")?,
        definition_hash: string_field("definition_hash")?,
        schema_fingerprint: string_field("schema_fingerprint")?,
        attempt_count: u32::try_from(attempt_count).map_err(|_| {
            SecondaryIndexError::InvalidStoredJob(
                "attempt count is outside the u32 range".to_owned(),
            )
        })?,
        claimable: row.try_get("", "claimable").map_err(storage_error)?,
    })
}

fn request_json(request: &SecondaryIndexRequest) -> JsonValue {
    json!({
        "operation": request.operation.as_str(),
        "index_name": request.spec.index_name.as_str(),
        "definition_hash": request.spec.definition_hash.as_str(),
        "schema_fingerprint": request.spec.schema_fingerprint.to_string(),
        "field_name": request.spec.field_name.as_str(),
        "index_kind": request.spec.kind.as_str(),
        "storage_value_contract": STORAGE_VALUE_CONTRACT,
    })
}

fn validate_worker_id(worker_id: &str) -> Result<(), SecondaryIndexError> {
    if worker_id.is_empty() {
        return Err(SecondaryIndexError::InvalidWorkerId {
            reason: "must not be empty",
        });
    }
    if worker_id.trim() != worker_id {
        return Err(SecondaryIndexError::InvalidWorkerId {
            reason: "must not contain leading or trailing whitespace",
        });
    }
    if worker_id.len() > MAX_WORKER_ID_BYTES {
        return Err(SecondaryIndexError::InvalidWorkerId {
            reason: "exceeds the storage limit",
        });
    }
    if worker_id.chars().any(char::is_control) {
        return Err(SecondaryIndexError::InvalidWorkerId {
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

fn validate_error_code(error_code: &str) -> Result<(), SecondaryIndexError> {
    if error_code.is_empty() {
        return Err(SecondaryIndexError::InvalidErrorCode {
            reason: "must not be empty",
        });
    }
    if error_code.trim() != error_code {
        return Err(SecondaryIndexError::InvalidErrorCode {
            reason: "must not contain leading or trailing whitespace",
        });
    }
    if error_code.len() > MAX_ERROR_CODE_BYTES {
        return Err(SecondaryIndexError::InvalidErrorCode {
            reason: "exceeds the storage limit",
        });
    }
    if error_code.chars().any(char::is_control) {
        return Err(SecondaryIndexError::InvalidErrorCode {
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

fn validate_lease_duration(lease_duration: Duration) -> Result<u64, SecondaryIndexError> {
    if lease_duration.subsec_nanos() != 0 {
        return Err(SecondaryIndexError::InvalidLeaseDuration);
    }
    let seconds = lease_duration.as_secs();
    if seconds == 0 || seconds > MAX_LEASE_SECONDS {
        return Err(SecondaryIndexError::InvalidLeaseDuration);
    }
    Ok(seconds)
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), SecondaryIndexError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        backend => Err(SecondaryIndexError::UnsupportedBackend(format!(
            "{backend:?}"
        ))),
    }
}

fn storage_error(error: impl std::fmt::Display) -> SecondaryIndexError {
    SecondaryIndexError::Storage(error.to_string())
}

fn value_type_tag(value_type: IndexValueType) -> &'static str {
    match value_type {
        IndexValueType::Boolean => "boolean",
        IndexValueType::Integer => "integer",
        IndexValueType::Decimal => "decimal",
        IndexValueType::String => "string",
        IndexValueType::Uuid => "uuid",
        IndexValueType::Timestamp => "timestamp",
    }
}

fn cardinality_tag(cardinality: FieldCardinality) -> &'static str {
    match cardinality {
        FieldCardinality::One => "one",
        FieldCardinality::Many => "many",
    }
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
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
) -> Result<Uuid, SecondaryIndexError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(storage_error)?;
            Uuid::parse_str(&value).map_err(storage_error)
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn schema_scope_values(spec: &SecondaryIndexSpec, backend: DbBackend) -> Vec<SqlValue> {
    vec![
        uuid_value(spec.tenant_id, backend),
        spec.schema.module.as_str().to_owned().into(),
        spec.schema.entity.as_str().to_owned().into(),
        i64::from(spec.schema.version.get()).into(),
    ]
}

fn select_schema_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "SELECT schema_fingerprint, status, schema_json FROM index_schemas WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 LIMIT 1"
    )
}

fn select_active_jobs_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let claimable = job_is_due(backend);
    format!(
        "SELECT job_id, request, CASE WHEN cursor IS NOT NULL THEN TRUE ELSE FALSE END AS creation_reserved, CAST(attempt_count AS BIGINT) AS attempt_count_value, CASE WHEN {claimable} THEN TRUE ELSE FALSE END AS claimable FROM index_jobs WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 AND kind = 'secondary_index' AND scope_kind = 'schema' AND state IN ('pending', 'running') ORDER BY created_at"
    )
}

fn insert_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = deadline_after_seconds(backend, 8);
    format!(
        "INSERT INTO index_jobs (tenant_id, job_id, kind, state, scope_kind, module_name, entity_name, schema_version, request, attempt_count, available_at, lease_owner, lease_expires_at, heartbeat_at) VALUES ({prefix}1, {prefix}2, 'secondary_index', 'running', 'schema', {prefix}3, {prefix}4, {prefix}5, {prefix}6, 1, CURRENT_TIMESTAMP, {prefix}7, {lease_expires}, CURRENT_TIMESTAMP)"
    )
}

fn claim_job_sql(backend: DbBackend) -> String {
    let due = due_job_where(backend, "secondary_index");
    let prefix = placeholder_prefix(backend);
    let lease_expires = deadline_after_seconds(backend, 5);
    format!(
        "UPDATE index_jobs SET state = 'running', lease_owner = {prefix}3, attempt_count = {prefix}4, lease_expires_at = {lease_expires}, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, completed_at = NULL, last_error_code = NULL, last_error_details = NULL WHERE {due}"
    )
}

fn supersede_job_sql(backend: DbBackend) -> String {
    let due = due_job_where(backend, "secondary_index");
    let prefix = placeholder_prefix(backend);
    // Keep the creation intent fence on the write even though the cooperating claim path
    // already inspects it while holding the physical-name advisory lock.
    format!(
        "UPDATE index_jobs SET state = 'failed', lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, last_error_code = 'secondary_index.superseded', last_error_details = {prefix}3, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE {due} AND cursor IS NULL"
    )
}

fn heartbeat_sql(backend: DbBackend) -> String {
    let lease = live_lease_where(backend, "secondary_index", 1, false);
    let lease_expires = deadline_after_seconds(backend, 5);
    format!(
        "UPDATE index_jobs SET lease_expires_at = {lease_expires}, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE {lease}"
    )
}

fn assert_current_sql(backend: DbBackend) -> String {
    let lease = live_lease_where(backend, "secondary_index", 1, false);
    format!("SELECT job_id FROM index_jobs WHERE {lease} LIMIT 1")
}

fn finish_job_sql(backend: DbBackend) -> String {
    let lease = live_lease_where(backend, "secondary_index", 4, false);
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = {prefix}1, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, last_error_code = {prefix}2, last_error_details = {prefix}3, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE {lease}"
    )
}

fn ddl_lock_key(spec: &SecondaryIndexSpec) -> String {
    format!("secondary-index\u{1f}{}", spec.index_name)
}

#[derive(Debug, PartialEq, Eq)]
struct IndexDefinition {
    method: String,
    columns: String,
    operator_classes: String,
    collations: String,
    options: String,
    key_count: i16,
    unique: bool,
    expressions: String,
    predicate: String,
}

struct PostgresIndexState {
    definition: IndexDefinition,
    comment: Option<String>,
    ready: bool,
}

async fn index_state(
    session: &mut PgConnection,
    table_oid: i64,
    index_name: &str,
) -> Result<Option<PostgresIndexState>, SecondaryIndexError> {
    use sqlx::Row;
    let row = sqlx::query(
        "SELECT am.amname::text AS method, i.indkey::text AS columns, i.indclass::text AS operator_classes,
            i.indcollation::text AS collations, i.indoption::text AS options, i.indnkeyatts AS key_count,
            i.indisunique AS unique_keys, COALESCE(pg_get_expr(i.indexprs, i.indrelid), '') AS expressions,
            COALESCE(pg_get_expr(i.indpred, i.indrelid), '') AS predicate,
            obj_description(c.oid, 'pg_class') AS comment, i.indisready AND i.indisvalid AND i.indislive AS ready
         FROM pg_catalog.pg_index AS i JOIN pg_catalog.pg_class AS c ON c.oid = i.indexrelid
         JOIN pg_catalog.pg_am AS am ON am.oid = c.relam
         WHERE i.indrelid = $1::bigint::oid AND c.relname = $2 AND c.relkind = 'i'")
        .bind(table_oid).bind(index_name).fetch_optional(session).await.map_err(storage_error)?;
    row.map(|row| {
        Ok(PostgresIndexState {
            definition: IndexDefinition {
                method: row.try_get("method").map_err(storage_error)?,
                columns: row.try_get("columns").map_err(storage_error)?,
                operator_classes: row.try_get("operator_classes").map_err(storage_error)?,
                collations: row.try_get("collations").map_err(storage_error)?,
                options: row.try_get("options").map_err(storage_error)?,
                key_count: row.try_get("key_count").map_err(storage_error)?,
                unique: row.try_get("unique_keys").map_err(storage_error)?,
                expressions: row.try_get("expressions").map_err(storage_error)?,
                predicate: row.try_get("predicate").map_err(storage_error)?,
            },
            comment: row.try_get("comment").map_err(storage_error)?,
            ready: row.try_get("ready").map_err(storage_error)?,
        })
    })
    .transpose()
}

async fn remaining_lease_budget(
    session: &mut PgConnection,
    lease: &SecondaryIndexLease,
) -> Result<(), SecondaryIndexError> {
    let remaining: Option<i64> = sqlx::query_scalar(
        "SELECT floor(EXTRACT(EPOCH FROM (lease_expires_at - clock_timestamp())) * 1000)::bigint
         FROM index_jobs WHERE tenant_id = $1 AND job_id = $2 AND lease_owner = $3
           AND attempt_count = $4 AND kind = 'secondary_index' AND state = 'running' AND cancel_requested = FALSE")
        .bind(lease.tenant_id).bind(lease.job_id).bind(&lease.worker_id)
        .bind(i64::from(lease.attempt_count)).fetch_optional(&mut *session).await.map_err(storage_error)?;
    let remaining = remaining
        .filter(|remaining| *remaining > 0)
        .ok_or(SecondaryIndexError::LeaseLost)?;
    sqlx::query("SELECT set_config('statement_timeout', $1, false)")
        .bind(format!("{}ms", remaining.min(300_000)))
        .execute(session)
        .await
        .map_err(storage_error)?;
    Ok(())
}

fn matches_schema_contract(
    spec: &SecondaryIndexSpec,
    fingerprint: &str,
    schema: JsonValue,
) -> bool {
    spec.schema_fingerprint.to_string() == fingerprint
        && serde_json::from_value::<IndexSchema>(schema).is_ok_and(|schema| {
            schema.reference == spec.schema
                && schema
                    .fingerprint()
                    .is_ok_and(|fp| fp == spec.schema_fingerprint)
        })
}

fn creation_intent(lease: &SecondaryIndexLease, table_oid: i64) -> JsonValue {
    json!({
        "phase": "creating_index",
        "table_oid": table_oid,
        "definition_hash": lease.spec.definition_hash,
    })
}

async fn creation_is_reserved(
    session: &mut PgConnection,
    lease: &SecondaryIndexLease,
    table_oid: i64,
) -> Result<bool, SecondaryIndexError> {
    let cursor: Option<JsonValue> = sqlx::query_scalar(
        "SELECT cursor FROM index_jobs WHERE tenant_id = $1 AND job_id = $2 AND lease_owner = $3
         AND attempt_count = $4 AND kind = 'secondary_index' AND state = 'running'
         AND lease_expires_at > clock_timestamp() AND cancel_requested = FALSE",
    )
    .bind(lease.tenant_id)
    .bind(lease.job_id)
    .bind(&lease.worker_id)
    .bind(i64::from(lease.attempt_count))
    .fetch_optional(session)
    .await
    .map_err(storage_error)?
    .ok_or(SecondaryIndexError::LeaseLost)?;
    match cursor {
        None => Ok(false),
        Some(cursor) if cursor == creation_intent(lease, table_oid) => Ok(true),
        Some(_) => Err(SecondaryIndexError::InvalidStoredJob(
            "secondary-index creation intent no longer matches table and definition".to_owned(),
        )),
    }
}

async fn reserve_creation(
    session: &mut PgConnection,
    lease: &SecondaryIndexLease,
    table_oid: i64,
    qualified_index: &str,
) -> Result<(), SecondaryIndexError> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(qualified_index)
        .fetch_one(&mut *session)
        .await
        .map_err(storage_error)?;
    if exists {
        return Err(SecondaryIndexError::IndexOwnershipConflict {
            index_name: lease.spec.index_name.clone(),
        });
    }
    let fence = live_lease_where(DbBackend::Postgres, "secondary_index", 1, false);
    let statement = format!(
        "UPDATE index_jobs SET cursor = $5, updated_at = clock_timestamp()
         WHERE {fence} AND cursor IS NULL",
    );
    let reserved = sqlx::query(&statement)
        .bind(lease.tenant_id)
        .bind(lease.job_id)
        .bind(&lease.worker_id)
        .bind(i64::from(lease.attempt_count))
        .bind(creation_intent(lease, table_oid))
        .execute(session)
        .await
        .map_err(storage_error)?;
    if reserved.rows_affected() != 1 {
        return Err(SecondaryIndexError::LeaseLost);
    }
    Ok(())
}

async fn execute_postgres_locked(
    session: &mut PgConnection,
    lease: &SecondaryIndexLease,
) -> Result<SecondaryIndexExecutionOutcome, SecondaryIndexError> {
    remaining_lease_budget(session, lease).await?;
    let spec = &lease.spec;
    let (fingerprint, status, schema_json): (String, String, JsonValue) = sqlx::query_as(
        "SELECT schema_fingerprint, status, schema_json FROM index_schemas WHERE tenant_id=$1 AND module_name=$2 AND entity_name=$3 AND schema_version=$4")
        .bind(spec.tenant_id).bind(spec.schema.module.as_str()).bind(spec.schema.entity.as_str())
        .bind(i64::from(spec.schema.version.get())).fetch_optional(&mut *session).await.map_err(storage_error)?
        .ok_or_else(|| SecondaryIndexError::SchemaNotRegistered(spec.schema.clone()))?;
    if !matches_schema_contract(spec, &fingerprint, schema_json) {
        return Err(SecondaryIndexError::SchemaFingerprintConflict);
    }
    if status != "active" && lease.operation != SecondaryIndexOperation::Retire {
        return Err(SecondaryIndexError::SchemaRetired(spec.schema.clone()));
    }
    let (table_oid, namespace, column_projection): (i64, String, String) = sqlx::query_as(
        "SELECT t.oid::bigint, n.nspname::text, (
            SELECT string_agg(CASE WHEN a.attisdropped THEN 'NULL::text AS ' || quote_ident('removed_' || a.attnum::text)
                   ELSE quote_ident(a.attname) END, ', ' ORDER BY a.attnum)
            FROM pg_catalog.pg_attribute AS a WHERE a.attrelid = t.oid AND a.attnum > 0
         ) FROM pg_catalog.pg_class AS t JOIN pg_catalog.pg_namespace AS n ON n.oid = t.relnamespace
         WHERE t.oid = to_regclass('index_entities') AND t.relkind = 'r' AND n.oid <> pg_my_temp_schema()")
        .fetch_optional(&mut *session).await.map_err(storage_error)?
        .ok_or_else(|| SecondaryIndexError::Storage("Index entity table is missing or partitioned".to_owned()))?;
    let table = format!("{}.index_entities", quote_identifier(&namespace));
    let name = &spec.index_name;
    let qualified_index = format!(
        "{}.{}",
        quote_identifier(&namespace),
        quote_identifier(name)
    );
    let mut existing = index_state(session, table_oid, name).await?;
    if existing.is_none() && lease.operation == SecondaryIndexOperation::Retire {
        // No DROP will be issued, even if the name belongs to some other table.
        return Ok(SecondaryIndexExecutionOutcome::Retired {
            index_name: name.clone(),
            dropped: false,
        });
    }
    if existing.is_none() && lease.operation == SecondaryIndexOperation::Reindex {
        return Err(SecondaryIndexError::IndexMissing(name.clone()));
    }
    let ownership_error = || SecondaryIndexError::IndexOwnershipConflict {
        index_name: name.clone(),
    };
    let owner = spec.owner_comment();
    let reserved = lease.operation == SecondaryIndexOperation::Ensure
        && creation_is_reserved(session, lease, table_oid).await?;
    if existing.as_ref().is_some_and(|index| {
        index.comment.as_deref() != Some(owner.as_str()) && !(reserved && index.comment.is_none())
    }) {
        return Err(ownership_error());
    }

    // Let PostgreSQL normalize the exact expected definition against the actual column types and
    // collations on an empty temporary table. Compare the catalog, not hand-normalized DDL text.
    let scratch = format!("index_definition_{}", Uuid::new_v4().simple());
    let scratch_table = format!("pg_temp.{}", quote_identifier(&scratch));
    // Preserve physical column numbers even if the owning table has had a dropped column.
    sqlx::raw_sql(&format!(
        "CREATE TEMP TABLE {} AS SELECT {column_projection} FROM {table} WITH NO DATA",
        quote_identifier(&scratch)
    ))
    .execute(&mut *session)
    .await
    .map_err(storage_error)?;
    let create = spec.create_statement(DbBackend::Postgres)?;
    let reference = create
        .replacen("CREATE INDEX CONCURRENTLY", "CREATE INDEX", 1)
        .replacen(" ON index_entities ", &format!(" ON {scratch_table} "), 1);
    sqlx::raw_sql(&reference)
        .execute(&mut *session)
        .await
        .map_err(storage_error)?;
    let scratch_oid: i64 = sqlx::query_scalar("SELECT to_regclass($1)::bigint")
        .bind(&scratch_table)
        .fetch_one(&mut *session)
        .await
        .map_err(storage_error)?;
    let expected = index_state(session, scratch_oid, name)
        .await?
        .ok_or_else(ownership_error)?
        .definition;
    sqlx::raw_sql(&format!("DROP TABLE {scratch_table}"))
        .execute(&mut *session)
        .await
        .map_err(storage_error)?;
    if existing
        .as_ref()
        .is_some_and(|index| index.definition != expected)
    {
        return Err(ownership_error());
    }
    let verify_ready = |state: PostgresIndexState| {
        if state.definition != expected || state.comment.as_deref() != Some(owner.as_str()) {
            return Err(ownership_error());
        }
        if !state.ready {
            return Err(SecondaryIndexError::IndexNotReady(name.clone()));
        }
        Ok(())
    };

    remaining_lease_budget(session, lease).await?;
    let outcome = match lease.operation {
        SecondaryIndexOperation::Ensure => {
            // An interrupted concurrent build can leave an invalid physical index. Rebuild
            // only if it was marked by Index or this exact job reserved its absent name.
            if existing.as_ref().is_some_and(|state| !state.ready) {
                sqlx::raw_sql(&format!("DROP INDEX CONCURRENTLY {qualified_index}"))
                    .execute(&mut *session)
                    .await
                    .map_err(storage_error)?;
                existing = None;
            }
            let created = existing.is_none();
            if created {
                if !reserved {
                    reserve_creation(session, lease, table_oid, &qualified_index).await?;
                }
                remaining_lease_budget(session, lease).await?;
                let ddl = create.replacen(" ON index_entities ", &format!(" ON {table} "), 1);
                sqlx::raw_sql(&ddl)
                    .execute(&mut *session)
                    .await
                    .map_err(storage_error)?;
                existing = index_state(session, table_oid, name).await?;
            }
            let state = existing.ok_or_else(ownership_error)?;
            if state.definition != expected
                || state
                    .comment
                    .as_deref()
                    .is_some_and(|comment| comment != owner)
            {
                return Err(ownership_error());
            }
            if !state.ready {
                return Err(SecondaryIndexError::IndexNotReady(name.clone()));
            }
            if state.comment.is_none() {
                remaining_lease_budget(session, lease).await?;
                sqlx::raw_sql(&format!(
                    "COMMENT ON INDEX {qualified_index} IS {}",
                    quote_literal(&owner)
                ))
                .execute(&mut *session)
                .await
                .map_err(storage_error)?;
            }
            verify_ready(
                index_state(session, table_oid, name)
                    .await?
                    .ok_or_else(ownership_error)?,
            )?;
            SecondaryIndexExecutionOutcome::Ready {
                index_name: name.clone(),
                created,
            }
        }
        SecondaryIndexOperation::Reindex => {
            sqlx::raw_sql(&format!("REINDEX INDEX CONCURRENTLY {qualified_index}"))
                .execute(&mut *session)
                .await
                .map_err(storage_error)?;
            verify_ready(
                index_state(session, table_oid, name)
                    .await?
                    .ok_or_else(ownership_error)?,
            )?;
            SecondaryIndexExecutionOutcome::Reindexed {
                index_name: name.clone(),
            }
        }
        SecondaryIndexOperation::Retire => {
            sqlx::raw_sql(&format!("DROP INDEX CONCURRENTLY {qualified_index}"))
                .execute(&mut *session)
                .await
                .map_err(storage_error)?;
            if index_state(session, table_oid, name).await?.is_some() {
                return Err(ownership_error());
            }
            SecondaryIndexExecutionOutcome::Retired {
                index_name: name.clone(),
                dropped: true,
            }
        }
    };
    remaining_lease_budget(session, lease).await?;
    Ok(outcome)
}
