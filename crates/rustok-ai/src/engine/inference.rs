use std::collections::BTreeSet;

use async_trait::async_trait;
use futures_util::StreamExt;
use rig::{
    client::CompletionClient,
    completion::{
        CompletionModel, CompletionRequest, Message, ToolDefinition as RigToolDefinition,
    },
    message::{AssistantContent, ToolCall as RigToolCall, ToolFunction, UserContent},
    providers::{
        anthropic, azure, chatgpt, cohere, copilot, deepseek, galadriel, gemini, groq, huggingface,
        hyperbolic, llamafile, minimax, mira, mistral, moonshot, ollama, openai, openrouter,
        perplexity, together, xai, xiaomimimo, zai,
    },
    streaming::StreamedAssistantContent,
    OneOrMany,
};
use secrecy::ExposeSecret;

use crate::{
    model::{
        AiProviderConfig, ChatMessage, ChatMessageRole, ProviderChatRequest, ProviderChatResponse,
        ProviderImageRequest, ProviderImageResponse, ProviderStreamEmitter, ProviderTestResult,
        ToolCall,
    },
    AiError, AiResult,
};

use super::ProviderSlug;

#[async_trait]
pub trait InferenceEngine: Send + Sync {
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
    ) -> AiResult<ProviderChatResponse>;
    async fn generate_image(
        &self,
        _config: &AiProviderConfig,
        _request: ProviderImageRequest,
    ) -> AiResult<ProviderImageResponse> {
        Err(AiError::Provider(
            "selected Rig provider does not expose image generation".to_string(),
        ))
    }
}

#[derive(Clone)]
pub struct RigInferenceEngine<M> {
    model: M,
    provider: String,
}

fn rig_engine<M>(model: M, provider: &ProviderSlug) -> Box<dyn InferenceEngine>
where
    M: CompletionModel + 'static,
{
    Box::new(RigInferenceEngine {
        model,
        provider: provider.to_string(),
    })
}

pub async fn inference_for_slug(
    slug: &ProviderSlug,
    config: &AiProviderConfig,
    secrets: &rustok_secrets::SecretResolverRegistry,
) -> AiResult<Box<dyn InferenceEngine>> {
    let descriptor = super::provider_catalog_entry(slug)
        .ok_or_else(|| AiError::InvalidConfig(format!("unknown provider integration `{slug}`")))?;
    let credential_field = descriptor.credentials.first();
    let credential = match credential_field {
        Some(field) => {
            let reference = config.credential_refs.get(field.key).ok_or_else(|| {
                AiError::InvalidConfig(format!(
                    "provider credential reference `{}` is required",
                    field.key
                ))
            })?;
            Some(
                secrets
                    .resolve(reference)
                    .await
                    .map_err(|error| AiError::InvalidConfig(error.to_string()))?,
            )
        }
        None => None,
    };
    let api_key = credential
        .as_ref()
        .map(ExposeSecret::expose_secret)
        .unwrap_or("");
    let base_url = config
        .settings
        .get("base_url")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    macro_rules! keyed_engine {
        ($provider:ident) => {{
            let mut builder = $provider::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine(client.completion_model(config.model.clone()), slug)
        }};
    }
    let engine = match slug.as_str() {
        "openai" | "openai_compatible" => {
            let mut builder = openai::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine(client.completion_model(config.model.clone()), slug)
        }
        "anthropic" => {
            let mut builder = anthropic::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine(client.completion_model(config.model.clone()), slug)
        }
        "gemini" => {
            let mut builder = gemini::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine(client.completion_model(config.model.clone()), slug)
        }
        "azure_openai" => keyed_engine!(azure),
        "chatgpt" => keyed_engine!(chatgpt),
        "github_copilot" => keyed_engine!(copilot),
        "cohere" => keyed_engine!(cohere),
        "deepseek" => keyed_engine!(deepseek),
        "galadriel" => keyed_engine!(galadriel),
        "groq" => keyed_engine!(groq),
        "hugging_face" => keyed_engine!(huggingface),
        "hyperbolic" => keyed_engine!(hyperbolic),
        "llamafile" => {
            let mut builder = llamafile::Client::builder().api_key(rig::client::Nothing);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine(client.completion_model(config.model.clone()), slug)
        }
        "minimax" => keyed_engine!(minimax),
        "mira" => keyed_engine!(mira),
        "mistral" => keyed_engine!(mistral),
        "moonshot" => keyed_engine!(moonshot),
        "ollama" => keyed_engine!(ollama),
        "openrouter" => keyed_engine!(openrouter),
        "perplexity" => keyed_engine!(perplexity),
        "together" => keyed_engine!(together),
        "xai" => keyed_engine!(xai),
        "xiaomi_mimo" => keyed_engine!(xiaomimimo),
        "zai" => keyed_engine!(zai),
        other => {
            return Err(AiError::InvalidConfig(format!(
                "Rig provider `{other}` is catalogued but its typed runtime factory is not linked"
            )))
        }
    };
    Ok(engine)
}

