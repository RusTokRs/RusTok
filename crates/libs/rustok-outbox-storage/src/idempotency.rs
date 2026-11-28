//! Durable, owner-scoped idempotency receipts for write-like port operations.

use chrono::{DateTime, Duration, FixedOffset};
#[cfg(test)]
use chrono::Utc;
use rustok_api::PortError;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{LockBehavior, LockType, OnConflict},
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const STATUS_PROCESSING: &str = "processing";
const STATUS_RELEASED: &str = "released";
const STATUS_COMPLETED: &str = "completed";
const STATUS_FAILED: &str = "failed";
const MAX_IDENTITY_LENGTH: usize = 191;
const PROCESSING_LEASE_MINUTES: i64 = 5;

mod entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "owner_operation_receipts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Option<Uuid>,
        pub scope_key: String,
        pub owner_slug: String,
        pub idempotency_key: String,
        pub operation: String,
        pub request_hash: String,
        pub lease_token: Uuid,
        pub status: String,
        pub response_json: Option<Json>,
        pub error_json: Option<Json>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
        pub completed_at: Option<DateTimeWithTimeZone>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// Checks that the database role can read the receipt schema without reading any stored receipts.
pub async fn verify_readable_schema(connection: &impl ConnectionTrait) -> Result<(), DbErr> {
    entity::Entity::find().limit(0).all(connection).await?;
    crate::database_now(connection).await?;
    Ok(())
}

/// The outcome of admitting a durable owner operation.
pub enum Admission {
    Run(Lease),
    Replay(Value),
    ReplayError(PortError),
}

/// A lease that permits one worker to complete or fail an admitted operation.
#[derive(Clone, Copy)]
pub struct Lease {
    pub operation_id: Uuid,
    pub token: Uuid,
}

/// Durable ownership scope for an idempotent operation receipt.
///
/// Platform operations deliberately have no tenant identity. Their stable
/// `scope_key` keeps uniqueness and request hashing separate from every tenant
/// without introducing a sentinel tenant UUID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerOperationScope {
    Platform,
    Tenant(Uuid),
}

impl OwnerOperationScope {
    fn tenant_id(self) -> Option<Uuid> {
        match self {
            Self::Platform => None,
            Self::Tenant(tenant_id) => Some(tenant_id),
        }
    }

    fn scope_key(self) -> String {
        match self {
            Self::Platform => "platform".to_string(),
            Self::Tenant(tenant_id) => format!("tenant:{tenant_id}"),
        }
    }
}

/// Derives an owner-facing receipt key from an untrusted caller key.
///
/// Transport adapters use one stable namespace owned by their bounded context. Binding tenant,
/// actor, operation, and an optional aggregate subject prevents one caller key from being replayed
/// under a different authorization or command meaning, while allowing native and remote adapters
/// to reach the same owner receipt for one logical command. Every component is length-delimited
/// and the optional subject has an explicit presence tag, so untrusted caller bytes cannot be
/// concatenated into another command identity. Do not substitute this framing for a deployed
/// namespace in place: durable receipts require an explicit dual-key migration to preserve
/// retries across the upgrade.
pub fn scoped_caller_idempotency_key(
    namespace: &str,
    tenant_id: Uuid,
    actor_id: Uuid,
    operation: &str,
    subject_id: Option<Uuid>,
    caller_key: &str,
) -> String {
    let mut digest = Sha256::new();
    hash_scoped_key_component(&mut digest, b"rustok.owner-operation-receipt-key.v1");
    hash_scoped_key_component(&mut digest, namespace.as_bytes());
    hash_scoped_key_component(&mut digest, tenant_id.as_bytes());
    hash_scoped_key_component(&mut digest, actor_id.as_bytes());
    hash_scoped_key_component(&mut digest, operation.as_bytes());
    match subject_id {
        Some(subject_id) => {
            digest.update([1]);
            hash_scoped_key_component(&mut digest, subject_id.as_bytes());
        }
        None => digest.update([0]),
    }
    hash_scoped_key_component(&mut digest, caller_key.trim().as_bytes());
    format!("{namespace}:{operation}:{}", hex::encode(digest.finalize()))
}

