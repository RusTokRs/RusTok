#![cfg(feature = "server")]

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

use crate::direct::{
    explain_result, generate_content_moderation, DirectExecutionRequest, DirectExecutionResult,
    DirectTaskHandler,
};
use crate::model::{AiContentModerationTaskInput, DirectExecutionTarget, ToolTrace};
use crate::service::AiOperatorContext;
use crate::{AiError, AiResult};
use loco_rs::app::AppContext;

pub struct ContentModerationHandler;

#[async_trait]
impl DirectTaskHandler for ContentModerationHandler {
    fn task_slug(&self) -> &'static str {
        "content_moderation"
    }

    async fn execute(
        &self,
        _app_ctx: &AppContext,
        _operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiContentModerationTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let started = std::time::Instant::now();
        let generated = generate_content_moderation(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            &input,
        )
        .await?;
        let operation_payload = serde_json::to_value(&generated).map_err(AiError::Json)?;
        let summary = format!(
            "Moderation decision: {} (severity {}).",
            generated.decision, generated.severity
        );
        let trace = ToolTrace {
            tool_name: "direct.content.moderation".to_string(),
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
            execution_target: DirectExecutionTarget::Moderation,
            appended_messages: vec![explanation],
            traces: vec![trace],
            metadata: json!({"direct_task": request.task_slug,"requested_locale": request.requested_locale,"resolved_locale": request.resolved_locale,"moderation": operation_payload,}),
        })
    }
}