#[async_trait]
impl<M> InferenceEngine for RigInferenceEngine<M>
where
    M: CompletionModel + 'static,
{
    async fn test_connection(&self, config: &AiProviderConfig) -> AiResult<ProviderTestResult> {
        let started = std::time::Instant::now();
        let response = self
            .complete(
                config,
                ProviderChatRequest {
                    model: config.model.clone(),
                    messages: vec![ChatMessage {
                        role: ChatMessageRole::User,
                        content: Some("Reply with OK.".to_string()),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: serde_json::json!({"connectivity_test": true}),
                    }],
                    tools: Vec::new(),
                    temperature: Some(0.0),
                    max_tokens: Some(8),
                    locale: None,
                },
            )
            .await?;
        Ok(ProviderTestResult {
            ok: true,
            provider: self.provider.clone(),
            model: Some(config.model.clone()),
            latency_ms: started.elapsed().as_millis() as i64,
            message: response
                .assistant_message
                .content
                .unwrap_or_else(|| "Provider responded successfully".to_string()),
        })
    }

    async fn complete(
        &self,
        _config: &AiProviderConfig,
        request: ProviderChatRequest,
    ) -> AiResult<ProviderChatResponse> {
        let request = map_request(request)?;
        complete_with(&self.model, request, &self.provider).await
    }

    async fn complete_stream(
        &self,
        _config: &AiProviderConfig,
        request: ProviderChatRequest,
        emitter: Option<ProviderStreamEmitter>,
    ) -> AiResult<ProviderChatResponse> {
        let request = map_request(request)?;
        stream_with(&self.model, request, &self.provider, emitter).await
    }
}

async fn complete_with<M: CompletionModel>(
    model: &M,
    request: CompletionRequest,
    provider: &str,
) -> AiResult<ProviderChatResponse> {
    let response = model
        .completion(request)
        .await
        .map_err(|error| AiError::Provider(error.to_string()))?;
    let raw_payload = serde_json::to_value(&response.raw_response)
        .unwrap_or_else(|_| serde_json::json!({"provider": provider}));
    Ok(map_response(
        response.choice.into_iter().collect(),
        response.message_id,
        raw_payload,
        provider,
    ))
}

async fn stream_with<M: CompletionModel>(
    model: &M,
    request: CompletionRequest,
    provider: &str,
    emitter: Option<ProviderStreamEmitter>,
) -> AiResult<ProviderChatResponse> {
    let mut stream = model
        .stream(request)
        .await
        .map_err(|error| AiError::Provider(error.to_string()))?;
    while let Some(item) = stream.next().await {
        match item.map_err(|error| AiError::Provider(error.to_string()))? {
            StreamedAssistantContent::Text(text) => {
                if let Some(emitter) = &emitter {
                    emitter.emit_text_delta(text.text);
                }
            }
            StreamedAssistantContent::ToolCall { .. }
            | StreamedAssistantContent::ToolCallDelta { .. }
            | StreamedAssistantContent::Reasoning(_)
            | StreamedAssistantContent::ReasoningDelta { .. }
            | StreamedAssistantContent::Final(_) => {}
        }
    }
    let raw_payload = stream
        .response
        .as_ref()
        .and_then(|value| serde_json::to_value(value).ok())
        .unwrap_or_else(|| serde_json::json!({"provider": provider, "streaming": true}));
    Ok(map_response(
        stream.choice.into_iter().collect(),
        stream.message_id,
        raw_payload,
        provider,
    ))
}

fn map_request(request: ProviderChatRequest) -> AiResult<CompletionRequest> {
    let mut messages = request
        .messages
        .into_iter()
        .map(map_message)
        .collect::<AiResult<Vec<_>>>()?;
    if messages.is_empty() {
        return Err(AiError::Validation(
            "provider request requires at least one message".to_string(),
        ));
    }
    let tools = request
        .tools
        .into_iter()
        .map(|tool| RigToolDefinition {
            name: tool.name,
            description: tool.description,
            parameters: tool.input_schema,
        })
        .collect();
    let history = OneOrMany::many(std::mem::take(&mut messages))
        .map_err(|error| AiError::Validation(error.to_string()))?;
    Ok(CompletionRequest {
        model: Some(request.model),
        preamble: None,
        chat_history: history,
        documents: Vec::new(),
        tools,
        temperature: request.temperature.map(f64::from),
        max_tokens: request.max_tokens.map(u64::from),
        tool_choice: None,
        additional_params: None,
        output_schema: None,
    })
}

