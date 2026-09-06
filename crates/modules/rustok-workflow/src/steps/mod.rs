use async_trait::async_trait;
use serde_json::Value;

use crate::error::WorkflowResult;

pub mod action;
pub mod alloy_script;
pub mod condition;
pub mod delay;
pub mod emit_event;
pub mod http;
pub mod notify;

pub use action::ActionStep;
pub use alloy_script::{AlloyScriptStep, ScriptRunner};
pub use condition::ConditionStep;
pub use delay::DelayStep;
pub use emit_event::EmitEventStep;
pub use http::HttpStep;
pub use notify::{NotificationSender, NotifyStep};

/// Context passed between workflow steps during execution.
/// Steps can read from and write to the context.
#[derive(Debug, Clone, Default)]
pub struct StepContext {
    /// Data available to steps — starts with trigger event payload, enriched by each step
    pub data: Value,
}

impl StepContext {
    pub fn new(data: Value) -> Self {
        Self { data }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    pub fn set(&mut self, key: impl Into<String>, value: Value) {
        if let Value::Object(ref mut map) = self.data {
            map.insert(key.into(), value);
        }
    }
}

/// Output produced by a single step execution.
#[derive(Debug, Clone)]
pub struct StepOutput {
    /// Updated context after this step (merged with previous)
    pub context: StepContext,
    /// Raw output data for logging
    pub data: Value,
    /// Whether execution should continue to the next step
    pub should_continue: bool,
}

impl StepOutput {
    pub fn continue_with(context: StepContext, data: Value) -> Self {
        Self {
            context,
            data,
            should_continue: true,
        }
    }

    pub fn stop_with(context: StepContext, data: Value) -> Self {
        Self {
            context,
            data,
            should_continue: false,
        }
    }
}

/// Trait implemented by every workflow step type.
#[async_trait]
pub trait WorkflowStep: Send + Sync {
    /// Name of this step type for logging
    fn step_type(&self) -> &'static str;

    /// Execute the step, returning updated context or an error.
    async fn execute(&self, config: &Value, context: StepContext) -> WorkflowResult<StepOutput>;
}
