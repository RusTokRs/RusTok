use chrono::Utc;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::receipt;
use crate::error::{MarketplaceLedgerError, MarketplaceLedgerResult};

const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 191;
const STATUS_PENDING: &str = "pending";
const STATUS_COMPLETED: &str = "completed";
const COMMAND_KIND_POST_ORDER_COMMISSIONS: &str = "post_order_commissions";

pub(crate) struct NewLedgerReceipt {
    pub transaction: DatabaseTransaction,
    pub receipt_id: Uuid,
    pub tenant_id: Uuid,
}

pub(crate) enum LedgerReceiptAdmission {
    Replay(receipt::Model),
    New(NewLedgerReceipt),
}

pub(crate) fn normalize_idempotency_key(
    value: impl Into<String>,
) -> MarketplaceLedgerResult<String> {
    let value = value.into().trim().to_string();
    if value.is_empty() || value.len() > MAX_IDEMPOTENCY_KEY_LENGTH {
        return Err(MarketplaceLedgerError::Validation(format!(
            "idempotency key must contain 1 to {MAX_IDEMPOTENCY_KEY_LENGTH} bytes"
        )));
    }
    Ok(value)
}

pub(crate) fn posting_request_hash<T: Serialize>(
    actor_id: Uuid,
    request: &T,
) -> MarketplaceLedgerResult<String> {
    command_request_hash(COMMAND_KIND_POST_ORDER_COMMISSIONS, actor_id, request)
}

pub(crate) fn command_request_hash<T: Serialize>(
    command_kind: &str,
    actor_id: Uuid,
    request: &T,
) -> MarketplaceLedgerResult<String> {
    let command_kind = normalize_command_kind(command_kind)?;
    let request = serde_json::to_value(request).map_err(|_| {
        MarketplaceLedgerError::Validation(
            "ledger command request could not be normalized".to_string(),
        )
    })?;
    let payload = serde_json::json!({
        "version": 1,
        "command_kind": command_kind,
        "actor_id": actor_id,
        "request": canonical_json(&request),
    });
    let encoded = serde_json::to_vec(&payload).map_err(|_| {
        MarketplaceLedgerError::Validation("ledger command request could not be hashed".to_string())
    })?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

pub(crate) async fn replay_existing<R: DeserializeOwned>(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: &str,
    request_hash: &str,
) -> MarketplaceLedgerResult<Option<R>> {
    replay_existing_command(
        db,
        tenant_id,
        idempotency_key,
        COMMAND_KIND_POST_ORDER_COMMISSIONS,
        request_hash,
    )
    .await
}

pub(crate) async fn replay_existing_command<R: DeserializeOwned>(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: &str,
    command_kind: &str,
    request_hash: &str,
) -> MarketplaceLedgerResult<Option<R>> {
    match find_receipt(db, tenant_id, idempotency_key).await? {
        Some(receipt) => replay_command_receipt(receipt, command_kind, request_hash).map(Some),
        None => Ok(None),
    }
}

pub(crate) async fn admit_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    actor_id: Uuid,
    idempotency_key: String,
    request_hash: &str,
) -> MarketplaceLedgerResult<LedgerReceiptAdmission> {
    admit_command_receipt(
        db,
        tenant_id,
        actor_id,
        idempotency_key,
        COMMAND_KIND_POST_ORDER_COMMISSIONS,
        request_hash,
    )
    .await
}