pub(crate) fn map_message(message: ChatMessage) -> AiResult<Message> {
    Ok(match message.role {
        ChatMessageRole::System => Message::system(message.content.unwrap_or_default()),
        ChatMessageRole::User => Message::user(message.content.unwrap_or_default()),
        ChatMessageRole::Tool => Message::tool_result(
            message.tool_call_id.ok_or_else(|| {
                AiError::Validation("tool message requires tool_call_id".to_string())
            })?,
            message.content.unwrap_or_default(),
        ),
        ChatMessageRole::Assistant => {
            let mut content = Vec::new();
            if let Some(text) = message.content.filter(|value| !value.is_empty()) {
                content.push(AssistantContent::text(text));
            }
            content.extend(message.tool_calls.into_iter().map(|call| {
                AssistantContent::ToolCall(RigToolCall::new(
                    call.id,
                    ToolFunction::new(call.name, call.arguments),
                ))
            }));
            if content.is_empty() {
                content.push(AssistantContent::text(""));
            }
            Message::Assistant {
                id: None,
                content: OneOrMany::many(content)
                    .map_err(|error| AiError::Serialization(error.to_string()))?,
            }
        }
    })
}

pub(crate) fn map_rig_message(message: Message) -> ChatMessage {
    match message {
        Message::System { content } => ChatMessage {
            role: ChatMessageRole::System,
            content: Some(content),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
            metadata: serde_json::json!({"engine": "rig_0_39"}),
        },
        Message::User { content } => {
            let mut text = String::new();
            let mut tool_call_id = None;
            for item in content {
                match item {
                    UserContent::Text(value) => text.push_str(&value.text),
                    UserContent::ToolResult(result) => {
                        tool_call_id = Some(result.id);
                        for value in result.content {
                            if let rig::message::ToolResultContent::Text(value) = value {
                                text.push_str(&value.text);
                            }
                        }
                    }
                    UserContent::Image(_)
                    | UserContent::Audio(_)
                    | UserContent::Video(_)
                    | UserContent::Document(_) => {}
                }
            }
            ChatMessage {
                role: if tool_call_id.is_some() {
                    ChatMessageRole::Tool
                } else {
                    ChatMessageRole::User
                },
                content: (!text.is_empty()).then_some(text),
                name: None,
                tool_call_id,
                tool_calls: Vec::new(),
                metadata: serde_json::json!({"engine": "rig_0_39"}),
            }
        }
        Message::Assistant { id, content } => {
            let response = map_response(
                content.into_iter().collect(),
                id,
                serde_json::Value::Null,
                "rig",
            );
            response.assistant_message
        }
    }
}

pub(crate) fn assistant_choice(message: &ChatMessage) -> AiResult<OneOrMany<AssistantContent>> {
    let Message::Assistant { content, .. } = map_message(message.clone())? else {
        return Err(AiError::Validation(
            "Rig model turn must be an assistant message".to_string(),
        ));
    };
    Ok(content)
}

fn map_response(
    content: Vec<AssistantContent>,
    message_id: Option<String>,
    raw_payload: serde_json::Value,
    provider: &str,
) -> ProviderChatResponse {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for item in content {
        match item {
            AssistantContent::Text(value) => text.push_str(&value.text),
            AssistantContent::ToolCall(call) => tool_calls.push(ToolCall {
                id: call.id,
                name: call.function.name,
                arguments: call.function.arguments,
            }),
            AssistantContent::Reasoning(_) | AssistantContent::Image(_) => {}
        }
    }
    ProviderChatResponse {
        assistant_message: ChatMessage {
            role: ChatMessageRole::Assistant,
            content: (!text.is_empty()).then_some(text),
            name: None,
            tool_call_id: None,
            tool_calls,
            metadata: serde_json::json!({
                "provider": provider,
                "message_id": message_id,
                "engine": "rig_0_39"
            }),
        },
        finish_reason: None,
        raw_payload,
    }
}

#[allow(dead_code)]
fn tool_names(request: &CompletionRequest) -> BTreeSet<String> {
    request.tools.iter().map(|tool| tool.name.clone()).collect()
}
