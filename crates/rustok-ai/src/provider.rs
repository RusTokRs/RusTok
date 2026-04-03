use std::time::Instant;

use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    error::{AiError, AiResult},
    model::{
        AiProviderConfig, ChatMessage, ChatMessageRole, ProviderChatRequest, ProviderChatResponse,
        ProviderKind, ProviderTestResult, ToolCall,
    },
};

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn test_connection(&self, config: &AiProviderConfig) -> AiResult<ProviderTestResult>;
    async fn complete(
        &self,
        config: &AiProviderConfig,
        request: ProviderChatRequest,
    ) -> AiResult<ProviderChatResponse>;
}

#[derive(Debug, Clone, Default)]
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
}

#[derive(Debug, Clone, Default)]
pub struct AnthropicProvider {
    client: reqwest::Client,
}

#[derive(Debug, Clone, Default)]
pub struct GeminiProvider {
    client: reqwest::Client,
}

pub fn provider_for_kind(kind: ProviderKind) -> Box<dyn ModelProvider> {
    match kind {
        ProviderKind::OpenAiCompatible => Box::new(OpenAiCompatibleProvider::new()),
        ProviderKind::Anthropic => Box::new(AnthropicProvider::new()),
        ProviderKind::Gemini => Box::new(GeminiProvider::new()),
    }
}

