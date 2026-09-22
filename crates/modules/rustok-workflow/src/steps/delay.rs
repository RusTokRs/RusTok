use async_trait::async_trait;
use serde_json::Value;
use tracing::info;

use super::{StepContext, StepOutput, WorkflowStep};
use crate::error::{WorkflowError, WorkflowResult};

/// Delay step — pauses execution for a specified number of milliseconds.
///
/// For short delays (< 60s) this uses tokio::time::sleep.
/// Per architecture, longer delays should use scheduled events via EventTransport
/// (planned for Phase 4).
///
/// Step config format:
/// ```json
/// { "delay_ms": 5000 }
/// ```
pub struct DelayStep;

const MAX_INLINE_DELAY_MS: u64 = 60_000; // 60 seconds

#[async_trait]
impl WorkflowStep for DelayStep {
    fn step_type(&self) -> &'static str {
        "delay"
    }

    async fn execute(&self, config: &Value, context: StepContext) -> WorkflowResult<StepOutput> {
        let delay_ms = config
            .get("delay_ms")
            .and_then(Value::as_u64)
            .ok_or_else(|| WorkflowError::InvalidStepConfig("delay: missing 'delay_ms'".into()))?;

        if delay_ms > MAX_INLINE_DELAY_MS {
            return Err(WorkflowError::InvalidStepConfig(format!(
                "delay: delay_ms={delay_ms} exceeds max inline delay {MAX_INLINE_DELAY_MS}ms. \
                 Use EventTransport-based scheduling for long delays."
            )));
        }

        info!(delay_ms = delay_ms, "Executing delay step");

        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;

        Ok(StepOutput::continue_with(
            context,
            serde_json::json!({ "delayed_ms": delay_ms }),
        ))
    }
}
