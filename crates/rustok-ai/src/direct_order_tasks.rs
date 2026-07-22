#![cfg(feature = "server")]

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

use crate::direct::{
    DirectExecutionRequest, DirectExecutionResult, DirectTaskHandler, direct_operator_port_context,
    explain_result, generate_order_analytics, generate_order_ops_assistant,
};
use crate::model::{AiOrderAnalyticsTaskInput, AiOrderOpsAssistantTaskInput};
use crate::model::{DirectExecutionTarget, ToolTrace};
use crate::service::{AiHostRuntime, AiOperatorContext};
use crate::{AiError, AiResult};
use rustok_ai_order::{
    ORDER_ANALYTICS_TASK_SLUG, ORDER_ANALYTICS_TOOL_NAME, ORDER_OPS_ASSISTANT_TASK_SLUG,
    ORDER_OPS_ASSISTANT_TOOL_NAME, order_ai_execution_policy,
};
use rustok_order::OrderStatusRequest;

const ORDER_STATUS_PORT_DEADLINE: Duration = Duration::from_secs(3);

async fn order_status_context(
    runtime: &AiHostRuntime,
    operator: &AiOperatorContext,
    locale: &str,
    task_slug: &str,
    order_ids: impl IntoIterator<Item = Uuid>,
) -> serde_json::Value {
    let deadline_ms = ORDER_STATUS_PORT_DEADLINE.as_millis() as u64;
    let Some(port) = runtime.order_status_port() else {
        return json!({
            "source": "degraded",
            "snapshots": [],
            "errors": [{
                "kind": "unavailable",
                "code": "ai_order.status_port_unavailable",
                "retryable": true,
            }],
            "deadline_ms": deadline_ms,
        });
    };
    let context =
        direct_operator_port_context(operator, locale, task_slug, ORDER_STATUS_PORT_DEADLINE);
    let mut snapshots = Vec::new();
    let mut errors = Vec::new();
    for order_id in order_ids {
        match tokio::time::timeout(
            ORDER_STATUS_PORT_DEADLINE,
            port.read_order_status(context.clone(), OrderStatusRequest { order_id }),
        )
        .await
        {
            Err(_) => errors.push(json!({
                "order_id": order_id,
                "kind": "deadline_exceeded",
                "code": "ai_order.status_port_deadline_exceeded",
                "retryable": true,
            })),
            Ok(Ok(snapshot)) => snapshots
                .push(serde_json::to_value(snapshot).expect("order status is serializable")),
            Ok(Err(error)) => errors.push(json!({
                "order_id": order_id,
                "kind": error.kind,
                "code": error.code,
                "retryable": error.retryable,
            })),
        }
    }
    json!({
        "source": if errors.is_empty() { "owner_port" } else { "degraded" },
        "snapshots": snapshots,
        "errors": errors,
        "deadline_ms": deadline_ms,
    })
}

pub struct OrderAnalyticsHandler;
pub struct OrderOpsAssistantHandler;

#[async_trait]
impl DirectTaskHandler for OrderAnalyticsHandler {
    fn task_slug(&self) -> &'static str {
        ORDER_ANALYTICS_TASK_SLUG
    }
    async fn execute(
        &self,
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiOrderAnalyticsTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let execution_policy = order_ai_execution_policy(ORDER_ANALYTICS_TASK_SLUG)
            .expect("order analytics execution policy must be registered");
        let status_context = order_status_context(
            runtime,
            operator,
            request.resolved_locale.as_str(),
            ORDER_ANALYTICS_TASK_SLUG,
            input.order_ids.clone(),
        )
        .await;
        let started = std::time::Instant::now();
        let generated = generate_order_analytics(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            &input,
            status_context.clone(),
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
            metadata: json!({
                "direct_task": request.task_slug,
                "requested_locale": request.requested_locale,
                "resolved_locale": request.resolved_locale,
                "order_analytics": operation_payload,
                "order_status_context": status_context,
                "review_required": execution_policy.review_required,
                "persistence": execution_policy.persistence,
            }),
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
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiOrderOpsAssistantTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let execution_policy = order_ai_execution_policy(ORDER_OPS_ASSISTANT_TASK_SLUG)
            .expect("order operations execution policy must be registered");
        let status_context = order_status_context(
            runtime,
            operator,
            request.resolved_locale.as_str(),
            ORDER_OPS_ASSISTANT_TASK_SLUG,
            [input.order_id],
        )
        .await;
        let started = std::time::Instant::now();
        let generated = generate_order_ops_assistant(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            &input,
            status_context.clone(),
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
            metadata: json!({
                "direct_task": request.task_slug,
                "requested_locale": request.requested_locale,
                "resolved_locale": request.resolved_locale,
                "order_ops_assistant": operation_payload,
                "order_status_context": status_context,
                "review_required": execution_policy.review_required,
                "persistence": execution_policy.persistence,
            }),
        })
    }
}