impl OpenAiCompatibleProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn require_provider(config: &AiProviderConfig) -> AiResult<()> {
        require_provider_kind(
            config,
            ProviderKind::OpenAiCompatible,
            "OpenAiCompatibleProvider",
        )?;
        require_base_model(config)
    }

    fn api_root(base_url: &str) -> String {
        let trimmed = base_url.trim_end_matches('/');
        if trimmed.ends_with("/v1") {
            trimmed.to_string()
        } else {
            format!("{trimmed}/v1")
        }
    }

    fn headers(config: &AiProviderConfig) -> AiResult<HeaderMap> {
        bearer_headers(config.api_key.as_deref())
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    async fn test_connection(&self, config: &AiProviderConfig) -> AiResult<ProviderTestResult> {
        Self::require_provider(config)?;
        let started = Instant::now();
        let response = self
            .client
            .get(format!("{}/models", Self::api_root(&config.base_url)))
            .headers(Self::headers(config)?)
            .send()
            .await?;
        let latency_ms = started.elapsed().as_millis() as i64;

        if response.status().is_success() {
            Ok(ProviderTestResult {
                ok: true,
                provider: config.provider_kind.slug().to_string(),
                model: Some(config.model.clone()),
                latency_ms,
                message: "Connection successful".to_string(),
            })
        } else {
            Err(provider_status_error("provider test", response).await)
        }
    }

    async fn complete(
        &self,
        config: &AiProviderConfig,
        request: ProviderChatRequest,
    ) -> AiResult<ProviderChatResponse> {
        Self::require_provider(config)?;

        #[derive(Serialize)]
        struct OpenAiTool<'a> {
            r#type: &'static str,
            function: OpenAiFunction<'a>,
        }

        #[derive(Serialize)]
        struct OpenAiFunction<'a> {
            name: &'a str,
            description: &'a str,
            parameters: &'a serde_json::Value,
        }

        let payload = json!({
            "model": request.model,
            "messages": request.messages.iter().map(openai_message_payload).collect::<Vec<_>>(),
            "tools": request.tools.iter().map(|tool| OpenAiTool {
                r#type: "function",
                function: OpenAiFunction {
                    name: &tool.name,
                    description: &tool.description,
                    parameters: &tool.input_schema,
                },
            }).collect::<Vec<_>>(),
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
        });

        let response = self
            .client
            .post(format!(
                "{}/chat/completions",
                Self::api_root(&config.base_url)
            ))
            .headers(Self::headers(config)?)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(provider_status_error("chat completion", response).await);
        }

        let raw_payload: Value = response.json().await?;
        let choice = raw_payload
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .ok_or_else(|| AiError::Provider("missing choice in provider response".to_string()))?;
        let message = choice
            .get("message")
            .ok_or_else(|| AiError::Provider("missing message in provider response".to_string()))?;

        Ok(ProviderChatResponse {
            assistant_message: ChatMessage {
                role: ChatMessageRole::Assistant,
                content: message
                    .get("content")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                name: None,
                tool_call_id: None,
                tool_calls: message
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .map(|calls| {
                        calls
                            .iter()
                            .filter_map(|call| {
                                Some(ToolCall {
                                    id: call.get("id")?.as_str()?.to_string(),
                                    name: call.get("function")?.get("name")?.as_str()?.to_string(),
                                    arguments: serde_json::from_str(
                                        call.get("function")?.get("arguments")?.as_str()?,
                                    )
                                    .ok()?,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                metadata: json!({ "provider": config.provider_kind.slug() }),
            },
            finish_reason: choice
                .get("finish_reason")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            raw_payload,
        })
    }
}

impl AnthropicProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn require_provider(config: &AiProviderConfig) -> AiResult<()> {
        require_provider_kind(config, ProviderKind::Anthropic, "AnthropicProvider")?;
        require_base_model(config)?;
        if config
            .api_key
            .as_ref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(AiError::InvalidConfig(
                "Anthropic provider requires api_key".to_string(),
            ));
        }
        Ok(())
    }

    fn api_root(base_url: &str) -> String {
        base_url.trim_end_matches('/').to_string()
    }

    fn headers(config: &AiProviderConfig) -> AiResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static("2023-06-01"),
        );
        if let Some(api_key) = config.api_key.as_ref().filter(|value| !value.is_empty()) {
            headers.insert(
                HeaderName::from_static("x-api-key"),
                HeaderValue::from_str(api_key)
                    .map_err(|err| AiError::InvalidConfig(format!("invalid api key: {err}")))?,
            );
        }
        Ok(headers)
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    async fn test_connection(&self, config: &AiProviderConfig) -> AiResult<ProviderTestResult> {
        Self::require_provider(config)?;
        let started = Instant::now();
        let response = self
            .client
            .get(format!("{}/v1/models", Self::api_root(&config.base_url)))
            .headers(Self::headers(config)?)
            .send()
            .await?;
        let latency_ms = started.elapsed().as_millis() as i64;
        if response.status().is_success() {
            Ok(ProviderTestResult {
                ok: true,
                provider: config.provider_kind.slug().to_string(),
                model: Some(config.model.clone()),
                latency_ms,
                message: "Connection successful".to_string(),
            })
        } else {
            Err(provider_status_error("provider test", response).await)
        }
    }

    async fn complete(
        &self,
        config: &AiProviderConfig,
        request: ProviderChatRequest,
    ) -> AiResult<ProviderChatResponse> {
        Self::require_provider(config)?;
        let system = request
            .messages
            .iter()
            .filter(|message| message.role == ChatMessageRole::System)
            .filter_map(|message| message.content.clone())
            .collect::<Vec<_>>()
            .join("\n\n");
        let messages = request
            .messages
            .iter()
            .filter(|message| message.role != ChatMessageRole::System)
            .map(anthropic_message_payload)
            .collect::<Vec<_>>();
        let tools = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.input_schema,
                })
            })
            .collect::<Vec<_>>();
        let payload = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(1024),
            "temperature": request.temperature,
            "system": if system.trim().is_empty() { Value::Null } else { Value::String(system) },
            "messages": messages,
            "tools": tools,
        });

        let response = self
            .client
            .post(format!("{}/v1/messages", Self::api_root(&config.base_url)))
            .headers(Self::headers(config)?)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(provider_status_error("chat completion", response).await);
        }

        let raw_payload: Value = response.json().await?;
        let content = raw_payload
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                AiError::Provider("missing content in Anthropic response".to_string())
            })?;

        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        for part in content {
            match part.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        text_parts.push(text.to_string());
                    }
                }
                Some("tool_use") => {
                    if let (Some(id), Some(name), Some(input)) = (
                        part.get("id").and_then(Value::as_str),
                        part.get("name").and_then(Value::as_str),
                        part.get("input"),
                    ) {
                        tool_calls.push(ToolCall {
                            id: id.to_string(),
                            name: name.to_string(),
                            arguments: input.clone(),
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(ProviderChatResponse {
            assistant_message: ChatMessage {
                role: ChatMessageRole::Assistant,
                content: if text_parts.is_empty() {
                    None
                } else {
                    Some(text_parts.join("\n"))
                },
                name: None,
                tool_call_id: None,
                tool_calls,
                metadata: json!({ "provider": config.provider_kind.slug() }),
            },
            finish_reason: raw_payload
                .get("stop_reason")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            raw_payload,
        })
    }
}

impl GeminiProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn require_provider(config: &AiProviderConfig) -> AiResult<()> {
        require_provider_kind(config, ProviderKind::Gemini, "GeminiProvider")?;
        require_base_model(config)
    }

    fn api_root(base_url: &str) -> String {
        let trimmed = base_url.trim_end_matches('/');
        if trimmed.ends_with("/v1beta") {
            trimmed.to_string()
        } else {
            format!("{trimmed}/v1beta")
        }
    }

    fn headers(config: &AiProviderConfig) -> AiResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(api_key) = config.api_key.as_ref().filter(|value| !value.is_empty()) {
            headers.insert(
                HeaderName::from_static("x-goog-api-key"),
                HeaderValue::from_str(api_key)
                    .map_err(|err| AiError::InvalidConfig(format!("invalid api key: {err}")))?,
            );
        }
        Ok(headers)
    }
}

