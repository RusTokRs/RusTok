use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    EntityName, IndexSchema, LocaleMode, ModuleName, SchemaRef, SchemaRegistry, SchemaVersion,
    SharedIndexSourceRegistry,
};

use super::{
    IndexReplayJobLeaseRequest,
    source_reconciliation_runner::{
        self, MAX_PASSES, RECONCILIATION_JOB_REQUEST_CONTRACT, ReconciliationJobRequest,
    },
    source_replay_job,
};

const RECONCILIATION_CURSOR_CONTRACT: &str = "index_reconciliation_cursor_v1";
const RECOVERY_ACTION: &str = "requeue";
const MAX_RECOVERY_REASON_BYTES: usize = 512;
const MAX_SCOPE_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexJobRequeueRequest {
    tenant_id: Uuid,
    job_id: Uuid,
    actor_id: Uuid,
    reason: String,
}

impl IndexJobRequeueRequest {
    pub fn new(
        tenant_id: Uuid,
        job_id: Uuid,
        actor_id: Uuid,
        reason: impl Into<String>,
    ) -> Result<Self, IndexJobRecoveryError> {
        if tenant_id.is_nil() {
            return Err(IndexJobRecoveryError::NilTenantId);
        }
        if job_id.is_nil() {
            return Err(IndexJobRecoveryError::NilJobId);
        }
        if actor_id.is_nil() {
            return Err(IndexJobRecoveryError::NilActorId);
        }
        let reason = reason.into();
        validate_reason(&reason)?;
        Ok(Self {
            tenant_id,
            job_id,
            actor_id,
            reason,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexJobRequeueOutcome {
    Requeued {
        audit_id: Uuid,
        job_id: Uuid,
        retry_epoch: u32,
    },
    NotFound,
    NotFailed,
    /// The configured owner/schema cannot safely resume this exact job.
    ScopeUnavailable,
}

#[derive(Clone)]
pub(super) struct PostgresIndexJobRecoveryStore {
    db: DatabaseConnection,
}

impl PostgresIndexJobRecoveryStore {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(super) async fn requeue_reconciliation(
        &self,
        request: IndexJobRequeueRequest,
        sources: &SharedIndexSourceRegistry,
        schemas: &SchemaRegistry,
    ) -> Result<IndexJobRequeueOutcome, IndexJobRecoveryError> {
        self.requeue_failed(request, RecoveryKind::Reconcile, sources, schemas)
            .await
    }

    pub(super) async fn requeue_replay(
        &self,
        request: IndexJobRequeueRequest,
        sources: &SharedIndexSourceRegistry,
        schemas: &SchemaRegistry,
    ) -> Result<IndexJobRequeueOutcome, IndexJobRecoveryError> {
        self.requeue_failed(request, RecoveryKind::Rebuild, sources, schemas)
            .await
    }

    async fn requeue_failed(
        &self,
        request: IndexJobRequeueRequest,
        kind: RecoveryKind,
        sources: &SharedIndexSourceRegistry,
        schemas: &SchemaRegistry,
    ) -> Result<IndexJobRequeueOutcome, IndexJobRecoveryError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| IndexJobRecoveryError::Storage)?;
        let result = requeue_in_transaction(&transaction, &request, kind, sources, schemas).await;
        match result {
            Ok(outcome) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| IndexJobRecoveryError::Storage)?;
                Ok(outcome)
            }
            Err(error) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| IndexJobRecoveryError::Storage)?;
                Err(error)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum RecoveryKind {
    Reconcile,
    Rebuild,
}

impl RecoveryKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Reconcile => "reconcile",
            Self::Rebuild => "rebuild",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecoveryScope {
    module_name: String,
    entity_name: String,
    schema_version: u32,
    state: String,
    attempt_count: u32,
    retry_epoch: u32,
    schema: SchemaRef,
    source_name: String,
    replay_scope: Option<IndexReplayJobLeaseRequest>,
}

async fn requeue_in_transaction(
    transaction: &DatabaseTransaction,
    request: &IndexJobRequeueRequest,
    kind: RecoveryKind,
    sources: &SharedIndexSourceRegistry,
    schemas: &SchemaRegistry,
) -> Result<IndexJobRequeueOutcome, IndexJobRecoveryError> {
    let backend = transaction.get_database_backend();
    ensure_supported_backend(backend)?;

    let Some(scope) = select_recovery_scope(transaction, request, backend, kind, false).await?
    else {
        return Ok(IndexJobRequeueOutcome::NotFound);
    };
    if let Some(replay_scope) = &scope.replay_scope {
        source_replay_job::lock_replay_scope(transaction, replay_scope, backend)
            .await
            .map_err(|_| IndexJobRecoveryError::Storage)?;
    } else {
        lock_reconciliation_scope(transaction, request.tenant_id, &scope, backend).await?;
    }

    let Some(locked) = select_recovery_scope(transaction, request, backend, kind, true).await?
    else {
        return Ok(IndexJobRequeueOutcome::NotFound);
    };
    if locked.module_name != scope.module_name
        || locked.entity_name != scope.entity_name
        || locked.schema_version != scope.schema_version
        || locked.replay_scope != scope.replay_scope
        || locked.source_name != scope.source_name
    {
        return Err(IndexJobRecoveryError::RecoveryRace);
    }
    if scope.state != "failed" || locked.state != "failed" {
        return Ok(IndexJobRequeueOutcome::NotFailed);
    }
    // The original read is not a lock. Do not apply this request to a replacement
    // failure that became visible while we waited for the scope.
    if locked.attempt_count != scope.attempt_count || locked.retry_epoch != scope.retry_epoch {
        return Err(IndexJobRecoveryError::RecoveryRace);
    }
    if locked.attempt_count == 0 || locked.attempt_count >= i32::MAX as u32 {
        return Err(IndexJobRecoveryError::InvalidStoredJob(
            "failed job has no usable lease generation",
        ));
    }

    let Some(registered) = schemas.get(&locked.schema) else {
        return Ok(IndexJobRequeueOutcome::ScopeUnavailable);
    };
    if !sources
        .source_for_schema(&locked.schema)
        .is_some_and(|source| source.source_name() == locked.source_name)
        || (locked
            .replay_scope
            .as_ref()
            .is_some_and(|scope| scope.locale().is_some())
            && registered.schema.locale_mode == LocaleMode::None)
    {
        return Ok(IndexJobRequeueOutcome::ScopeUnavailable);
    }
    let scope_values = vec![
        uuid_value(request.tenant_id, backend),
        locked.module_name.clone().into(),
        locked.entity_name.clone().into(),
        i64::from(locked.schema_version).into(),
    ];
    let mut registration_values = scope_values.clone();
    registration_values.push(registered.fingerprint.to_string().into());
    let lock = if backend == DbBackend::Postgres {
        " FOR SHARE"
    } else {
        ""
    };
    let ready = transaction.query_one_raw(Statement::from_sql_and_values(
        backend,
        format!("SELECT schema_json FROM index_schemas WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 \
            AND schema_version = $4 AND schema_fingerprint = $5 AND status = 'active'{lock}"),
        registration_values,
    )).await.map_err(|_| IndexJobRecoveryError::Storage)?;
    let Some(ready) = ready else {
        return Ok(IndexJobRequeueOutcome::ScopeUnavailable);
    };
    let json: JsonValue = ready
        .try_get("", "schema_json")
        .map_err(|_| IndexJobRecoveryError::Storage)?;
    if !serde_json::from_value::<IndexSchema>(json)
        .is_ok_and(|schema| schema.canonical() == registered.schema.canonical())
    {
        return Ok(IndexJobRequeueOutcome::ScopeUnavailable);
    }
    // Both acquire paths hold the same scope advisory lock. Do not revive an older failed job
    // behind an authoritative succeeded/running/pending job or a more recent failed job.
    let current = if let Some(replay_scope) = &locked.replay_scope {
        source_replay_job::select_current_replay_job(transaction, replay_scope, backend)
            .await
            .map_err(|_| IndexJobRecoveryError::Storage)?
    } else {
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                source_reconciliation_runner::select_jobs_sql(backend),
                scope_values,
            ))
            .await
            .map_err(|_| IndexJobRecoveryError::Storage)?;
        row.as_ref()
            .map(|row| source_replay_job::stored_uuid(row, "job_id", backend))
            .transpose()
            .map_err(|_| IndexJobRecoveryError::Storage)?
    };
    if current != Some(request.job_id) {
        return Ok(IndexJobRequeueOutcome::ScopeUnavailable);
    }

