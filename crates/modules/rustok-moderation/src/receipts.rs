use chrono::Utc;
use rustok_api::{PortActor, PortContext};
use rustok_core::generate_id;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait, sea_query::OnConflict,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::moderation_receipt;
use crate::error::{ModerationError, ModerationResult};

const MAX_IDEMPOTENCY_KEY_BYTES: usize = 191;
const STATUS_PROCESSING: &str = "processing";
const STATUS_COMPLETED: &str = "completed";

pub(crate) struct NewModerationReceipt {
    pub transaction: DatabaseTransaction,
    pub receipt_id: Uuid,
    pub tenant_id: Uuid,
    operation_kind: String,
}

pub(crate) enum ModerationReceiptAdmission {
    Replay(moderation_receipt::Model),
    New(NewModerationReceipt),
}

pub(crate) fn required_idempotency_key(context: &PortContext) -> ModerationResult<String> {
    normalize_idempotency_key(
        context.idempotency_key.clone().ok_or_else(|| {
            ModerationError::Validation("idempotency key is required".to_string())
        })?,
    )
}

pub(crate) fn normalize_idempotency_key(value: impl Into<String>) -> ModerationResult<String> {
    let value = value.into().trim().to_string();
    if value.is_empty() || value.len() > MAX_IDEMPOTENCY_KEY_BYTES {
        return Err(ModerationError::Validation(format!(
            "idempotency key must contain 1 to {MAX_IDEMPOTENCY_KEY_BYTES} bytes"
        )));
    }
    Ok(value)
}

