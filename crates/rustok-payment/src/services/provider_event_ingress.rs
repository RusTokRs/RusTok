use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use crate::entities::provider_event;
use crate::error::PaymentError;
use crate::providers::{
    PaymentProviderError, PaymentProviderRegistry, PaymentProviderWebhookRequest,
    PaymentProviderWebhookResult,
};

use super::{
    CompleteProviderEvent, FailProviderEvent, PaymentProviderEventJournal, ReceiveProviderEvent,
    PROVIDER_EVENT_DEAD_LETTER, PROVIDER_EVENT_PROCESSED, PROVIDER_EVENT_PROCESSING,
};

const DEFAULT_WEBHOOK_LEASE_SECONDS: i64 = 30;
const DEFAULT_WEBHOOK_MAX_ATTEMPTS: i32 = 10;

#[derive(Clone, Debug)]
pub struct PaymentProviderEventContext {
    pub event_id: Uuid,
    pub tenant_id: Uuid,
    pub provider_id: String,
    pub delivery_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Error)]
#[error("{code}: {message}")]
pub struct PaymentProviderEventApplyError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl PaymentProviderEventApplyError {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[async_trait]
pub trait PaymentProviderEventApplier: Send + Sync {
    async fn apply(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError>;
}

#[derive(Clone, Debug)]
pub struct PaymentProviderEventExecution {
    pub inbox_event: provider_event::Model,
    pub provider_event: PaymentProviderWebhookResult,
    pub replayed: bool,
}

#[derive(Debug, Error)]
pub enum PaymentProviderEventIngressError {
    #[error(transparent)]
    Provider(#[from] PaymentProviderError),
    #[error(transparent)]
    Payment(#[from] PaymentError),
    #[error("payment provider event {0} is currently processing")]
    InProgress(Uuid),
    #[error("payment provider event {0} is in the dead-letter queue")]
    DeadLetter(Uuid),
    #[error("payment provider event apply failed: {0}")]
    Apply(#[from] PaymentProviderEventApplyError),
    #[error(
        "payment provider event apply failed: {apply}; recording the failure also failed: {journal}"
    )]
    ApplyAndJournal {
        apply: PaymentProviderEventApplyError,
        journal: PaymentError,
    },
}

pub type PaymentProviderEventIngressResult<T> = Result<T, PaymentProviderEventIngressError>;

pub struct PaymentProviderEventIngressService {
    registry: PaymentProviderRegistry,
    journal: PaymentProviderEventJournal,
    applier: Arc<dyn PaymentProviderEventApplier>,
    lease_seconds: i64,
    max_attempts: i32,
}

impl PaymentProviderEventIngressService {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        registry: PaymentProviderRegistry,
        applier: Arc<dyn PaymentProviderEventApplier>,
    ) -> Self {
        Self {
            registry,
            journal: PaymentProviderEventJournal::new(db),
            applier,
            lease_seconds: DEFAULT_WEBHOOK_LEASE_SECONDS,
            max_attempts: DEFAULT_WEBHOOK_MAX_ATTEMPTS,
        }
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    pub fn with_max_attempts(mut self, max_attempts: i32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Verifies/parses the provider request before storage, records only a
    /// payload hash, claims the durable inbox event, applies the normalized
    /// event through an owner command, and marks it processed afterward.
    pub async fn ingest(
        &self,
        request: PaymentProviderWebhookRequest,
        lease_owner: impl Into<String>,
    ) -> PaymentProviderEventIngressResult<PaymentProviderEventExecution> {
        let normalized = self
            .registry
            .execute_webhook(request.provider_id.as_str(), request.clone())
            .await?;
        let inbox_event = self
            .journal
            .receive(ReceiveProviderEvent {
                tenant_id: request.tenant_id,
                provider_id: request.provider_id.clone(),
                delivery_id: request.delivery_id.clone(),
                idempotency_key: request.idempotency_key.clone(),
                raw_payload: request.raw_payload.clone(),
                signature_verified: true,
            })
            .await?;

        if inbox_event.status == PROVIDER_EVENT_PROCESSED {
            return Ok(PaymentProviderEventExecution {
                inbox_event,
                provider_event: normalized,
                replayed: true,
            });
        }
        if inbox_event.status == PROVIDER_EVENT_DEAD_LETTER {
            return Err(PaymentProviderEventIngressError::DeadLetter(inbox_event.id));
        }

        let lease_owner = lease_owner.into();
        let Some(claimed) = self
            .journal
            .claim_processing(
                request.tenant_id,
                inbox_event.id,
                lease_owner.as_str(),
                self.lease_seconds,
            )
            .await?
        else {
            let current = self.journal.get(request.tenant_id, inbox_event.id).await?;
            return match current.status.as_str() {
                PROVIDER_EVENT_PROCESSED => Ok(PaymentProviderEventExecution {
                    inbox_event: current,
                    provider_event: normalized,
                    replayed: true,
                }),
                PROVIDER_EVENT_DEAD_LETTER => {
                    Err(PaymentProviderEventIngressError::DeadLetter(current.id))
                }
                PROVIDER_EVENT_PROCESSING => {
                    Err(PaymentProviderEventIngressError::InProgress(current.id))
                }
                _ => Err(PaymentProviderEventIngressError::InProgress(current.id)),
            };
        };

        let context = PaymentProviderEventContext {
            event_id: claimed.id,
            tenant_id: claimed.tenant_id,
            provider_id: claimed.provider_id.clone(),
            delivery_id: claimed.delivery_id.clone(),
            idempotency_key: claimed.idempotency_key.clone(),
        };
        if let Err(apply) = self.applier.apply(context, normalized.clone()).await {
            let failure = self
                .journal
                .mark_failed(FailProviderEvent {
                    tenant_id: request.tenant_id,
                    event_id: claimed.id,
                    lease_owner,
                    error_code: apply.code.clone(),
                    error_message: apply.message.clone(),
                    retryable: apply.retryable,
                    max_attempts: self.max_attempts,
                })
                .await;
            return match failure {
                Ok(_) => Err(apply.into()),
                Err(journal) => Err(PaymentProviderEventIngressError::ApplyAndJournal {
                    apply,
                    journal,
                }),
            };
        }

        let processed = self
            .journal
            .mark_processed(CompleteProviderEvent {
                tenant_id: request.tenant_id,
                event_id: claimed.id,
                lease_owner,
                event_type: normalized.event_type.clone(),
                external_reference: normalized.external_reference.clone(),
                event_metadata: normalized.metadata.clone(),
            })
            .await?;
        Ok(PaymentProviderEventExecution {
            inbox_event: processed,
            provider_event: normalized,
            replayed: false,
        })
    }

    pub fn journal(&self) -> &PaymentProviderEventJournal {
        &self.journal
    }
}