pub(crate) async fn admit_command_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    actor_id: Uuid,
    idempotency_key: String,
    command_kind: &str,
    request_hash: &str,
) -> MarketplaceLedgerResult<LedgerReceiptAdmission> {
    let command_kind = normalize_command_kind(command_kind)?;
    if let Some(existing) = find_receipt(db, tenant_id, idempotency_key.as_str()).await? {
        return Ok(LedgerReceiptAdmission::Replay(existing));
    }

    let transaction = db.begin().await?;
    let receipt_id = generate_id();
    let insert = receipt::ActiveModel {
        id: Set(receipt_id),
        tenant_id: Set(tenant_id),
        actor_id: Set(actor_id),
        idempotency_key: Set(idempotency_key.clone()),
        command_kind: Set(command_kind.to_string()),
        request_hash: Set(request_hash.to_string()),
        status: Set(STATUS_PENDING.to_string()),
        response_json: Set(None),
        created_at: Set(Utc::now().into()),
        completed_at: Set(None),
    }
    .insert(&transaction)
    .await;

    match insert {
        Ok(_) => Ok(LedgerReceiptAdmission::New(NewLedgerReceipt {
            transaction,
            receipt_id,
            tenant_id,
        })),
        Err(error) if is_unique_constraint(&error) => {
            transaction.rollback().await?;
            let existing = find_receipt(db, tenant_id, idempotency_key.as_str())
                .await?
                .ok_or(error)?;
            Ok(LedgerReceiptAdmission::Replay(existing))
        }
        Err(error) => {
            transaction.rollback().await?;
            Err(error.into())
        }
    }
}

pub(crate) fn replay_receipt<R: DeserializeOwned>(
    receipt: receipt::Model,
    expected_request_hash: &str,
) -> MarketplaceLedgerResult<R> {
    replay_command_receipt(
        receipt,
        COMMAND_KIND_POST_ORDER_COMMISSIONS,
        expected_request_hash,
    )
}

pub(crate) fn replay_command_receipt<R: DeserializeOwned>(
    receipt: receipt::Model,
    expected_command_kind: &str,
    expected_request_hash: &str,
) -> MarketplaceLedgerResult<R> {
    if receipt.command_kind != expected_command_kind
        || receipt.request_hash != expected_request_hash
    {
        return Err(MarketplaceLedgerError::IdempotencyConflict);
    }
    if receipt.status != STATUS_COMPLETED || receipt.completed_at.is_none() {
        return Err(MarketplaceLedgerError::ReceiptCorrupt);
    }
    let response = receipt
        .response_json
        .ok_or(MarketplaceLedgerError::ReceiptCorrupt)?;
    serde_json::from_value(response).map_err(|_| MarketplaceLedgerError::ReceiptCorrupt)
}

pub(crate) async fn complete_receipt<R: Serialize + Clone>(
    receipt: NewLedgerReceipt,
    response: &R,
) -> MarketplaceLedgerResult<R> {
    let response_json = serde_json::to_value(response).map_err(|_| {
        MarketplaceLedgerError::Validation(
            "ledger command response could not be serialized".to_string(),
        )
    })?;
    let result = receipt::Entity::update_many()
        .col_expr(
            receipt::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_COMPLETED),
        )
        .col_expr(
            receipt::Column::ResponseJson,
            sea_orm::sea_query::Expr::value(Some(response_json)),
        )
        .col_expr(
            receipt::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(Utc::now().fixed_offset())),
        )
        .filter(receipt::Column::Id.eq(receipt.receipt_id))
        .filter(receipt::Column::TenantId.eq(receipt.tenant_id))
        .filter(receipt::Column::Status.eq(STATUS_PENDING))
        .exec(&receipt.transaction)
        .await?;
    if result.rows_affected != 1 {
        receipt.transaction.rollback().await?;
        return Err(MarketplaceLedgerError::ReceiptCorrupt);
    }
    receipt.transaction.commit().await?;
    Ok(response.clone())
}

pub(crate) async fn rollback_receipt<T>(
    receipt: NewLedgerReceipt,
    error: MarketplaceLedgerError,
) -> MarketplaceLedgerResult<T> {
    receipt.transaction.rollback().await?;
    Err(error)
}

async fn find_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> MarketplaceLedgerResult<Option<receipt::Model>> {
    receipt::Entity::find()
        .filter(receipt::Column::TenantId.eq(tenant_id))
        .filter(receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(db)
        .await
        .map_err(Into::into)
}

fn normalize_command_kind(value: &str) -> MarketplaceLedgerResult<&str> {
    let value = value.trim();
    if value.is_empty() || value.len() > 80 {
        return Err(MarketplaceLedgerError::Validation(
            "ledger command kind must contain 1 to 80 bytes".to_string(),
        ));
    }
    Ok(value)
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

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
