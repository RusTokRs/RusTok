//! Durable, owner-scoped idempotency receipts for write-like port operations.

use chrono::{Duration, Utc};
use rustok_api::PortError;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const STATUS_PROCESSING: &str = "processing";
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
    let owner_slug = validate_identity(owner_slug, "owner_slug")?;
    let idempotency_key = validate_identity(idempotency_key, "idempotency_key")?;
    let operation = validate_identity(operation, "operation")?;
    let scope_key = scope.scope_key();
    let request_hash = request_hash(&owner_slug, &operation, &scope_key, request)?;

    if let Some(existing) = find(database, &scope_key, &owner_slug, &idempotency_key).await? {
        return inspect_or_reclaim(database, existing, &operation, &request_hash).await;
    }

    let now = Utc::now().fixed_offset();
    let id = generate_id();
    let lease_token = generate_id();
    let insert = entity::ActiveModel {
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
    }
    .insert(database)
    .await;

    match insert {
        Ok(_) => Ok(Admission::Run(Lease {
            operation_id: id,
            token: lease_token,
        })),
        Err(error) if is_unique_constraint(&error) => {
            let existing = find(database, &scope_key, &owner_slug, &idempotency_key)
                .await?
                .ok_or_else(|| database_error(error))?;
            inspect_or_reclaim(database, existing, &operation, &request_hash).await
        }
        Err(error) => Err(database_error(error)),
    }
}

/// Stores a successful response inside the caller's owner transaction.
pub async fn complete<C, T>(connection: &C, lease: Lease, response: &T) -> Result<(), PortError>
where
    C: ConnectionTrait,
    T: Serialize,
{
    let response = serde_json::to_value(response).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_encode", error.to_string())
    })?;
    let now = Utc::now().fixed_offset();
    let update = entity::Entity::update_many()
        .col_expr(
            entity::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_COMPLETED),
        )
        .col_expr(
            entity::Column::ResponseJson,
            sea_orm::sea_query::Expr::value(Some(response)),
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
        .exec(connection)
        .await
        .map_err(database_error)?;

    if update.rows_affected != 1 {
        return Err(PortError::invariant_violation(
            "outbox.operation_receipt_state_invalid",
            "owner operation receipt was not processing during completion",
        ));
    }
    Ok(())
}

/// Stores a terminal failure after the caller's owner transaction has rolled
/// back. A replay returns the same typed failure instead of rerunning work.
pub async fn fail<C>(connection: &C, lease: Lease, error: &PortError) -> Result<(), PortError>
where
    C: ConnectionTrait,
{
    let error_json = serde_json::to_value(error).map_err(|encoding_error| {
        PortError::invariant_violation(
            "outbox.operation_receipt_encode",
            encoding_error.to_string(),
        )
    })?;
    let now = Utc::now().fixed_offset();
    entity::Entity::update_many()
        .col_expr(
            entity::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_FAILED),
        )
        .col_expr(
            entity::Column::ErrorJson,
            sea_orm::sea_query::Expr::value(Some(error_json)),
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
        .exec(connection)
        .await
        .map_err(database_error)?;
    Ok(())
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
        STATUS_PROCESSING => {
            let stale_before =
                Utc::now().fixed_offset() - Duration::minutes(PROCESSING_LEASE_MINUTES);
            if existing.updated_at > stale_before {
                return Err(PortError::unavailable(
                    "outbox.operation_receipt_in_progress",
                    "the owner operation for this idempotency key is still processing",
                ));
            }

            let now = Utc::now().fixed_offset();
            let lease_token = generate_id();
            let claim = entity::Entity::update_many()
                .col_expr(
                    entity::Column::LeaseToken,
                    sea_orm::sea_query::Expr::value(lease_token),
                )
                .col_expr(
                    entity::Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(now),
                )
                .filter(entity::Column::Id.eq(existing.id))
                .filter(entity::Column::Status.eq(STATUS_PROCESSING))
                .filter(entity::Column::UpdatedAt.eq(existing.updated_at))
                .exec(database)
                .await
                .map_err(database_error)?;
            if claim.rows_affected == 1 {
                Ok(Admission::Run(Lease {
                    operation_id: existing.id,
                    token: lease_token,
                }))
            } else {
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
        "request": canonical_json(&request),
    });
    let bytes = serde_json::to_vec(&value).map_err(|error| {
        PortError::validation(
            "outbox.operation_receipt_request_invalid",
            error.to_string(),
        )
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            let mut output = serde_json::Map::new();
            for key in keys {
                output.insert(key.clone(), canonical_json(&values[key]));
            }
            Value::Object(output)
        }
        value => value.clone(),
    }
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
