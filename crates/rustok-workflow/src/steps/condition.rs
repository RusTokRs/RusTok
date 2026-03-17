use async_trait::async_trait;
use serde_json::Value;
use tracing::debug;

use super::{StepContext, StepOutput, WorkflowStep};
use crate::error::{WorkflowError, WorkflowResult};

/// Condition step — evaluates a simple equality condition against the context.
///
/// Config format:
/// ```json
/// {
///   "field": "event.status",
///   "operator": "eq",
///   "value": "paid",
///   "stop_on_false": true
/// }
/// ```
///
/// Supported operators: "eq", "ne", "exists", "not_exists"
pub struct ConditionStep;

#[async_trait]
impl WorkflowStep for ConditionStep {
    fn step_type(&self) -> &'static str {
        "condition"
    }

    async fn execute(&self, config: &Value, context: StepContext) -> WorkflowResult<StepOutput> {
        let field = config
            .get("field")
            .and_then(Value::as_str)
            .ok_or_else(|| WorkflowError::InvalidStepConfig("condition: missing 'field'".into()))?;

        let operator = config
            .get("operator")
            .and_then(Value::as_str)
            .unwrap_or("eq");

        let stop_on_false = config
            .get("stop_on_false")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        let actual = resolve_field(field, &context.data);

        let result = match operator {
            "eq" => {
                let expected = config.get("value");
                actual.as_deref() == expected
            }
            "ne" => {
                let expected = config.get("value");
                actual.as_deref() != expected
            }
            "exists" => actual.is_some(),
            "not_exists" => actual.is_none(),
            op => {
                return Err(WorkflowError::InvalidStepConfig(format!(
                    "condition: unknown operator '{op}'"
                )))
            }
        };

        debug!(field = field, operator = operator, result = result, "Condition evaluated");

        let output = serde_json::json!({ "field": field, "result": result });

        if result || !stop_on_false {
            Ok(StepOutput::continue_with(context, output))
        } else {
            Ok(StepOutput::stop_with(context, output))
        }
    }
}

/// Resolves a dot-notation path like "event.status" against a JSON value.
fn resolve_field<'a>(path: &str, data: &'a Value) -> Option<&'a Value> {
    path.split('.').fold(Some(data), |current, key| {
        current.and_then(|v| v.get(key))
    })
}
