use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Transport-agnostic context for outbox owner boundary calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortContext {
    pub tenant_id: String,
    pub correlation_id: String,
    pub deadline_ms: Option<u64>,
    pub idempotency_key: Option<String>,
}

impl PortContext {
    pub fn require_deadline_semantics(&self) -> Result<(), PortError> {
        if self.deadline_ms.unwrap_or_default() == 0 {
            return Err(PortError::new(
                PortErrorKind::Timeout,
                "outbox.deadline_required",
                "outbox port calls require deadline semantics",
                true,
            ));
        }
        Ok(())
    }

    pub fn require_write_semantics(&self) -> Result<(), PortError> {
        self.require_deadline_semantics()?;
        if self
            .idempotency_key
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(PortError::new(
                PortErrorKind::Validation,
                "outbox.idempotency_required",
                "outbox write-like port calls require an idempotency key",
                false,
            ));
        }
        Ok(())
    }
}

/// Transport-neutral error returned by outbox owner ports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortError {
    pub kind: PortErrorKind,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl PortError {
    pub fn new(
        kind: PortErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortErrorKind {
    Validation,
    Unavailable,
    Timeout,
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
        _request: OutboxRelayRunOnceRequest,
    ) -> Result<OutboxRelayRunOnceProjection, PortError> {
        context.require_write_semantics()?;
        let claimed_count = crate::OutboxRelay::process_pending_once(self)
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
    PortError::new(
        PortErrorKind::Unavailable,
        "outbox.relay_failed",
        error.to_string(),
        true,
    )
}
