use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_events::EventEnvelope;
use sea_orm::DatabaseTransaction;
use serde::{Deserialize, Serialize};

/// Object-safe transactional boundary for appending a domain event to the
/// platform outbox owned by the caller's database transaction.
#[async_trait]
pub trait TransactionalEventWriter: Send + Sync {
    async fn write_event(
        &self,
        transaction: &DatabaseTransaction,
        envelope: EventEnvelope,
    ) -> rustok_core::Result<()>;
}

/// Require shared write semantics for relay control calls.
pub fn require_outbox_relay_policy(context: &PortContext) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .map_err(|error| match error.kind {
            PortErrorKind::Timeout => PortError::timeout(
                "outbox.deadline_required",
                "outbox port calls require deadline semantics",
            ),
            PortErrorKind::Validation => PortError::validation(
                "outbox.idempotency_required",
                "outbox write-like port calls require an idempotency key",
            ),
            _ => error,
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxRelayRunOnceRequest {
    pub max_batch_hint: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxRelayRunOnceProjection {
    pub claimed_count: usize,
    pub success_total: u64,
    pub failure_total: u64,
    pub retry_total: u64,
    pub dlq_total: u64,
    pub processed_total: u64,
}

/// Transport-neutral owner boundary for operator/worker relay execution.
#[async_trait]
pub trait OutboxRelayPort: Send + Sync {
    async fn process_pending_once(
        &self,
        context: PortContext,
        request: OutboxRelayRunOnceRequest,
    ) -> Result<OutboxRelayRunOnceProjection, PortError>;
}

#[async_trait]
impl OutboxRelayPort for crate::OutboxRelay {
    async fn process_pending_once(
        &self,
        context: PortContext,
        request: OutboxRelayRunOnceRequest,
    ) -> Result<OutboxRelayRunOnceProjection, PortError> {
        require_outbox_relay_policy(&context)?;
        let claimed_count = self
            .process_pending_once(request.max_batch_hint)
            .await
            .map_err(map_outbox_error)?;
        let metrics = self.metrics();
        Ok(OutboxRelayRunOnceProjection {
            claimed_count,
            success_total: metrics.success_total,
            failure_total: metrics.failure_total,
            retry_total: metrics.retry_total,
            dlq_total: metrics.dlq_total,
            processed_total: metrics.processed_total,
        })
    }
}

fn map_outbox_error(error: rustok_core::Error) -> PortError {
    match error {
        rustok_core::Error::Validation(message) => {
            PortError::validation("outbox.max_batch_hint_invalid", message)
        }
        other => PortError::unavailable("outbox.relay_failed", other.to_string()),
    }
}