fn hash_scoped_key_component(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

/// Atomically admits an owner-scoped idempotent operation or returns its
/// stored terminal result. The same explicit owner scope, owner, and key
/// cannot be rebound to another operation or request payload.
pub async fn admit<T: Serialize>(
    database: &DatabaseConnection,
    scope: OwnerOperationScope,
    owner_slug: &str,
    idempotency_key: &str,
    operation: &str,
    request: &T,
) -> Result<Admission, PortError> {
    if let OwnerOperationScope::Tenant(tenant_id) = scope
        && tenant_id.is_nil()
    {
        return Err(PortError::validation(
            "outbox.operation_receipt_tenant_invalid",
            "tenant_id cannot be nil",
        ));
    }
    let owner_slug = validate_identity(owner_slug, "owner_slug")?;
    let idempotency_key = validate_identity(idempotency_key, "idempotency_key")?;
    let operation = validate_identity(operation, "operation")?;
    let scope_key = scope.scope_key();
    let request_hash = request_hash(&owner_slug, &operation, &scope_key, request)?;

    if let Some(existing) = find(database, &scope_key, &owner_slug, &idempotency_key).await? {
        return inspect_or_reclaim(database, existing, &operation, &request_hash).await;
    }

    crate::observe_database_clock(database).await.map_err(database_error)?;
    let txn = database.begin().await.map_err(database_error)?;
    let now = crate::database_now(&txn).await.map_err(database_error)?.fixed_offset();
    let id = generate_id();
    let lease_token = generate_id();
    let insert = entity::Entity::insert(entity::ActiveModel {
        id: Set(id),
        tenant_id: Set(scope.tenant_id()),
        scope_key: Set(scope_key.clone()),
        owner_slug: Set(owner_slug.clone()),
        idempotency_key: Set(idempotency_key.clone()),
        operation: Set(operation.clone()),
        request_hash: Set(request_hash.clone()),
        lease_token: Set(lease_token),
        status: Set(STATUS_PROCESSING.to_string()),
        response_json: Set(None),
        error_json: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        completed_at: Set(None),
    })
    .on_conflict(OnConflict::columns([entity::Column::ScopeKey, entity::Column::OwnerSlug, entity::Column::IdempotencyKey]).do_nothing().to_owned())
    .exec_without_returning(&txn)
    .await;

    match insert {
        Ok(1) => {
            let mut expected = entity::Model {
                id, tenant_id: scope.tenant_id(), scope_key: scope_key.clone(),
                owner_slug: owner_slug.clone(), idempotency_key: idempotency_key.clone(),
                operation: operation.clone(), request_hash: request_hash.clone(),
                lease_token, status: STATUS_PROCESSING.to_string(),
                response_json: None, error_json: None,
                created_at: now, updated_at: now, completed_at: None,
            };
            verify_receipt_state(&txn, &expected).await?;
            // Stamp after a possible unique-index wait.
            let now = crate::database_now(&txn).await.map_err(database_error)?.fixed_offset();
            let updated = entity::Entity::update_many()
                .col_expr(entity::Column::UpdatedAt, sea_orm::sea_query::Expr::value(now))
                .filter(entity::Column::Id.eq(id))
                .filter(entity::Column::LeaseToken.eq(lease_token))
                .filter(entity::Column::Status.eq(STATUS_PROCESSING))
                .exec(&txn).await.map_err(database_error)?;
            if updated.rows_affected != 1 {
                txn.rollback().await.map_err(database_error)?;
                return Err(lease_lost());
            }
            expected.updated_at = now;
            verify_receipt_state(&txn, &expected).await?;
            txn.commit().await.map_err(database_error)?;
            Ok(Admission::Run(Lease { operation_id: id, token: lease_token }))
        }
        Ok(0) => {
            txn.rollback().await.map_err(database_error)?;
            let existing = find(database, &scope_key, &owner_slug, &idempotency_key)
                .await?.ok_or_else(lease_lost)?;
            inspect_or_reclaim(database, existing, &operation, &request_hash).await
        }
        Ok(_) => {
            txn.rollback().await.map_err(database_error)?;
            Err(lease_lost())
        }
        Err(error) if is_unique_constraint(&error) => {
            txn.rollback().await.map_err(database_error)?;
            let existing = find(database, &scope_key, &owner_slug, &idempotency_key)
                .await?
                .ok_or_else(|| database_error(error))?;
            inspect_or_reclaim(database, existing, &operation, &request_hash).await
        }
        Err(error) => {
            txn.rollback().await.map_err(database_error)?;
            Err(database_error(error))
        }
    }
}

fn lease_lost() -> PortError {
    PortError::unavailable(
        "outbox.operation_receipt_lease_lost",
        "receipt is no longer live and owned by this worker",
    )
}

fn lease_is_live(updated_at: DateTime<FixedOffset>, now: DateTime<FixedOffset>) -> bool {
    updated_at <= now
        && now.signed_duration_since(updated_at) < Duration::minutes(PROCESSING_LEASE_MINUTES)
}

// First acquire write ownership; only then sample time. This also works
// for SQLite, whose writer serialization replaces PostgreSQL row locking.
async fn lock_processing(
    txn: &DatabaseTransaction,
    lease: Lease,
) -> Result<Option<(entity::Model, DateTime<FixedOffset>)>, PortError> {
    lock_receipt(txn, lease, STATUS_PROCESSING).await
}

async fn lock_receipt(
    txn: &DatabaseTransaction,
    lease: Lease,
    status: &str,
) -> Result<Option<(entity::Model, DateTime<FixedOffset>)>, PortError> {
    let query = entity::Entity::find_by_id(lease.operation_id)
        .filter(entity::Column::LeaseToken.eq(lease.token))
        .filter(entity::Column::Status.eq(status));
    // Do not UPDATE a receipt merely to lock it: even a no-op UPDATE can
    // invoke triggers and mutate the supposedly authoritative snapshot.
    let row = match txn.get_database_backend() {
        DbBackend::Postgres => query.lock_with_behavior(LockType::Update, LockBehavior::Nowait)
            .one(txn).await.map_err(database_error)?,
        DbBackend::Sqlite => {
            // A zero-row write reserves the SQLite writer without firing
            // row-level triggers. Read the receipt only after that fence.
            txn.execute_unprepared("UPDATE owner_operation_receipts SET lease_token = lease_token WHERE 1 = 0")
                .await.map_err(database_error)?;
            query.one(txn).await.map_err(database_error)?
        }
        _ => return Err(database_error(DbErr::Custom("unsupported receipt database backend".into()))),
    };
    let Some(row) = row else { return Ok(None); };
    if row.lease_token != lease.token || row.status != status
        || row.response_json.is_some() || row.error_json.is_some() || row.completed_at.is_some()
    {
        return Err(lease_lost());
    }
    let now = crate::database_now(txn).await.map_err(database_error)?.fixed_offset();
    Ok(Some((row, now)))
}

async fn live_processing_time(
    txn: &DatabaseTransaction,
    lease: Lease,
) -> Result<Option<DateTime<FixedOffset>>, PortError> {
    Ok(lock_processing(txn, lease).await?
        .filter(|(row, now)| lease_is_live(row.updated_at, *now))
        .map(|(_, now)| now))
}

/// Locks and verifies a processing receipt inside an owner transaction.
///
/// This must precede the owner's aggregate lock whenever receipt reclamation
/// and aggregate ownership are coordinated. The row lock remains held until
/// transaction end. Expired, reclaimed or terminal receipts cannot acquire
/// live authority. This does not renew the lease; the owner must roll back
/// on error and finish before expiry. External effects require idempotency.
pub async fn lock_current(
    transaction: &DatabaseTransaction,
    lease: Lease,
) -> Result<(), PortError> {
    if lease.operation_id.is_nil() || lease.token.is_nil() {
        return Err(PortError::validation(
            "outbox.operation_receipt_lease_invalid",
            "lease operation_id and token must not be nil",
        ));
    }
    if live_processing_time(transaction, lease).await?.is_none() {
        return Err(lease_lost());
    }
    Ok(())
}

/// Renews a processing lease, postponing reclamation by another admission.
///
/// An operation that may run longer than the processing lease (see
/// `PROCESSING_LEASE_MINUTES`) must renew periodically; without renewal a
/// later admission with the same idempotency key reclaims the receipt and
/// runs the operation concurrently. Renewal is fenced by the lease token:
/// a worker that already lost the lease learns so here instead of at
/// completion time, after its side effects have escaped.
pub async fn renew_lease(database: &DatabaseConnection, lease: Lease) -> Result<(), PortError> {
    if lease.operation_id.is_nil() || lease.token.is_nil() {
        return Err(PortError::validation(
            "outbox.operation_receipt_lease_invalid",
            "lease operation_id and token must not be nil",
        ));
    }
    crate::observe_database_clock(database).await.map_err(database_error)?;
    let txn = database.begin().await.map_err(database_error)?;
    let Some((mut expected, now)) = lock_processing(&txn, lease).await?
        .filter(|(row, now)| lease_is_live(row.updated_at, *now)) else {
        txn.rollback().await.map_err(database_error)?;
        return Err(lease_lost());
    };
    let update = entity::Entity::update_many()
        .col_expr(entity::Column::UpdatedAt, sea_orm::sea_query::Expr::value(now))
        .filter(entity::Column::Id.eq(lease.operation_id))
        .filter(entity::Column::LeaseToken.eq(lease.token))
        .filter(entity::Column::Status.eq(STATUS_PROCESSING))
        .exec(&txn)
        .await
        .map_err(database_error)?;
    if update.rows_affected != 1 {
        txn.rollback().await.map_err(database_error)?;
        return Err(lease_lost());
    }
    expected.updated_at = now;
    verify_receipt_state(&txn, &expected).await?;
    txn.commit().await.map_err(database_error)?;
    Ok(())
}

/// Stores a successful response inside the caller's owner transaction.
pub async fn complete<C, T>(connection: &C, lease: Lease, response: &T) -> Result<(), PortError>
where
    C: ConnectionTrait + TransactionTrait,
    T: Serialize,
{
    if lease.operation_id.is_nil() || lease.token.is_nil() {
        return Err(PortError::validation(
            "outbox.operation_receipt_lease_invalid",
            "lease operation_id and token must not be nil",
        ));
    }
    let response = serde_json::to_value(response).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_encode", error.to_string())
    })?;
    let txn = connection.begin().await.map_err(database_error)?;
    let Some((mut expected, now)) = lock_processing(&txn, lease).await?
        .filter(|(row, now)| lease_is_live(row.updated_at, *now)) else {
        txn.rollback().await.map_err(database_error)?;
        return Err(PortError::unavailable(
            "outbox.operation_receipt_state_invalid", "receipt is no longer live and owned by this worker",
        ));
    };
    let update = entity::Entity::update_many()
        .col_expr(
            entity::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_COMPLETED),
        )
        .col_expr(
            entity::Column::ResponseJson,
            sea_orm::sea_query::Expr::value(Some(response.clone())),
        )
        .col_expr(
            entity::Column::ErrorJson,
            sea_orm::sea_query::Expr::value(Option::<Value>::None),
        )
        .col_expr(
            entity::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .col_expr(
            entity::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .filter(entity::Column::Id.eq(lease.operation_id))
        .filter(entity::Column::LeaseToken.eq(lease.token))
        .filter(entity::Column::Status.eq(STATUS_PROCESSING))
        .exec(&txn)
        .await
        .map_err(database_error)?;

    if update.rows_affected != 1 {
        txn.rollback().await.map_err(database_error)?;
        // A stale worker cannot know whether a newer lease holder already completed, failed, or
        // reclaimed this receipt. Its caller must retry and let a later admission replay the
        // durable outcome or reclaim processing ownership; reporting a terminal invariant here
        // would incorrectly make that unknown outcome non-retryable.
        return Err(PortError::unavailable(
            "outbox.operation_receipt_state_invalid",
            "owner operation receipt was not processing during completion",
        ));
    }
    expected.status = STATUS_COMPLETED.to_string();
    expected.response_json = Some(response);
    expected.error_json = None;
    expected.updated_at = now;
    expected.completed_at = Some(now);
    if let Err(error) = verify_receipt_state(&txn, &expected).await {
        txn.rollback().await.map_err(database_error)?;
        return Err(error);
    }
    txn.commit().await.map_err(database_error)?;
    Ok(())
}

/// Stores a terminal failure after the caller's owner transaction has rolled
/// back. A replay returns the same typed failure instead of rerunning work.
pub async fn fail<C>(connection: &C, lease: Lease, error: &PortError) -> Result<(), PortError>
where
    C: ConnectionTrait + TransactionTrait,
{
    if lease.operation_id.is_nil() || lease.token.is_nil() {
        return Err(PortError::validation(
            "outbox.operation_receipt_lease_invalid",
            "lease operation_id and token must not be nil",
        ));
    }
    // A transient failure must not become a permanent replay result.
    // The owner must roll back its effects and use release_retryable instead.
    if error.retryable {
        return Err(PortError::validation(
            "outbox.operation_receipt_fail_retryable",
            "retryable owner-operation failures must release the receipt, not finalize it",
        ));
    }
    let error_json = serde_json::to_value(error).map_err(|encoding_error| {
        PortError::invariant_violation(
            "outbox.operation_receipt_encode",
            encoding_error.to_string(),
        )
    })?;
    let txn = connection.begin().await.map_err(database_error)?;
    let Some((mut expected, now)) = lock_processing(&txn, lease).await?
        .filter(|(row, now)| lease_is_live(row.updated_at, *now)) else {
        txn.rollback().await.map_err(database_error)?;
        return Err(PortError::unavailable(
            "outbox.operation_receipt_state_invalid", "receipt is no longer live and owned by this worker",
        ));
    };
    let update = entity::Entity::update_many()
        .col_expr(
            entity::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_FAILED),
        )
        .col_expr(
            entity::Column::ErrorJson,
            sea_orm::sea_query::Expr::value(Some(error_json.clone())),
        )
        .col_expr(
            entity::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .col_expr(
            entity::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .filter(entity::Column::Id.eq(lease.operation_id))
        .filter(entity::Column::LeaseToken.eq(lease.token))
        .filter(entity::Column::Status.eq(STATUS_PROCESSING))
        .exec(&txn)
        .await
        .map_err(database_error)?;

    if update.rows_affected != 1 {
        txn.rollback().await.map_err(database_error)?;
        // A stale worker must not report a terminal domain error after another worker reclaimed
        // the lease or finalized the receipt. The outcome is unknown to this caller, so preserve
        // retryability and let a later admission replay or reclaim the durable receipt.
        return Err(PortError::unavailable(
            "outbox.operation_receipt_state_invalid",
            "owner operation receipt was not processing during failure finalization",
        ));
    }
    expected.status = STATUS_FAILED.to_string();
    expected.error_json = Some(error_json);
    expected.updated_at = now;
    expected.completed_at = Some(now);
    if let Err(error) = verify_receipt_state(&txn, &expected).await {
        txn.rollback().await.map_err(database_error)?;
        return Err(error);
    }
    txn.commit().await.map_err(database_error)?;
    Ok(())
}

/// Releases a retryable failure's processing lease instead of recording it as
/// a permanent error. An outage, timeout or ambiguous commit must be retried
/// with the *same* request key: persisting it as a terminal failure would make
/// the outage permanent for the caller.
///
/// The lease token and processing state guard are essential. If an uncertain
/// transaction actually committed, its receipt is already completed and cannot
/// be released. Preserve the receipt identity and request fingerprint, revoke the
/// old token, and allow immediate exact-request re-admission. If another worker
/// reclaimed the lease, we cannot release it.
/// Call only after the command transaction has been rolled back or commit has
/// returned, never while a write transaction is still live.
pub async fn release_retryable<C>(
    connection: &C,
    lease: Lease,
    error: &PortError,
) -> Result<(), PortError>
where
    C: ConnectionTrait + TransactionTrait,
{
    if lease.operation_id.is_nil() || lease.token.is_nil() {
        return Err(PortError::validation(
            "outbox.operation_receipt_lease_invalid",
            "lease operation_id and token must not be nil",
        ));
    }
    if !error.retryable {
        return Err(PortError::validation(
            "outbox.operation_receipt_release_terminal",
            "only retryable owner-operation failures may release a receipt",
        ));
    }
    let txn = connection.begin().await.map_err(database_error)?;
    let Some((mut expected, now)) = lock_processing(&txn, lease).await?
        .filter(|(row, now)| lease_is_live(row.updated_at, *now)) else {
        txn.rollback().await.map_err(database_error)?;
        return Err(PortError::unavailable(
            "outbox.operation_receipt_release_uncertain", "receipt is no longer live and owned by this worker",
        ));
    };
    let token = generate_id();
    let released = entity::Entity::update_many()
        .col_expr(entity::Column::Status, sea_orm::sea_query::Expr::value(STATUS_RELEASED))
        .col_expr(entity::Column::LeaseToken, sea_orm::sea_query::Expr::value(token))
        .col_expr(entity::Column::UpdatedAt, sea_orm::sea_query::Expr::value(now))
        .filter(entity::Column::Id.eq(lease.operation_id))
        .filter(entity::Column::LeaseToken.eq(lease.token))
        .filter(entity::Column::Status.eq(STATUS_PROCESSING))
        .exec(&txn)
        .await
        .map_err(database_error)?;
    if released.rows_affected != 1 {
        txn.rollback().await.map_err(database_error)?;
        return Err(PortError::unavailable(
            "outbox.operation_receipt_release_uncertain",
            "receipt was completed or reclaimed before retryable release",
        ));
    }
    expected.status = STATUS_RELEASED.to_string();
    expected.lease_token = token;
    expected.updated_at = now;
    verify_receipt_state(&txn, &expected).await?;
    txn.commit().await.map_err(database_error)?;
    Ok(())
}

/// Bind the database-wide receipt window, including explicit indefinite
/// retention. A different local configuration never overwrites a binding.
pub async fn verify_receipt_retention_policy(
    database: &DatabaseConnection, window: Option<Duration>,
) -> Result<(), PortError> {
    let txn = database.begin().await.map_err(database_error)?;
    lock_receipt_retention_policy(&txn, window).await?;
    txn.commit().await.map_err(database_error)
}

async fn lock_receipt_retention_policy(
    txn: &DatabaseTransaction, window: Option<Duration>,
) -> Result<(), PortError> {
    if window.is_some_and(|w| w <= Duration::zero() || w > Duration::days(3650)
        || w != Duration::milliseconds(w.num_milliseconds())) {
        return Err(PortError::validation("outbox.operation_receipt_retention_invalid",
            "invalid operation receipt retention window"));
    }
    let expected = window.map(|w| w.num_milliseconds());
    let backend = txn.get_database_backend();
    if backend == DbBackend::Postgres {
        txn.execute_unprepared("SET TRANSACTION ISOLATION LEVEL READ COMMITTED").await.map_err(database_error)?;
    }
    let locked = txn.execute_unprepared("UPDATE outbox_owner_receipt_retention_policy SET bound = bound WHERE id = 1")
        .await.map_err(database_error)?;
    if locked.rows_affected() != 1 { return Err(database_error(DbErr::Custom("missing receipt retention policy".into()))); }
    let read = || async {
        let row = txn.query_one_raw(sea_orm::Statement::from_string(backend,
            "SELECT bound, retention_ms FROM outbox_owner_receipt_retention_policy WHERE id = 1"))
            .await.map_err(database_error)?.ok_or_else(|| database_error(DbErr::Custom("missing receipt retention policy".into())))?;
        let bound: i32 = row.try_get("", "bound").map_err(database_error)?;
        let retention: Option<i64> = row.try_get("", "retention_ms").map_err(database_error)?;
        Ok::<_, PortError>((bound, retention))
    };
    match read().await? {
        (0, None) => {
            let sql = match backend {
                DbBackend::Postgres => "UPDATE outbox_owner_receipt_retention_policy SET bound = 1, retention_ms = $1 WHERE id = 1 AND bound = 0",
                DbBackend::Sqlite => "UPDATE outbox_owner_receipt_retention_policy SET bound = 1, retention_ms = ? WHERE id = 1 AND bound = 0",
                _ => return Err(database_error(DbErr::Custom("unsupported receipt retention backend".into()))),
            };
            let changed = txn.execute_raw(sea_orm::Statement::from_sql_and_values(backend, sql, [expected.into()]))
                .await.map_err(database_error)?;
            if changed.rows_affected() != 1 || read().await? != (1, expected) {
                return Err(database_error(DbErr::Custom("receipt retention policy binding failed".into())));
            }
        }
        (1, stored) if stored == expected => {}
        _ => return Err(PortError::validation("outbox.operation_receipt_retention_mismatch",
            "receipt retention differs from durable policy; coordinated reconfiguration required")),
    }
    Ok(())
}

/// Deletes up to `batch_size` terminal (`completed`/`failed`) receipts
/// whose completion is older than `older_than`, and reports how many were
/// removed.
///
/// Receipts are the replay memory of the idempotency primitive: pruning one
/// means a later retry with the same key admits and runs the operation
/// again, so the window is an operator policy, not a correctness
/// requirement. Processing and released receipts are never touched — they
/// still bind a request and may be completed or re-admitted.
pub async fn prune_terminal_receipts(
    database: &DatabaseConnection,
    older_than: Duration,
    batch_size: u64,
) -> Result<u64, PortError> {
    if older_than <= Duration::zero() || older_than > Duration::days(3650)
        || older_than != Duration::milliseconds(older_than.num_milliseconds()) {
        return Err(PortError::validation(
            "outbox.operation_receipt_retention_invalid",
            "operation receipt retention window must be whole milliseconds, positive and at most 3650 days",
        ));
    }
    if !(1..=crate::MAX_RETENTION_BATCH_SIZE).contains(&batch_size) {
        return Err(PortError::validation(
            "outbox.operation_receipt_retention_invalid",
            format!("operation receipt prune batch size must be between 1 and {}", crate::MAX_RETENTION_BATCH_SIZE),
        ));
    }
    let cutoff = crate::observe_database_clock(database)
        .await.map_err(database_error)?
        .checked_sub_signed(older_than)
        .ok_or_else(|| PortError::validation(
            "outbox.operation_receipt_retention_invalid",
            "operation receipt retention cutoff is out of range",
        ))?.fixed_offset();
    let txn = database.begin().await.map_err(database_error)?;
    lock_receipt_retention_policy(&txn, Some(older_than)).await?;
    let candidates = entity::Entity::find()
        .select_only()
        .column(entity::Column::Id)
        .filter(entity::Column::Status.is_in([STATUS_COMPLETED, STATUS_FAILED]))
        .filter(entity::Column::CompletedAt.is_not_null())
        .filter(entity::Column::CompletedAt.lte(cutoff))
        .order_by_asc(entity::Column::CompletedAt)
        .order_by_asc(entity::Column::Id)
        .limit(batch_size);
    let candidates = if txn.get_database_backend() == DbBackend::Postgres {
        candidates.lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
    } else { candidates };
    let candidate_ids = candidates.into_tuple::<Uuid>().all(&txn).await.map_err(database_error)?;
    if candidate_ids.is_empty() {
        txn.commit().await.map_err(database_error)?;
        return Ok(0);
    }
    // Check the clock again after the policy and candidate row locks.
    let cutoff = cutoff.min(crate::database_now(&txn).await.map_err(database_error)?
        .checked_sub_signed(older_than).ok_or_else(|| PortError::validation(
            "outbox.operation_receipt_retention_invalid", "operation receipt retention cutoff is out of range"))?.fixed_offset());
    let removed = entity::Entity::delete_many()
        .filter(entity::Column::Id.is_in(candidate_ids))
        .filter(entity::Column::Status.is_in([STATUS_COMPLETED, STATUS_FAILED]))
        .filter(entity::Column::CompletedAt.lte(cutoff))
        .exec(&txn)
        .await
        .map_err(database_error)?;
    txn.commit().await.map_err(database_error)?;
    Ok(removed.rows_affected)
}

async fn inspect_or_reclaim(
    database: &DatabaseConnection,
    existing: entity::Model,
    operation: &str,
    request_hash: &str,
) -> Result<Admission, PortError> {
    if existing.operation != operation || existing.request_hash != request_hash {
        return Err(PortError::conflict(
            "outbox.operation_receipt_conflict",
            "the idempotency key is already bound to a different owner request",
        ));
    }

    match existing.status.as_str() {
        STATUS_COMPLETED => existing
            .response_json
            .map(Admission::Replay)
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "outbox.operation_receipt_corrupt",
                    "completed owner operation receipt has no response",
                )
            }),
        STATUS_FAILED => {
            let value = existing.error_json.ok_or_else(|| {
                PortError::invariant_violation(
                    "outbox.operation_receipt_corrupt",
                    "failed owner operation receipt has no error",
                )
            })?;
            serde_json::from_value(value)
                .map(Admission::ReplayError)
                .map_err(|error| {
                    PortError::invariant_violation(
                        "outbox.operation_receipt_corrupt",
                        error.to_string(),
                    )
                })
        }
        STATUS_PROCESSING | STATUS_RELEASED => {
            let stale_before = crate::observe_database_clock(database)
                .await.map_err(database_error)?.fixed_offset()
                .checked_sub_signed(Duration::minutes(PROCESSING_LEASE_MINUTES))
                .ok_or_else(|| database_error(DbErr::Custom("receipt lease cutoff is out of range".to_string())))?;
            if existing.status == STATUS_PROCESSING && existing.updated_at > stale_before {
                return Err(PortError::unavailable(
                    "outbox.operation_receipt_in_progress",
                    "the owner operation for this idempotency key is still processing",
                ));
            }

            let txn = database.begin().await.map_err(database_error)?;
            let previous = Lease { operation_id: existing.id, token: existing.lease_token };
            let Some((locked, now)) = lock_receipt(&txn, previous, existing.status.as_str()).await? else {
                txn.rollback().await.map_err(database_error)?;
                return Err(lease_lost());
            };
            if locked != existing
                || locked.updated_at > now
                || locked.response_json.is_some()
                || locked.error_json.is_some()
                || locked.completed_at.is_some()
                || (locked.status == STATUS_PROCESSING && lease_is_live(locked.updated_at, now))
            {
                txn.rollback().await.map_err(database_error)?;
                return Err(PortError::unavailable(
                    "outbox.operation_receipt_in_progress",
                    "receipt expiry changed before reclaim",
                ));
            }
            let lease_token = generate_id();
            let claim = entity::Entity::update_many()
                .col_expr(entity::Column::Status, sea_orm::sea_query::Expr::value(STATUS_PROCESSING))
                .col_expr(
                    entity::Column::LeaseToken,
                    sea_orm::sea_query::Expr::value(lease_token),
                )
                .col_expr(
                    entity::Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(now),
                )
                .filter(entity::Column::Id.eq(existing.id))
                .filter(entity::Column::Status.eq(existing.status.as_str()))
                .filter(entity::Column::UpdatedAt.eq(existing.updated_at))
                .filter(entity::Column::LeaseToken.eq(existing.lease_token))
                .exec(&txn)
                .await
                .map_err(database_error)?;
            if claim.rows_affected == 1 {
                let mut expected = locked;
                expected.status = STATUS_PROCESSING.to_string();
                expected.lease_token = lease_token;
                expected.updated_at = now;
                verify_receipt_state(&txn, &expected).await?;
                txn.commit().await.map_err(database_error)?;
                Ok(Admission::Run(Lease {
                    operation_id: existing.id,
                    token: lease_token,
                }))
            } else {
                txn.rollback().await.map_err(database_error)?;
                Err(PortError::unavailable(
                    "outbox.operation_receipt_in_progress",
                    "the owner operation was reclaimed by another worker",
                ))
            }
        }
        _ => Err(PortError::invariant_violation(
            "outbox.operation_receipt_corrupt",
            "owner operation receipt has an unknown state",
        )),
    }
}

