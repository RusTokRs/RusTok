use std::time::Duration;

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    FieldCardinality, FieldName, IndexSchema, IndexValueType, SchemaFingerprint, SchemaRef,
};

const MAX_WORKER_ID_BYTES: usize = 191;
const MAX_ERROR_CODE_BYTES: usize = 128;
const MAX_LEASE_SECONDS: u64 = 86_400;
const OWNED_INDEX_COMMENT_PREFIX: &str = "rustok-index:";
const STORAGE_VALUE_CONTRACT: &str = "tagged_index_value_v1";

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
            "rustok-index-secondary-v1\u{1f}{STORAGE_VALUE_CONTRACT}\u{1f}{tenant_id}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
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
        let field = quote_literal(self.field_name.as_str());
        match (backend, self.kind) {
            (DbBackend::Postgres, SecondaryIndexKind::Scalar) => Ok(format!(
                "CREATE INDEX CONCURRENTLY {name} ON index_entities (locale_key, {}, entity_id) WHERE {predicate}",
                postgres_scalar_expression(self.value_type, &field),
            )),
            (DbBackend::Postgres, SecondaryIndexKind::JsonContainment) => Ok(format!(
                "CREATE INDEX CONCURRENTLY {name} ON index_entities USING gin (((payload -> {field}) -> 'value') jsonb_path_ops) WHERE {predicate}",
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

    fn comment_statement(&self) -> String {
        format!(
            "COMMENT ON INDEX {} IS {}",
            quote_identifier(&self.index_name),
            quote_literal(&self.owner_comment()),
        )
    }

    fn predicate(&self, backend: DbBackend) -> String {
        let tenant = quote_literal(&self.tenant_id.to_string());
        let tenant = match backend {
            DbBackend::Postgres => format!("{tenant}::uuid"),
            _ => tenant,
        };
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
    #[error("secondary-index schema is not persisted for this tenant: {0}")]
    SchemaNotRegistered(SchemaRef),
    #[error("secondary-index schema is retired: {0}")]
    SchemaRetired(SchemaRef),
    #[error("persisted schema fingerprint does not match the secondary-index plan")]
    SchemaFingerprintConflict,
    #[error("stored secondary-index job is invalid: {0}")]
    InvalidStoredJob(String),
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
        self.assert_current(lease, backend).await?;
        match lease.operation {
            SecondaryIndexOperation::Ensure => self.ensure_index(&lease.spec, backend).await,
            SecondaryIndexOperation::Reindex => self.reindex(&lease.spec, backend).await,
            SecondaryIndexOperation::Retire => self.retire(&lease.spec, backend).await,
        }
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
        self.lock_index(transaction, request.spec(), backend)
            .await?;
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
            self.supersede_expired_job(transaction, &stored, request, backend)
                .await?;
        }

        let job_id;
        let attempt_count;
        if let Some(stored) = reclaim {
            job_id = stored.job_id;
            attempt_count = stored.attempt_count.checked_add(1).ok_or_else(|| {
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
    ) -> Result<(), SecondaryIndexError> {
        if backend == DbBackend::Sqlite {
            return Ok(());
        }
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                vec![format!("secondary-index\u{1f}{}", spec.index_name).into()],
            ))
            .await
            .map_err(storage_error)?;
        Ok(())
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
        if fingerprint != spec.schema_fingerprint.to_string() {
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

    async fn ensure_index(
        &self,
        spec: &SecondaryIndexSpec,
        backend: DbBackend,
    ) -> Result<SecondaryIndexExecutionOutcome, SecondaryIndexError> {
        if let Some(existing) = self.inspect_index(spec, backend).await? {
            verify_index_owner(spec, &existing, backend)?;
            if !existing.ready || !existing.valid {
                return Err(SecondaryIndexError::IndexNotReady(spec.index_name.clone()));
            }
            if backend == DbBackend::Postgres && existing.comment.is_none() {
                self.db
                    .execute_unprepared(&spec.comment_statement())
                    .await
                    .map_err(storage_error)?;
            }
            return Ok(SecondaryIndexExecutionOutcome::Ready {
                index_name: spec.index_name.clone(),
                created: false,
            });
        }

        self.db
            .execute_unprepared(&spec.create_statement(backend)?)
            .await
            .map_err(storage_error)?;
        if backend == DbBackend::Postgres {
            self.db
                .execute_unprepared(&spec.comment_statement())
                .await
                .map_err(storage_error)?;
        }
        let existing = self
            .inspect_index(spec, backend)
            .await?
            .ok_or_else(|| SecondaryIndexError::IndexMissing(spec.index_name.clone()))?;
        verify_index_owner(spec, &existing, backend)?;
        if !existing.ready || !existing.valid {
            return Err(SecondaryIndexError::IndexNotReady(spec.index_name.clone()));
        }
        Ok(SecondaryIndexExecutionOutcome::Ready {
            index_name: spec.index_name.clone(),
            created: true,
        })
    }

    async fn reindex(
        &self,
        spec: &SecondaryIndexSpec,
        backend: DbBackend,
    ) -> Result<SecondaryIndexExecutionOutcome, SecondaryIndexError> {
        let existing = self
            .inspect_index(spec, backend)
            .await?
            .ok_or_else(|| SecondaryIndexError::IndexMissing(spec.index_name.clone()))?;
        verify_index_owner(spec, &existing, backend)?;
        self.db
            .execute_unprepared(&spec.reindex_statement(backend)?)
            .await
            .map_err(storage_error)?;
        if backend == DbBackend::Postgres {
            self.db
                .execute_unprepared(&spec.comment_statement())
                .await
                .map_err(storage_error)?;
        }
        let rebuilt = self
            .inspect_index(spec, backend)
            .await?
            .ok_or_else(|| SecondaryIndexError::IndexMissing(spec.index_name.clone()))?;
        verify_index_owner(spec, &rebuilt, backend)?;
        if !rebuilt.ready || !rebuilt.valid {
            return Err(SecondaryIndexError::IndexNotReady(spec.index_name.clone()));
        }
        Ok(SecondaryIndexExecutionOutcome::Reindexed {
            index_name: spec.index_name.clone(),
        })
    }

    async fn retire(
        &self,
        spec: &SecondaryIndexSpec,
        backend: DbBackend,
    ) -> Result<SecondaryIndexExecutionOutcome, SecondaryIndexError> {
        let Some(existing) = self.inspect_index(spec, backend).await? else {
            return Ok(SecondaryIndexExecutionOutcome::Retired {
                index_name: spec.index_name.clone(),
                dropped: false,
            });
        };
        verify_index_owner(spec, &existing, backend)?;
        self.db
            .execute_unprepared(&spec.drop_statement(backend)?)
            .await
            .map_err(storage_error)?;
        if self.inspect_index(spec, backend).await?.is_some() {
            return Err(SecondaryIndexError::Storage(
                "secondary index remained present after retirement".to_owned(),
            ));
        }
        Ok(SecondaryIndexExecutionOutcome::Retired {
            index_name: spec.index_name.clone(),
            dropped: true,
        })
    }

    async fn inspect_index(
        &self,
        spec: &SecondaryIndexSpec,
        backend: DbBackend,
    ) -> Result<Option<ExistingIndex>, SecondaryIndexError> {
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                inspect_index_sql(backend),
                vec![spec.index_name.clone().into()],
            ))
            .await
            .map_err(storage_error)?;
        match (backend, row) {
            (_, None) => Ok(None),
            (DbBackend::Postgres, Some(row)) => Ok(Some(ExistingIndex {
                ready: row.try_get("", "is_ready").map_err(storage_error)?,
                valid: row.try_get("", "is_valid").map_err(storage_error)?,
                comment: row.try_get("", "index_comment").map_err(storage_error)?,
            })),
            (DbBackend::Sqlite, Some(_)) if cfg!(test) => Ok(Some(ExistingIndex {
                ready: true,
                valid: true,
                comment: None,
            })),
            (backend, Some(_)) => Err(SecondaryIndexError::UnsupportedBackend(format!(
                "{backend:?}"
            ))),
        }
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
    operation: String,
    index_name: String,
    definition_hash: String,
    schema_fingerprint: String,
    attempt_count: u32,
    claimable: bool,
}

#[derive(Debug)]
struct ExistingIndex {
    ready: bool,
    valid: bool,
    comment: Option<String>,
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

fn verify_index_owner(
    spec: &SecondaryIndexSpec,
    existing: &ExistingIndex,
    backend: DbBackend,
) -> Result<(), SecondaryIndexError> {
    if backend != DbBackend::Postgres {
        return Ok(());
    }
    let expected = spec.owner_comment();
    if existing
        .comment
        .as_deref()
        .is_some_and(|comment| comment != expected.as_str())
    {
        return Err(SecondaryIndexError::IndexOwnershipConflict {
            index_name: spec.index_name.clone(),
        });
    }
    Ok(())
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

fn postgres_scalar_expression(value_type: IndexValueType, field: &str) -> String {
    let value = format!("(payload -> {field}) ->> 'value'");
    match value_type {
        IndexValueType::Boolean => format!("(({value})::boolean)"),
        IndexValueType::Integer => format!("(({value})::bigint)"),
        IndexValueType::Decimal => format!("(({value})::numeric)"),
        IndexValueType::String => format!("(({value}) COLLATE \"C\")"),
        IndexValueType::Uuid => format!("(({value})::uuid)"),
        IndexValueType::Timestamp => {
            format!("((regexp_replace({value}, '[^0-9]', '', 'g')) COLLATE \"C\")")
        }
    }
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
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
        "SELECT schema_fingerprint, status FROM index_schemas WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 LIMIT 1"
    )
}

fn select_active_jobs_sql(backend: DbBackend) -> String {
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
        "SELECT job_id, request, {attempt_count} AS attempt_count_value, {claimable} AS claimable FROM index_jobs WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 AND kind = 'secondary_index' AND scope_kind = 'schema' AND state IN ('pending', 'running') ORDER BY created_at"
    )
}

fn insert_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 8);
    format!(
        "INSERT INTO index_jobs (tenant_id, job_id, kind, state, scope_kind, module_name, entity_name, schema_version, request, attempt_count, available_at, lease_owner, lease_expires_at, heartbeat_at) VALUES ({prefix}1, {prefix}2, 'secondary_index', 'running', 'schema', {prefix}3, {prefix}4, {prefix}5, {prefix}6, 1, CURRENT_TIMESTAMP, {prefix}7, {lease_expires}, CURRENT_TIMESTAMP)"
    )
}

fn claim_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 5);
    format!(
        "UPDATE index_jobs SET state = 'running', lease_owner = {prefix}3, attempt_count = {prefix}4, lease_expires_at = {lease_expires}, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, completed_at = NULL, last_error_code = NULL, last_error_details = NULL WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'secondary_index' AND ((state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP))"
    )
}

