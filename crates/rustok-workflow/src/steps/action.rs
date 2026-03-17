use async_trait::async_trait;
use serde_json::Value;
use tracing::info;

use super::{StepContext, StepOutput, WorkflowStep};
use crate::error::WorkflowResult;

/// Action step — logs the action config and passes context through.
/// In a full implementation this would call registered module service methods.
pub struct ActionStep;

#[async_trait]
impl WorkflowStep for ActionStep {
    fn step_type(&self) -> &'static str {
        "action"
    }

    async fn execute(&self, config: &Value, context: StepContext) -> WorkflowResult<StepOutput> {
        let action = config
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        info!(action = action, "Executing action step");

        Ok(StepOutput::continue_with(context, serde_json::json!({ "action": action, "status": "ok" })))
    }
}