    let retry_epoch = locked
        .retry_epoch
        .checked_add(1)
        .filter(|epoch| i32::try_from(*epoch).is_ok())
        .ok_or(IndexJobRecoveryError::CounterOverflow)?;
    let mut values = vec![
        uuid_value(request.tenant_id, backend),
        uuid_value(request.job_id, backend),
        i64::from(locked.attempt_count).into(),
        i64::from(locked.retry_epoch).into(),
        i64::from(retry_epoch).into(),
    ];
    if matches!(kind, RecoveryKind::Reconcile) {
        values.push(SqlValue::Json(Some(Box::new(initial_cursor()))));
    }
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            update_failed_job_sql(backend, kind),
            values,
        ))
        .await
        .map_err(|_| IndexJobRecoveryError::Storage)?;
    if updated.rows_affected() != 1 {
        return Err(IndexJobRecoveryError::RecoveryRace);
    }

    let audit_id = Uuid::new_v4();
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            insert_audit_sql(backend),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(audit_id, backend),
                uuid_value(request.job_id, backend),
                uuid_value(request.actor_id, backend),
                request.reason.clone().into(),
                i64::from(locked.attempt_count).into(),
                i64::from(retry_epoch).into(),
                kind.as_str().into(),
            ],
        ))
        .await
        .map_err(|_| IndexJobRecoveryError::Storage)?;

    Ok(IndexJobRequeueOutcome::Requeued {
        audit_id,
        job_id: request.job_id,
        retry_epoch,
    })
}

