use std::sync::Arc;
use uuid::Uuid;

use crate::entities::provider_event;
use crate::error::{PaymentError, PaymentResult};
use crate::providers::PaymentProviderWebhookResult;

use super::{
    CompleteProviderEvent, FailProviderEvent, PROVIDER_EVENT_DEAD_LETTER, PROVIDER_EVENT_PROCESSED,
    PaymentProviderEventApplier, PaymentProviderEventJournal,
};

const DEFAULT_RECOVERY_LEASE_SECONDS: i64 = 30;
const DEFAULT_RECOVERY_MAX_ATTEMPTS: i32 = 10;
const DEFAULT_RECOVERY_BATCH_LIMIT: u64 = 50;

#[derive(Clone, Debug)]
pub enum PaymentProviderEventRecoveryOutcome {
    Processed(provider_event::Model),
    Retryable(provider_event::Model),
    DeadLetter(provider_event::Model),
    InProgress(provider_event::Model),
}

#[derive(Clone, Debug, Default)]
pub struct PaymentProviderEventRecoveryReport {
    pub scanned: usize,
    pub processed: usize,
    pub retryable: usize,
    pub dead_letter: usize,
    pub in_progress: usize,
    pub errors: usize,
    pub failures: Vec<PaymentProviderEventRecoveryFailure>,
}

#[derive(Clone, Debug)]
pub struct PaymentProviderEventRecoveryFailure {
    pub event_id: Uuid,
    pub status: String,
    pub error_code: Option<String>,
}

pub struct PaymentProviderEventRecoveryService {
    journal: PaymentProviderEventJournal,
    applier: Arc<dyn PaymentProviderEventApplier>,
    lease_seconds: i64,
    max_attempts: i32,
}