fn supersede_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'failed', lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, last_error_code = 'secondary_index.superseded', last_error_details = {prefix}3, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'secondary_index' AND ((state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP))"
    )
}

fn heartbeat_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 5);
    format!(
        "UPDATE index_jobs SET lease_expires_at = {lease_expires}, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'secondary_index' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP"
    )
}

fn assert_current_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "SELECT job_id FROM index_jobs WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'secondary_index' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP LIMIT 1"
    )
}

fn finish_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = {prefix}1, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, last_error_code = {prefix}2, last_error_details = {prefix}3, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}4 AND job_id = {prefix}5 AND kind = 'secondary_index' AND state = 'running' AND lease_owner = {prefix}6 AND attempt_count = {prefix}7 AND lease_expires_at > CURRENT_TIMESTAMP"
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

fn inspect_index_sql(backend: DbBackend) -> String {
    match backend {
        DbBackend::Postgres => "SELECT index_data.indisready AS is_ready, index_data.indisvalid AS is_valid, obj_description(index_class.oid, 'pg_class') AS index_comment FROM pg_class AS index_class JOIN pg_index AS index_data ON index_data.indexrelid = index_class.oid JOIN pg_class AS table_class ON table_class.oid = index_data.indrelid JOIN pg_namespace AS namespace ON namespace.oid = table_class.relnamespace WHERE table_class.relname = 'index_entities' AND index_class.relname = $1 LIMIT 1".to_owned(),
        DbBackend::Sqlite if cfg!(test) => "SELECT name FROM sqlite_master WHERE type = 'index' AND name = ?1 LIMIT 1".to_owned(),
        backend => unreachable!("unsupported backend {backend:?} was validated"),
}
}