async fn select_recovery_scope(
    transaction: &DatabaseTransaction,
    request: &IndexJobRequeueRequest,
    backend: DbBackend,
    kind: RecoveryKind,
    for_update: bool,
) -> Result<Option<RecoveryScope>, IndexJobRecoveryError> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            select_job_sql(backend, kind, for_update),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(request.job_id, backend),
            ],
        ))
        .await
        .map_err(|_| IndexJobRecoveryError::Storage)?
        .map(|row| decode_scope(&row, request.tenant_id, backend, kind))
        .transpose()
}

fn decode_scope(
    row: &QueryResult,
    tenant_id: Uuid,
    backend: DbBackend,
    kind: RecoveryKind,
) -> Result<RecoveryScope, IndexJobRecoveryError> {
    let scope_kind: String = row
        .try_get("", "scope_kind")
        .map_err(|_| IndexJobRecoveryError::Storage)?;
    if matches!(kind, RecoveryKind::Reconcile) && scope_kind != "schema" {
        return Err(IndexJobRecoveryError::InvalidStoredJob(
            "reconciliation recovery requires schema scope",
        ));
    }
    let module_name: String = row
        .try_get("", "module_name")
        .map_err(|_| IndexJobRecoveryError::Storage)?;
    let entity_name: String = row
        .try_get("", "entity_name")
        .map_err(|_| IndexJobRecoveryError::Storage)?;
    validate_scope_name(&module_name)?;
    validate_scope_name(&entity_name)?;

    let schema_version: i64 = row
        .try_get("", "schema_version_value")
        .map_err(|_| IndexJobRecoveryError::Storage)?;
    let schema_version = u32::try_from(schema_version).map_err(|_| {
        IndexJobRecoveryError::InvalidStoredJob("job recovery schema version is invalid")
    })?;
    if schema_version == 0 {
        return Err(IndexJobRecoveryError::InvalidStoredJob(
            "job recovery schema version must be positive",
        ));
    }

    let attempt_count: i64 = row
        .try_get("", "attempt_count_value")
        .map_err(|_| IndexJobRecoveryError::Storage)?;
    let attempt_count = u32::try_from(attempt_count).map_err(|_| {
        IndexJobRecoveryError::InvalidStoredJob("job recovery attempt count is invalid")
    })?;
    let retry_epoch: i64 = row
        .try_get("", "retry_epoch_value")
        .map_err(|_| IndexJobRecoveryError::Storage)?;
    let retry_epoch = u32::try_from(retry_epoch).map_err(|_| {
        IndexJobRecoveryError::InvalidStoredJob("job recovery retry epoch is invalid")
    })?;

    let invalid_scope =
        |_| IndexJobRecoveryError::InvalidStoredJob("invalid recovery schema identity");
    let schema = SchemaRef {
        module: ModuleName::new(&module_name).map_err(invalid_scope)?,
        entity: EntityName::new(&entity_name).map_err(invalid_scope)?,
        version: SchemaVersion::new(schema_version),
    };
    let (replay_scope, source_name) = match kind {
        RecoveryKind::Rebuild => {
            let saved = source_replay_job::stored_job(row, backend).map_err(|_| {
                IndexJobRecoveryError::InvalidStoredJob("invalid replay scope or request")
            })?;
            let source_name = saved.source_name;
            let duration = std::time::Duration::from_secs(1);
            let scope = match saved.locale {
                Some(locale) => IndexReplayJobLeaseRequest::for_locale(
                    tenant_id,
                    schema.clone(),
                    locale,
                    &source_name,
                    "index-recovery",
                    duration,
                ),
                None => IndexReplayJobLeaseRequest::new(
                    tenant_id,
                    schema.clone(),
                    &source_name,
                    "index-recovery",
                    duration,
                ),
            }
            .map_err(|_| IndexJobRecoveryError::InvalidStoredJob("invalid replay scope"))?;
            (Some(scope), source_name)
        }
        RecoveryKind::Reconcile => {
            let request: JsonValue = row
                .try_get("", "request")
                .map_err(|_| IndexJobRecoveryError::Storage)?;
            let request: ReconciliationJobRequest =
                serde_json::from_value(request).map_err(|_| {
                    IndexJobRecoveryError::InvalidStoredJob("invalid reconciliation request")
                })?;
            if request.contract != RECONCILIATION_JOB_REQUEST_CONTRACT
                || !(1..=MAX_PASSES).contains(&request.pass_count)
            {
                return Err(IndexJobRecoveryError::InvalidStoredJob(
                    "invalid reconciliation pass contract",
                ));
            }
            (None, request.source_name)
        }
    };

    Ok(RecoveryScope {
        module_name,
        entity_name,
        schema_version,
        state: row
            .try_get("", "state")
            .map_err(|_| IndexJobRecoveryError::Storage)?,
        attempt_count,
        retry_epoch,
        schema,
        source_name,
        replay_scope,
    })
}

