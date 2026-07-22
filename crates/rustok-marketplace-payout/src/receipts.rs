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

use crate::entities::schedule_receipt;
use crate::error::{MarketplacePayoutError, MarketplacePayoutResult};

const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 191;
const STATUS_PENDING: &str = "pending";
const STATUS_COMPLETED: &str = "completed";
const COMMAND_KIND: &str = "schedule_payout";

pub(crate) struct NewPayoutReceipt {
    pub transaction: DatabaseTransaction,
    pub receipt_id: Uuid,
    pub tenant_id: Uuid,
}

pub(crate) enum PayoutReceiptAdmission {
    Replay(schedule_receipt::Model),
    New(NewPayoutReceipt),
}

pub(crate) fn normalize_idempotency_key(
    value: impl Into<String>,
) -> MarketplacePayoutResult<String> {
    let value = value.into().trim().to_string();
    if value.is_empty() || value.len() > MAX_IDEMPOTENCY_KEY_LENGTH {
        return Err(MarketplacePayoutError::Validation(format!(
            "idempotency key must contain 1 to {MAX_IDEMPOTENCY_KEY_LENGTH} bytes"
        )));
    }
    Ok(value)
}

pub(crate) fn schedule_request_hash<T: Serialize>(
    actor_id: Uuid,
    request: &T,
) -> MarketplacePayoutResult<String> {
    let request = serde_json::to_value(request).map_err(|_| {
        MarketplacePayoutError::Validation(
            "payout schedule request could not be normalized".to_string(),
        )
    })?;
    let payload = serde_json::json!({
        "version": 1,
        "command_kind": COMMAND_KIND,
        "actor_id": actor_id,
        "request": canonical_json(&request),
    });
    let encoded = serde_json::to_vec(&payload).map_err(|_| {
        MarketplacePayoutError::Validation(
            "payout schedule request could not be hashed".to_string(),
        )
    })?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

pub(crate) async fn replay_existing<R: DeserializeOwned>(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: &str,
    request_hash: &str,
) -> MarketplacePayoutResult<Option<R>> {
    match find_receipt(db, tenant_id, idempotency_key).await? {
        Some(receipt) => replay_receipt(receipt, request_hash).map(Some),
        None => Ok(None),
    }
}

pub(crate) async fn admit_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    actor_id: Uuid,
    idempotency_key: String,
    request_hash: &str,
) -> MarketplacePayoutResult<PayoutReceiptAdmission> {
    if let Some(existing) = find_receipt(db, tenant_id, idempotency_key.as_str()).await? {
        return Ok(PayoutReceiptAdmission::Replay(existing));
    }
    let transaction = db.begin().await?;
    let receipt_id = generate_id();
    let insert = schedule_receipt::ActiveModel {
        id: Set(receipt_id),
        tenant_id: Set(tenant_id),
        actor_id: Set(actor_id),
        idempotency_key: Set(idempotency_key.clone()),
        command_kind: Set(COMMAND_KIND.to_string()),
        request_hash: Set(request_hash.to_string()),
        status: Set(STATUS_PENDING.to_string()),
        response_json: Set(None),
        created_at: Set(Utc::now().into()),
        completed_at: Set(None),
    }
    .insert(&transaction)
    .await;
    match insert {
        Ok(_) => Ok(PayoutReceiptAdmission::New(NewPayoutReceipt {
            transaction,
            receipt_id,
            tenant_id,
        })),
        Err(error) if is_unique_constraint(&error) => {
            transaction.rollback().await?;
            let existing = find_receipt(db, tenant_id, idempotency_key.as_str())
                .await?
                .ok_or(error)?;
            Ok(PayoutReceiptAdmission::Replay(existing))
        }
        Err(error) => {
            transaction.rollback().await?;
            Err(error.into())
        }
    }
}

pub(crate) fn replay_receipt<R: DeserializeOwned>(
    receipt: schedule_receipt::Model,
    expected_request_hash: &str,
) -> MarketplacePayoutResult<R> {
    if receipt.command_kind != COMMAND_KIND || receipt.request_hash != expected_request_hash {
        return Err(MarketplacePayoutError::IdempotencyConflict);
    }
    if receipt.status != STATUS_COMPLETED || receipt.completed_at.is_none() {
        return Err(MarketplacePayoutError::ReceiptCorrupt);
    }
    let response = receipt
        .response_json
        .ok_or(MarketplacePayoutError::ReceiptCorrupt)?;
    serde_json::from_value(response).map_err(|_| MarketplacePayoutError::ReceiptCorrupt)
}

pub(crate) async fn complete_receipt<R: Serialize + Clone>(
    receipt: NewPayoutReceipt,
    response: &R,
) -> MarketplacePayoutResult<R> {
    let response_json = serde_json::to_value(response).map_err(|_| {
        MarketplacePayoutError::Validation(
            "payout schedule response could not be serialized".to_string(),
        )
    })?;
    let result = schedule_receipt::Entity::update_many()
        .col_expr(
            schedule_receipt::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_COMPLETED),
        )
        .col_expr(
            schedule_receipt::Column::ResponseJson,
            sea_orm::sea_query::Expr::value(Some(response_json)),
        )
        .col_expr(
            schedule_receipt::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(Utc::now().fixed_offset())),
        )
        .filter(schedule_receipt::Column::Id.eq(receipt.receipt_id))
        .filter(schedule_receipt::Column::TenantId.eq(receipt.tenant_id))
        .filter(schedule_receipt::Column::Status.eq(STATUS_PENDING))
        .exec(&receipt.transaction)
        .await?;
    if result.rows_affected != 1 {
        receipt.transaction.rollback().await?;
        return Err(MarketplacePayoutError::ReceiptCorrupt);
    }
    receipt.transaction.commit().await?;
    Ok(response.clone())
}

pub(crate) async fn rollback_receipt<T>(
    receipt: NewPayoutReceipt,
    error: MarketplacePayoutError,
) -> MarketplacePayoutResult<T> {
    receipt.transaction.rollback().await?;
    Err(error)
}

async fn find_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> MarketplacePayoutResult<Option<schedule_receipt::Model>> {
    schedule_receipt::Entity::find()
        .filter(schedule_receipt::Column::TenantId.eq(tenant_id))
        .filter(schedule_receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(db)
        .await
        .map_err(Into::into)
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
