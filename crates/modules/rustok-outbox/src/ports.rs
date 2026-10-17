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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn test_context() -> PortContext {
        PortContext::new(
            "tenant-1",
            rustok_api::PortActor::service("outbox-test"),
            "en",
            "outbox-test",
        )
    }

    #[test]
    fn require_outbox_relay_policy_valid() {
        let ctx = test_context()
            .with_idempotency_key("idem-123")
            .with_deadline(Duration::from_secs(5));
        assert!(require_outbox_relay_policy(&ctx).is_ok());
    }

    #[test]
    fn require_outbox_relay_policy_missing_idempotency() {
        let ctx = test_context().with_deadline(Duration::from_secs(5));
        let err = require_outbox_relay_policy(&ctx).unwrap_err();
        assert_eq!(err.kind, PortErrorKind::Validation);
        assert_eq!(err.code, "outbox.idempotency_required");
    }

    #[test]
    fn require_outbox_relay_policy_missing_deadline() {
        let ctx = test_context().with_idempotency_key("idem-123");
        let err = require_outbox_relay_policy(&ctx).unwrap_err();
        assert_eq!(err.kind, PortErrorKind::Timeout);
        assert_eq!(err.code, "outbox.deadline_required");
    }

    #[test]
    fn map_outbox_error_validation_and_internal() {
        let err_val = rustok_core::Error::Validation("bad batch hint".to_string());
        let mapped_val = map_outbox_error(err_val);
        assert_eq!(mapped_val.kind, PortErrorKind::Validation);
        assert_eq!(mapped_val.code, "outbox.max_batch_hint_invalid");

        let err_internal = rustok_core::Error::Internal("db unreachable".to_string());
        let mapped_internal = map_outbox_error(err_internal);
        assert_eq!(mapped_internal.kind, PortErrorKind::Unavailable);
        assert_eq!(mapped_internal.code, "outbox.relay_failed");
    }

    #[test]
    fn outbox_relay_request_and_projection_serde() {
        let req = OutboxRelayRunOnceRequest {
            max_batch_hint: Some(42),
        };
        let req_json = serde_json::to_string(&req).expect("serialize request");
        let req_de: OutboxRelayRunOnceRequest =
            serde_json::from_str(&req_json).expect("deserialize request");
        assert_eq!(req, req_de);

        let proj = OutboxRelayRunOnceProjection {
            claimed_count: 5,
            success_total: 4,
            failure_total: 1,
            retry_total: 2,
            dlq_total: 0,
            processed_total: 5,
        };
        let proj_json = serde_json::to_string(&proj).expect("serialize projection");
        let proj_de: OutboxRelayRunOnceProjection =
            serde_json::from_str(&proj_json).expect("deserialize projection");
        assert_eq!(proj, proj_de);
    }
}