async fn lock_reconciliation_scope(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    scope: &RecoveryScope,
    backend: DbBackend,
) -> Result<(), IndexJobRecoveryError> {
    if backend == DbBackend::Sqlite {
        return Ok(());
    }
    let lock_key = format!(
        "reconcile\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        tenant_id, scope.module_name, scope.entity_name, scope.schema_version,
    );
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![lock_key.into()],
        ))
        .await
        .map_err(|_| IndexJobRecoveryError::Storage)?;
    Ok(())
}

fn initial_cursor() -> JsonValue {
    json!({
        "contract": RECONCILIATION_CURSOR_CONTRACT,
        "completed_passes": 0,
        "source_cursor": null,
        "pages_processed": 0,
        "mutation_count": 0,
        "applied_count": 0,
        "duplicate_count": 0,
        "stale_count": 0
    })
}

fn validate_reason(value: &str) -> Result<(), IndexJobRecoveryError> {
    if value.is_empty() {
        return Err(IndexJobRecoveryError::InvalidReason(
            "reason must not be empty",
        ));
    }
    if value.len() > MAX_RECOVERY_REASON_BYTES {
        return Err(IndexJobRecoveryError::InvalidReason(
            "reason exceeds 512 bytes",
        ));
    }
    if value.trim() != value {
        return Err(IndexJobRecoveryError::InvalidReason(
            "reason must be trimmed",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(IndexJobRecoveryError::InvalidReason(
            "reason must not contain control characters",
        ));
    }
    Ok(())
}

fn validate_scope_name(value: &str) -> Result<(), IndexJobRecoveryError> {
    if value.is_empty()
        || value.len() > MAX_SCOPE_NAME_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(IndexJobRecoveryError::InvalidStoredJob(
            "reconciliation recovery scope name is invalid",
        ));
    }
    Ok(())
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexJobRecoveryError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        _ => Err(IndexJobRecoveryError::UnsupportedBackend),
    }
}

fn placeholder_prefix(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "$",
        DbBackend::Sqlite => "?",
        _ => unreachable!("unsupported backend was validated"),
    }
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.to_string().into(),
        _ => unreachable!("unsupported backend was validated"),
    }
}

