use chrono::{DateTime, Duration, FixedOffset, Utc};
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, sea_query::Expr,
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
const MAX_RAW_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_EVENT_METADATA_BYTES: usize = 64 * 1024;
const MAX_EVENT_METADATA_DEPTH: usize = 16;
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
pub struct VerifiedProviderEvent {
    pub event_type: String,
    pub external_reference: Option<String>,
    pub event_metadata: Value,
}

#[derive(Clone, Debug)]
pub struct CheckpointProviderEvent {
    pub tenant_id: Uuid,
    pub event_id: Uuid,
    pub lease_owner: String,
    pub event_type: String,
    pub external_reference: Option<String>,
    pub event_metadata: Value,
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

    /// Low-level receipt used by journal tests and compatibility callers. New
    /// webhook ingress should use `receive_verified` so normalized facts are
    /// durable in the same insert as the payload digest and delivery identity.
    pub async fn receive(
        &self,
        input: ReceiveProviderEvent,
    ) -> PaymentResult<provider_event::Model> {
        self.receive_internal(input, None).await
    }

    /// Persist signature-verified normalized facts atomically with the inbox
    /// receipt. The raw body is only hashed and is never stored.
    pub async fn receive_verified(
        &self,
        input: ReceiveProviderEvent,
        normalized: VerifiedProviderEvent,
    ) -> PaymentResult<provider_event::Model> {
        self.receive_internal(input, Some(normalized)).await
    }

