#![cfg(feature = "server")]

use async_trait::async_trait;
use chrono::Utc;
use loco_rs::app::AppContext;
use serde_json::json;

use crate::direct::{
    explain_result, generate_order_analytics, generate_order_ops_assistant, DirectExecutionRequest,
    DirectExecutionResult, DirectTaskHandler,
};
use crate::model::{AiOrderAnalyticsTaskInput, AiOrderOpsAssistantTaskInput};
use crate::model::{DirectExecutionTarget, ToolTrace};
use crate::service::AiOperatorContext;
use crate::{AiError, AiResult};
use rustok_ai_order::{
    ORDER_ANALYTICS_TASK_SLUG, ORDER_ANALYTICS_TOOL_NAME, ORDER_OPS_ASSISTANT_TASK_SLUG,
    ORDER_OPS_ASSISTANT_TOOL_NAME,
};

pub struct OrderAnalyticsHandler;
pub struct OrderOpsAssistantHandler;

#[async_trait]
impl DirectTaskHandler for OrderAnalyticsHandler {
    fn task_slug(&self) -> &'static str {
        ORDER_ANALYTICS_TASK_SLUG
    }
    async fn execute(
        &self,
        _app_ctx: &AppContext,
        _operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiOrderAnalyticsTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let started = std::time::Instant::now();
        let generated = generate_order_analytics(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            &input,
        )
        .await?;
        let operation_payload = serde_json::to_value(&generated).map_err(AiError::Json)?;
        let summary = "Prepared order analytics summary with findings and risk flags.".to_string();
        let trace = ToolTrace {
            tool_name: ORDER_ANALYTICS_TOOL_NAME.to_string(),
            input_payload: request.task_input_json.clone(),
            output_payload: Some(operation_payload.clone()),
            status: "completed".to_string(),
            duration_ms: started.elapsed().as_millis() as i64,
            sensitive: false,
            error_message: None,
            created_at: Utc::now(),
        };
        let explanation = explain_result(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            input.assistant_prompt.as_deref(),
            &summary,
            &operation_payload,
            request.stream_emitter.clone(),
        )
        .await;
        Ok(DirectExecutionResult {
            execution_target: DirectExecutionTarget::Orders,
            appended_messages: vec![explanation],
            traces: vec![trace],
            metadata: json!({"direct_task": request.task_slug,"requested_locale": request.requested_locale,"resolved_locale": request.resolved_locale,"order_analytics": operation_payload,}),
        })
    }
}

#[async_trait]
impl DirectTaskHandler for OrderOpsAssistantHandler {
    fn task_slug(&self) -> &'static str {
        ORDER_OPS_ASSISTANT_TASK_SLUG
    }
    async fn execute(
        &self,
        _app_ctx: &AppContext,
        _operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiOrderOpsAssistantTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let started = std::time::Instant::now();
        let generated = generate_order_ops_assistant(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            &input,
        )
        .await?;
        let operation_payload = serde_json::to_value(&generated).map_err(AiError::Json)?;
        let summary = format!(
            "Prepared order operation suggestion: {}.",
            generated.recommended_action
        );
        let trace = ToolTrace {
            tool_name: ORDER_OPS_ASSISTANT_TOOL_NAME.to_string(),
            input_payload: request.task_input_json.clone(),
            output_payload: Some(operation_payload.clone()),
            status: "completed".to_string(),
            duration_ms: started.elapsed().as_millis() as i64,
            sensitive: true,
            error_message: None,
            created_at: Utc::now(),
        };
        let explanation = explain_result(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            input.assistant_prompt.as_deref(),
            &summary,
            &operation_payload,
            request.stream_emitter.clone(),
        )
        .await;
        Ok(DirectExecutionResult {
            execution_target: DirectExecutionTarget::Orders,
            appended_messages: vec![explanation],
            traces: vec![trace],
            metadata: json!({"direct_task": request.task_slug,"requested_locale": request.requested_locale,"resolved_locale": request.resolved_locale,"order_ops_assistant": operation_payload,}),
        })
    }
}