impl PaymentProviderEventRecoveryService {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        applier: Arc<dyn PaymentProviderEventApplier>,
    ) -> Self {
        Self {
            journal: PaymentProviderEventJournal::new(db),
            applier,
            lease_seconds: DEFAULT_RECOVERY_LEASE_SECONDS,
            max_attempts: DEFAULT_RECOVERY_MAX_ATTEMPTS,
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

    pub async fn resume(
        &self,
        tenant_id: Uuid,
        event_id: Uuid,
        lease_owner: impl Into<String>,
    ) -> PaymentResult<PaymentProviderEventRecoveryOutcome> {
        let lease_owner = lease_owner.into();
        let Some(claimed) = self
            .journal
            .claim_processing(
                tenant_id,
                event_id,
                lease_owner.as_str(),
                self.lease_seconds,
            )
            .await?
        else {
            let current = self.journal.get(tenant_id, event_id).await?;
            return Ok(match current.status.as_str() {
                PROVIDER_EVENT_PROCESSED => PaymentProviderEventRecoveryOutcome::Processed(current),
                PROVIDER_EVENT_DEAD_LETTER => {
                    PaymentProviderEventRecoveryOutcome::DeadLetter(current)
                }
                _ => PaymentProviderEventRecoveryOutcome::InProgress(current),
            });
        };

        let normalized = match normalized_from_inbox(&claimed) {
            Ok(event) => event,
            Err(error) => {
                let failed = self
                    .journal
                    .mark_failed(FailProviderEvent {
                        tenant_id,
                        event_id,
                        lease_owner,
                        error_code: "payment.webhook_normalized_checkpoint_missing".to_string(),
                        error_message: error.to_string(),
                        retryable: false,
                        max_attempts: self.max_attempts,
                    })
                    .await?;
                return Ok(PaymentProviderEventRecoveryOutcome::DeadLetter(failed));
            }
        };
        let context = super::PaymentProviderEventContext {
            event_id: claimed.id,
            tenant_id: claimed.tenant_id,
            provider_id: claimed.provider_id.clone(),
            delivery_id: claimed.delivery_id.clone(),
            idempotency_key: claimed.idempotency_key.clone(),
        };

        if let Err(apply) = self.applier.apply(context, normalized.clone()).await {
            let failed = self
                .journal
                .mark_failed(FailProviderEvent {
                    tenant_id,
                    event_id,
                    lease_owner,
                    error_code: apply.code,
                    error_message: apply.message,
                    retryable: apply.retryable,
                    max_attempts: self.max_attempts,
                })
                .await?;
            return Ok(if failed.status == PROVIDER_EVENT_DEAD_LETTER {
                PaymentProviderEventRecoveryOutcome::DeadLetter(failed)
            } else {
                PaymentProviderEventRecoveryOutcome::Retryable(failed)
            });
        }

        let processed = self
            .journal
            .mark_processed(CompleteProviderEvent {
                tenant_id,
                event_id,
                lease_owner,
                event_type: normalized.event_type,
                external_reference: normalized.external_reference,
                event_metadata: normalized.metadata,
            })
            .await?;
        Ok(PaymentProviderEventRecoveryOutcome::Processed(processed))
    }

    pub async fn run(
        &self,
        tenant_id: Uuid,
        worker_id: &str,
        limit: Option<u64>,
    ) -> PaymentResult<PaymentProviderEventRecoveryReport> {
        let events = self
            .journal
            .list_retryable(tenant_id, limit.unwrap_or(DEFAULT_RECOVERY_BATCH_LIMIT))
            .await?;
        let mut report = PaymentProviderEventRecoveryReport {
            scanned: events.len(),
            ..Default::default()
        };

        for event in events {
            let lease_owner = format!("{worker_id}:{}:{}", event.id, Uuid::new_v4());
            match self.resume(tenant_id, event.id, lease_owner).await {
                Ok(PaymentProviderEventRecoveryOutcome::Processed(_)) => report.processed += 1,
                Ok(PaymentProviderEventRecoveryOutcome::Retryable(current)) => {
                    report.retryable += 1;
                    report.failures.push(safe_failure(current));
                }
                Ok(PaymentProviderEventRecoveryOutcome::DeadLetter(current)) => {
                    report.dead_letter += 1;
                    report.failures.push(safe_failure(current));
                }
                Ok(PaymentProviderEventRecoveryOutcome::InProgress(_)) => report.in_progress += 1,
                Err(error) => {
                    report.errors += 1;
                    report.failures.push(PaymentProviderEventRecoveryFailure {
                        event_id: event.id,
                        status: event.status,
                        error_code: Some(safe_recovery_error_code(&error).to_string()),
                    });
                }
            }
        }
        Ok(report)
    }

    pub fn journal(&self) -> &PaymentProviderEventJournal {
        &self.journal
    }
}

fn normalized_from_inbox(
    event: &provider_event::Model,
) -> PaymentResult<PaymentProviderWebhookResult> {
    let event_type = event.event_type.clone().ok_or_else(|| {
        PaymentError::Validation(format!(
            "payment provider event {} has no normalized event type",
            event.id
        ))
    })?;
    let metadata = event.event_metadata.clone().ok_or_else(|| {
        PaymentError::Validation(format!(
            "payment provider event {} has no normalized metadata",
            event.id
        ))
    })?;
    Ok(PaymentProviderWebhookResult {
        provider_id: event.provider_id.clone(),
        delivery_id: event.delivery_id.clone(),
        external_reference: event.external_reference.clone(),
        event_type,
        replay_key: event.idempotency_key.clone(),
        metadata,
    })
}

fn safe_failure(event: provider_event::Model) -> PaymentProviderEventRecoveryFailure {
    PaymentProviderEventRecoveryFailure {
        event_id: event.id,
        status: event.status,
        error_code: event.error_code,
    }
}

fn safe_recovery_error_code(error: &PaymentError) -> &'static str {
    match error {
        PaymentError::Database(_) => "payment.webhook_recovery_storage_unavailable",
        PaymentError::InvalidTransition { .. } => "payment.webhook_recovery_state_conflict",
        PaymentError::Validation(_) => "payment.webhook_recovery_validation_failed",
        PaymentError::ProviderUnavailable { .. } => "payment.webhook_recovery_provider_unavailable",
        PaymentError::ProviderRejected { .. } => "payment.webhook_recovery_provider_rejected",
        PaymentError::ProviderInvalidResponse { .. } => {
            "payment.webhook_recovery_provider_invalid_response"
        }
        PaymentError::ProviderOutcomeUnknown { .. } => {
            "payment.webhook_recovery_provider_outcome_unknown"
        }
        PaymentError::ProviderConfiguration { .. } => {
            "payment.webhook_recovery_provider_not_configured"
        }
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => "payment.webhook_recovery_owner_missing",
    }
}