    async fn receive_internal(
        &self,
        input: ReceiveProviderEvent,
        normalized: Option<VerifiedProviderEvent>,
    ) -> PaymentResult<provider_event::Model> {
        let input = normalize_receive_input(input)?;
        let normalized = normalized.map(normalize_verified_event).transpose()?;
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
            return self
                .adopt_existing(existing, &input, payload_hash.as_str(), normalized.as_ref())
                .await;
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
            event_type: Set(normalized.as_ref().map(|value| value.event_type.clone())),
            external_reference: Set(normalized
                .as_ref()
                .and_then(|value| value.external_reference.clone())),
            event_metadata: Set(normalized
                .as_ref()
                .map(|value| value.event_metadata.clone())),
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
                    self.adopt_existing(
                        existing,
                        &input,
                        payload_hash.as_str(),
                        normalized.as_ref(),
                    )
                    .await
                } else {
                    Err(insert_error.into())
                }
            }
        }
    }

    async fn adopt_existing(
        &self,
        existing: provider_event::Model,
        input: &ReceiveProviderEvent,
        payload_hash: &str,
        normalized: Option<&VerifiedProviderEvent>,
    ) -> PaymentResult<provider_event::Model> {
        ensure_same_delivery(&existing, input, payload_hash)?;
        let Some(normalized) = normalized else {
            return Ok(existing);
        };

        if existing.event_type.is_none() && existing.event_metadata.is_none() {
            let update = provider_event::Entity::update_many()
                .col_expr(
                    provider_event::Column::EventType,
                    Expr::value(Some(normalized.event_type.clone())),
                )
                .col_expr(
                    provider_event::Column::ExternalReference,
                    Expr::value(normalized.external_reference.clone()),
                )
                .col_expr(
                    provider_event::Column::EventMetadata,
                    Expr::value(Some(normalized.event_metadata.clone())),
                )
                .col_expr(
                    provider_event::Column::UpdatedAt,
                    Expr::current_timestamp().into(),
                )
                .filter(provider_event::Column::TenantId.eq(existing.tenant_id))
                .filter(provider_event::Column::Id.eq(existing.id))
                .filter(provider_event::Column::EventType.is_null())
                .filter(provider_event::Column::EventMetadata.is_null())
                .exec(&self.db)
                .await?;
            if update.rows_affected > 0 {
                return self.get(existing.tenant_id, existing.id).await;
            }
            let current = self.get(existing.tenant_id, existing.id).await?;
            ensure_same_normalized_event(&current, normalized)?;
            return Ok(current);
        }

        ensure_same_normalized_event(&existing, normalized)?;
        Ok(existing)
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
                PaymentError::Validation(format!("payment provider event {event_id} was not found"))
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

    pub async fn claim_processing(
        &self,
        tenant_id: Uuid,
        event_id: Uuid,
        lease_owner: impl Into<String>,
        lease_seconds: i64,
    ) -> PaymentResult<Option<provider_event::Model>> {
        let lease_owner =
            normalize_required(lease_owner.into(), "lease_owner", MAX_LEASE_OWNER_LENGTH)?;
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
                provider_event::Column::ProcessedAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                provider_event::Column::UpdatedAt,
                Expr::current_timestamp().into(),
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

    pub async fn claim_dead_letter_replay(
        &self,
        tenant_id: Uuid,
        event_id: Uuid,
        lease_owner: impl Into<String>,
        lease_seconds: i64,
    ) -> PaymentResult<Option<provider_event::Model>> {
        let lease_owner =
            normalize_required(lease_owner.into(), "lease_owner", MAX_LEASE_OWNER_LENGTH)?;
        let lease_seconds = lease_seconds.clamp(1, MAX_LEASE_SECONDS);
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);
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
                provider_event::Column::ProcessedAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                provider_event::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(provider_event::Column::TenantId.eq(tenant_id))
            .filter(provider_event::Column::Id.eq(event_id))
            .filter(provider_event::Column::Status.eq(PROVIDER_EVENT_DEAD_LETTER))
            .filter(provider_event::Column::EventType.is_not_null())
            .filter(provider_event::Column::EventMetadata.is_not_null())
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, event_id).await.map(Some)
    }

    /// Compatibility checkpoint for pre-inbox-normalization callers and tests.
    pub async fn checkpoint_normalized(
        &self,
        input: CheckpointProviderEvent,
    ) -> PaymentResult<provider_event::Model> {
        let lease_owner =
            normalize_required(input.lease_owner, "lease_owner", MAX_LEASE_OWNER_LENGTH)?;
        let normalized = normalize_verified_event(VerifiedProviderEvent {
            event_type: input.event_type,
            external_reference: input.external_reference,
            event_metadata: input.event_metadata,
        })?;
        let now = Utc::now().fixed_offset();
        let update = provider_event::Entity::update_many()
            .col_expr(
                provider_event::Column::EventType,
                Expr::value(Some(normalized.event_type)),
            )
            .col_expr(
                provider_event::Column::ExternalReference,
                Expr::value(normalized.external_reference),
            )
            .col_expr(
                provider_event::Column::EventMetadata,
                Expr::value(Some(normalized.event_metadata)),
            )
            .col_expr(
                provider_event::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(provider_event::Column::TenantId.eq(input.tenant_id))
            .filter(provider_event::Column::Id.eq(input.event_id))
            .filter(provider_event::Column::Status.eq(PROVIDER_EVENT_PROCESSING))
            .filter(provider_event::Column::LeaseOwner.eq(lease_owner))
            .filter(provider_event::Column::LeaseExpiresAt.gt(now))
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return self
                .transition_conflict(input.tenant_id, input.event_id, PROVIDER_EVENT_PROCESSING)
                .await;
        }
        self.get(input.tenant_id, input.event_id).await
    }

    pub async fn mark_processed(
        &self,
        input: CompleteProviderEvent,
    ) -> PaymentResult<provider_event::Model> {
        let lease_owner =
            normalize_required(input.lease_owner, "lease_owner", MAX_LEASE_OWNER_LENGTH)?;
        let normalized = normalize_verified_event(VerifiedProviderEvent {
            event_type: input.event_type,
            external_reference: input.external_reference,
            event_metadata: input.event_metadata,
        })?;
        let now = Utc::now().fixed_offset();

        let update = provider_event::Entity::update_many()
            .col_expr(
                provider_event::Column::Status,
                Expr::value(PROVIDER_EVENT_PROCESSED),
            )
            .col_expr(
                provider_event::Column::EventType,
                Expr::value(Some(normalized.event_type)),
            )
            .col_expr(
                provider_event::Column::ExternalReference,
                Expr::value(normalized.external_reference),
            )
            .col_expr(
                provider_event::Column::EventMetadata,
                Expr::value(Some(normalized.event_metadata)),
            )
            .col_expr(
                provider_event::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_event::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                provider_event::Column::ErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_event::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(provider_event::Column::ProcessedAt, Expr::value(Some(now)))
            .col_expr(
                provider_event::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(provider_event::Column::TenantId.eq(input.tenant_id))
            .filter(provider_event::Column::Id.eq(input.event_id))
            .filter(provider_event::Column::Status.eq(PROVIDER_EVENT_PROCESSING))
            .filter(provider_event::Column::LeaseOwner.eq(lease_owner))
            .filter(provider_event::Column::LeaseExpiresAt.gt(now))
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            return self
                .transition_conflict(input.tenant_id, input.event_id, PROVIDER_EVENT_PROCESSED)
                .await;
        }
        self.get(input.tenant_id, input.event_id).await
    }

    pub async fn mark_failed(
        &self,
        input: FailProviderEvent,
    ) -> PaymentResult<provider_event::Model> {
        let lease_owner =
            normalize_required(input.lease_owner, "lease_owner", MAX_LEASE_OWNER_LENGTH)?;
        let error_code = normalize_required(input.error_code, "error_code", MAX_ERROR_CODE_LENGTH)?;
        let error_message = normalize_required(
            input.error_message,
            "error_message",
            MAX_ERROR_MESSAGE_LENGTH,
        )?;
        let max_attempts = input.max_attempts.clamp(1, MAX_FAILURE_ATTEMPTS);
        let current = self.get(input.tenant_id, input.event_id).await?;
        let dead_letter = !input.retryable || current.attempt_count >= max_attempts;
        let target = if dead_letter {
            PROVIDER_EVENT_DEAD_LETTER
        } else {
            PROVIDER_EVENT_FAILED
        };
        let now = Utc::now().fixed_offset();
        let processed_at = dead_letter.then_some(now);

        let update = provider_event::Entity::update_many()
            .col_expr(provider_event::Column::Status, Expr::value(target))
            .col_expr(
                provider_event::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_event::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                provider_event::Column::ErrorCode,
                Expr::value(Some(error_code)),
            )
            .col_expr(
                provider_event::Column::ErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                provider_event::Column::ProcessedAt,
                Expr::value(processed_at),
            )
            .col_expr(
                provider_event::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(provider_event::Column::TenantId.eq(input.tenant_id))
            .filter(provider_event::Column::Id.eq(input.event_id))
            .filter(provider_event::Column::Status.eq(PROVIDER_EVENT_PROCESSING))
            .filter(provider_event::Column::LeaseOwner.eq(lease_owner))
            .filter(provider_event::Column::LeaseExpiresAt.gt(now))
            .filter(provider_event::Column::AttemptCount.eq(current.attempt_count))
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            return self
                .transition_conflict(input.tenant_id, input.event_id, target)
                .await;
        }
        self.get(input.tenant_id, input.event_id).await
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
                            .add(provider_event::Column::Status.eq(PROVIDER_EVENT_PROCESSING))
                            .add(provider_event::Column::LeaseExpiresAt.lte(now)),
                    ),
            )
            .order_by_asc(provider_event::Column::UpdatedAt)
            .limit(limit.clamp(1, 100))
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn list_dead_letters(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> PaymentResult<Vec<provider_event::Model>> {
        provider_event::Entity::find()
            .filter(provider_event::Column::TenantId.eq(tenant_id))
            .filter(provider_event::Column::Status.eq(PROVIDER_EVENT_DEAD_LETTER))
            .order_by_desc(provider_event::Column::UpdatedAt)
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
    if input.raw_payload.is_empty() || input.raw_payload.len() > MAX_RAW_PAYLOAD_BYTES {
        return Err(PaymentError::Validation(format!(
            "payment provider event payload must contain 1 to {MAX_RAW_PAYLOAD_BYTES} bytes"
        )));
    }
    input.provider_id =
        normalize_required(input.provider_id, "provider_id", MAX_PROVIDER_ID_LENGTH)?;
    input.delivery_id = normalize_required(input.delivery_id, "delivery_id", MAX_EVENT_KEY_LENGTH)?;
    input.idempotency_key = normalize_required(
        input.idempotency_key,
        "idempotency_key",
        MAX_EVENT_KEY_LENGTH,
    )?;
    Ok(input)
}

fn normalize_verified_event(
    mut event: VerifiedProviderEvent,
) -> PaymentResult<VerifiedProviderEvent> {
    event.event_type = normalize_required(event.event_type, "event_type", MAX_EVENT_TYPE_LENGTH)?;
    event.external_reference = normalize_optional(
        event.external_reference,
        "external_reference",
        MAX_EXTERNAL_REFERENCE_LENGTH,
    )?;
    event.event_metadata = normalize_event_metadata(event.event_metadata)?;
    Ok(event)
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

fn ensure_same_normalized_event(
    existing: &provider_event::Model,
    normalized: &VerifiedProviderEvent,
) -> PaymentResult<()> {
    if existing.event_type.as_deref() != Some(normalized.event_type.as_str())
        || existing.external_reference != normalized.external_reference
        || existing.event_metadata.as_ref() != Some(&normalized.event_metadata)
    {
        return Err(PaymentError::Validation(format!(
            "provider event {} is already bound to different normalized facts",
            existing.id
        )));
    }
    Ok(())
}

fn hash_payload(payload: &[u8]) -> String {
    Sha256::digest(payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn normalize_required(value: String, label: &str, max_length: usize) -> PaymentResult<String> {
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

fn normalize_event_metadata(value: Value) -> PaymentResult<Value> {
    if !value.is_object() {
        return Err(PaymentError::Validation(
            "payment provider event metadata must be an object".to_string(),
        ));
    }
    let encoded = serde_json::to_vec(&value).map_err(|error| {
        PaymentError::Validation(format!(
            "payment provider event metadata could not be encoded: {error}"
        ))
    })?;
    if encoded.len() > MAX_EVENT_METADATA_BYTES || json_depth(&value) > MAX_EVENT_METADATA_DEPTH {
        return Err(PaymentError::Validation(
            "payment provider event metadata exceeds size or depth limits".to_string(),
        ));
    }
    Ok(value)
}

fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(values) => 1 + values.iter().map(json_depth).max().unwrap_or(0),
        Value::Object(values) => 1 + values.values().map(json_depth).max().unwrap_or(0),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_hash_is_stable_and_lowercase() {
        assert_eq!(
            hash_payload(b"provider-event"),
            "a205dcfc615aaecccec56e39b0ef2f028ac49c0cc4a94385549b4f73a0e88037"
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

    #[test]
    fn metadata_is_bounded() {
        assert!(normalize_event_metadata(serde_json::json!({"event": "captured"})).is_ok());
        let mut nested = serde_json::json!({});
        for _ in 0..20 {
            nested = serde_json::json!({"next": nested});
        }
        assert!(normalize_event_metadata(nested).is_err());
    }
}
