use chrono::{Duration, Utc};
use rustok_core::generate_id;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, Set,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::provider_event;
use crate::error::{PaymentError, PaymentResult};

pub const PROVIDER_EVENT_RECEIVED: &str = "received";
pub const PROVIDER_EVENT_PROCESSING: &str = "processing";
pub const PROVIDER_EVENT_FAILED: &str = "failed";
pub const PROVIDER_EVENT_PROCESSED: &str = "processed";
pub const PROVIDER_EVENT_DEAD_LETTER: &str = "dead_letter";

const MAX_PROVIDER_ID_LENGTH: usize = 100;
const MAX_EVENT_KEY_LENGTH: usize = 191;
const MAX_EVENT_TYPE_LENGTH: usize = 191;
const MAX_EXTERNAL_REFERENCE_LENGTH: usize = 191;
const MAX_LEASE_OWNER_LENGTH: usize = 191;
const MAX_ERROR_CODE_LENGTH: usize = 100;
const MAX_ERROR_MESSAGE_LENGTH: usize = 2000;
const MAX_LEASE_SECONDS: i64 = 300;
const MAX_FAILURE_ATTEMPTS: i32 = 25;

#[derive(Clone, Debug)]
pub struct ReceiveProviderEvent {
    pub tenant_id: Uuid,
    pub provider_id: String,
    pub delivery_id: String,
    pub idempotency_key: String,
    pub raw_payload: Vec<u8>,
    pub signature_verified: bool,
}

#[derive(Clone, Debug)]
pub struct CompleteProviderEvent {
    pub tenant_id: Uuid,
    pub event_id: Uuid,
    pub lease_owner: String,
    pub event_type: String,
    pub external_reference: Option<String>,
    pub event_metadata: Value,
}

#[derive(Clone, Debug)]
pub struct FailProviderEvent {
    pub tenant_id: Uuid,
    pub event_id: Uuid,
    pub lease_owner: String,
    pub error_code: String,
    pub error_message: String,
    pub retryable: bool,
    pub max_attempts: i32,
}

#[derive(Clone)]
pub struct PaymentProviderEventJournal {
    db: DatabaseConnection,
}

