use std::sync::Arc;

use chrono::Utc;
use rustok_core::generate_id;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::seller_command_receipt;
use crate::error::{MarketplaceSellerError, MarketplaceSellerResult};
use crate::external_events::event_for_completed_command;
use crate::seller_events::append_receipted_seller_event;

const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 191;
const RECEIPT_STATUS_PENDING: &str = "pending";
const RECEIPT_STATUS_COMPLETED: &str = "completed";

pub(crate) struct NewCommandReceipt {
    pub transaction: DatabaseTransaction,
    pub receipt_id: Uuid,
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub idempotency_key: String,
    pub command_kind: String,
    event_bus: TransactionalEventBus,
}

pub(crate) enum CommandReceiptAdmission {
    Replay(seller_command_receipt::Model),
    New(NewCommandReceipt),
}

pub(crate) fn normalize_idempotency_key(
    value: impl Into<String>,
) -> MarketplaceSellerResult<String> {
    let value = value.into().trim().to_string();
    if value.is_empty() || value.len() > MAX_IDEMPOTENCY_KEY_LENGTH {
        return Err(MarketplaceSellerError::Validation(format!(
            "idempotency key must contain 1 to {MAX_IDEMPOTENCY_KEY_LENGTH} bytes"
        )));
    }
    Ok(value)
}

