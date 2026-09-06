use async_trait::async_trait;
use serde_json::Value;
use tracing::info;

use super::{StepContext, StepOutput, WorkflowStep};
use crate::error::WorkflowResult;

/// Emit event step — records an event to be published via EventBus.
/// The engine is responsible for actually publishing the event using the
/// event type and payload defined in the step config.
pub struct EmitEventStep;

#[async_trait]
impl WorkflowStep for EmitEventStep {
    fn step_type(&self) -> &'static str {
        "emit_event"
    }

    async fn execute(&self, config: &Value, context: StepContext) -> WorkflowResult<StepOutput> {
        let event_type = config
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or("workflow.event.emitted");

        info!(
            event_type = event_type,
            "Emit event step: scheduling event emission"
        );

        let output = serde_json::json!({
            "event_type": event_type,
            "status": "emitted"
        });

        Ok(StepOutput::continue_with(context, output))
    }
}
