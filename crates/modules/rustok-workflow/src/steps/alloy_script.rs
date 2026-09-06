use async_trait::async_trait;
use serde_json::Value;
use tracing::info;

use super::{StepContext, StepOutput, WorkflowStep};
use crate::error::{WorkflowError, WorkflowResult};

/// Abstract interface for running Alloy/Rhai scripts from a workflow step.
/// The application registers a concrete implementation that delegates to
/// `ScriptOrchestrator`.
#[async_trait]
pub trait ScriptRunner: Send + Sync {
    /// Run a script by name with the provided JSON params.
    /// Returns the script output as a JSON value.
    async fn run_script(&self, script_name: &str, params: Value) -> WorkflowResult<Value>;
}

/// Workflow step that executes a Rhai script via the Alloy scripting engine.
///
/// Step config format:
/// ```json
/// {
///   "script_name": "generate_invoice",
///   "params": { "order_id": "{{context.order_id}}" }
/// }
/// ```
pub struct AlloyScriptStep {
    runner: Option<std::sync::Arc<dyn ScriptRunner>>,
}

impl AlloyScriptStep {
    pub fn new(runner: std::sync::Arc<dyn ScriptRunner>) -> Self {
        Self {
            runner: Some(runner),
        }
    }

    /// Creates a stub step that logs a warning when no runner is registered.
    pub fn stub() -> Self {
        Self { runner: None }
    }
}

#[async_trait]
impl WorkflowStep for AlloyScriptStep {
    fn step_type(&self) -> &'static str {
        "alloy_script"
    }

    async fn execute(&self, config: &Value, context: StepContext) -> WorkflowResult<StepOutput> {
        let script_name = config
            .get("script_name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                WorkflowError::InvalidStepConfig("alloy_script: missing 'script_name'".into())
            })?;

        let params = config
            .get("params")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));

        let runner = self.runner.as_ref().ok_or_else(|| {
            WorkflowError::StepFailed(format!(
                "alloy_script: no ScriptRunner registered for script '{script_name}'"
            ))
        })?;

        info!(script_name = script_name, "Executing alloy_script step");

        let result = runner.run_script(script_name, params).await?;

        let mut new_context = context.clone();
        new_context.set("alloy_result", result.clone());

        Ok(StepOutput::continue_with(
            new_context,
            serde_json::json!({ "script": script_name, "result": result }),
        ))
    }
}