pub(crate) fn command_request_hash<T: Serialize>(
    command_kind: &str,
    actor_id: Uuid,
    request: &T,
) -> MarketplaceSellerResult<String> {
    let request = serde_json::to_value(request).map_err(|_| {
        MarketplaceSellerError::Validation(
            "marketplace seller command could not be normalized".to_string(),
        )
    })?;
    let payload = serde_json::json!({
        "version": 1,
        "command_kind": command_kind,
        "actor_id": actor_id,
        "request": canonical_json(&request),
    });
    let encoded = serde_json::to_vec(&payload).map_err(|_| {
        MarketplaceSellerError::Validation(
            "marketplace seller command could not be hashed".to_string(),
        )
    })?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

pub(crate) async fn admit_command(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    actor_id: Uuid,
    idempotency_key: String,
    command_kind: &str,
    request_hash: &str,
) -> MarketplaceSellerResult<CommandReceiptAdmission> {
    if let Some(existing) = find_receipt(db, tenant_id, idempotency_key.as_str()).await? {
        return Ok(CommandReceiptAdmission::Replay(existing));
    }

    let transaction = db.begin().await?;
    let receipt_id = generate_id();
    let now = Utc::now();
    let insert = seller_command_receipt::ActiveModel {
        id: Set(receipt_id),
        tenant_id: Set(tenant_id),
        actor_id: Set(actor_id),
        idempotency_key: Set(idempotency_key.clone()),
        command_kind: Set(command_kind.to_string()),
        request_hash: Set(request_hash.to_string()),
        status: Set(RECEIPT_STATUS_PENDING.to_string()),
        response_kind: Set(None),
        response_json: Set(None),
        created_at: Set(now.into()),
        completed_at: Set(None),
    }
    .insert(&transaction)
    .await;

    match insert {
        Ok(_) => Ok(CommandReceiptAdmission::New(NewCommandReceipt {
            transaction,
            receipt_id,
            tenant_id,
            actor_id,
            idempotency_key,
            command_kind: command_kind.to_string(),
            event_bus: TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
        })),
        Err(error) if is_unique_constraint(&error) => {
            transaction.rollback().await?;
            let existing = find_receipt(db, tenant_id, idempotency_key.as_str())
                .await?
                .ok_or(error)?;
            Ok(CommandReceiptAdmission::Replay(existing))
        }
        Err(error) => {
            transaction.rollback().await?;
            Err(error.into())
        }
    }
}

pub(crate) fn replay_command<R: DeserializeOwned>(
    receipt: seller_command_receipt::Model,
    expected_command_kind: &str,
    expected_request_hash: &str,
    expected_response_kind: &str,
) -> MarketplaceSellerResult<R> {
    if receipt.command_kind != expected_command_kind
        || receipt.request_hash != expected_request_hash
    {
        return Err(MarketplaceSellerError::IdempotencyConflict(
            receipt.idempotency_key,
        ));
    }
    if receipt.status != RECEIPT_STATUS_COMPLETED
        || receipt.response_kind.as_deref() != Some(expected_response_kind)
        || receipt.completed_at.is_none()
    {
        return Err(MarketplaceSellerError::CommandReceiptCorrupt(
            receipt.idempotency_key,
        ));
    }
    let response = receipt.response_json.ok_or_else(|| {
        MarketplaceSellerError::CommandReceiptCorrupt(receipt.idempotency_key.clone())
    })?;
    serde_json::from_value(response)
        .map_err(|_| MarketplaceSellerError::CommandReceiptCorrupt(receipt.idempotency_key))
}

pub(crate) async fn complete_command<R: Serialize + Clone>(
    receipt: NewCommandReceipt,
    response_kind: &str,
    response: &R,
) -> MarketplaceSellerResult<R> {
    let response_json = serde_json::to_value(response).map_err(|_| {
        MarketplaceSellerError::Validation(
            "marketplace seller command result could not be serialized".to_string(),
        )
    })?;

    if let Err(error) = append_receipted_seller_event(
        &receipt.transaction,
        receipt.tenant_id,
        receipt.actor_id,
        receipt.command_kind.as_str(),
        response_kind,
        &response_json,
    )
    .await
    {
        receipt.transaction.rollback().await?;
        return Err(error);
    }

    let external_event = match event_for_completed_command(
        receipt.command_kind.as_str(),
        response_kind,
        &response_json,
    ) {
        Ok(event) => event,
        Err(error) => {
            receipt.transaction.rollback().await?;
            return Err(error);
        }
    };
    if let Err(error) = receipt
        .event_bus
        .publish_contract_in_tx(
            &receipt.transaction,
            receipt.tenant_id,
            Some(receipt.actor_id),
            external_event,
        )
        .await
    {
        tracing::error!(
            tenant_id = %receipt.tenant_id,
            actor_id = %receipt.actor_id,
            receipt_id = %receipt.receipt_id,
            command_kind = receipt.command_kind.as_str(),
            error = %error,
            "Marketplace seller transactional event publication failed"
        );
        receipt.transaction.rollback().await?;
        return Err(MarketplaceSellerError::Database(sea_orm::DbErr::Custom(
            "marketplace seller event publication unavailable".to_string(),
        )));
    }

    let result = seller_command_receipt::Entity::update_many()
        .col_expr(
            seller_command_receipt::Column::Status,
            sea_orm::sea_query::Expr::value(RECEIPT_STATUS_COMPLETED),
        )
        .col_expr(
            seller_command_receipt::Column::ResponseKind,
            sea_orm::sea_query::Expr::value(Some(response_kind.to_string())),
        )
        .col_expr(
            seller_command_receipt::Column::ResponseJson,
            sea_orm::sea_query::Expr::value(Some(response_json)),
        )
        .col_expr(
            seller_command_receipt::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(Utc::now().fixed_offset())),
        )
        .filter(seller_command_receipt::Column::Id.eq(receipt.receipt_id))
        .filter(seller_command_receipt::Column::TenantId.eq(receipt.tenant_id))
        .filter(seller_command_receipt::Column::Status.eq(RECEIPT_STATUS_PENDING))
        .exec(&receipt.transaction)
        .await?;
    if result.rows_affected != 1 {
        let key = receipt.idempotency_key;
        receipt.transaction.rollback().await?;
        return Err(MarketplaceSellerError::CommandReceiptCorrupt(key));
    }
    receipt.transaction.commit().await?;
    Ok(response.clone())
}

pub(crate) async fn rollback_command<T>(
    receipt: NewCommandReceipt,
    error: MarketplaceSellerError,
) -> MarketplaceSellerResult<T> {
    receipt.transaction.rollback().await?;
    Err(error)
}

async fn find_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> MarketplaceSellerResult<Option<seller_command_receipt::Model>> {
    seller_command_receipt::Entity::find()
        .filter(seller_command_receipt::Column::TenantId.eq(tenant_id))
        .filter(seller_command_receipt::Column::IdempotencyKey.eq(idempotency_key))
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
            let mut canonical = serde_json::Map::new();
            for key in keys {
                canonical.insert(key.clone(), canonical_json(&values[key]));
            }
            Value::Object(canonical)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_hash_is_stable_across_json_key_order() {
        let left = command_request_hash(
            "create_seller",
            Uuid::nil(),
            &serde_json::json!({"metadata": {"b": 2, "a": 1}}),
        )
        .unwrap();
        let right = command_request_hash(
            "create_seller",
            Uuid::nil(),
            &serde_json::json!({"metadata": {"a": 1, "b": 2}}),
        )
        .unwrap();
        assert_eq!(left, right);
        assert_eq!(left.len(), 64);
        assert!(left.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(left, left.to_ascii_lowercase());
    }
}