fn select_job_sql(backend: DbBackend, kind: RecoveryKind, for_update: bool) -> String {
    let prefix = placeholder_prefix(backend);
    let lock = if backend == DbBackend::Postgres && for_update {
        " FOR UPDATE"
    } else {
        ""
    };
    let extra = if matches!(kind, RecoveryKind::Rebuild) {
        "job_id, locale_key, last_error_code, FALSE AS claimable, "
    } else {
        ""
    };
    let kind = kind.as_str();
    format!(
        "SELECT {extra}request, scope_kind, module_name, entity_name, CAST(schema_version AS BIGINT) AS schema_version_value, state, CAST(attempt_count AS BIGINT) AS attempt_count_value, CAST(retry_epoch AS BIGINT) AS retry_epoch_value FROM index_jobs WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = '{kind}' LIMIT 1{lock}"
    )
}

fn update_failed_job_sql(backend: DbBackend, kind: RecoveryKind) -> String {
    let prefix = placeholder_prefix(backend);
    let reset_cursor = if matches!(kind, RecoveryKind::Reconcile) {
        format!("cursor = {prefix}6, ")
    } else {
        String::new()
    };
    let kind = kind.as_str();
    format!(
        "UPDATE index_jobs SET state = 'pending', {reset_cursor}failure_count = 0, available_at = CURRENT_TIMESTAMP, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = NULL, cancel_requested = FALSE, last_error_code = NULL, last_error_details = NULL, completed_at = NULL, updated_at = CURRENT_TIMESTAMP, retry_epoch = {prefix}5 WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = '{kind}' AND state = 'failed' AND attempt_count = {prefix}3 AND retry_epoch = {prefix}4"
    )
}

