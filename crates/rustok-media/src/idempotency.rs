use chrono::{Duration, Utc};
use rustok_api::PortError;
use rustok_core::generate_id;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::port_operation;

const STATUS_PROCESSING: &str = "processing";
const STATUS_COMPLETED: &str = "completed";
const STATUS_FAILED: &str = "failed";
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 191;
const PROCESSING_LEASE_MINUTES: i64 = 5;

pub(crate) enum Admission {
    Run(OperationLease),
    Replay(Value),
    ReplayError(PortError),
}

#[derive(Clone, Copy)]
pub(crate) struct OperationLease {
    pub operation_id: Uuid,
    pub token: Uuid,
}

pub(crate) async fn admit<T: Serialize>(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: &str,
    operation: &str,
    request: &T,
) -> Result<Admission, PortError> {
    let idempotency_key = idempotency_key.trim();
    if idempotency_key.is_empty() || idempotency_key.len() > MAX_IDEMPOTENCY_KEY_LENGTH {
        return Err(PortError::validation(
            "media.idempotency_key_invalid",
            format!("media idempotency key must contain 1 to {MAX_IDEMPOTENCY_KEY_LENGTH} bytes"),
        ));
    }
    let request_hash = request_hash(operation, tenant_id, request)?;
    if let Some(existing) = find(db, tenant_id, idempotency_key).await? {
        return inspect_or_reclaim(db, existing, operation, &request_hash).await;
    }

    let now = Utc::now().fixed_offset();
    let id = generate_id();
    let lease_token = generate_id();
    let insert = port_operation::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        idempotency_key: Set(idempotency_key.to_string()),
        operation: Set(operation.to_string()),
        request_hash: Set(request_hash.clone()),
        lease_token: Set(lease_token),
        status: Set(STATUS_PROCESSING.to_string()),
        response_json: Set(None),
        error_json: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        completed_at: Set(None),
    }
    .insert(db)
    .await;
    match insert {
        Ok(_) => Ok(Admission::Run(OperationLease {
            operation_id: id,
            token: lease_token,
        })),
        Err(error) if is_unique_constraint(&error) => {
            let existing = find(db, tenant_id, idempotency_key)
                .await?
                .ok_or_else(|| database_error(error))?;
            inspect_or_reclaim(db, existing, operation, &request_hash).await
        }
        Err(error) => Err(database_error(error)),
    }
}