#[async_trait]
impl ModelProvider for GeminiProvider {
    async fn test_connection(&self, config: &AiProviderConfig) -> AiResult<ProviderTestResult> {
        Self::require_provider(config)?;
        let started = Instant::now();
        let response = self
            .client
            .get(format!("{}/models", Self::api_root(&config.base_url)))
            .headers(Self::headers(config)?)
            .send()
            .await?;
        let latency_ms = started.elapsed().as_millis() as i64;
        if response.status().is_success() {
            Ok(ProviderTestResult {
                ok: true,
                provider: config.provider_kind.slug().to_string(),
                model: Some(config.model.clone()),
                latency_ms,
                message: "Connection successful".to_string(),
            })
        } else {
            Err(provider_status_error("provider test", response).await)
        }
    }

    async fn complete(
        &self,
        config: &AiProviderConfig,
        request: ProviderChatRequest,
    ) -> AiResult<ProviderChatResponse> {
        Self::require_provider(config)?;
        let system = request
            .messages
            .iter()
            .filter(|message| message.role == ChatMessageRole::System)
            .filter_map(|message| message.content.clone())
            .collect::<Vec<_>>()
            .join("\n\n");
        let contents = request
            .messages
            .iter()
            .filter(|message| message.role != ChatMessageRole::System)
            .map(gemini_message_payload)
            .collect::<Vec<_>>();
        let tool_declarations = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                })
            })
            .collect::<Vec<_>>();
        let payload = json!({
            "systemInstruction": if system.trim().is_empty() {
                Value::Null
            } else {
                json!({ "parts": [{ "text": system }] })
            },
            "contents": contents,
            "generationConfig": {
                "temperature": request.temperature,
                "maxOutputTokens": request.max_tokens,
            },
            "tools": if tool_declarations.is_empty() {
                Vec::<Value>::new()
            } else {
                vec![json!({ "functionDeclarations": tool_declarations })]
            },
        });

        let response = self
            .client
            .post(format!(
                "{}/models/{}:generateContent",
                Self::api_root(&config.base_url),
                request.model
            ))
            .headers(Self::headers(config)?)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(provider_status_error("chat completion", response).await);
        }

        let raw_payload: Value = response.json().await?;
        let candidate = raw_payload
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| AiError::Provider("missing candidate in Gemini response".to_string()))?;
        let parts = candidate
            .get("content")
            .and_then(|content| content.get("parts"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                AiError::Provider("missing content parts in Gemini response".to_string())
            })?;

        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        for part in parts {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                text_parts.push(text.to_string());
                continue;
            }
            if let Some(function_call) = part.get("functionCall") {
                if let Some(name) = function_call.get("name").and_then(Value::as_str) {
                    tool_calls.push(ToolCall {
                        id: format!("gemini-{name}-{}", tool_calls.len() + 1),
                        name: name.to_string(),
                        arguments: function_call
                            .get("args")
                            .cloned()
                            .unwrap_or_else(|| json!({})),
                    });
                }
            }
        }

        Ok(ProviderChatResponse {
            assistant_message: ChatMessage {
                role: ChatMessageRole::Assistant,
                content: if text_parts.is_empty() {
                    None
                } else {
                    Some(text_parts.join("\n"))
                },
                name: None,
                tool_call_id: None,
                tool_calls,
                metadata: json!({ "provider": config.provider_kind.slug() }),
            },
            finish_reason: candidate
                .get("finishReason")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            raw_payload,
        })
    }
}

