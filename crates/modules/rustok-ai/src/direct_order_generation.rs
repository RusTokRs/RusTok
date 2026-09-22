#![cfg(feature = "server")]

use std::sync::Arc;

use serde_json::{Value, json};

use crate::direct::complete_typed;
use crate::engine::InferenceEngine;
use crate::model::{
    AiOrderAnalyticsTaskInput, AiOrderOpsAssistantTaskInput, AiProviderConfig, ChatMessage,
    ChatMessageRole, ProviderChatRequest,
};
use crate::{AiError, AiResult};
use rustok_ai_order::{
    GeneratedOrderAnalytics, GeneratedOrderOpsAssistant, validate_order_analytics_payload,
    validate_order_ops_assistant_payload,
};

async fn complete_direct_order<T>(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    direct_generation: &str,
    schema_instruction: &str,
    input_payload: Value,
) -> AiResult<T>
where
    T: for<'de> serde::Deserialize<'de> + schemars::JsonSchema,
{
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

    complete_typed(
        provider,
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
    .await
}

pub(crate) async fn generate_order_analytics(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    input: &AiOrderAnalyticsTaskInput,
    order_status_context: Value,
) -> AiResult<GeneratedOrderAnalytics> {
    let locale_instruction = concat!(
        "Return valid JSON only with keys `summary`, `key_findings`, `risk_flags`, `recommended_actions`. ",
        "All array values must be arrays of strings."
    );
    let generated: GeneratedOrderAnalytics = complete_direct_order(
        provider,
        provider_config,
        system_prompt,
        target_locale,
        "order_analytics",
        locale_instruction,
        json!({
            "request": serde_json::to_value(input).map_err(AiError::Json)?,
            "order_status_context": order_status_context,
        }),
    )
    .await?;
    validate_order_analytics_payload(&generated).map_err(AiError::Validation)?;
    Ok(generated)
}

pub(crate) async fn generate_order_ops_assistant(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    input: &AiOrderOpsAssistantTaskInput,
    order_status_context: Value,
) -> AiResult<GeneratedOrderOpsAssistant> {
    let locale_instruction = concat!(
        "Return valid JSON only with keys `recommended_action`, `rationale`, `prefill`, `requires_human`, `confidence`. ",
        "`confidence` must be an integer from 0 to 100."
    );
    let decision: GeneratedOrderOpsAssistant = complete_direct_order(
        provider,
        provider_config,
        system_prompt,
        target_locale,
        "order_ops_assistant",
        locale_instruction,
        json!({
            "request": serde_json::to_value(input).map_err(AiError::Json)?,
            "order_status_context": order_status_context,
        }),
    )
    .await?;
    validate_order_ops_assistant_payload(&decision).map_err(AiError::Validation)?;
    Ok(decision)
}
