#![cfg(feature = "server")]

use std::sync::Arc;

use serde_json::json;

use crate::direct::parse_json_object_from_text;
use crate::model::{
    AiOrderAnalyticsTaskInput, AiOrderOpsAssistantTaskInput, AiProviderConfig, ChatMessage,
    ChatMessageRole, ProviderChatRequest,
};
use crate::provider::ModelProvider;
use crate::{AiError, AiResult};
use rustok_ai_order::{
    validate_order_ops_assistant_confidence, GeneratedOrderAnalytics, GeneratedOrderOpsAssistant,
};

pub(crate) async fn generate_order_analytics(
    provider: &Arc<dyn ModelProvider>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    input: &AiOrderAnalyticsTaskInput,
) -> AiResult<GeneratedOrderAnalytics> {
    let locale_instruction = concat!(
        "Return valid JSON only with keys `summary`, `key_findings`, `risk_flags`, `recommended_actions`. ",
        "All array values must be arrays of strings."
    );
    let system = match system_prompt {
        Some(system_prompt) if !system_prompt.trim().is_empty() => {
            format!("{system_prompt}\n\n{locale_instruction}")
        }
        _ => locale_instruction.to_string(),
    };
    let prompt = json!({
        "task": "order_analytics",
        "target_locale": target_locale,
        "input": input,
    })
    .to_string();
    let response = provider
        .complete(
            provider_config,
            ProviderChatRequest {
                model: provider_config.model.clone(),
                messages: vec![
                    ChatMessage {
                        role: ChatMessageRole::System,
                        content: Some(system),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: json!({"locale": target_locale, "direct_generation": "order_analytics"}),
                    },
                    ChatMessage {
                        role: ChatMessageRole::User,
                        content: Some(prompt),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: json!({"locale": target_locale, "direct_generation": "order_analytics"}),
                    },
                ],
                tools: Vec::new(),
                temperature: provider_config.temperature,
                max_tokens: provider_config.max_tokens,
                locale: Some(target_locale.to_string()),
            },
        )
        .await?;
    let content = response.assistant_message.content.ok_or_else(|| {
        AiError::Provider("provider returned empty content for order_analytics".to_string())
    })?;
    let parsed = parse_json_object_from_text(&content)?;
    serde_json::from_value(parsed).map_err(AiError::Json)
}

pub(crate) async fn generate_order_ops_assistant(
    provider: &Arc<dyn ModelProvider>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    input: &AiOrderOpsAssistantTaskInput,
) -> AiResult<GeneratedOrderOpsAssistant> {
    let locale_instruction = concat!(
        "Return valid JSON only with keys `recommended_action`, `rationale`, `prefill`, `requires_human`, `confidence`. ",
        "`confidence` must be an integer from 0 to 100."
    );
    let system = match system_prompt {
        Some(system_prompt) if !system_prompt.trim().is_empty() => {
            format!("{system_prompt}\n\n{locale_instruction}")
        }
        _ => locale_instruction.to_string(),
    };
    let prompt = json!({
        "task": "order_ops_assistant",
        "target_locale": target_locale,
        "input": input,
    })
    .to_string();
    let response = provider
        .complete(
            provider_config,
            ProviderChatRequest {
                model: provider_config.model.clone(),
                messages: vec![
                    ChatMessage {
                        role: ChatMessageRole::System,
                        content: Some(system),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: json!({"locale": target_locale, "direct_generation": "order_ops_assistant"}),
                    },
                    ChatMessage {
                        role: ChatMessageRole::User,
                        content: Some(prompt),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: json!({"locale": target_locale, "direct_generation": "order_ops_assistant"}),
                    },
                ],
                tools: Vec::new(),
                temperature: provider_config.temperature,
                max_tokens: provider_config.max_tokens,
                locale: Some(target_locale.to_string()),
            },
        )
        .await?;
    let content = response.assistant_message.content.ok_or_else(|| {
        AiError::Provider("provider returned empty content for order_ops_assistant".to_string())
    })?;
    let parsed = parse_json_object_from_text(&content)?;
    let decision: GeneratedOrderOpsAssistant =
        serde_json::from_value(parsed).map_err(AiError::Json)?;
    validate_order_ops_assistant_confidence(decision.confidence).map_err(AiError::Validation)?;
    Ok(decision)
}