fn require_provider_kind(
    config: &AiProviderConfig,
    expected: ProviderKind,
    provider_name: &str,
) -> AiResult<()> {
    if config.provider_kind != expected {
        return Err(AiError::InvalidConfig(format!(
            "{provider_name} expects provider_kind={}",
            expected.slug()
        )));
    }
    Ok(())
}

fn require_base_model(config: &AiProviderConfig) -> AiResult<()> {
    if config.base_url.trim().is_empty() {
        return Err(AiError::InvalidConfig("base_url is required".to_string()));
    }
    if config.model.trim().is_empty() {
        return Err(AiError::InvalidConfig("model is required".to_string()));
    }
    Ok(())
}

fn bearer_headers(api_key: Option<&str>) -> AiResult<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Some(api_key) = api_key.filter(|value| !value.is_empty()) {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}"))
                .map_err(|err| AiError::InvalidConfig(format!("invalid api key: {err}")))?,
        );
    }
    Ok(headers)
}

fn openai_message_payload(message: &ChatMessage) -> Value {
    let mut value = json!({
        "role": match message.role {
            ChatMessageRole::System => "system",
            ChatMessageRole::User => "user",
            ChatMessageRole::Assistant => "assistant",
            ChatMessageRole::Tool => "tool",
        },
        "content": message.content,
    });
    if let Some(name) = message.name.as_ref() {
        value["name"] = Value::String(name.clone());
    }
    if let Some(tool_call_id) = message.tool_call_id.as_ref() {
        value["tool_call_id"] = Value::String(tool_call_id.clone());
    }
    value
}

fn anthropic_message_payload(message: &ChatMessage) -> Value {
    let role = match message.role {
        ChatMessageRole::User | ChatMessageRole::Tool => "user",
        ChatMessageRole::Assistant => "assistant",
        ChatMessageRole::System => "user",
    };

    if message.role == ChatMessageRole::Tool {
        json!({
            "role": role,
            "content": [{
                "type": "tool_result",
                "tool_use_id": message.tool_call_id,
                "content": message.content.clone().unwrap_or_default(),
            }]
        })
    } else {
        json!({
            "role": role,
            "content": [{
                "type": "text",
                "text": message.content.clone().unwrap_or_default(),
            }]
        })
    }
}

fn gemini_message_payload(message: &ChatMessage) -> Value {
    let role = match message.role {
        ChatMessageRole::Assistant => "model",
        _ => "user",
    };
    let parts = if message.role == ChatMessageRole::Tool {
        vec![json!({
            "functionResponse": {
                "name": message.name.clone().unwrap_or_else(|| "tool".to_string()),
                "response": {
                    "name": message.name.clone().unwrap_or_else(|| "tool".to_string()),
                    "content": message.content.clone().unwrap_or_default(),
                }
            }
        })]
    } else {
        vec![json!({ "text": message.content.clone().unwrap_or_default() })]
    };
    json!({
        "role": role,
        "parts": parts,
    })
}

async fn provider_status_error(operation: &str, response: reqwest::Response) -> AiError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    AiError::Provider(format!("{operation} failed with status {status}: {body}"))
}
