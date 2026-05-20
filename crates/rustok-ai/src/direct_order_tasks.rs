#![cfg(feature = "server")]

use async_trait::async_trait;
use loco_rs::app::AppContext;

use crate::direct::{DirectExecutionRequest, DirectExecutionResult, DirectTaskHandler};
use crate::model::{AiOrderAnalyticsTaskInput, AiOrderOpsAssistantTaskInput};
use crate::service::AiOperatorContext;
use crate::{AiError, AiResult};

pub struct OrderAnalyticsHandler;
pub struct OrderOpsAssistantHandler;

#[async_trait]
impl DirectTaskHandler for OrderAnalyticsHandler {
    fn task_slug(&self) -> &'static str { "order_analytics" }
    async fn execute(&self, _app_ctx: &AppContext, _operator: &AiOperatorContext, request: DirectExecutionRequest) -> AiResult<DirectExecutionResult> {
        let _input: AiOrderAnalyticsTaskInput = serde_json::from_value(request.task_input_json).map_err(AiError::Json)?;
        Err(AiError::Validation("order_analytics direct handler is not implemented yet".to_string()))
    }
}

#[async_trait]
impl DirectTaskHandler for OrderOpsAssistantHandler {
    fn task_slug(&self) -> &'static str { "order_ops_assistant" }
    async fn execute(&self, _app_ctx: &AppContext, _operator: &AiOperatorContext, request: DirectExecutionRequest) -> AiResult<DirectExecutionResult> {
        let _input: AiOrderOpsAssistantTaskInput = serde_json::from_value(request.task_input_json).map_err(AiError::Json)?;
        Err(AiError::Validation("order_ops_assistant direct handler is not implemented yet".to_string()))
    }
}