async fn verify_receipt_state(
    txn: &DatabaseTransaction,
    expected: &entity::Model,
) -> Result<(), PortError> {
    let saved = entity::Entity::find_by_id(expected.id)
        .one(txn).await.map_err(database_error)?;
    if saved.as_ref() != Some(expected) {
        return Err(PortError::unavailable(
            "outbox.operation_receipt_state_invalid",
            "owner operation receipt transition readback mismatch",
        ));
    }
    Ok(())
}

async fn find(
    database: &DatabaseConnection,
    scope_key: &str,
    owner_slug: &str,
    idempotency_key: &str,
) -> Result<Option<entity::Model>, PortError> {
    entity::Entity::find()
        .filter(entity::Column::ScopeKey.eq(scope_key))
        .filter(entity::Column::OwnerSlug.eq(owner_slug))
        .filter(entity::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await
        .map_err(database_error)
}

fn validate_identity(value: &str, field: &'static str) -> Result<String, PortError> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_IDENTITY_LENGTH {
        return Err(PortError::validation(
            "outbox.operation_receipt_identity_invalid",
            format!("{field} must contain 1 to {MAX_IDENTITY_LENGTH} bytes"),
        ));
    }
    Ok(value.to_string())
}

fn request_hash<T: Serialize>(
    owner_slug: &str,
    operation: &str,
    scope_key: &str,
    request: &T,
) -> Result<String, PortError> {
    let request = serde_json::to_value(request).map_err(|error| {
        PortError::validation(
            "outbox.operation_receipt_request_invalid",
            error.to_string(),
        )
    })?;
    let value = serde_json::json!({
        "owner_slug": owner_slug,
        "operation": operation,
        "scope_key": scope_key,
        "request": canonical_json(&request, 0)?,
    });
    let bytes = serde_json::to_vec(&value).map_err(|error| {
        PortError::validation(
            "outbox.operation_receipt_request_invalid",
            error.to_string(),
        )
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

/// Deepest request shape that gets fingerprinted. Anything deeper is a
/// caller bug (or an attempt to exhaust the stack through recursion), and
/// is rejected instead of being hashed.
const MAX_REQUEST_HASH_DEPTH: usize = 64;

fn canonical_json(value: &Value, depth: usize) -> Result<Value, PortError> {
    if depth > MAX_REQUEST_HASH_DEPTH {
        return Err(PortError::validation(
            "outbox.operation_receipt_request_invalid",
            "owner operation request is nested too deeply to fingerprint",
        ));
    }
    Ok(match value {
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| canonical_json(value, depth + 1))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            let mut output = serde_json::Map::new();
            for key in keys {
                output.insert(key.clone(), canonical_json(&values[key], depth + 1)?);
            }
            Value::Object(output)
        }
        // Numerically equal values must fingerprint identically regardless
        // of the client's JSON spelling: `1` and `1.0` are the same
        // request, and rejecting the replay as a key conflict would be a
        // false positive. Integral floats inside the exact integer range
        // therefore canonicalise to integers. (Receipts stored before this
        // canonicalisation keep their original hash; only requests that
        // actually mix spellings across retries observe a difference.)
        Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                value.clone()
            } else {
                match number.as_f64() {
                    // 2^53 bounds the range where an f64 names one exact integer.
                    Some(float)
                        if float.is_finite()
                            && float.fract().abs().to_bits() == 0
                            && float.abs() < 9_007_199_254_740_992.0 =>
                    {
                        Value::Number((float as i64).into())
                    }
                    _ => value.clone(),
                }
            }
        }
        value => value.clone(),
    })
}

