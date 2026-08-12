use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use uuid::Uuid;

const RECONCILIATION_CURSOR_CONTRACT: &str = "index_reconciliation_cursor_v1";
const RECONCILIATION_RECOVERY_ACTION: &str = "requeue";
const MAX_RECOVERY_REASON_BYTES: usize = 512;
const MAX_SCOPE_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReconciliationRequeueRequest {
    tenant_id: Uuid,
    job_id: Uuid,
    actor_id: Uuid,
    reason: String,
}

impl IndexReconciliationRequeueRequest {
    pub fn new(
        tenant_id: Uuid,
        job_id: Uuid,
        actor_id: Uuid,
        reason: impl Into<String>,
    ) -> Result<Self, IndexReconciliationRecoveryError> {
        if tenant_id.is_nil() {
            return Err(IndexReconciliationRecoveryError::NilTenantId);
        }
        if job_id.is_nil() {
            return Err(IndexReconciliationRecoveryError::NilJobId);
        }
        if actor_id.is_nil() {
            return Err(IndexReconciliationRecoveryError::NilActorId);
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
pub enum IndexReconciliationRequeueOutcome {
    Requeued {
        audit_id: Uuid,
        job_id: Uuid,
        retry_epoch: u32,
    },
    NotFound,
    NotFailed,
}

#[derive(Clone)]
pub struct PostgresIndexReconciliationRecoveryStore {
    db: DatabaseConnection,
}

impl PostgresIndexReconciliationRecoveryStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn requeue_failed(
        &self,
        request: IndexReconciliationRequeueRequest,
    ) -> Result<IndexReconciliationRequeueOutcome, IndexReconciliationRecoveryError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
        let result = requeue_in_transaction(&transaction, &request).await;
        match result {
            Ok(outcome) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
                Ok(outcome)
            }
            Err(error) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
                Err(error)
            }
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
}

async fn requeue_in_transaction(
    transaction: &DatabaseTransaction,
    request: &IndexReconciliationRequeueRequest,
) -> Result<IndexReconciliationRequeueOutcome, IndexReconciliationRecoveryError> {
    let backend = transaction.get_database_backend();
    ensure_supported_backend(backend)?;

    let Some(scope) = select_recovery_scope(transaction, request, backend, false).await? else {
        return Ok(IndexReconciliationRequeueOutcome::NotFound);
    };
    lock_reconciliation_scope(transaction, request.tenant_id, &scope, backend).await?;

    let Some(locked) = select_recovery_scope(transaction, request, backend, true).await? else {
        return Ok(IndexReconciliationRequeueOutcome::NotFound);
    };
    if locked.module_name != scope.module_name
        || locked.entity_name != scope.entity_name
        || locked.schema_version != scope.schema_version
    {
        return Err(IndexReconciliationRecoveryError::RecoveryRace);
    }
    if locked.state != "failed" {
        return Ok(IndexReconciliationRequeueOutcome::NotFailed);
    }
    if locked.attempt_count == 0 {
        return Err(IndexReconciliationRecoveryError::InvalidStoredJob(
            "failed reconciliation job attempt count must be positive",
        ));
    }

    let retry_epoch = locked
        .retry_epoch
        .checked_add(1)
        .ok_or(IndexReconciliationRecoveryError::CounterOverflow)?;
    let cursor = initial_cursor();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            update_failed_job_sql(backend),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(request.job_id, backend),
                i64::from(locked.attempt_count).into(),
                i64::from(locked.retry_epoch).into(),
                i64::from(retry_epoch).into(),
                SqlValue::Json(Some(Box::new(cursor))),
            ],
        ))
        .await
        .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
    if updated.rows_affected() != 1 {
        return Err(IndexReconciliationRecoveryError::RecoveryRace);
    }

    let audit_id = Uuid::new_v4();
    transaction
        .execute(Statement::from_sql_and_values(
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
            ],
        ))
        .await
        .map_err(|_| IndexReconciliationRecoveryError::Storage)?;

    Ok(IndexReconciliationRequeueOutcome::Requeued {
        audit_id,
        job_id: request.job_id,
        retry_epoch,
    })
}

async fn select_recovery_scope(
    transaction: &DatabaseTransaction,
    request: &IndexReconciliationRequeueRequest,
    backend: DbBackend,
    for_update: bool,
) -> Result<Option<RecoveryScope>, IndexReconciliationRecoveryError> {
    transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            select_job_sql(backend, for_update),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(request.job_id, backend),
            ],
        ))
        .await
        .map_err(|_| IndexReconciliationRecoveryError::Storage)?
        .map(|row| decode_scope(&row))
        .transpose()
}