fn insert_audit_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "INSERT INTO index_job_recovery_audits (tenant_id, audit_id, job_id, actor_id, action, reason, prior_attempt_count, retry_epoch, job_kind) VALUES ({prefix}1, {prefix}2, {prefix}3, {prefix}4, '{RECOVERY_ACTION}', {prefix}5, {prefix}6, {prefix}7, {prefix}8)"
    )
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IndexJobRecoveryError {
    #[error("Index job recovery tenant id must not be nil")]
    NilTenantId,
    #[error("Index job recovery job id must not be nil")]
    NilJobId,
    #[error("Index job recovery actor id must not be nil")]
    NilActorId,
    #[error("Index job recovery reason is invalid: {0}")]
    InvalidReason(&'static str),
    #[error("stored Index job recovery state is invalid: {0}")]
    InvalidStoredJob(&'static str),
    #[error("Index job recovery counter overflow")]
    CounterOverflow,
    #[error("Index job recovery raced with another state transition")]
    RecoveryRace,
    #[error("Index job recovery does not support this database backend")]
    UnsupportedBackend,
    #[error("Index job recovery storage operation failed")]
    Storage,
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    use super::*;
    use crate::{
        IndexSource, IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest,
        IndexSourcePage, IndexSourceScanRequest, materialize_index_schema_registry,
        materialize_index_source_registry, register_index_schema_source, register_index_source,
    };
    use rustok_core::ModuleRuntimeExtensions;

    const TEST_SOURCE: &str = "catalog-product";

    fn test_schema() -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("catalog").unwrap(),
                entity: EntityName::new("product").unwrap(),
                version: SchemaVersion::INITIAL,
            },
            locale_mode: LocaleMode::None,
            fields: vec![crate::IndexField {
                name: crate::FieldName::new("id").unwrap(),
                value_type: crate::IndexValueType::Uuid,
                cardinality: crate::FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            }],
            links: Vec::new(),
        }
    }

    struct EmptySource;

    #[async_trait::async_trait]
    impl IndexSource for EmptySource {
        async fn load(
            &self,
            request: IndexSourceLoadRequest,
        ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
            Ok(IndexSourceLoadBatch::new(&request, vec![]).unwrap())
        }

        async fn scan(
            &self,
            request: IndexSourceScanRequest,
        ) -> Result<IndexSourcePage, IndexSourceFailure> {
            Ok(IndexSourcePage::new(&request, vec![], None).unwrap())
        }
    }

    async fn requeue_reconciliation(
        store: &PostgresIndexJobRecoveryStore,
        request: IndexJobRequeueRequest,
    ) -> Result<IndexJobRequeueOutcome, IndexJobRecoveryError> {
        let schema = test_schema();
        let mut extensions = ModuleRuntimeExtensions::default();
        register_index_schema_source(&mut extensions, "catalog", schema.clone()).unwrap();
        register_index_source(
            &mut extensions,
            "catalog",
            TEST_SOURCE,
            [schema.reference.clone()],
            EmptySource,
        )
        .unwrap();
        let schemas = materialize_index_schema_registry(&extensions)
            .unwrap()
            .unwrap();
        let sources = materialize_index_source_registry(&extensions)
            .unwrap()
            .unwrap();
        store
            .requeue_reconciliation(request, &sources, schemas.registry())
            .await
    }

    async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        for statement in [
            "CREATE TABLE index_schemas (tenant_id TEXT, module_name TEXT, entity_name TEXT, schema_version INTEGER, schema_fingerprint TEXT, status TEXT, schema_json JSON, PRIMARY KEY (tenant_id, module_name, entity_name, schema_version))",
            r#"CREATE TABLE index_jobs (
                tenant_id TEXT NOT NULL,
                job_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                state TEXT NOT NULL,
                scope_kind TEXT NOT NULL,
                module_name TEXT,
                entity_name TEXT,
                schema_version INTEGER,
                request JSON NOT NULL,
                cursor JSON,
                locale_key TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                attempt_count INTEGER NOT NULL,
                available_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                lease_owner TEXT,
                lease_expires_at TEXT,
                heartbeat_at TEXT,
                cancel_requested INTEGER NOT NULL DEFAULT 0,
                last_error_code TEXT,
                last_error_details JSON,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                completed_at TEXT,
                retry_epoch INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (tenant_id, job_id)
            )"#,
            r#"CREATE TABLE index_job_recovery_audits (
                tenant_id TEXT NOT NULL,
                audit_id TEXT NOT NULL,
                job_id TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                action TEXT NOT NULL,
                reason TEXT NOT NULL,
                prior_attempt_count INTEGER NOT NULL,
                retry_epoch INTEGER NOT NULL,
                job_kind TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (tenant_id, audit_id),
                UNIQUE (tenant_id, job_id, retry_epoch)
            )"#,
            r#"CREATE TRIGGER index_job_recovery_audits_immutable_update
                BEFORE UPDATE ON index_job_recovery_audits
                FOR EACH ROW BEGIN
                    SELECT RAISE(ABORT, 'append-only');
                END"#,
            r#"CREATE TRIGGER index_job_recovery_audits_immutable_delete
                BEFORE DELETE ON index_job_recovery_audits
                FOR EACH ROW BEGIN
                    SELECT RAISE(ABORT, 'append-only');
                END"#,
        ] {
            db.execute_unprepared(statement)
                .await
                .expect("recovery fixture");
        }
        db
    }

    async fn insert_failed(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        job_id: Uuid,
        attempt_count: u32,
    ) {
        let schema = test_schema();
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT OR IGNORE INTO index_schemas VALUES (?1, 'catalog', 'product', 1, ?2, 'active', ?3)",
            vec![tenant_id.to_string().into(), schema.fingerprint().unwrap().to_string().into(), serde_json::to_value(schema).unwrap().into()],
        )).await.unwrap();
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO index_jobs (tenant_id, job_id, kind, state, scope_kind, module_name, entity_name, schema_version, request, cursor, attempt_count, last_error_code, last_error_details, completed_at) VALUES (?1, ?2, 'reconcile', 'failed', 'schema', 'catalog', 'product', 1, ?4, '{}', ?3, 'index.reconciliation_page_failed', '{}', CURRENT_TIMESTAMP)",
            vec![
                tenant_id.to_string().into(),
                job_id.to_string().into(),
                i64::from(attempt_count).into(),
                json!({"contract": RECONCILIATION_JOB_REQUEST_CONTRACT, "source_name": TEST_SOURCE, "pass_count": 1}).into(),
            ],
        ))
        .await
        .expect("failed reconciliation job");
    }

    #[tokio::test]
    async fn requeue_resets_same_job_and_appends_immutable_audit() {
        let db = database().await;
        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        insert_failed(&db, tenant_id, job_id, 3).await;

        let outcome = requeue_reconciliation(
            &PostgresIndexJobRecoveryStore::new(db.clone()),
            IndexJobRequeueRequest::new(
                tenant_id,
                job_id,
                actor_id,
                "operator approved bounded retry",
            )
            .unwrap(),
        )
        .await
        .expect("requeue");
        let IndexJobRequeueOutcome::Requeued {
            audit_id,
            job_id: returned_job_id,
            retry_epoch,
        } = outcome
        else {
            panic!("expected requeue");
        };
        assert_eq!(returned_job_id, job_id);
        assert_eq!(retry_epoch, 1);

        let job = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT state, cursor, attempt_count, cancel_requested, last_error_code, last_error_details, completed_at, retry_epoch FROM index_jobs WHERE tenant_id = ?1 AND job_id = ?2",
                vec![tenant_id.to_string().into(), job_id.to_string().into()],
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(job.try_get::<String>("", "state").unwrap(), "pending");
        assert_eq!(job.try_get::<i64>("", "attempt_count").unwrap(), 3);
        assert_eq!(job.try_get::<i64>("", "cancel_requested").unwrap(), 0);
        assert_eq!(job.try_get::<i64>("", "retry_epoch").unwrap(), 1);
        assert!(
            job.try_get::<Option<String>>("", "last_error_code")
                .unwrap()
                .is_none()
        );
        assert!(
            job.try_get::<Option<JsonValue>>("", "last_error_details")
                .unwrap()
                .is_none()
        );
        assert!(
            job.try_get::<Option<String>>("", "completed_at")
                .unwrap()
                .is_none()
        );
        let cursor: JsonValue = job.try_get("", "cursor").unwrap();
        assert_eq!(cursor, initial_cursor());

        let audit = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT actor_id, action, reason, prior_attempt_count, retry_epoch FROM index_job_recovery_audits WHERE tenant_id = ?1 AND audit_id = ?2",
                vec![tenant_id.to_string().into(), audit_id.to_string().into()],
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            audit.try_get::<String>("", "actor_id").unwrap(),
            actor_id.to_string()
        );
        assert_eq!(audit.try_get::<String>("", "action").unwrap(), "requeue");
        assert_eq!(
            audit.try_get::<String>("", "reason").unwrap(),
            "operator approved bounded retry"
        );
        assert_eq!(audit.try_get::<i64>("", "prior_attempt_count").unwrap(), 3);
        assert_eq!(audit.try_get::<i64>("", "retry_epoch").unwrap(), 1);

        assert!(
            db.execute_unprepared("UPDATE index_job_recovery_audits SET reason = 'changed'")
                .await
                .is_err()
        );
        assert!(
            db.execute_unprepared("DELETE FROM index_job_recovery_audits")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn requeue_is_one_shot_for_each_failed_epoch() {
        let db = database().await;
        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        insert_failed(&db, tenant_id, job_id, 1).await;
        let store = PostgresIndexJobRecoveryStore::new(db.clone());
        let request =
            IndexJobRequeueRequest::new(tenant_id, job_id, actor_id, "approved retry").unwrap();

        assert!(matches!(
            requeue_reconciliation(&store, request.clone())
                .await
                .unwrap(),
            IndexJobRequeueOutcome::Requeued { retry_epoch: 1, .. }
        ));
        assert_eq!(
            requeue_reconciliation(&store, request).await.unwrap(),
            IndexJobRequeueOutcome::NotFailed
        );
        let count = db
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count_value FROM index_job_recovery_audits".to_owned(),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<i64>("", "count_value")
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn request_rejects_nil_identity_and_unbounded_reason() {
        assert!(matches!(
            IndexJobRequeueRequest::new(Uuid::nil(), Uuid::new_v4(), Uuid::new_v4(), "reason"),
            Err(IndexJobRecoveryError::NilTenantId)
        ));
        assert!(matches!(
            IndexJobRequeueRequest::new(Uuid::new_v4(), Uuid::nil(), Uuid::new_v4(), "reason"),
            Err(IndexJobRecoveryError::NilJobId)
        ));
        assert!(matches!(
            IndexJobRequeueRequest::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::nil(), "reason"),
            Err(IndexJobRecoveryError::NilActorId)
        ));
        assert!(matches!(
            IndexJobRequeueRequest::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), " reason "),
            Err(IndexJobRecoveryError::InvalidReason(_))
        ));
        assert!(matches!(
            IndexJobRequeueRequest::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                "x".repeat(MAX_RECOVERY_REASON_BYTES + 1)
            ),
            Err(IndexJobRecoveryError::InvalidReason(_))
        ));
    }
}