impl PaymentProviderEventJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Records a signature-verified provider delivery without retaining the
    /// raw body. Replays with the same provider identity and payload hash are
    /// adopted; a key collision with another payload is rejected.
    pub async fn receive(
        &self,
        input: ReceiveProviderEvent,
    ) -> PaymentResult<provider_event::Model> {
        let input = normalize_receive_input(input)?;
        let payload_hash = hash_payload(input.raw_payload.as_slice());

        if let Some(existing) = self
            .find_existing(
                input.tenant_id,
                input.provider_id.as_str(),
                input.delivery_id.as_str(),
                input.idempotency_key.as_str(),
            )
            .await?
        {
            ensure_same_delivery(&existing, &input, payload_hash.as_str())?;
            return Ok(existing);
        }

        let now = Utc::now();
        let insert = provider_event::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(input.tenant_id),
            provider_id: Set(input.provider_id.clone()),
            delivery_id: Set(input.delivery_id.clone()),
            idempotency_key: Set(input.idempotency_key.clone()),
            payload_hash: Set(payload_hash.clone()),
            signature_verified: Set(true),
            status: Set(PROVIDER_EVENT_RECEIVED.to_string()),
            event_type: Set(None),
            external_reference: Set(None),
            event_metadata: Set(None),
            attempt_count: Set(0),
            lease_owner: Set(None),
            lease_expires_at: Set(None),
            error_code: Set(None),
            error_message: Set(None),
            received_at: Set(now.into()),
            updated_at: Set(now.into()),
            processed_at: Set(None),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(model) => Ok(model),
            Err(insert_error) => {
                if let Some(existing) = self
                    .find_existing(
                        input.tenant_id,
                        input.provider_id.as_str(),
                        input.delivery_id.as_str(),
                        input.idempotency_key.as_str(),
                    )
                    .await?
                {
                    ensure_same_delivery(&existing, &input, payload_hash.as_str())?;
                    Ok(existing)
                } else {
                    Err(insert_error.into())
                }
            }
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        event_id: Uuid,
    ) -> PaymentResult<provider_event::Model> {
        provider_event::Entity::find_by_id(event_id)
            .filter(provider_event::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                PaymentError::Validation(format!(
                    "payment provider event {event_id} was not found"
                ))
            })
    }

    pub async fn find_by_delivery(
        &self,
        tenant_id: Uuid,
        provider_id: &str,
        delivery_id: &str,
    ) -> PaymentResult<Option<provider_event::Model>> {
        provider_event::Entity::find()
            .filter(provider_event::Column::TenantId.eq(tenant_id))
            .filter(provider_event::Column::ProviderId.eq(provider_id))
            .filter(provider_event::Column::DeliveryId.eq(delivery_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn find_by_idempotency_key(
        &self,
        tenant_id: Uuid,
        provider_id: &str,
        idempotency_key: &str,
    ) -> PaymentResult<Option<provider_event::Model>> {
        provider_event::Entity::find()
            .filter(provider_event::Column::TenantId.eq(tenant_id))
            .filter(provider_event::Column::ProviderId.eq(provider_id))
            .filter(provider_event::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    /// Claims a new/failed delivery or an expired processing lease. The
    /// attempt counter is incremented atomically with the lease acquisition.
    pub async fn claim_processing(
        &self,
        tenant_id: Uuid,
        event_id: Uuid,
        lease_owner: impl Into<String>,
        lease_seconds: i64,
    ) -> PaymentResult<Option<provider_event::Model>> {
        let lease_owner = normalize_required(
            lease_owner.into(),
            "lease_owner",
            MAX_LEASE_OWNER_LENGTH,
        )?;
        let lease_seconds = lease_seconds.clamp(1, MAX_LEASE_SECONDS);
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);
        let claimable = Condition::any()
            .add(provider_event::Column::Status.eq(PROVIDER_EVENT_RECEIVED))
            .add(provider_event::Column::Status.eq(PROVIDER_EVENT_FAILED))
            .add(
                Condition::all()
                    .add(provider_event::Column::Status.eq(PROVIDER_EVENT_PROCESSING))
                    .add(provider_event::Column::LeaseExpiresAt.lte(now)),
            );
        let update = provider_event::Entity::update_many()
            .col_expr(
                provider_event::Column::Status,
                Expr::value(PROVIDER_EVENT_PROCESSING),
            )
            .col_expr(
                provider_event::Column::LeaseOwner,
                Expr::value(Some(lease_owner)),
            )
            .col_expr(
                provider_event::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(
                provider_event::Column::AttemptCount,
                Expr::col(provider_event::Column::AttemptCount).add(1),
            )
            .col_expr(
                provider_event::Column::ErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_event::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_event::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(provider_event::Column::TenantId.eq(tenant_id))
            .filter(provider_event::Column::Id.eq(event_id))
            .filter(claimable)
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, event_id).await.map(Some)
    }

    pub async fn mark_processed(
        &self,
        input: CompleteProviderEvent,
    ) -> PaymentResult<provider_event::Model> {
        let lease_owner = normalize_required(
            input.lease_owner,
            "lease_owner",
            MAX_LEASE_OWNER_LENGTH,
        )?;
        let event_type = normalize_required(
            input.event_type,
            "event_type",
            MAX_EVENT_TYPE_LENGTH,
        )?;
        let external_reference = normalize_optional(
            input.external_reference,
            "external_reference",
            MAX_EXTERNAL_REFERENCE_LENGTH,
        )?;
        let now = Utc::now().fixed_offset();
        let update = provider_event::Entity::update_many()
            .col_expr(
                provider_event::Column::Status,
                Expr::value(PROVIDER_EVENT_PROCESSED),
            )
            .col_expr(
                provider_event::Column::EventType,
                Expr::value(Some(event_type)),
            )
            .col_expr(
                provider_event::Column::ExternalReference,
                Expr::value(external_reference),
            )
            .col_expr(
                provider_event::Column::EventMetadata,
                Expr::value(Some(input.event_metadata)),
            )
            .col_expr(
                provider_event::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_event::Column::LeaseExpiresAt,
                Expr::value(Option::<<provider_event::Model as sea_orm::ModelTrait>::Entity>::None),
            )
            .filter(provider_event::Column::TenantId.eq(input.tenant_id))
            .filter(provider_event::Column::Id.eq(input.event_id))
            .filter(provider_event::Column::Status.eq(PROVIDER_EVENT_PROCESSING))
            .filter(provider_event::Column::LeaseOwner.eq(lease_owner))
            .filter(provider_event::Column::LeaseExpiresAt.gt(now))
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return self.transition_conflict(input.tenant_id, input.event_id, "processed").await;
        }

        let mut active: provider_event::ActiveModel = self.get(input.tenant_id, input.event_id).await?.into();
        active.error_code = Set(None);
        active.error_message = Set(None);
        active.updated_at = Set(Utc::now().into());
        active.processed_at = Set(Some(Utc::now().into()));
        active.update(&self.db).await.map_err(Into::into)
    }

    pub async fn mark_failed(
        &self,
        input: FailProviderEvent,
    ) -> PaymentResult<provider_event::Model> {
        let lease_owner = normalize_required(
            input.lease_owner,
            "lease_owner",
            MAX_LEASE_OWNER_LENGTH,
        )?;
        let error_code = normalize_required(
            input.error_code,
            "error_code",
            MAX_ERROR_CODE_LENGTH,
        )?;
        let error_message = normalize_required(
            input.error_message,
            "error_message",
            MAX_ERROR_MESSAGE_LENGTH,
        )?;
        let max_attempts = input.max_attempts.clamp(1, MAX_FAILURE_ATTEMPTS);
        let current = self.get(input.tenant_id, input.event_id).await?;
        if current.status != PROVIDER_EVENT_PROCESSING
            || current.lease_owner.as_deref() != Some(lease_owner.as_str())
        {
            return Err(PaymentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_EVENT_FAILED.to_string(),
            });
        }
        let dead_letter = !input.retryable || current.attempt_count >= max_attempts;
        let target = if dead_letter {
            PROVIDER_EVENT_DEAD_LETTER
        } else {
            PROVIDER_EVENT_FAILED
        };
        let now = Utc::now();
        let mut active: provider_event::ActiveModel = current.into();
        active.status = Set(target.to_string());
        active.lease_owner = Set(None);
        active.lease_expires_at = Set(None);
        active.error_code = Set(Some(error_code));
        active.error_message = Set(Some(error_message));
        active.updated_at = Set(now.into());
        active.processed_at = Set(dead_letter.then(|| now.into()));
        active.update(&self.db).await.map_err(Into::into)
    }

    pub async fn list_retryable(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> PaymentResult<Vec<provider_event::Model>> {
        let now = Utc::now().fixed_offset();
        provider_event::Entity::find()
            .filter(provider_event::Column::TenantId.eq(tenant_id))
            .filter(
                Condition::any()
                    .add(provider_event::Column::Status.eq(PROVIDER_EVENT_RECEIVED))
                    .add(provider_event::Column::Status.eq(PROVIDER_EVENT_FAILED))
                    .add(
                        Condition::all()
                            .add(
                                provider_event::Column::Status.eq(PROVIDER_EVENT_PROCESSING),
                            )
                            .add(provider_event::Column::LeaseExpiresAt.lte(now)),
                    ),
            )
            .order_by_asc(provider_event::Column::UpdatedAt)
            .limit(limit.clamp(1, 100))
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    async fn find_existing(
        &self,
        tenant_id: Uuid,
        provider_id: &str,
        delivery_id: &str,
        idempotency_key: &str,
    ) -> PaymentResult<Option<provider_event::Model>> {
        let by_delivery = self
            .find_by_delivery(tenant_id, provider_id, delivery_id)
            .await?;
        let by_idempotency = self
            .find_by_idempotency_key(tenant_id, provider_id, idempotency_key)
            .await?;
        match (by_delivery, by_idempotency) {
            (Some(delivery), Some(idempotency)) if delivery.id != idempotency.id => {
                Err(PaymentError::Validation(
                    "provider delivery and idempotency keys resolve to different inbox events"
                        .to_string(),
                ))
            }
            (Some(event), _) | (_, Some(event)) => Ok(Some(event)),
            (None, None) => Ok(None),
        }
    }

    async fn transition_conflict(
        &self,
        tenant_id: Uuid,
        event_id: Uuid,
        target: &str,
    ) -> PaymentResult<provider_event::Model> {
        let current = self.get(tenant_id, event_id).await?;
        if current.status == target {
            Ok(current)
        } else {
            Err(PaymentError::InvalidTransition {
                from: current.status,
                to: target.to_string(),
            })
        }
    }
}

fn normalize_receive_input(mut input: ReceiveProviderEvent) -> PaymentResult<ReceiveProviderEvent> {
    if !input.signature_verified {
        return Err(PaymentError::Validation(
            "payment provider event signature must be verified before inbox insertion".to_string(),
        ));
    }
    if input.raw_payload.is_empty() {
        return Err(PaymentError::Validation(
            "payment provider event payload must not be empty".to_string(),
        ));
    }
    input.provider_id = normalize_required(
        input.provider_id,
        "provider_id",
        MAX_PROVIDER_ID_LENGTH,
    )?;
    input.delivery_id = normalize_required(
        input.delivery_id,
        "delivery_id",
        MAX_EVENT_KEY_LENGTH,
    )?;
    input.idempotency_key = normalize_required(
        input.idempotency_key,
        "idempotency_key",
        MAX_EVENT_KEY_LENGTH,
    )?;
    Ok(input)
}

fn ensure_same_delivery(
    existing: &provider_event::Model,
    input: &ReceiveProviderEvent,
    payload_hash: &str,
) -> PaymentResult<()> {
    if existing.tenant_id != input.tenant_id
        || existing.provider_id != input.provider_id
        || existing.delivery_id != input.delivery_id
        || existing.idempotency_key != input.idempotency_key
        || existing.payload_hash != payload_hash
        || !existing.signature_verified
    {
        return Err(PaymentError::Validation(format!(
            "provider event key `{}` is already bound to another delivery",
            input.idempotency_key
        )));
    }
    Ok(())
}

fn hash_payload(payload: &[u8]) -> String {
    format!("{:x}", Sha256::digest(payload))
}

fn normalize_required(
    value: String,
    label: &str,
    max_length: usize,
) -> PaymentResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > max_length {
        return Err(PaymentError::Validation(format!(
            "{label} must contain 1 to {max_length} bytes"
        )));
    }
    Ok(value)
}

fn normalize_optional(
    value: Option<String>,
    label: &str,
    max_length: usize,
) -> PaymentResult<Option<String>> {
    value
        .map(|value| normalize_required(value, label, max_length))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_hash_is_stable_and_lowercase() {
        assert_eq!(
            hash_payload(b"provider-event"),
            "bd83c3ad78c28abfcdb04352f1f68cce58809ac757b22bf96b673c7cd8a16f5c"
        );
    }

    #[test]
    fn unsigned_deliveries_are_rejected_before_storage() {
        let error = normalize_receive_input(ReceiveProviderEvent {
            tenant_id: Uuid::new_v4(),
            provider_id: "manual".to_string(),
            delivery_id: "delivery-1".to_string(),
            idempotency_key: "event-1".to_string(),
            raw_payload: b"{}".to_vec(),
            signature_verified: false,
        })
        .expect_err("unsigned provider delivery must fail");
        assert!(error.to_string().contains("signature"));
    }
}