pub(crate) fn request_hash<T: Serialize>(
    operation_kind: &str,
    actor: &PortActor,
    request: &T,
) -> ModerationResult<String> {
    let request = serde_json::to_value(request).map_err(|_| {
        ModerationError::Validation("moderation command could not be normalized".to_string())
    })?;
    let payload = serde_json::json!({
        "version": 1,
        "operation_kind": operation_kind,
        "actor": actor,
        "request": canonical_json(&request),
    });
    let encoded = serde_json::to_vec(&payload).map_err(|_| {
        ModerationError::Validation("moderation command could not be hashed".to_string())
    })?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

pub(crate) async fn replay_existing<R: DeserializeOwned>(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    operation_kind: &str,
    idempotency_key: &str,
    expected_hash: &str,
) -> ModerationResult<Option<R>> {
    match find_receipt(db, tenant_id, operation_kind, idempotency_key).await? {
        Some(receipt) => replay(receipt, operation_kind, expected_hash).map(Some),
        None => Ok(None),
    }
}

pub(crate) async fn admit(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    operation_kind: &str,
    idempotency_key: String,
    hash: &str,
) -> ModerationResult<ModerationReceiptAdmission> {
    if let Some(existing) =
        find_receipt(db, tenant_id, operation_kind, idempotency_key.as_str()).await?
    {
        return Ok(ModerationReceiptAdmission::Replay(existing));
    }

    let transaction = db.begin().await?;
    let receipt_id = generate_id();
    let now = Utc::now();
    moderation_receipt::Entity::insert(moderation_receipt::ActiveModel {
        id: Set(receipt_id),
        tenant_id: Set(tenant_id),
        operation_kind: Set(operation_kind.to_string()),
        idempotency_key: Set(idempotency_key.clone()),
        request_hash: Set(hash.to_string()),
        status: Set(STATUS_PROCESSING.to_string()),
        response_json: Set(None),
        error_code: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        completed_at: Set(None),
    })
    .on_conflict(
        OnConflict::columns([
            moderation_receipt::Column::TenantId,
            moderation_receipt::Column::OperationKind,
            moderation_receipt::Column::IdempotencyKey,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(&transaction)
    .await?;

    let stored = find_receipt_in(
        &transaction,
        tenant_id,
        operation_kind,
        idempotency_key.as_str(),
    )
    .await?
    .ok_or_else(|| {
        ModerationError::Invariant(
            "receipt admission completed without a readable receipt".to_string(),
        )
    })?;

    if stored.id != receipt_id {
        transaction.rollback().await?;
        return Ok(ModerationReceiptAdmission::Replay(stored));
    }

    Ok(ModerationReceiptAdmission::New(NewModerationReceipt {
        transaction,
        receipt_id,
        tenant_id,
        operation_kind: operation_kind.to_string(),
    }))
}

pub(crate) fn replay<R: DeserializeOwned>(
    receipt: moderation_receipt::Model,
    expected_operation_kind: &str,
    expected_hash: &str,
) -> ModerationResult<R> {
    if receipt.operation_kind != expected_operation_kind || receipt.request_hash != expected_hash {
        return Err(ModerationError::IdempotencyConflict);
    }
    if receipt.status != STATUS_COMPLETED || receipt.completed_at.is_none() {
        return Err(ModerationError::CommandReceiptCorrupt);
    }
    let response = receipt
        .response_json
        .ok_or(ModerationError::CommandReceiptCorrupt)?;
    serde_json::from_value(response).map_err(|_| ModerationError::CommandReceiptCorrupt)
}

pub(crate) async fn complete<R>(receipt: NewModerationReceipt, response: &R) -> ModerationResult<R>
where
    R: Clone + Serialize,
{
    let response_json = serde_json::to_value(response).map_err(|_| {
        ModerationError::Invariant("moderation response could not be serialized".to_string())
    })?;
    let now = Utc::now().fixed_offset();
    let result = moderation_receipt::Entity::update_many()
        .col_expr(
            moderation_receipt::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_COMPLETED),
        )
        .col_expr(
            moderation_receipt::Column::ResponseJson,
            sea_orm::sea_query::Expr::value(Some(response_json)),
        )
        .col_expr(
            moderation_receipt::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .col_expr(
            moderation_receipt::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .filter(moderation_receipt::Column::Id.eq(receipt.receipt_id))
        .filter(moderation_receipt::Column::TenantId.eq(receipt.tenant_id))
        .filter(moderation_receipt::Column::OperationKind.eq(receipt.operation_kind.as_str()))
        .filter(moderation_receipt::Column::Status.eq(STATUS_PROCESSING))
        .exec(&receipt.transaction)
        .await?;
    if result.rows_affected != 1 {
        receipt.transaction.rollback().await?;
        return Err(ModerationError::CommandReceiptCorrupt);
    }
    receipt.transaction.commit().await?;
    Ok(response.clone())
}

pub(crate) async fn rollback<T>(
    receipt: NewModerationReceipt,
    error: ModerationError,
) -> ModerationResult<T> {
    receipt.transaction.rollback().await?;
    Err(error)
}

async fn find_receipt<C>(
    connection: &C,
    tenant_id: Uuid,
    operation_kind: &str,
    idempotency_key: &str,
) -> ModerationResult<Option<moderation_receipt::Model>>
where
    C: ConnectionTrait,
{
    moderation_receipt::Entity::find()
        .filter(moderation_receipt::Column::TenantId.eq(tenant_id))
        .filter(moderation_receipt::Column::OperationKind.eq(operation_kind))
        .filter(moderation_receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(connection)
        .await
        .map_err(Into::into)
}

async fn find_receipt_in(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_kind: &str,
    idempotency_key: &str,
) -> ModerationResult<Option<moderation_receipt::Model>> {
    find_receipt(transaction, tenant_id, operation_kind, idempotency_key).await
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

#[cfg(test)]
mod tests {
    use rustok_api::PortActor;

    use super::*;

    #[test]
    fn command_hash_is_stable_for_object_key_order() {
        let left = request_hash(
            "submit_report",
            &PortActor::user(Uuid::nil().to_string()),
            &serde_json::json!({"metadata": {"b": 2, "a": 1}}),
        )
        .unwrap();
        let right = request_hash(
            "submit_report",
            &PortActor::user(Uuid::nil().to_string()),
            &serde_json::json!({"metadata": {"a": 1, "b": 2}}),
        )
        .unwrap();

        assert_eq!(left, right);
        assert_eq!(left.len(), 64);
        assert_eq!(left, left.to_ascii_lowercase());
    }
}
