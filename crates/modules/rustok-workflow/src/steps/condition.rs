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
            .or_else(|| {
                config
                    .get("on_false")
                    .and_then(Value::as_str)
                    .map(|s| s.eq_ignore_ascii_case("stop"))
            })
            .unwrap_or(true);

        let actual = resolve_field(field, &context.data);

        let result = match operator {
            "eq" | "equals" | "==" => {
                let expected = config.get("value");
                actual == expected
            }
            "ne" | "not_equals" | "!=" => {
                let expected = config.get("value");
                actual != expected
            }
            "exists" => actual.is_some(),
            "not_exists" => actual.is_none(),
            op => {
                return Err(WorkflowError::InvalidStepConfig(format!(
                    "condition: unknown operator '{op}'"
                )));
            }
        };

        debug!(
            field = field,
            operator = operator,
            result = result,
            "Condition evaluated"
        );

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
    path.split('.')
        .try_fold(data, |current, key| current.get(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_condition_equals_aliases() {
        let step = ConditionStep;
        let context = StepContext::new(json!({
            "webhook": {
                "payload": {
                    "event": "contact.updated"
                }
            }
        }));

        for op in ["eq", "equals", "=="] {
            let config = json!({
                "field": "webhook.payload.event",
                "operator": op,
                "value": "contact.updated",
                "on_false": "stop"
            });
            let out = step.execute(&config, context.clone()).await.unwrap();
            assert!(out.should_continue);
            assert_eq!(out.data["result"], true);
        }
    }

    #[tokio::test]
    async fn test_condition_stop_on_false() {
        let step = ConditionStep;
        let context = StepContext::new(json!({ "status": "pending" }));

        let config = json!({
            "field": "status",
            "operator": "eq",
            "value": "completed",
            "stop_on_false": true
        });
        let out = step.execute(&config, context.clone()).await.unwrap();
        assert!(!out.should_continue);
        assert_eq!(out.data["result"], false);

        // With on_false: "stop"
        let config_alt = json!({
            "field": "status",
            "operator": "eq",
            "value": "completed",
            "on_false": "stop"
        });
        let out_alt = step.execute(&config_alt, context.clone()).await.unwrap();
        assert!(!out_alt.should_continue);

        // When stop_on_false is false, execution continues even if condition is false
        let config_continue = json!({
            "field": "status",
            "operator": "eq",
            "value": "completed",
            "stop_on_false": false
        });
        let out_continue = step.execute(&config_continue, context).await.unwrap();
        assert!(out_continue.should_continue);
    }

    #[tokio::test]
    async fn test_condition_exists_and_not_exists() {
        let step = ConditionStep;
        let context = StepContext::new(json!({ "user": { "id": "123" } }));

        let config_exists = json!({
            "field": "user.id",
            "operator": "exists"
        });
        let out = step.execute(&config_exists, context.clone()).await.unwrap();
        assert!(out.should_continue);
        assert_eq!(out.data["result"], true);

        let config_not_exists = json!({
            "field": "user.missing",
            "operator": "not_exists"
        });
        let out = step.execute(&config_not_exists, context).await.unwrap();
        assert!(out.should_continue);
        assert_eq!(out.data["result"], true);
    }

    #[tokio::test]
    async fn test_condition_unknown_operator() {
        let step = ConditionStep;
        let context = StepContext::default();
        let config = json!({
            "field": "foo",
            "operator": "unknown_op"
        });
        let err = step.execute(&config, context).await.unwrap_err();
        assert!(matches!(err, WorkflowError::InvalidStepConfig(_)));
    }
}