fn decode_scope(row: &QueryResult) -> Result<RecoveryScope, IndexReconciliationRecoveryError> {
    let scope_kind: String = row
        .try_get("", "scope_kind")
        .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
    if scope_kind != "schema" {
        return Err(IndexReconciliationRecoveryError::InvalidStoredJob(
            "reconciliation recovery requires schema scope",
        ));
    }
    let module_name: String = row
        .try_get("", "module_name")
        .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
    let entity_name: String = row
        .try_get("", "entity_name")
        .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
    validate_scope_name(&module_name)?;
    validate_scope_name(&entity_name)?;

    let schema_version: i64 = row
        .try_get("", "schema_version_value")
        .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
    let schema_version = u32::try_from(schema_version).map_err(|_| {
        IndexReconciliationRecoveryError::InvalidStoredJob(
            "reconciliation recovery schema version is invalid",
        )
    })?;
    if schema_version == 0 {
        return Err(IndexReconciliationRecoveryError::InvalidStoredJob(
            "reconciliation recovery schema version must be positive",
        ));
    }

    let attempt_count: i64 = row
        .try_get("", "attempt_count_value")
        .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
    let attempt_count = u32::try_from(attempt_count).map_err(|_| {
        IndexReconciliationRecoveryError::InvalidStoredJob(
            "reconciliation recovery attempt count is invalid",
        )
    })?;
    let retry_epoch: i64 = row
        .try_get("", "retry_epoch_value")
        .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
    let retry_epoch = u32::try_from(retry_epoch).map_err(|_| {
        IndexReconciliationRecoveryError::InvalidStoredJob(
            "reconciliation recovery retry epoch is invalid",
        )
    })?;

    Ok(RecoveryScope {
        module_name,
        entity_name,
        schema_version,
        state: row
            .try_get("", "state")
            .map_err(|_| IndexReconciliationRecoveryError::Storage)?,
        attempt_count,
        retry_epoch,
    })
}

async fn lock_reconciliation_scope(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    scope: &RecoveryScope,
    backend: DbBackend,
) -> Result<(), IndexReconciliationRecoveryError> {
    if backend == DbBackend::Sqlite {
        return Ok(());
    }
    let lock_key = format!(
        "reconcile\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        tenant_id, scope.module_name, scope.entity_name, scope.schema_version,
    );
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![lock_key.into()],
        ))
        .await
        .map_err(|_| IndexReconciliationRecoveryError::Storage)?;
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

fn validate_reason(value: &str) -> Result<(), IndexReconciliationRecoveryError> {
    if value.is_empty() {
        return Err(IndexReconciliationRecoveryError::InvalidReason(
            "reason must not be empty",
        ));
    }
    if value.len() > MAX_RECOVERY_REASON_BYTES {
        return Err(IndexReconciliationRecoveryError::InvalidReason(
            "reason exceeds 512 bytes",
        ));
    }
    if value.trim() != value {
        return Err(IndexReconciliationRecoveryError::InvalidReason(
            "reason must be trimmed",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(IndexReconciliationRecoveryError::InvalidReason(
            "reason must not contain control characters",
        ));
    }
    Ok(())
}

fn validate_scope_name(value: &str) -> Result<(), IndexReconciliationRecoveryError> {
    if value.is_empty()
        || value.len() > MAX_SCOPE_NAME_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(IndexReconciliationRecoveryError::InvalidStoredJob(
            "reconciliation recovery scope name is invalid",
        ));
    }
    Ok(())
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexReconciliationRecoveryError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        _ => Err(IndexReconciliationRecoveryError::UnsupportedBackend),
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

fn select_job_sql(backend: DbBackend, for_update: bool) -> String {
    let prefix = placeholder_prefix(backend);
    let lock = if backend == DbBackend::Postgres && for_update {
        " FOR UPDATE"
    } else {
        ""
    };
    format!(
        "SELECT scope_kind, module_name, entity_name, CAST(schema_version AS BIGINT) AS schema_version_value, state, CAST(attempt_count AS BIGINT) AS attempt_count_value, CAST(retry_epoch AS BIGINT) AS retry_epoch_value FROM index_jobs WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' LIMIT 1{lock}"
    )
}

fn update_failed_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'pending', cursor = {prefix}6, attempt_count = 0, available_at = CURRENT_TIMESTAMP, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = NULL, cancel_requested = FALSE, last_error_code = NULL, last_error_details = NULL, completed_at = NULL, updated_at = CURRENT_TIMESTAMP, retry_epoch = {prefix}5 WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'failed' AND attempt_count = {prefix}3 AND retry_epoch = {prefix}4"
    )
}

