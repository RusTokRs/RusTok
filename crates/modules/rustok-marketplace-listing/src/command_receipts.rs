use chrono::Utc;
use rustok_core::generate_id;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dto::MarketplaceListingResponse;
use crate::entities::listing_command_receipt;
use crate::error::{MarketplaceListingError, MarketplaceListingResult};
use crate::external_events::event_for_completed_command;

const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 191;
const STATUS_PENDING: &str = "pending";
const STATUS_COMPLETED: &str = "completed";

pub(crate) struct NewListingCommandReceipt {
    pub transaction: DatabaseTransaction,
    pub receipt_id: Uuid,
    pub tenant_id: Uuid,
    actor_id: Uuid,
    command_kind: String,
    event_bus: TransactionalEventBus,
}

pub(crate) enum ListingCommandAdmission {
    Replay(listing_command_receipt::Model),
    New(NewListingCommandReceipt),
}

pub(crate) fn normalize_idempotency_key(
    value: impl Into<String>,
) -> MarketplaceListingResult<String> {
    let value = value.into().trim().to_string();
    if value.is_empty() || value.len() > MAX_IDEMPOTENCY_KEY_LENGTH {
        return Err(MarketplaceListingError::Validation(format!(
            "idempotency key must contain 1 to {MAX_IDEMPOTENCY_KEY_LENGTH} bytes"
        )));
    }
    Ok(value)
}

pub(crate) fn request_hash<T: Serialize>(
    command_kind: &str,
    actor_id: Uuid,
    request: &T,
) -> MarketplaceListingResult<String> {
    let request = serde_json::to_value(request).map_err(|_| {
        MarketplaceListingError::Validation(
            "marketplace listing command could not be normalized".to_string(),
        )
    })?;
    let payload = serde_json::json!({
        "version": 1,
        "command_kind": command_kind,
        "actor_id": actor_id,
        "request": canonical_json(&request),
    });
    let encoded = serde_json::to_vec(&payload).map_err(|_| {
        MarketplaceListingError::Validation(
            "marketplace listing command could not be hashed".to_string(),
        )
    })?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

pub(crate) async fn replay_existing<R: DeserializeOwned>(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: &str,
    expected_command_kind: &str,
    expected_hash: &str,
) -> MarketplaceListingResult<Option<R>> {
    match find_receipt(db, tenant_id, idempotency_key).await? {
        Some(receipt) => replay(receipt, expected_command_kind, expected_hash).map(Some),
        None => Ok(None),
    }
}

pub(crate) async fn admit(
    db: &DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
    actor_id: Uuid,
    idempotency_key: String,
    command_kind: &str,
    hash: &str,
) -> MarketplaceListingResult<ListingCommandAdmission> {
    if let Some(existing) = find_receipt(db, tenant_id, idempotency_key.as_str()).await? {
        return Ok(ListingCommandAdmission::Replay(existing));
    }
    let transaction = db.begin().await?;
    let receipt_id = generate_id();
    let insert = listing_command_receipt::ActiveModel {
        id: Set(receipt_id),
        tenant_id: Set(tenant_id),
        actor_id: Set(actor_id),
        idempotency_key: Set(idempotency_key.clone()),
        command_kind: Set(command_kind.to_string()),
        request_hash: Set(hash.to_string()),
        status: Set(STATUS_PENDING.to_string()),
        response_json: Set(None),
        created_at: Set(Utc::now().into()),
        completed_at: Set(None),
    }
    .insert(&transaction)
    .await;
    match insert {
        Ok(_) => Ok(ListingCommandAdmission::New(NewListingCommandReceipt {
            transaction,
            receipt_id,
            tenant_id,
            actor_id,
            command_kind: command_kind.to_string(),
            event_bus,
        })),
        Err(error) if is_unique_constraint(&error) => {
            transaction.rollback().await?;
            let existing = find_receipt(db, tenant_id, idempotency_key.as_str())
                .await?
                .ok_or(error)?;
            Ok(ListingCommandAdmission::Replay(existing))
        }
        Err(error) => {
            transaction.rollback().await?;
            Err(error.into())
        }
    }
}

pub(crate) fn replay<R: DeserializeOwned>(
    receipt: listing_command_receipt::Model,
    expected_command_kind: &str,
    expected_hash: &str,
) -> MarketplaceListingResult<R> {
    if receipt.command_kind != expected_command_kind || receipt.request_hash != expected_hash {
        return Err(MarketplaceListingError::IdempotencyConflict);
    }
    if receipt.status != STATUS_COMPLETED || receipt.completed_at.is_none() {
        return Err(MarketplaceListingError::CommandReceiptCorrupt);
    }
    let response = receipt
        .response_json
        .ok_or(MarketplaceListingError::CommandReceiptCorrupt)?;
    serde_json::from_value(response).map_err(|_| MarketplaceListingError::CommandReceiptCorrupt)
}

pub(crate) async fn complete(
    receipt: NewListingCommandReceipt,
    response: &MarketplaceListingResponse,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let NewListingCommandReceipt {
        transaction,
        receipt_id,
        tenant_id,
        actor_id,
        command_kind,
        event_bus,
    } = receipt;
    let response_json = serde_json::to_value(response).map_err(|_| {
        MarketplaceListingError::Validation(
            "marketplace listing command result could not be serialized".to_string(),
        )
    })?;
    let event = match event_for_completed_command(command_kind.as_str(), response) {
        Ok(event) => event,
        Err(error) => {
            transaction.rollback().await?;
            return Err(error);
        }
    };
    if let Err(error) = event_bus
        .publish_contract_in_tx(&transaction, tenant_id, Some(actor_id), event)
        .await
    {
        tracing::error!(
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            receipt_id = %receipt_id,
            command_kind = command_kind.as_str(),
            error = %error,
            "Marketplace listing transactional event publication failed"
        );
        transaction.rollback().await?;
        return Err(MarketplaceListingError::EventPublicationUnavailable);
    }
    let result = listing_command_receipt::Entity::update_many()
        .col_expr(
            listing_command_receipt::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_COMPLETED),
        )
        .col_expr(
            listing_command_receipt::Column::ResponseJson,
            sea_orm::sea_query::Expr::value(Some(response_json)),
        )
        .col_expr(
            listing_command_receipt::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(Some(Utc::now().fixed_offset())),
        )
        .filter(listing_command_receipt::Column::Id.eq(receipt_id))
        .filter(listing_command_receipt::Column::TenantId.eq(tenant_id))
        .filter(listing_command_receipt::Column::Status.eq(STATUS_PENDING))
        .exec(&transaction)
        .await?;
    if result.rows_affected != 1 {
        transaction.rollback().await?;
        return Err(MarketplaceListingError::CommandReceiptCorrupt);
    }
    transaction.commit().await?;
    Ok(response.clone())
}

pub(crate) async fn rollback<T>(
    receipt: NewListingCommandReceipt,
    error: MarketplaceListingError,
) -> MarketplaceListingResult<T> {
    receipt.transaction.rollback().await?;
    Err(error)
}

async fn find_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    key: &str,
) -> MarketplaceListingResult<Option<listing_command_receipt::Model>> {
    listing_command_receipt::Entity::find()
        .filter(listing_command_receipt::Column::TenantId.eq(tenant_id))
        .filter(listing_command_receipt::Column::IdempotencyKey.eq(key))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_hash_is_stable_lowercase_sha256_hex() {
        let left = request_hash(
            "create_listing",
            Uuid::nil(),
            &serde_json::json!({"metadata": {"b": 2, "a": 1}}),
        )
        .unwrap();
        let right = request_hash(
            "create_listing",
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
