use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use async_trait::async_trait;
use base64::Engine;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    error::{AiError, AiResult},
    model::{
        AiProviderConfig, ChatMessage, ChatMessageRole, ProviderChatRequest, ProviderChatResponse,
        ProviderImageRequest, ProviderImageResponse, ProviderKind, ProviderStreamEmitter,
        ProviderTestResult, ToolCall,
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
    async fn complete_stream(
        &self,
        config: &AiProviderConfig,
        request: ProviderChatRequest,
        emitter: Option<ProviderStreamEmitter>,
    ) -> AiResult<ProviderChatResponse> {
        let response = self.complete(config, request).await?;
        if let (Some(emitter), Some(content)) =
            (emitter, response.assistant_message.content.clone())
        {
            if !content.trim().is_empty() {
                emitter.emit_text_delta(content);
            }
        }
        Ok(response)
    }
    async fn generate_image(
        &self,
        config: &AiProviderConfig,
        request: ProviderImageRequest,
    ) -> AiResult<ProviderImageResponse>;
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

    async fn complete_stream(
        &self,
        config: &AiProviderConfig,
        request: ProviderChatRequest,
        emitter: Option<ProviderStreamEmitter>,
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
            "stream": true,
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

        let mut stream = response.bytes_stream();
        let mut line_buffer = String::new();
        let mut content_buffer = String::new();
        let mut finish_reason = None;
        let mut tool_calls = BTreeMap::<usize, PartialOpenAiToolCall>::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            line_buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(position) = line_buffer.find('\n') {
                let mut line = line_buffer[..position].to_string();
                line_buffer.drain(..=position);
                if line.ends_with('\r') {
                    line.pop();
                }
                let line = line.trim();
                if line.is_empty() || !line.starts_with("data:") {
                    continue;
                }
                let data = line["data:".len()..].trim();
                if data == "[DONE]" {
                    break;
                }
                let payload: Value = serde_json::from_str(data).map_err(|err| {
                    AiError::Provider(format!("invalid streaming payload from provider: {err}"))
                })?;
                let Some(choice) = payload
                    .get("choices")
                    .and_then(Value::as_array)
                    .and_then(|choices| choices.first())
                else {
                    continue;
                };
                if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                    finish_reason = Some(reason.to_string());
                }
                let Some(delta) = choice.get("delta") else {
                    continue;
                };
                if let Some(text) = delta.get("content").and_then(Value::as_str) {
                    if !text.is_empty() {
                        content_buffer.push_str(text);
                        if let Some(emitter) = emitter.as_ref() {
                            emitter.emit_text_delta(text.to_string());
                        }
                    }
                }
                if let Some(parts) = delta.get("tool_calls").and_then(Value::as_array) {
                    for part in parts {
                        let index = part
                            .get("index")
                            .and_then(Value::as_u64)
                            .unwrap_or(tool_calls.len() as u64)
                            as usize;
                        let entry = tool_calls.entry(index).or_default();
                        if let Some(id) = part.get("id").and_then(Value::as_str) {
                            entry.id = Some(id.to_string());
                        }
                        if let Some(function) = part.get("function") {
                            if let Some(name) = function.get("name").and_then(Value::as_str) {
                                entry.name.push_str(name);
                            }
                            if let Some(arguments) =
                                function.get("arguments").and_then(Value::as_str)
                            {
                                entry.arguments.push_str(arguments);
                            }
                        }
                    }
                }
            }
        }

        let tool_calls = tool_calls
            .into_iter()
            .map(|(index, value)| {
                let id = value
                    .id
                    .unwrap_or_else(|| format!("openai-tool-call-{}", index + 1));
                let name = value.name.trim().to_string();
                if name.is_empty() {
                    return Err(AiError::Provider(
                        "streamed tool call is missing function name".to_string(),
                    ));
                }
                let arguments = if value.arguments.trim().is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&value.arguments).map_err(|err| {
                        AiError::Provider(format!(
                            "invalid streamed tool call arguments for `{name}`: {err}"
                        ))
                    })?
                };
                Ok(ToolCall {
                    id,
                    name,
                    arguments,
                })
            })
            .collect::<AiResult<Vec<_>>>()?;

        Ok(ProviderChatResponse {
            assistant_message: ChatMessage {
                role: ChatMessageRole::Assistant,
                content: if content_buffer.is_empty() {
                    None
                } else {
                    Some(content_buffer)
                },
                name: None,
                tool_call_id: None,
                tool_calls,
                metadata: json!({ "provider": config.provider_kind.slug(), "streaming": true }),
            },
            finish_reason,
            raw_payload: json!({ "streaming": true }),
        })
    }

    async fn generate_image(
        &self,
        config: &AiProviderConfig,
        request: ProviderImageRequest,
    ) -> AiResult<ProviderImageResponse> {
        Self::require_provider(config)?;

        let mut payload = json!({
            "model": request.model,
            "prompt": request.prompt,
            "n": 1,
            "response_format": "b64_json",
        });
        if let Some(size) = request.size.filter(|value| !value.trim().is_empty()) {
            payload["size"] = Value::String(size);
        }
        if let Some(negative_prompt) = request
            .negative_prompt
            .filter(|value| !value.trim().is_empty())
        {
            payload["negative_prompt"] = Value::String(negative_prompt);
        }

        let response = self
            .client
            .post(format!(
                "{}/images/generations",
                Self::api_root(&config.base_url)
            ))
            .headers(Self::headers(config)?)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(provider_status_error("image generation", response).await);
        }

        let raw_payload: Value = response.json().await?;
        let image = raw_payload
            .get("data")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .ok_or_else(|| {
                AiError::Provider("missing image data in provider response".to_string())
            })?;
        let base64_image = image
            .get("b64_json")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AiError::Provider(
                    "provider did not return `b64_json` for generated image".to_string(),
                )
            })?;

        Ok(ProviderImageResponse {
            bytes: base64::engine::general_purpose::STANDARD
                .decode(base64_image)
                .map_err(|err| AiError::Provider(format!("invalid image payload: {err}")))?,
            mime_type: image
                .get("mime_type")
                .and_then(Value::as_str)
                .unwrap_or("image/png")
                .to_string(),
            revised_prompt: image
                .get("revised_prompt")
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

    async fn complete_stream(
        &self,
        config: &AiProviderConfig,
        request: ProviderChatRequest,
        emitter: Option<ProviderStreamEmitter>,
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
            "stream": true,
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

        let mut state = AnthropicStreamingState::default();
        consume_sse_events(response, |_, data| {
            let payload: Value = serde_json::from_str(data).map_err(|err| {
                AiError::Provider(format!("invalid Anthropic streaming payload: {err}"))
            })?;
            apply_anthropic_stream_payload(&payload, emitter.as_ref(), &mut state)
        })
        .await?;

        Ok(ProviderChatResponse {
            assistant_message: ChatMessage {
                role: ChatMessageRole::Assistant,
                content: if state.content_buffer.trim().is_empty() {
                    None
                } else {
                    Some(state.content_buffer)
                },
                name: None,
                tool_call_id: None,
                tool_calls: finalize_anthropic_tool_calls(state.tool_calls)?,
                metadata: json!({ "provider": config.provider_kind.slug(), "streaming": true }),
            },
            finish_reason: state.finish_reason,
            raw_payload: json!({ "streaming": true }),
        })
    }

    async fn generate_image(
        &self,
        config: &AiProviderConfig,
        _request: ProviderImageRequest,
    ) -> AiResult<ProviderImageResponse> {
        Self::require_provider(config)?;
        Err(AiError::Provider(
            "AnthropicProvider does not support image generation".to_string(),
        ))
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

#[derive(Default)]
struct PartialOpenAiToolCall {
    id: Option<String>,
    name: String,
    arguments: String,
}

#[derive(Default)]
struct PartialAnthropicToolCall {
    id: Option<String>,
    name: String,
    input_json: String,
    input_value: Option<Value>,
}

#[derive(Default)]
struct AnthropicStreamingState {
    content_buffer: String,
    finish_reason: Option<String>,
    tool_calls: BTreeMap<usize, PartialAnthropicToolCall>,
}

#[derive(Default)]
struct GeminiStreamingState {
    content_buffer: String,
    finish_reason: Option<String>,
    tool_calls: Vec<ToolCall>,
    seen_tool_calls: BTreeSet<String>,
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

    async fn complete_stream(
        &self,
        config: &AiProviderConfig,
        request: ProviderChatRequest,
        emitter: Option<ProviderStreamEmitter>,
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
                "{}/models/{}:streamGenerateContent?alt=sse",
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

        let mut state = GeminiStreamingState::default();
        consume_sse_events(response, |_, data| {
            let payload: Value = serde_json::from_str(data).map_err(|err| {
                AiError::Provider(format!("invalid Gemini streaming payload: {err}"))
            })?;
            apply_gemini_stream_payload(&payload, emitter.as_ref(), &mut state)
        })
        .await?;

        Ok(ProviderChatResponse {
            assistant_message: ChatMessage {
                role: ChatMessageRole::Assistant,
                content: if state.content_buffer.trim().is_empty() {
                    None
                } else {
                    Some(state.content_buffer)
                },
                name: None,
                tool_call_id: None,
                tool_calls: state.tool_calls,
                metadata: json!({ "provider": config.provider_kind.slug(), "streaming": true }),
            },
            finish_reason: state.finish_reason,
            raw_payload: json!({ "streaming": true }),
        })
    }

    async fn generate_image(
        &self,
        config: &AiProviderConfig,
        request: ProviderImageRequest,
    ) -> AiResult<ProviderImageResponse> {
        Self::require_provider(config)?;

        let prompt = match request.negative_prompt {
            Some(negative_prompt) if !negative_prompt.trim().is_empty() => {
                format!(
                    "{}\n\nNegative prompt: {}",
                    request.prompt.trim(),
                    negative_prompt.trim()
                )
            }
            _ => request.prompt,
        };

        let response = self
            .client
            .post(format!(
                "{}/models/{}:generateContent",
                Self::api_root(&config.base_url),
                request.model
            ))
            .headers(Self::headers(config)?)
            .json(&json!({
                "contents": [{
                    "role": "user",
                    "parts": [{ "text": prompt }],
                }],
                "generationConfig": {
                    "responseModalities": ["TEXT", "IMAGE"],
                }
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(provider_status_error("image generation", response).await);
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

        let mut revised_prompt = Vec::new();
        for part in parts {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                revised_prompt.push(text.to_string());
            }
            if let Some(inline_data) = part.get("inlineData") {
                let encoded = inline_data
                    .get("data")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        AiError::Provider(
                            "missing inlineData.data in Gemini image response".to_string(),
                        )
                    })?;
                return Ok(ProviderImageResponse {
                    bytes: base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .map_err(|err| {
                            AiError::Provider(format!("invalid Gemini image payload: {err}"))
                        })?,
                    mime_type: inline_data
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or("image/png")
                        .to_string(),
                    revised_prompt: if revised_prompt.is_empty() {
                        None
                    } else {
                        Some(revised_prompt.join("\n"))
                    },
                    raw_payload,
                });
            }
        }

        Err(AiError::Provider(
            "Gemini response did not contain inline image data".to_string(),
        ))
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

async fn consume_sse_events<F>(response: reqwest::Response, mut handle: F) -> AiResult<()>
where
    F: FnMut(Option<&str>, &str) -> AiResult<bool>,
{
    let mut stream = response.bytes_stream();
    let mut line_buffer = String::new();
    let mut current_event: Option<String> = None;
    let mut data_lines = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        line_buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(position) = line_buffer.find('\n') {
            let mut line = line_buffer[..position].to_string();
            line_buffer.drain(..=position);
            if line.ends_with('\r') {
                line.pop();
            }
            let line = line.trim_end();
            if line.is_empty() {
                if !data_lines.is_empty() {
                    let data = data_lines.join("\n");
                    let should_continue = handle(current_event.as_deref(), data.trim())?;
                    data_lines.clear();
                    current_event = None;
                    if !should_continue {
                        return Ok(());
                    }
                } else {
                    current_event = None;
                }
                continue;
            }
            if let Some(value) = line.strip_prefix("event:") {
                current_event = Some(value.trim().to_string());
                continue;
            }
            if let Some(value) = line.strip_prefix("data:") {
                data_lines.push(value.trim_start().to_string());
            }
        }
    }

    if !data_lines.is_empty() {
        let data = data_lines.join("\n");
        handle(current_event.as_deref(), data.trim())?;
    }

    Ok(())
}

fn apply_anthropic_stream_payload(
    payload: &Value,
    emitter: Option<&ProviderStreamEmitter>,
    state: &mut AnthropicStreamingState,
) -> AiResult<bool> {
    let event_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if event_type == "error" {
        let message = payload
            .get("error")
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("Anthropic streaming request failed");
        return Err(AiError::Provider(message.to_string()));
    }

    match event_type {
        "message_start" => {
            if let Some(reason) = payload
                .get("message")
                .and_then(|value| value.get("stop_reason"))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                state.finish_reason = Some(reason.to_string());
            }
        }
        "message_delta" => {
            if let Some(reason) = payload
                .get("delta")
                .and_then(|value| value.get("stop_reason"))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                state.finish_reason = Some(reason.to_string());
            }
        }
        "content_block_start" => {
            let index = payload
                .get("index")
                .and_then(Value::as_u64)
                .unwrap_or(state.tool_calls.len() as u64) as usize;
            if let Some(block) = payload.get("content_block") {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            emit_incremental_text(&mut state.content_buffer, text, emitter);
                        }
                    }
                    Some("tool_use") => {
                        let entry = state.tool_calls.entry(index).or_default();
                        if let Some(id) = block.get("id").and_then(Value::as_str) {
                            entry.id = Some(id.to_string());
                        }
                        if let Some(name) = block.get("name").and_then(Value::as_str) {
                            entry.name = name.to_string();
                        }
                        if let Some(input) = block.get("input") {
                            entry.input_value = Some(input.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        "content_block_delta" => {
            let index = payload
                .get("index")
                .and_then(Value::as_u64)
                .unwrap_or(state.tool_calls.len() as u64) as usize;
            if let Some(delta) = payload.get("delta") {
                match delta.get("type").and_then(Value::as_str) {
                    Some("text_delta") => {
                        if let Some(text) = delta.get("text").and_then(Value::as_str) {
                            emit_incremental_text(&mut state.content_buffer, text, emitter);
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(partial_json) =
                            delta.get("partial_json").and_then(Value::as_str)
                        {
                            state
                                .tool_calls
                                .entry(index)
                                .or_default()
                                .input_json
                                .push_str(partial_json);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    Ok(true)
}

fn finalize_anthropic_tool_calls(
    tool_calls: BTreeMap<usize, PartialAnthropicToolCall>,
) -> AiResult<Vec<ToolCall>> {
    tool_calls
        .into_iter()
        .map(|(index, value)| {
            let name = value.name.trim().to_string();
            if name.is_empty() {
                return Err(AiError::Provider(
                    "streamed Anthropic tool call is missing name".to_string(),
                ));
            }
            let arguments = if value.input_json.trim().is_empty() {
                value.input_value.unwrap_or_else(|| json!({}))
            } else {
                serde_json::from_str(&value.input_json).map_err(|err| {
                    AiError::Provider(format!(
                        "invalid streamed Anthropic tool input for `{name}`: {err}"
                    ))
                })?
            };
            Ok(ToolCall {
                id: value
                    .id
                    .unwrap_or_else(|| format!("anthropic-tool-call-{}", index + 1)),
                name,
                arguments,
            })
        })
        .collect()
}

fn apply_gemini_stream_payload(
    payload: &Value,
    emitter: Option<&ProviderStreamEmitter>,
    state: &mut GeminiStreamingState,
) -> AiResult<bool> {
    match payload {
        Value::Array(items) => {
            for item in items {
                apply_gemini_stream_payload(item, emitter, state)?;
            }
            return Ok(true);
        }
        Value::Object(_) => {}
        _ => return Ok(true),
    }

    if let Some(error_message) = payload
        .get("error")
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str)
    {
        return Err(AiError::Provider(error_message.to_string()));
    }

    let candidate = payload
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|items| items.first());
    let Some(candidate) = candidate else {
        return Ok(true);
    };

    if let Some(reason) = candidate
        .get("finishReason")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        state.finish_reason = Some(reason.to_string());
    }

    let Some(parts) = candidate
        .get("content")
        .and_then(|value| value.get("parts"))
        .and_then(Value::as_array)
    else {
        return Ok(true);
    };

    let mut chunk_text = String::new();
    for part in parts {
        if let Some(text) = part.get("text").and_then(Value::as_str) {
            chunk_text.push_str(text);
        }
        if let Some(function_call) = part.get("functionCall") {
            let Some(name) = function_call.get("name").and_then(Value::as_str) else {
                continue;
            };
            let arguments = function_call
                .get("args")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let dedupe_key = format!(
                "{name}:{}",
                serde_json::to_string(&arguments).unwrap_or_default()
            );
            if state.seen_tool_calls.insert(dedupe_key) {
                state.tool_calls.push(ToolCall {
                    id: format!("gemini-{name}-{}", state.tool_calls.len() + 1),
                    name: name.to_string(),
                    arguments,
                });
            }
        }
    }

    if !chunk_text.is_empty() {
        emit_incremental_text(&mut state.content_buffer, &chunk_text, emitter);
    }

    Ok(true)
}

fn emit_incremental_text(
    content_buffer: &mut String,
    incoming_text: &str,
    emitter: Option<&ProviderStreamEmitter>,
) {
    if incoming_text.is_empty() {
        return;
    }

    let delta = if incoming_text.starts_with(content_buffer.as_str()) {
        let delta = incoming_text[content_buffer.len()..].to_string();
        *content_buffer = incoming_text.to_string();
        delta
    } else {
        content_buffer.push_str(incoming_text);
        incoming_text.to_string()
    };

    if !delta.is_empty() {
        if let Some(emitter) = emitter {
            emitter.emit_text_delta(delta);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::ProviderStreamEvent;

    #[test]
    fn anthropic_stream_payload_accumulates_text_and_tool_input() {
        let deltas = Arc::new(Mutex::new(String::new()));
        let emitter = ProviderStreamEmitter::new({
            let deltas = Arc::clone(&deltas);
            move |event| {
                let ProviderStreamEvent::TextDelta(delta) = event;
                deltas
                    .lock()
                    .expect("delta mutex poisoned")
                    .push_str(&delta);
            }
        });
        let mut state = AnthropicStreamingState::default();

        apply_anthropic_stream_payload(
            &json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": { "type": "tool_use", "id": "toolu_1", "name": "lookup", "input": {} }
            }),
            Some(&emitter),
            &mut state,
        )
        .expect("tool_use start should parse");
        apply_anthropic_stream_payload(
            &json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": { "type": "input_json_delta", "partial_json": "{\"slug\":\"demo\"" }
            }),
            Some(&emitter),
            &mut state,
        )
        .expect("first input delta should parse");
        apply_anthropic_stream_payload(
            &json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": { "type": "input_json_delta", "partial_json": ",\"active\":true}" }
            }),
            Some(&emitter),
            &mut state,
        )
        .expect("second input delta should parse");
        apply_anthropic_stream_payload(
            &json!({
                "type": "content_block_delta",
                "index": 1,
                "delta": { "type": "text_delta", "text": "Привет" }
            }),
            Some(&emitter),
            &mut state,
        )
        .expect("text delta should parse");
        apply_anthropic_stream_payload(
            &json!({
                "type": "message_delta",
                "delta": { "stop_reason": "end_turn" }
            }),
            Some(&emitter),
            &mut state,
        )
        .expect("message delta should parse");

        let tool_calls =
            finalize_anthropic_tool_calls(state.tool_calls).expect("tool calls should finalize");
        assert_eq!(state.content_buffer, "Привет");
        assert_eq!(
            deltas.lock().expect("delta mutex poisoned").as_str(),
            "Привет"
        );
        assert_eq!(state.finish_reason.as_deref(), Some("end_turn"));
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "lookup");
        assert_eq!(
            tool_calls[0].arguments,
            json!({ "slug": "demo", "active": true })
        );
    }

    #[test]
    fn gemini_stream_payload_handles_cumulative_chunks_and_tool_calls() {
        let deltas = Arc::new(Mutex::new(String::new()));
        let emitter = ProviderStreamEmitter::new({
            let deltas = Arc::clone(&deltas);
            move |event| {
                let ProviderStreamEvent::TextDelta(delta) = event;
                deltas
                    .lock()
                    .expect("delta mutex poisoned")
                    .push_str(&delta);
            }
        });
        let mut state = GeminiStreamingState::default();

        apply_gemini_stream_payload(
            &json!({
                "candidates": [{
                    "content": { "parts": [{ "text": "Hel" }] }
                }]
            }),
            Some(&emitter),
            &mut state,
        )
        .expect("first chunk should parse");
        apply_gemini_stream_payload(
            &json!({
                "candidates": [{
                    "content": { "parts": [{ "text": "Hello" }] },
                    "finishReason": "STOP"
                }]
            }),
            Some(&emitter),
            &mut state,
        )
        .expect("second chunk should parse");
        apply_gemini_stream_payload(
            &json!({
                "candidates": [{
                    "content": { "parts": [{
                        "functionCall": { "name": "lookup", "args": { "slug": "demo" } }
                    }] }
                }]
            }),
            Some(&emitter),
            &mut state,
        )
        .expect("tool call chunk should parse");

        assert_eq!(state.content_buffer, "Hello");
        assert_eq!(
            deltas.lock().expect("delta mutex poisoned").as_str(),
            "Hello"
        );
        assert_eq!(state.finish_reason.as_deref(), Some("STOP"));
        assert_eq!(state.tool_calls.len(), 1);
        assert_eq!(state.tool_calls[0].name, "lookup");
        assert_eq!(state.tool_calls[0].arguments, json!({ "slug": "demo" }));
    }
}