pub(crate) async fn complete<T: Serialize>(
    db: &DatabaseConnection,
    lease: OperationLease,
    response: &T,
) -> Result<(), PortError> {
    let response = serde_json::to_value(response).map_err(|error| {
        PortError::invariant_violation("media.idempotency_encode", error.to_string())
    })?;
    let now = Utc::now().fixed_offset();
    let update = port_operation::Entity::update_many()
        .col_expr(
            port_operation::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_COMPLETED),
        )
        .col_expr(
            port_operation::Column::ResponseJson,
            sea_orm::sea_query::Expr::value(Some(response)),
        )
        .col_expr(
            port_operation::Column::ErrorJson,
            sea_orm::sea_query::Expr::value(Option::<Value>::None),
        )
        .col_expr(
            port_operation::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .col_expr(
            port_operation::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .filter(port_operation::Column::Id.eq(lease.operation_id))
        .filter(port_operation::Column::LeaseToken.eq(lease.token))
        .filter(port_operation::Column::Status.eq(STATUS_PROCESSING))
        .exec(db)
        .await
        .map_err(database_error)?;
    if update.rows_affected != 1 {
        return Err(PortError::invariant_violation(
            "media.idempotency_state_invalid",
            "media operation receipt was not processing during completion",
        ));
    }
    Ok(())
}

pub(crate) async fn fail(
    db: &DatabaseConnection,
    lease: OperationLease,
    error: &PortError,
) -> Result<(), PortError> {
    let error_json = serde_json::to_value(error).map_err(|encoding_error| {
        PortError::invariant_violation("media.idempotency_encode", encoding_error.to_string())
    })?;
    let now = Utc::now().fixed_offset();
    port_operation::Entity::update_many()
        .col_expr(
            port_operation::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_FAILED),
        )
        .col_expr(
            port_operation::Column::ErrorJson,
            sea_orm::sea_query::Expr::value(Some(error_json)),
        )
        .col_expr(
            port_operation::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .col_expr(
            port_operation::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .filter(port_operation::Column::Id.eq(lease.operation_id))
        .filter(port_operation::Column::LeaseToken.eq(lease.token))
        .filter(port_operation::Column::Status.eq(STATUS_PROCESSING))
        .exec(db)
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn inspect_or_reclaim(
    db: &DatabaseConnection,
    existing: port_operation::Model,
    operation: &str,
    request_hash: &str,
) -> Result<Admission, PortError> {
    if existing.operation != operation || existing.request_hash != request_hash {
        return Err(PortError::conflict(
            "media.idempotency_conflict",
            "the idempotency key is already bound to a different media request",
        ));
    }
    match existing.status.as_str() {
        STATUS_COMPLETED => existing
            .response_json
            .map(Admission::Replay)
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "media.idempotency_receipt_corrupt",
                    "completed media operation receipt has no response",
                )
            }),
        STATUS_FAILED => {
            let value = existing.error_json.ok_or_else(|| {
                PortError::invariant_violation(
                    "media.idempotency_receipt_corrupt",
                    "failed media operation receipt has no error",
                )
            })?;
            serde_json::from_value(value)
                .map(Admission::ReplayError)
                .map_err(|error| {
                    PortError::invariant_violation(
                        "media.idempotency_receipt_corrupt",
                        error.to_string(),
                    )
                })
        }
        STATUS_PROCESSING => {
            let stale_before =
                Utc::now().fixed_offset() - Duration::minutes(PROCESSING_LEASE_MINUTES);
            if existing.updated_at > stale_before {
                return Err(PortError::unavailable(
                    "media.idempotency_in_progress",
                    "the media operation for this idempotency key is still processing",
                ));
            }
            let now = Utc::now().fixed_offset();
            let lease_token = generate_id();
            let claim = port_operation::Entity::update_many()
                .col_expr(
                    port_operation::Column::LeaseToken,
                    sea_orm::sea_query::Expr::value(lease_token),
                )
                .col_expr(
                    port_operation::Column::UpdatedAt,
                    sea_orm::sea_query::Expr::value(now),
                )
                .filter(port_operation::Column::Id.eq(existing.id))
                .filter(port_operation::Column::Status.eq(STATUS_PROCESSING))
                .filter(port_operation::Column::UpdatedAt.eq(existing.updated_at))
                .exec(db)
                .await
                .map_err(database_error)?;
            if claim.rows_affected == 1 {
                Ok(Admission::Run(OperationLease {
                    operation_id: existing.id,
                    token: lease_token,
                }))
            } else {
                Err(PortError::unavailable(
                    "media.idempotency_in_progress",
                    "the media operation was reclaimed by another worker",
                ))
            }
        }
        _ => Err(PortError::invariant_violation(
            "media.idempotency_receipt_corrupt",
            "media operation receipt has an unknown state",
        )),
    }
}

async fn find(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<port_operation::Model>, PortError> {
    port_operation::Entity::find()
        .filter(port_operation::Column::TenantId.eq(tenant_id))
        .filter(port_operation::Column::IdempotencyKey.eq(idempotency_key))
        .one(db)
        .await
        .map_err(database_error)
}

fn request_hash<T: Serialize>(
    operation: &str,
    tenant_id: Uuid,
    request: &T,
) -> Result<String, PortError> {
    let request = serde_json::to_value(request).map_err(|error| {
        PortError::validation("media.idempotency_request_invalid", error.to_string())
    })?;
    let value = serde_json::json!({
        "operation": operation,
        "tenant_id": tenant_id,
        "request": canonical_json(&request),
    });
    let bytes = serde_json::to_vec(&value).map_err(|error| {
        PortError::validation("media.idempotency_request_invalid", error.to_string())
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
    PortError::unavailable("media.idempotency_database", error.to_string())
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