fn insert_audit_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "INSERT INTO index_reconciliation_recovery_audits (tenant_id, audit_id, job_id, actor_id, action, reason, prior_attempt_count, retry_epoch) VALUES ({prefix}1, {prefix}2, {prefix}3, {prefix}4, '{RECONCILIATION_RECOVERY_ACTION}', {prefix}5, {prefix}6, {prefix}7)"
    )
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IndexReconciliationRecoveryError {
    #[error("Index reconciliation recovery tenant id must not be nil")]
    NilTenantId,
    #[error("Index reconciliation recovery job id must not be nil")]
    NilJobId,
    #[error("Index reconciliation recovery actor id must not be nil")]
    NilActorId,
    #[error("Index reconciliation recovery reason is invalid: {0}")]
    InvalidReason(&'static str),
    #[error("stored Index reconciliation recovery state is invalid: {0}")]
    InvalidStoredJob(&'static str),
    #[error("Index reconciliation recovery counter overflow")]
    CounterOverflow,
    #[error("Index reconciliation recovery raced with another state transition")]
    RecoveryRace,
    #[error("Index reconciliation recovery does not support this database backend")]
    UnsupportedBackend,
    #[error("Index reconciliation recovery storage operation failed")]
    Storage,
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    use super::*;

    async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        for statement in [
            r#"CREATE TABLE index_jobs (
                tenant_id TEXT NOT NULL,
                job_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                state TEXT NOT NULL,
                scope_kind TEXT NOT NULL,
                module_name TEXT,
                entity_name TEXT,
                schema_version INTEGER,
                cursor JSON,
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
                PRIMARY KEY (tenant_id, job_id)
            )"#,
            r#"CREATE TABLE index_reconciliation_recovery_audits (
                tenant_id TEXT NOT NULL,
                audit_id TEXT NOT NULL,
                job_id TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                action TEXT NOT NULL,
                reason TEXT NOT NULL,
                prior_attempt_count INTEGER NOT NULL,
                retry_epoch INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (tenant_id, audit_id),
                UNIQUE (tenant_id, job_id, retry_epoch)
            )"#,
            r#"CREATE TRIGGER index_reconciliation_recovery_audits_immutable_update
                BEFORE UPDATE ON index_reconciliation_recovery_audits
                FOR EACH ROW BEGIN
                    SELECT RAISE(ABORT, 'append-only');
                END"#,
            r#"CREATE TRIGGER index_reconciliation_recovery_audits_immutable_delete
                BEFORE DELETE ON index_reconciliation_recovery_audits
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
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO index_jobs (tenant_id, job_id, kind, state, scope_kind, module_name, entity_name, schema_version, cursor, attempt_count, last_error_code, last_error_details, completed_at) VALUES (?1, ?2, 'reconcile', 'failed', 'schema', 'catalog', 'product', 1, '{}', ?3, 'index.reconciliation_page_failed', '{}', CURRENT_TIMESTAMP)",
            vec![
                tenant_id.to_string().into(),
                job_id.to_string().into(),
                i64::from(attempt_count).into(),
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

        let outcome = PostgresIndexReconciliationRecoveryStore::new(db.clone())
            .requeue_failed(
                IndexReconciliationRequeueRequest::new(
                    tenant_id,
                    job_id,
                    actor_id,
                    "operator approved bounded retry",
                )
                .unwrap(),
            )
            .await
            .expect("requeue");
        let IndexReconciliationRequeueOutcome::Requeued {
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
            .query_one(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT state, cursor, attempt_count, cancel_requested, last_error_code, last_error_details, completed_at, retry_epoch FROM index_jobs WHERE tenant_id = ?1 AND job_id = ?2",
                vec![tenant_id.to_string().into(), job_id.to_string().into()],
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(job.try_get::<String>("", "state").unwrap(), "pending");
        assert_eq!(job.try_get::<i64>("", "attempt_count").unwrap(), 0);
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
            .query_one(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT actor_id, action, reason, prior_attempt_count, retry_epoch FROM index_reconciliation_recovery_audits WHERE tenant_id = ?1 AND audit_id = ?2",
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
            db.execute_unprepared(
                "UPDATE index_reconciliation_recovery_audits SET reason = 'changed'"
            )
            .await
            .is_err()
        );
        assert!(
            db.execute_unprepared("DELETE FROM index_reconciliation_recovery_audits")
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
        let store = PostgresIndexReconciliationRecoveryStore::new(db.clone());
        let request =
            IndexReconciliationRequeueRequest::new(tenant_id, job_id, actor_id, "approved retry")
                .unwrap();

        assert!(matches!(
            store.requeue_failed(request.clone()).await.unwrap(),
            IndexReconciliationRequeueOutcome::Requeued { retry_epoch: 1, .. }
        ));
        assert_eq!(
            store.requeue_failed(request).await.unwrap(),
            IndexReconciliationRequeueOutcome::NotFailed
        );
        let count = db
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count_value FROM index_reconciliation_recovery_audits"
                    .to_owned(),
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
            IndexReconciliationRequeueRequest::new(
                Uuid::nil(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                "reason"
            ),
            Err(IndexReconciliationRecoveryError::NilTenantId)
        ));
        assert!(matches!(
            IndexReconciliationRequeueRequest::new(
                Uuid::new_v4(),
                Uuid::nil(),
                Uuid::new_v4(),
                "reason"
            ),
            Err(IndexReconciliationRecoveryError::NilJobId)
        ));
        assert!(matches!(
            IndexReconciliationRequeueRequest::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::nil(),
                "reason"
            ),
            Err(IndexReconciliationRecoveryError::NilActorId)
        ));
        assert!(matches!(
            IndexReconciliationRequeueRequest::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                " reason "
            ),
            Err(IndexReconciliationRecoveryError::InvalidReason(_))
        ));
        assert!(matches!(
            IndexReconciliationRequeueRequest::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                "x".repeat(MAX_RECOVERY_REASON_BYTES + 1)
            ),
            Err(IndexReconciliationRecoveryError::InvalidReason(_))
        ));
    }
}
