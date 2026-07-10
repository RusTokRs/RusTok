#![cfg(feature = "server")]

use std::sync::Arc;

use serde_json::{json, Value};

use crate::direct::parse_json_object_from_text;
use crate::engine::InferenceEngine;
use crate::model::{
    AiOrderAnalyticsTaskInput, AiOrderOpsAssistantTaskInput, AiProviderConfig, ChatMessage,
    ChatMessageRole, ProviderChatRequest,
};
use crate::{AiError, AiResult};
use rustok_ai_order::{
    validate_order_analytics_payload, validate_order_ops_assistant_payload,
    GeneratedOrderAnalytics, GeneratedOrderOpsAssistant,
};

async fn complete_direct_order_json(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    direct_generation: &str,
    schema_instruction: &str,
    input_payload: Value,
) -> AiResult<Value> {
    let system = match system_prompt {
        Some(system_prompt) if !system_prompt.trim().is_empty() => {
            format!("{system_prompt}\n\n{schema_instruction}")
        }
        _ => schema_instruction.to_string(),
    };
    let prompt = json!({
        "task": direct_generation,
        "target_locale": target_locale,
        "input": input_payload,
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
                        metadata: json!({"locale": target_locale, "direct_generation": direct_generation}),
                    },
                    ChatMessage {
                        role: ChatMessageRole::User,
                        content: Some(prompt),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: json!({"locale": target_locale, "direct_generation": direct_generation}),
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
        AiError::Provider(format!(
            "provider returned empty content for {direct_generation}"
        ))
    })?;

    parse_json_object_from_text(&content)
}

pub(crate) async fn generate_order_analytics(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    input: &AiOrderAnalyticsTaskInput,
) -> AiResult<GeneratedOrderAnalytics> {
    let locale_instruction = concat!(
        "Return valid JSON only with keys `summary`, `key_findings`, `risk_flags`, `recommended_actions`. ",
        "All array values must be arrays of strings."
    );
    let parsed = complete_direct_order_json(
        provider,
        provider_config,
        system_prompt,
        target_locale,
        "order_analytics",
        locale_instruction,
        serde_json::to_value(input).map_err(AiError::Json)?,
    )
    .await?;
    let generated: GeneratedOrderAnalytics =
        serde_json::from_value(parsed).map_err(AiError::Json)?;
    validate_order_analytics_payload(&generated).map_err(AiError::Validation)?;
    Ok(generated)
}

pub(crate) async fn generate_order_ops_assistant(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    input: &AiOrderOpsAssistantTaskInput,
) -> AiResult<GeneratedOrderOpsAssistant> {
    let locale_instruction = concat!(
        "Return valid JSON only with keys `recommended_action`, `rationale`, `prefill`, `requires_human`, `confidence`. ",
        "`confidence` must be an integer from 0 to 100."
    );
    let parsed = complete_direct_order_json(
        provider,
        provider_config,
        system_prompt,
        target_locale,
        "order_ops_assistant",
        locale_instruction,
        serde_json::to_value(input).map_err(AiError::Json)?,
    )
    .await?;
    let decision: GeneratedOrderOpsAssistant =
        serde_json::from_value(parsed).map_err(AiError::Json)?;
    validate_order_ops_assistant_payload(&decision).map_err(AiError::Validation)?;
    Ok(decision)
}