fn database_error(error: sea_orm::DbErr) -> PortError {
    PortError::unavailable("outbox.operation_receipt_database", error.to_string())
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use crate::SysEventsMigration;

    #[tokio::test]
    async fn receipt_schema_probe_requires_migrations() {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        assert!(verify_readable_schema(&database).await.is_err());
        SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .unwrap();
        verify_readable_schema(&database).await.unwrap();
    }

    #[test]
    fn request_hash_ignores_key_order_and_number_spelling() {
        let scope = OwnerOperationScope::Platform;
        let first = request_hash(
            "test-owner",
            "test-op",
            &scope.scope_key(),
            &serde_json::json!({"amount": 1, "nested": {"b": 2, "a": [1.0, 2]}}),
        )
        .expect("hash");
        let second = request_hash(
            "test-owner",
            "test-op",
            &scope.scope_key(),
            &serde_json::json!({"nested": {"a": [1, 2.0], "b": 2.0}, "amount": 1.0}),
        )
        .expect("hash");
        assert_eq!(first, second);

        let different = request_hash(
            "test-owner",
            "test-op",
            &scope.scope_key(),
            &serde_json::json!({"amount": 1.5}),
        )
        .expect("hash");
        assert_ne!(first, different);
    }

    #[test]
    fn caller_key_scope_is_stable_and_binds_command_identity() {
        let tenant_id = Uuid::from_u128(0x11111111111111111111111111111111);
        let actor_id = Uuid::from_u128(0x22222222222222222222222222222222);
        let subject_id = Uuid::from_u128(0x33333333333333333333333333333333);

        let key = scoped_caller_idempotency_key(
            "commerce-graphql-relation",
            tenant_id,
            actor_id,
            "remove_product_relation",
            Some(subject_id),
            " retry-123 ",
        );
        assert_eq!(
            key,
            "commerce-graphql-relation:remove_product_relation:ec63497e3c9f7f42c6b0cb5d2d663f02e74f7a7d6dad063ea507aef29ef8c5c0",
        );
        assert_eq!(
            key,
            scoped_caller_idempotency_key(
                "commerce-graphql-relation",
                tenant_id,
                actor_id,
                "remove_product_relation",
                Some(subject_id),
                "retry-123",
            )
        );
        assert_ne!(
            key,
            scoped_caller_idempotency_key(
                "commerce-graphql-relation",
                tenant_id,
                actor_id,
                "reorder_product_relations",
                Some(subject_id),
                "retry-123",
            )
        );
        assert_ne!(
            key,
            scoped_caller_idempotency_key(
                "commerce-graphql-relation",
                tenant_id,
                Uuid::from_u128(0x44444444444444444444444444444444),
                "remove_product_relation",
                Some(subject_id),
                "retry-123",
            )
        );

        // Without a presence tag and field boundaries these two inputs serialize to the same byte
        // stream: `subject_id` is sixteen ASCII `A` bytes and is otherwise indistinguishable from
        // the leading caller-key bytes. A scoped key must never alias distinct owner subjects.
        let boundary_subject = Uuid::from_bytes(*b"AAAAAAAAAAAAAAAA");
        assert_ne!(
            scoped_caller_idempotency_key(
                "commerce-graphql-relation",
                tenant_id,
                actor_id,
                "remove_product_relation",
                Some(boundary_subject),
                "x",
            ),
            scoped_caller_idempotency_key(
                "commerce-graphql-relation",
                tenant_id,
                actor_id,
                "remove_product_relation",
                None,
                "AAAAAAAAAAAAAAAAx",
            ),
        );
    }

    #[tokio::test]
    async fn terminal_receipt_pruning_rejects_unbounded_direct_requests() {
        let database = Database::connect("sqlite::memory:").await.expect("database");
        for (window, batch) in [
            (Duration::days(3651), 1),
            (Duration::zero(), 1),
            (Duration::days(1), 0),
            (Duration::days(1), crate::MAX_RETENTION_BATCH_SIZE + 1),
        ] {
            let error = prune_terminal_receipts(&database, window, batch)
                .await.expect_err("invalid retention");
            assert_eq!(error.code, "outbox.operation_receipt_retention_invalid");
        }
    }

    #[tokio::test]
    async fn terminal_receipt_pruning_removes_only_aged_terminal_rows() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database should connect");
        SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration should apply");
        let tenant = Uuid::new_v4();
        let request = serde_json::json!({"request": true});
        let lease = match admit(
            &database,
            OwnerOperationScope::Tenant(tenant),
            "test-owner",
            "aged-receipt",
            "test-prune",
            &request,
        )
        .await
        .expect("receipt should admit")
        {
            Admission::Run(lease) => lease,
            _ => panic!("first admission must run"),
        };
        complete(&database, lease, &serde_json::json!({"ok": true}))
            .await
            .expect("receipt should complete");

        // Nothing is old enough yet.
        assert_eq!(
            prune_terminal_receipts(&database, Duration::days(90), 10)
                .await
                .expect("prune"),
            0
        );

        for sql in [
            "DELETE FROM owner_operation_receipts",
            "UPDATE owner_operation_receipts SET completed_at = '2000-01-01T00:00:00+00:00'",
            "UPDATE owner_operation_receipts SET request_hash = 'changed'",
            "UPDATE owner_operation_receipts SET response_json = '{}'",
            "DELETE FROM outbox_owner_receipt_retention_policy",
            "INSERT OR REPLACE INTO outbox_owner_receipt_retention_policy VALUES (1, 1, 1)",
        ] {
            assert!(database.execute_unprepared(sql).await.is_err(), "{sql}");
        }
        assert!(prune_terminal_receipts(&database, Duration::days(1), 10).await.is_err());
        // Seed aged history offline in this isolated fixture only.
        database.execute_unprepared("DROP TRIGGER outbox_owner_receipt_update_guard").await.unwrap();
        let aged = (Utc::now() - Duration::days(120)).fixed_offset();
        entity::Entity::update_many()
            .col_expr(entity::Column::CreatedAt, sea_orm::sea_query::Expr::value(aged))
            .col_expr(entity::Column::UpdatedAt, sea_orm::sea_query::Expr::value(aged))
            .col_expr(
                entity::Column::CompletedAt,
                sea_orm::sea_query::Expr::value(aged),
            )
            .filter(entity::Column::Id.eq(lease.operation_id))
            .exec(&database)
            .await
            .expect("age receipt");
        crate::migration::create_outbox_owner_receipt_integrity(&SchemaManager::new(&database)).await.unwrap();
        assert_eq!(
            prune_terminal_receipts(&database, Duration::days(90), 10)
                .await
                .expect("prune"),
            1
        );
        assert!(matches!(
            admit(
                &database,
                OwnerOperationScope::Tenant(tenant),
                "test-owner",
                "aged-receipt",
                "test-prune",
                &request,
            )
            .await
            .expect("re-admission"),
            Admission::Run(_)
        ));
    }

    #[tokio::test]
    async fn lease_renewal_is_fenced_by_the_lease_token() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database should connect");
        SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration should apply");

        let lease = match admit(
            &database,
            OwnerOperationScope::Tenant(Uuid::new_v4()),
            "test-owner",
            "renewable-lease",
            "test-renewal",
            &serde_json::json!({"request": true}),
        )
        .await
        .expect("receipt should admit")
        {
            Admission::Run(lease) => lease,
            Admission::Replay(_) | Admission::ReplayError(_) => {
                panic!("first admission must run")
            }
        };

        renew_lease(&database, lease)
            .await
            .expect("the lease holder can renew");

        let lost = renew_lease(
            &database,
            Lease {
                operation_id: lease.operation_id,
                token: Uuid::new_v4(),
            },
        )
        .await
        .expect_err("a stale token cannot renew");
        assert_eq!(lost.code, "outbox.operation_receipt_lease_lost");
        assert!(lost.retryable);
    }

    #[test]
    fn receipt_live_interval_is_half_open_and_rejects_future_stamp() {
        let now = Utc::now().fixed_offset();
        assert!(lease_is_live(now, now));
        assert!(lease_is_live(now - Duration::minutes(5) + Duration::milliseconds(1), now));
        assert!(!lease_is_live(now - Duration::minutes(5), now));
        assert!(!lease_is_live(now + Duration::milliseconds(1), now));
    }

    #[tokio::test]
    async fn expired_receipt_cannot_renew_lock_finish_or_erase_fingerprint() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        SysEventsMigration.up(&SchemaManager::new(&db)).await.unwrap();
        let scope = OwnerOperationScope::Tenant(Uuid::new_v4());
        let request = serde_json::json!({"request": true});
        let lease = match admit(&db, scope, "owner", "expired", "write", &request).await.unwrap() {
            Admission::Run(lease) => lease,
            _ => panic!("fresh receipt must run"),
        };
        db.execute_unprepared("DROP TRIGGER outbox_owner_receipt_update_guard").await.unwrap();
        let expired = (Utc::now() - Duration::minutes(6)).fixed_offset();
        entity::Entity::update_many()
            .col_expr(entity::Column::CreatedAt, sea_orm::sea_query::Expr::value(expired))
            .col_expr(entity::Column::UpdatedAt, sea_orm::sea_query::Expr::value(expired))
            .filter(entity::Column::Id.eq(lease.operation_id))
            .exec(&db).await.unwrap();
        crate::migration::create_outbox_owner_receipt_integrity(&SchemaManager::new(&db)).await.unwrap();
        assert!(renew_lease(&db, lease).await.unwrap_err().retryable);
        let txn = db.begin().await.unwrap();
        assert!(lock_current(&txn, lease).await.unwrap_err().retryable);
        txn.rollback().await.unwrap();
        assert!(complete(&db, lease, &()).await.unwrap_err().retryable);
        assert!(fail(&db, lease, &PortError::validation("test.terminal", "terminal")).await.unwrap_err().retryable);
        assert!(release_retryable(&db, lease, &PortError::unavailable("test.retry", "retry")).await.unwrap_err().retryable);
        assert!(matches!(admit(&db, scope, "owner", "expired", "write", &serde_json::json!({"changed": true})).await, Err(error) if !error.retryable));
        let reclaimed = match admit(&db, scope, "owner", "expired", "write", &request).await.unwrap() {
            Admission::Run(lease) => lease,
            _ => panic!("expired receipt must be reclaimed"),
        };
        assert_eq!(reclaimed.operation_id, lease.operation_id);
        assert_ne!(reclaimed.token, lease.token);
        let txn = db.begin().await.unwrap();
        lock_current(&txn, reclaimed).await.unwrap();
        complete(&txn, reclaimed, &()).await.unwrap();
        txn.commit().await.unwrap();
        assert!(matches!(admit(&db, scope, "owner", "expired", "write", &request).await.unwrap(), Admission::Replay(_)));
    }

    #[tokio::test]
    async fn rewritten_receipt_transitions_are_not_acknowledged() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        SysEventsMigration.up(&SchemaManager::new(&db)).await.unwrap();
        let scope = OwnerOperationScope::Tenant(Uuid::new_v4());
        let request = serde_json::json!({"request": true});
        // Isolate application readback from the database mutation guard.
        db.execute_unprepared("DROP TRIGGER outbox_owner_receipt_update_guard").await.unwrap();
        db.execute_unprepared(
            "CREATE TRIGGER rewrite_receipt AFTER INSERT ON owner_operation_receipts
             BEGIN UPDATE owner_operation_receipts SET request_hash = 'changed' WHERE id = NEW.id; END"
        ).await.unwrap();
        assert!(matches!(admit(&db, scope, "owner", "key", "write", &request).await,
            Err(error) if error.retryable));
        assert!(entity::Entity::find().all(&db).await.unwrap().is_empty());
        db.execute_unprepared("DROP TRIGGER rewrite_receipt").await.unwrap();

        let lease = match admit(&db, scope, "owner", "key", "write", &request).await.unwrap() {
            Admission::Run(lease) => lease,
            _ => panic!("fresh receipt must run"),
        };
        let before = entity::Entity::find_by_id(lease.operation_id).one(&db).await.unwrap().unwrap();
        for (kind, trigger) in [
            ("renew", "CREATE TRIGGER rewrite_receipt AFTER UPDATE OF updated_at ON owner_operation_receipts WHEN NEW.status = 'processing' BEGIN UPDATE owner_operation_receipts SET request_hash = 'changed' WHERE id = NEW.id; END"),
            ("complete", "CREATE TRIGGER rewrite_receipt AFTER UPDATE OF status ON owner_operation_receipts WHEN NEW.status = 'completed' BEGIN UPDATE owner_operation_receipts SET response_json = '{}' WHERE id = NEW.id; END"),
            ("fail", "CREATE TRIGGER rewrite_receipt AFTER UPDATE OF status ON owner_operation_receipts WHEN NEW.status = 'failed' BEGIN UPDATE owner_operation_receipts SET error_json = '{}' WHERE id = NEW.id; END"),
        ] {
            db.execute_unprepared(trigger).await.unwrap();
            let result = match kind {
                "renew" => renew_lease(&db, lease).await,
                "complete" => {
                    let txn = db.begin().await.unwrap();
                    let result = complete(&txn, lease, &serde_json::json!({"ok": true})).await;
                    // The nested receipt savepoint must itself be rolled back.
                    assert_eq!(entity::Entity::find_by_id(lease.operation_id).one(&txn).await.unwrap(), Some(before.clone()));
                    txn.rollback().await.unwrap();
                    result
                }
                _ => fail(&db, lease, &PortError::validation("test.terminal", "terminal")).await,
            };
            let error = result.expect_err("rewritten transition cannot report success");
            assert!(error.retryable);
            assert_eq!(entity::Entity::find_by_id(lease.operation_id).one(&db).await.unwrap(), Some(before.clone()));
            db.execute_unprepared("DROP TRIGGER rewrite_receipt").await.unwrap();
        }
        crate::migration::create_outbox_owner_receipt_integrity(&SchemaManager::new(&db)).await.unwrap();
        complete(&db, lease, &serde_json::json!({"ok": true})).await.unwrap();
        assert!(matches!(admit(&db, scope, "owner", "key", "write", &request).await.unwrap(),
            Admission::Replay(value) if value == serde_json::json!({"ok": true})));
    }

    #[tokio::test]
    async fn receipt_completion_rejects_a_lost_lease_retryably() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database should connect");
        SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration should apply");

        let lease = match admit(
            &database,
            OwnerOperationScope::Tenant(Uuid::new_v4()),
            "test-owner",
            "lost-completion-lease",
            "test-completion",
            &serde_json::json!({"request": true}),
        )
        .await
        .expect("receipt should admit")
        {
            Admission::Run(lease) => lease,
            Admission::Replay(_) | Admission::ReplayError(_) => {
                panic!("first admission must run")
            }
        };
        let error = complete(
            &database,
            Lease {
                operation_id: lease.operation_id,
                token: Uuid::new_v4(),
            },
            &serde_json::json!({"ok": true}),
        )
        .await
        .expect_err("completion must reject a stale lease token");

        assert_eq!(error.kind, rustok_api::PortErrorKind::Unavailable);
        assert_eq!(error.code, "outbox.operation_receipt_state_invalid");
        assert!(
            error.retryable,
            "lost receipt ownership leaves the command outcome unknown and retryable"
        );
    }

    #[tokio::test]
    async fn failed_receipt_finalization_rejects_a_lost_lease() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database should connect");
        SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration should apply");

        let error = fail(
            &database,
            Lease {
                operation_id: Uuid::new_v4(),
                token: Uuid::new_v4(),
            },
            &PortError::validation("test.terminal", "terminal test failure"),
        )
        .await
        .expect_err("failure finalization must reject an unknown or reclaimed lease");

        assert_eq!(error.code, "outbox.operation_receipt_state_invalid");
        assert!(
            error.retryable,
            "lost receipt ownership must remain retryable"
        );
    }

    #[tokio::test]
    async fn retryable_release_re_admits_same_command_without_erasing_a_committed_result() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database should connect");
        SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration should apply");
        let tenant = Uuid::new_v4();
        let request = serde_json::json!({"actor": "author", "post_id": Uuid::new_v4()});
        let first = match admit(
            &database,
            OwnerOperationScope::Tenant(tenant),
            "comments",
            "stable-command",
            "create_comment",
            &request,
        )
        .await
        .expect("first admission")
        {
            Admission::Run(lease) => lease,
            _ => panic!("first request must run"),
        };

        let retryable = PortError::unavailable("comments.database", "temporary outage");
        let invalid_finalization = fail(&database, first, &retryable)
            .await
            .expect_err("temporary failures must not become terminal replay results");
        assert_eq!(invalid_finalization.code, "outbox.operation_receipt_fail_retryable");
        let stored = entity::Entity::find_by_id(first.operation_id)
            .one(&database).await.unwrap().unwrap();
        assert_eq!(stored.status, STATUS_PROCESSING);
        assert_eq!(stored.lease_token, first.token);
        assert!(stored.error_json.is_none());
        assert!(stored.completed_at.is_none());

        let lost = release_retryable(
            &database,
            Lease {
                operation_id: first.operation_id,
                token: Uuid::new_v4(),
            },
            &PortError::unavailable("comments.timeout", "temporary outage"),
        )
        .await
        .expect_err("stale worker cannot release another lease");
        assert!(lost.retryable);

        let terminal = release_retryable(
            &database,
            first,
            &PortError::validation("comments.bad_input", "invalid"),
        )
        .await
        .expect_err("nonretryable error must be persisted, not released");
        assert_eq!(terminal.code, "outbox.operation_receipt_release_terminal");

        release_retryable(
            &database,
            first,
            &PortError::unavailable("comments.database", "retryable"),
        )
        .await
        .expect("temporary failure releases the lease");

        let released = entity::Entity::find_by_id(first.operation_id)
            .one(&database).await.unwrap().unwrap();
        assert_eq!(released.status, STATUS_RELEASED);
        for sql in [
            "DELETE FROM owner_operation_receipts",
            "UPDATE owner_operation_receipts SET request_hash = 'changed'",
            "UPDATE owner_operation_receipts SET owner_slug = 'other'",
            "UPDATE owner_operation_receipts SET rowid = rowid + 100",
        ] {
            assert!(database.execute_unprepared(sql).await.is_err(), "{sql}");
        }
        database.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        let ignored = database.execute_unprepared(
            "INSERT OR REPLACE INTO owner_operation_receipts SELECT id, tenant_id, scope_key, owner_slug, idempotency_key, operation, request_hash, lease_token, 'processing', NULL, NULL, created_at, created_at, NULL FROM owner_operation_receipts"
        ).await.unwrap();
        assert_eq!(ignored.rows_affected(), 0);
        assert_eq!(entity::Entity::find_by_id(first.operation_id).one(&database).await.unwrap(), Some(released.clone()));
        assert_eq!(released.request_hash, stored.request_hash);
        assert_ne!(released.lease_token, first.token);
        assert!(released.completed_at.is_none());
        assert!(renew_lease(&database, first).await.is_err());
        assert!(complete(&database, first, &serde_json::json!({"stale": true})).await.is_err());
        assert!(release_retryable(&database, first, &retryable).await.is_err());
        let conflict = match admit(
            &database, OwnerOperationScope::Tenant(tenant), "comments",
            "stable-command", "create_comment", &serde_json::json!({"different": true}),
        ).await {
            Err(error) => error,
            Ok(_) => panic!("released request fingerprint must remain reserved"),
        };
        assert_eq!(conflict.code, "outbox.operation_receipt_conflict");
        assert_eq!(prune_terminal_receipts(&database, Duration::milliseconds(1), 10).await.unwrap(), 0);

        let second = match admit(
            &database,
            OwnerOperationScope::Tenant(tenant),
            "comments",
            "stable-command",
            "create_comment",
            &request,
        )
        .await
        .expect("retry admission")
        {
            Admission::Run(lease) => lease,
            _ => panic!("released command must run again"),
        };
        assert_eq!(second.operation_id, first.operation_id);
        assert_ne!(second.token, first.token);
        assert_ne!(second.token, released.lease_token);
        assert!(complete(&database, first, &serde_json::json!({"stale": true})).await.is_err());
        complete(&database, second, &serde_json::json!({"comment": "ok"}))
            .await
            .expect("commit receipt");

        let uncertain = release_retryable(
            &database,
            second,
            &PortError::unavailable("comments.commit", "unknown outcome"),
        )
        .await
        .expect_err("committed receipt must never be removed");
        assert!(uncertain.retryable);
        assert!(matches!(
            admit(
                &database, OwnerOperationScope::Tenant(tenant), "comments",
                "stable-command", "create_comment", &request,
            ).await.expect("replay"),
            Admission::Replay(value) if value == serde_json::json!({"comment": "ok"})
        ));
    }

    #[tokio::test]
    async fn completed_receipts_replay_and_keys_remain_owner_scoped() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database should connect");
        SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration should apply");
        let tenant_id = Uuid::new_v4();
        let request = serde_json::json!({"translation": "one"});
        let lease = match admit(
            &database,
            OwnerOperationScope::Tenant(tenant_id),
            "media",
            "owner-receipt-1",
            "apply_translation",
            &request,
        )
        .await
        .expect("receipt should admit")
        {
            Admission::Run(lease) => lease,
            Admission::Replay(_) | Admission::ReplayError(_) => {
                panic!("first admission must run")
            }
        };
        complete(&database, lease, &serde_json::json!({"ok": true}))
            .await
            .expect("receipt should complete");

        let replay = admit(
            &database,
            OwnerOperationScope::Tenant(tenant_id),
            "media",
            "owner-receipt-1",
            "apply_translation",
            &request,
        )
        .await
        .expect("receipt should replay");
        assert!(
            matches!(replay, Admission::Replay(value) if value == serde_json::json!({"ok": true}))
        );

        let distinct_owner = admit(
            &database,
            OwnerOperationScope::Tenant(tenant_id),
            "taxonomy",
            "owner-receipt-1",
            "apply_translation",
            &request,
        )
        .await
        .expect("owners should have independent idempotency namespaces");
        assert!(matches!(distinct_owner, Admission::Run(_)));
    }
}
