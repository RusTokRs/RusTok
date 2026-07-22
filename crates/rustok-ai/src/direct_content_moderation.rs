#![cfg(feature = "server")]

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

use crate::direct::{
    DirectExecutionRequest, DirectExecutionResult, DirectTaskHandler, explain_result,
    generate_content_moderation,
};
use crate::model::{AiContentModerationTaskInput, DirectExecutionTarget, ToolTrace};
use crate::service::{AiHostRuntime, AiOperatorContext};
use crate::{AiError, AiResult};
use rustok_ai_content::{CONTENT_MODERATION_TASK_SLUG, CONTENT_MODERATION_TOOL_NAME};

pub struct ContentModerationHandler;

#[async_trait]
impl DirectTaskHandler for ContentModerationHandler {
    fn task_slug(&self) -> &'static str {
        CONTENT_MODERATION_TASK_SLUG
    }

    async fn execute(
        &self,
        _runtime: &AiHostRuntime,
        _operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        execute_content_moderation(request).await
    }
}

async fn execute_content_moderation(
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
        tool_name: CONTENT_MODERATION_TOOL_NAME.to_string(),
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

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use async_trait::async_trait;
    use rustok_ai_content::CONTENT_MODERATION_TOOL_NAME;
    use serde_json::json;
    use uuid::Uuid;

    use super::execute_content_moderation;
    use crate::{
        AiProviderConfig, ChatMessage, ChatMessageRole, DirectExecutionRequest,
        DirectExecutionTarget, InferenceEngine, ProviderChatRequest, ProviderChatResponse,
        ProviderSlug, ProviderStructuredRequest, ProviderTargetAuth, ProviderTestResult,
        model::AiContentModerationTaskInput,
    };

    struct ModerationEngine;

    #[async_trait]
    impl InferenceEngine for ModerationEngine {
        async fn test_connection(
            &self,
            _config: &AiProviderConfig,
        ) -> crate::AiResult<ProviderTestResult> {
            unreachable!("direct execution does not probe provider connectivity")
        }

        async fn complete(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
        ) -> crate::AiResult<ProviderChatResponse> {
            unreachable!("content moderation uses typed generation")
        }

        async fn complete_stream(
            &self,
            _config: &AiProviderConfig,
            request: ProviderChatRequest,
            _emitter: Option<crate::ProviderStreamEmitter>,
        ) -> crate::AiResult<ProviderChatResponse> {
            assert_eq!(request.locale.as_deref(), Some("ru"));
            assert_eq!(
                request.messages[0].metadata["direct_explanation"],
                json!(true)
            );
            Ok(ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: Some("Отправлено на проверку оператору.".to_string()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({"source": "scripted"}),
                },
                finish_reason: Some("stop".to_string()),
                raw_payload: json!({}),
            })
        }

        async fn complete_structured(
            &self,
            request: ProviderStructuredRequest,
        ) -> crate::AiResult<serde_json::Value> {
            assert_eq!(request.request.locale.as_deref(), Some("ru"));
            assert_eq!(
                request.request.messages[0].metadata["direct_generation"],
                json!("content_moderation")
            );
            Ok(json!({
                "decision": "REVIEW",
                "labels": ["safety"],
                "severity": 72,
                "explanation": "Potentially unsafe wording requires human review.",
                "requires_human": true,
                "recommended_action": "route_to_moderator"
            }))
        }
    }

    fn provider_config() -> AiProviderConfig {
        AiProviderConfig {
            tenant_id: Uuid::nil(),
            provider_slug: ProviderSlug::new("openai_compatible").unwrap(),
            target_auth: ProviderTargetAuth::SecretRefs,
            model: "test-model".to_string(),
            settings: BTreeMap::new(),
            credential_refs: BTreeMap::new(),
            temperature: None,
            max_tokens: None,
            capabilities: Vec::new(),
            usage_policy: Default::default(),
        }
    }

    #[tokio::test]
    async fn direct_content_moderation_preserves_validated_result_and_sensitive_trace() {
        let input = AiContentModerationTaskInput {
            content_type: Some("post".to_string()),
            title: Some("Проверка текста".to_string()),
            body: Some("Текст для модерации".to_string()),
            locale: Some("ru".to_string()),
            assistant_prompt: Some("Explain the moderation result.".to_string()),
            ..Default::default()
        };

        let result = execute_content_moderation(DirectExecutionRequest {
            task_slug: "content_moderation".to_string(),
            task_input_json: serde_json::to_value(input).unwrap(),
            requested_locale: Some("ru".to_string()),
            resolved_locale: "ru".to_string(),
            system_prompt: None,
            provider_config: provider_config(),
            provider: Arc::new(ModerationEngine),
            stream_emitter: None,
        })
        .await
        .unwrap();

        assert_eq!(result.execution_target, DirectExecutionTarget::Moderation);
        assert_eq!(result.metadata["direct_task"], json!("content_moderation"));
        assert_eq!(result.metadata["resolved_locale"], json!("ru"));
        assert_eq!(result.metadata["moderation"]["decision"], json!("review"));
        assert_eq!(result.metadata["moderation"]["severity"], json!(72));
        assert_eq!(result.traces.len(), 1);
        assert_eq!(result.traces[0].tool_name, CONTENT_MODERATION_TOOL_NAME);
        assert_eq!(result.traces[0].status, "completed");
        assert!(result.traces[0].sensitive);
        assert_eq!(
            result.traces[0].output_payload.as_ref().unwrap()["decision"],
            json!("review")
        );
        assert_eq!(
            result.appended_messages[0].content.as_deref(),
            Some("Отправлено на проверку оператору.")
        );
        assert_eq!(
            result.appended_messages[0].metadata["direct_explanation"],
            json!(true)
        );
    }
}
