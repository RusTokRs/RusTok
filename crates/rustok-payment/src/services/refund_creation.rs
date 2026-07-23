use chrono::Utc;
use rust_decimal::Decimal;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dto::{
    CreateRefundInput, PaymentCollectionStatusKind, RefundResponse, RefundStatusKind,
};
use crate::entities::{payment_collection, refund_creation};
use crate::error::{PaymentError, PaymentResult};

const REFUND_STATUS_PENDING: &str = "pending";
const REFUND_STATUS_COMPLETED: &str = "refunded";
const MAX_CREATION_KEY_LENGTH: usize = 191;

#[derive(Clone)]
pub struct PaymentRefundCreationService {
    db: DatabaseConnection,
}

impl PaymentRefundCreationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create_or_replay(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        creation_key: impl Into<String>,
        input: CreateRefundInput,
    ) -> PaymentResult<RefundResponse> {
        let creation_key = normalize_creation_key(creation_key.into())?;
        let reason = normalize_reason(input.reason);
        let request_hash = refund_request_hash(input.amount, reason.as_deref(), &input.metadata)?;

        if let Some(existing) = self
            .find_by_creation_key(tenant_id, collection_id, creation_key.as_str())
            .await?
        {
            return replay_existing(existing, request_hash.as_str());
        }

        let txn = self.db.begin().await?;
        let collection = payment_collection::Entity::find_by_id(collection_id)
            .filter(payment_collection::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(PaymentError::PaymentCollectionNotFound(collection_id))?;
        if PaymentCollectionStatusKind::from_raw(collection.status.as_str())
            != PaymentCollectionStatusKind::Captured
        {
            return Err(PaymentError::InvalidTransition {
                from: collection.status,
                to: REFUND_STATUS_PENDING.to_string(),
            });
        }
        if input.amount <= Decimal::ZERO {
            return Err(PaymentError::Validation(
                "refund amount must be greater than zero".to_string(),
            ));
        }

        let reserved = refund_creation::Entity::find()
            .filter(refund_creation::Column::PaymentCollectionId.eq(collection_id))
            .filter(
                refund_creation::Column::Status
                    .is_in([REFUND_STATUS_PENDING, REFUND_STATUS_COMPLETED]),
            )
            .all(&txn)
            .await?
            .into_iter()
            .fold(Decimal::ZERO, |sum, refund| sum + refund.amount);
        let remaining = collection.captured_amount - reserved;
        if input.amount > remaining {
            return Err(PaymentError::Validation(format!(
                "refund amount exceeds remaining refundable amount of {remaining}"
            )));
        }

        let now = Utc::now();
        let refund_id = generate_id();
        let insert = refund_creation::ActiveModel {
            id: Set(refund_id),
            tenant_id: Set(tenant_id),
            payment_collection_id: Set(collection_id),
            status: Set(REFUND_STATUS_PENDING.to_string()),
            currency_code: Set(collection.currency_code),
            amount: Set(input.amount),
            reason: Set(reason),
            metadata: Set(input.metadata),
            creation_key: Set(Some(creation_key.clone())),
            creation_request_hash: Set(Some(request_hash.clone())),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            refunded_at: Set(None),
            cancelled_at: Set(None),
        }
        .insert(&txn)
        .await;

        match insert {
            Ok(refund) => {
                txn.commit().await?;
                Ok(to_response(refund))
            }
            Err(error) if is_unique_constraint(&error) => {
                txn.rollback().await?;
                let existing = self
                    .find_by_creation_key(tenant_id, collection_id, creation_key.as_str())
                    .await?
                    .ok_or(error)?;
                replay_existing(existing, request_hash.as_str())
            }
            Err(error) => Err(error.into()),
        }
    }

    pub async fn find_by_creation_key(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        creation_key: &str,
    ) -> PaymentResult<Option<refund_creation::Model>> {
        refund_creation::Entity::find()
            .filter(refund_creation::Column::TenantId.eq(tenant_id))
            .filter(refund_creation::Column::PaymentCollectionId.eq(collection_id))
            .filter(refund_creation::Column::CreationKey.eq(creation_key))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }
}

fn replay_existing(
    existing: refund_creation::Model,
    request_hash: &str,
) -> PaymentResult<RefundResponse> {
    if existing.creation_request_hash.as_deref() != Some(request_hash) {
        return Err(PaymentError::Validation(format!(
            "refund creation key `{}` is already bound to another request",
            existing.creation_key.as_deref().unwrap_or("unknown")
        )));
    }
    if RefundStatusKind::from_raw(existing.status.as_str()) == RefundStatusKind::Unknown {
        return Err(PaymentError::Validation(format!(
            "refund {} has an unsupported lifecycle status",
            existing.id
        )));
    }
    Ok(to_response(existing))
}

fn to_response(refund: refund_creation::Model) -> RefundResponse {
    RefundResponse {
        id: refund.id,
        tenant_id: refund.tenant_id,
        payment_collection_id: refund.payment_collection_id,
        status: refund.status,
        currency_code: refund.currency_code,
        amount: refund.amount,
        reason: refund.reason,
        metadata: refund.metadata,
        created_at: refund.created_at.with_timezone(&Utc),
        updated_at: refund.updated_at.with_timezone(&Utc),
        refunded_at: refund.refunded_at.map(|value| value.with_timezone(&Utc)),
        cancelled_at: refund.cancelled_at.map(|value| value.with_timezone(&Utc)),
    }
}

fn normalize_creation_key(value: String) -> PaymentResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > MAX_CREATION_KEY_LENGTH {
        return Err(PaymentError::Validation(format!(
            "refund creation key must contain 1 to {MAX_CREATION_KEY_LENGTH} bytes"
        )));
    }
    Ok(value)
}

fn normalize_reason(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn refund_request_hash(
    amount: Decimal,
    reason: Option<&str>,
    metadata: &Value,
) -> PaymentResult<String> {
    let payload = serde_json::json!({
        "version": 1,
        "amount": amount.normalize().to_string(),
        "reason": reason,
        "metadata": canonical_json(metadata),
    });
    let encoded = serde_json::to_vec(&payload).map_err(|error| {
        PaymentError::Validation(format!("failed to hash refund request: {error}"))
    })?;
    Ok(Sha256::digest(encoded)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
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
    fn refund_hash_is_stable_across_object_key_order() {
        let left = refund_request_hash(
            Decimal::new(1000, 2),
            Some("customer request"),
            &serde_json::json!({"b": 2, "a": 1}),
        )
        .unwrap();
        let right = refund_request_hash(
            Decimal::new(1000, 2),
            Some("customer request"),
            &serde_json::json!({"a": 1, "b": 2}),
        )
        .unwrap();
        assert_eq!(left, right);
    }
}
