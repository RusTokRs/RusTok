use async_trait::async_trait;
use futures_util::StreamExt;
use rig::{
    DynModel,
    completion::{
        CompletionRequest, CompletionResponse, Message, ToolDefinition as RigToolDefinition,
    },
    image_generation::ImageGenerationRequestBuilder,
    message::{
        AssistantContent, AssistantMessage, CallId, ToolCall as RigToolCall, ToolFunction,
        ToolName, UserContent,
    },
    operation::{Completion, ImageGeneration},
    providers::{anthropic, chatgpt, cohere, copilot, gemini, ollama, openai, xai},
    streaming::{Item, StreamEvent},
};
use rig_bedrock::client::BedrockRuntime;
use secrecy::ExposeSecret;

use crate::{
    AiError, AiResult,
    model::{
        AiProviderConfig, ChatMessage, ChatMessageRole, ProviderChatRequest, ProviderChatResponse,
        ProviderImageRequest, ProviderImageResponse, ProviderStreamEmitter,
        ProviderStructuredRequest, ProviderStructuredResponse, ProviderTestResult, ProviderUsage,
        ToolCall,
    },
};

use super::{ProviderSlug, catalog::ProviderIntegration};

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
    async fn complete_structured(
        &self,
        request: ProviderStructuredRequest,
    ) -> AiResult<ProviderStructuredResponse>;
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
pub struct RigInferenceEngine {
    model: DynModel<Completion>,
    provider: String,
    image: Option<DynModel<ImageGeneration>>,
}

fn rig_engine(
    model: impl Into<DynModel<Completion>>,
    provider: &ProviderSlug,
) -> Box<dyn InferenceEngine> {
    Box::new(RigInferenceEngine {
        model: model.into(),
        provider: provider.to_string(),
        image: None,
    })
}

fn rig_engine_with_image(
    model: impl Into<DynModel<Completion>>,
    image: impl Into<DynModel<ImageGeneration>>,
    provider: &ProviderSlug,
) -> Box<dyn InferenceEngine> {
    Box::new(RigInferenceEngine {
        model: model.into(),
        provider: provider.to_string(),
        image: Some(image.into()),
    })
}

pub(super) fn openai_config(
    dialect: &'static openai::wire::Dialect,
    key: &str,
    base_url: &str,
) -> openai::OpenAIConfig {
    let mut config = openai::OpenAIConfig::with_key(dialect, key.to_owned());
    if !base_url.is_empty() {
        config = config.with_base_url(base_url.to_owned());
    }
    config
}

// The persisted local integration slug remains stable when Rig renames its adapter.
pub(super) fn llamacpp_base_url(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1") {
        base.to_owned()
    } else {
        format!("{base}/v1")
    }
}

pub(super) fn bedrock_runtime(config: &AiProviderConfig) -> AiResult<BedrockRuntime> {
    let region = config
        .settings
        .get("region")
        .and_then(serde_json::Value::as_str)
        .filter(|region| !region.trim().is_empty())
        .ok_or_else(|| AiError::InvalidConfig("Bedrock region is required".to_string()))?;
    let mut builder = BedrockRuntime::builder().region(region);
    if let Some(profile) = config
        .settings
        .get("profile")
        .and_then(serde_json::Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        builder = builder.profile_name(profile);
    }
    Ok(builder.build())
}

pub(super) async fn gemini_grpc_runtime(
    key: &str,
    base_url: &str,
) -> AiResult<rig_gemini_grpc::GeminiGrpc> {
    let mut config = gemini::GeminiConfig::new(key.to_owned());
    if !base_url.is_empty() {
        config = config.with_base_url(base_url.to_owned());
    }
    rig_gemini_grpc::GeminiGrpc::connect(&config)
        .await
        .map_err(|error| AiError::InvalidConfig(error.to_string()))
}

pub async fn inference_for_slug(
    slug: &ProviderSlug,
    config: &AiProviderConfig,
    secrets: &rustok_secrets::SecretResolverRegistry,
) -> AiResult<Box<dyn InferenceEngine>> {
    let descriptor = super::provider_catalog_entry(slug)
        .ok_or_else(|| AiError::InvalidConfig(format!("unknown provider integration `{slug}`")))?;
    let integration = descriptor.integration;
    let credential = if matches!(config.target_auth, crate::ProviderTargetAuth::SecretRefs) {
        match descriptor.credentials.first() {
            Some(field) => {
                let reference = config.credential_refs.get(field.key).ok_or_else(|| {
                    AiError::InvalidConfig(format!(
                        "provider credential reference `{}` is required",
                        field.key
                    ))
                })?;
                Some(
                    secrets
                        .resolve_for_tenant(config.tenant_id, reference)
                        .await
                        .map_err(|error| AiError::InvalidConfig(error.to_string()))?,
                )
            }
            None => None,
        }
    } else {
        None
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
    let image_model = config
        .settings
        .get("image_model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&config.model);
    macro_rules! keyed_engine {
        ($dialect:expr) => {{
            let client = openai_config($dialect, api_key, base_url).client();
            rig_engine(client.completion(config.model.clone()), slug)
        }};
    }
    let engine = match integration {
        ProviderIntegration::OpenAi | ProviderIntegration::OpenAiCompatible => {
            let client = openai_config(&openai::wire::OPENAI, api_key, base_url).client();
            let model = if integration == ProviderIntegration::OpenAiCompatible {
                client.chat(config.model.clone()).erase()
            } else {
                client.responses(config.model.clone()).erase()
            };
            rig_engine_with_image(model, client.image_generation(image_model), slug)
        }
        ProviderIntegration::Anthropic => {
            let mut target = anthropic::AnthropicConfig::new(api_key.to_owned());
            if !base_url.is_empty() {
                target = target.with_base_url(base_url.to_owned());
            }
            rig_engine(target.client().completion(config.model.clone()), slug)
        }
        ProviderIntegration::Gemini => {
            let mut target = gemini::GeminiConfig::new(api_key.to_owned());
            if !base_url.is_empty() {
                target = target.with_base_url(base_url.to_owned());
            }
            let client = target.client();
            rig_engine_with_image(
                client.completion(config.model.clone()),
                client.image_generation(image_model),
                slug,
            )
        }
        ProviderIntegration::AzureOpenAi => {
            let version = config
                .settings
                .get("api_version")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    AiError::InvalidConfig("Azure API version is required".to_string())
                })?;
            let client = openai_config(&openai::wire::AZURE, api_key, base_url)
                .with_api_version(version.to_owned())
                .client();
            rig_engine(client.completion(config.model.clone()), slug)
        }
        ProviderIntegration::ChatGpt => keyed_engine!(&chatgpt::DIALECT),
        ProviderIntegration::GithubCopilot => keyed_engine!(&copilot::DIALECT),
        ProviderIntegration::Cohere => {
            let mut target = cohere::CohereConfig::new(api_key.to_owned());
            if !base_url.is_empty() {
                target = target.with_base_url(base_url.to_owned());
            }
            let mut model = target.client().completion(config.model.clone());
            model.wire = model.wire.with_route(cohere::ChatRoute::Native);
            rig_engine(model, slug)
        }
        ProviderIntegration::DeepSeek => keyed_engine!(&openai::wire::DEEPSEEK),
        ProviderIntegration::Groq => keyed_engine!(&openai::wire::GROQ),
        ProviderIntegration::HuggingFace => {
            let client = openai_config(&openai::wire::HUGGINGFACE, api_key, base_url).client();
            rig_engine_with_image(
                client.completion(config.model.clone()),
                client.image_generation(image_model),
                slug,
            )
        }
        ProviderIntegration::Hyperbolic => keyed_engine!(&openai::wire::HYPERBOLIC),
        ProviderIntegration::Llamafile => {
            let base = if base_url.is_empty() {
                String::new()
            } else {
                llamacpp_base_url(base_url)
            };
            let client = openai_config(&openai::wire::LLAMACPP, api_key, &base).client();
            rig_engine(client.completion(config.model.clone()), slug)
        }
        ProviderIntegration::MiniMax => keyed_engine!(&openai::wire::MINIMAX),
        ProviderIntegration::Mira => keyed_engine!(&openai::wire::MIRA),
        ProviderIntegration::Mistral => keyed_engine!(&openai::wire::MISTRAL),
        ProviderIntegration::Moonshot => keyed_engine!(&openai::wire::MOONSHOT),
        ProviderIntegration::Ollama => {
            let mut target = ollama::OllamaConfig::new();
            if !base_url.is_empty() {
                target = target.with_base_url(base_url.to_owned());
            }
            rig_engine(
                target.client().native_completion(config.model.clone()),
                slug,
            )
        }
        ProviderIntegration::OpenRouter => keyed_engine!(&openai::wire::OPENROUTER),
        ProviderIntegration::Perplexity => keyed_engine!(&openai::wire::PERPLEXITY),
        ProviderIntegration::Together => keyed_engine!(&openai::wire::TOGETHER),
        ProviderIntegration::Xai => {
            let client = openai_config(&xai::DIALECT, api_key, base_url).client();
            rig_engine_with_image(
                client.completion(config.model.clone()),
                client.image_generation(image_model),
                slug,
            )
        }
        ProviderIntegration::XiaomiMimo => keyed_engine!(&openai::wire::XIAOMIMIMO),
        ProviderIntegration::Zai => keyed_engine!(&openai::wire::ZAI),
        ProviderIntegration::AwsBedrock => {
            let client = bedrock_runtime(config)?;
            rig_engine_with_image(
                client.completion(config.model.clone()),
                client.image_generation(image_model),
                slug,
            )
        }
        ProviderIntegration::VertexAi => {
            let project = config
                .settings
                .get("project")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| AiError::InvalidConfig("Vertex project is required".to_string()))?;
            let mut builder = rig_vertexai::VertexAi::builder().with_project(project);
            if let Some(location) = config
                .settings
                .get("location")
                .and_then(serde_json::Value::as_str)
            {
                builder = builder.with_location(location);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine(client.completion(config.model.clone()), slug)
        }
        ProviderIntegration::GeminiGrpc => {
            let client = gemini_grpc_runtime(api_key, base_url).await?;
            rig_engine(client.completion(config.model.clone()), slug)
        }
        ProviderIntegration::VoyageAi | ProviderIntegration::Fastembed => {
            return Err(AiError::InvalidConfig(format!(
                "Rig provider `{slug}` does not expose a chat runtime factory"
            )));
        }
    };
    Ok(engine)
}

#[async_trait]
impl InferenceEngine for RigInferenceEngine {
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

    async fn complete_structured(
        &self,
        request: ProviderStructuredRequest,
    ) -> AiResult<ProviderStructuredResponse> {
        let mut completion = map_request(request.request)?;
        completion.output_schema = Some(
            schemars::Schema::try_from(request.output_schema)
                .map_err(|error| AiError::Validation(error.to_string()))?,
        );
        let response = complete_with(&self.model, completion, &self.provider).await?;
        let usage = extract_usage(&response.raw_payload);
        let content = response.assistant_message.content.ok_or_else(|| {
            AiError::Provider("Rig structured output returned empty content".to_string())
        })?;
        let output = serde_json::from_str(content.trim()).map_err(AiError::Json)?;
        Ok(ProviderStructuredResponse { output, usage })
    }

    async fn generate_image(
        &self,
        _config: &AiProviderConfig,
        request: ProviderImageRequest,
    ) -> AiResult<ProviderImageResponse> {
        let image = self.image.as_ref().ok_or_else(|| {
            AiError::Provider(format!(
                "Rig provider `{}` does not expose image generation",
                self.provider
            ))
        })?;
        let (width, height) = parse_image_size(request.size.as_deref())?;
        let mut builder = ImageGenerationRequestBuilder::new(request.prompt)
            .width(width)
            .height(height);
        if let Some(negative_prompt) = request.negative_prompt {
            builder =
                builder.additional_params(serde_json::json!({"negative_prompt": negative_prompt}));
        }
        let response = image
            .call(builder.build())
            .await
            .map_err(|error| AiError::Provider(error.to_string()))?;
        let mime_type = rig::embeddings::image_media_type(&response.image)
            .ok_or_else(|| AiError::Provider("unsupported generated image format".to_string()))?;
        Ok(ProviderImageResponse {
            bytes: response.image,
            mime_type: mime_type.to_owned(),
            revised_prompt: None,
            raw_payload: serde_json::json!({"provider": self.provider, "usage": response.usage}),
        })
    }
}

fn parse_image_size(value: Option<&str>) -> AiResult<(u32, u32)> {
    let value = value.unwrap_or("1024x1024");
    let (width, height) = value.split_once('x').ok_or_else(|| {
        AiError::Validation("image size must use WIDTHxHEIGHT format".to_string())
    })?;
    let width = width
        .parse::<u32>()
        .map_err(|_| AiError::Validation("invalid image width".to_string()))?;
    let height = height
        .parse::<u32>()
        .map_err(|_| AiError::Validation("invalid image height".to_string()))?;
    if width == 0 || height == 0 {
        return Err(AiError::Validation(
            "image dimensions must be positive".to_string(),
        ));
    }
    Ok((width, height))
}

async fn complete_with(
    model: &DynModel<Completion>,
    request: CompletionRequest,
    provider: &str,
) -> AiResult<ProviderChatResponse> {
    let response = model
        .call(request)
        .await
        .map_err(|error| AiError::Provider(error.to_string()))?;
    map_completion(response, provider)
}

fn map_completion(response: CompletionResponse, provider: &str) -> AiResult<ProviderChatResponse> {
    if let Some(error) = rig::message::turn_failure(
        &response.choice,
        Some(&response.stop()),
        response.finish_reason().as_ref(),
    ) {
        return Err(AiError::Provider(error));
    }
    if response.stop().is_failure()
        || response.unfinished_calls().next().is_some()
        || (matches!(
            response.finish_reason(),
            Some(rig::completion::FinishReason::Length)
        ) && response
            .choice
            .iter()
            .any(|item| matches!(item, AssistantContent::ToolCall(_))))
    {
        return Err(AiError::Provider(
            "Rig completion failed or returned unfinished tool calls".to_string(),
        ));
    }
    let finish_reason = response
        .finish_reason()
        .map(|reason| {
            use rig::completion::FinishReason;
            Ok(match reason {
                FinishReason::Stop => "stop".to_string(),
                FinishReason::Length => "length".to_string(),
                FinishReason::ToolCalls => "tool_calls".to_string(),
                FinishReason::ContentFilter => "content_filter".to_string(),
                FinishReason::Other(value) => value,
                _ => {
                    return Err(AiError::Provider(
                        "unsupported Rig finish reason".to_string(),
                    ));
                }
            })
        })
        .transpose()?;
    let raw_payload = serde_json::json!({
        "provider": provider,
        "response_id": response.response_id(),
        "provider_request_id": response.provider_request_id,
        "usage": response.usage,
    });
    let mut mapped = map_response(
        response.choice,
        response.origin.response_id,
        raw_payload,
        provider,
    )?;
    mapped.finish_reason = finish_reason;
    Ok(mapped)
}

async fn stream_with(
    model: &DynModel<Completion>,
    request: CompletionRequest,
    provider: &str,
    emitter: Option<ProviderStreamEmitter>,
) -> AiResult<ProviderChatResponse> {
    let mut stream = model
        .stream(request)
        .map_err(|error| AiError::Provider(error.to_string()))?;
    while let Some(item) = stream.next().await {
        match item.map_err(|error| AiError::Provider(error.to_string()))? {
            Item::Event(StreamEvent::Text { text, .. }) => {
                if let Some(emitter) = &emitter {
                    emitter.emit_text_delta(text);
                }
            }
            Item::Event(StreamEvent::End {
                content: AssistantContent::ToolCall(call),
                ..
            }) => {
                if let Some(emitter) = &emitter {
                    emitter.emit_tool_call(ToolCall {
                        id: call.id.to_string(),
                        name: call.function.name.to_string(),
                        arguments: rig_arguments(&call.function),
                    });
                }
            }
            Item::Unknown(_) => {
                tracing::debug!(provider, "received an unmodeled provider streaming item");
            }
            _ => {}
        }
    }
    // The SDK owns assembly and detects truncated replies; never synthesize a successful response.
    let response = stream
        .finish()
        .await
        .map_err(|error| AiError::Provider(error.to_string()))?;
    let response = map_completion(response, provider)?;
    if let (Some(usage), Some(emitter)) = (extract_usage(&response.raw_payload), emitter.as_ref()) {
        emitter.emit_usage(usage);
    }
    Ok(response)
}

pub(crate) fn extract_usage(payload: &serde_json::Value) -> Option<ProviderUsage> {
    let usage = payload.get("usage")?;
    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(serde_json::Value::as_u64)?;
    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(serde_json::Value::as_u64)?;
    let total_tokens = usage
        .get("total_tokens")
        .and_then(serde_json::Value::as_u64);
    Some(ProviderUsage::normalized(
        input_tokens,
        output_tokens,
        total_tokens,
    ))
}

#[cfg(test)]
mod usage_tests {
    use super::extract_usage;

    #[test]
    fn normalizes_openai_and_anthropic_token_names() {
        let openai = extract_usage(&serde_json::json!({"usage":{"prompt_tokens":3,"completion_tokens":5,"total_tokens":8}})).unwrap();
        assert_eq!(openai.total_tokens, 8);
        let anthropic =
            extract_usage(&serde_json::json!({"usage":{"input_tokens":3,"output_tokens":5}}))
                .unwrap();
        assert_eq!(anthropic.total_tokens, 8);
        assert!(extract_usage(&serde_json::json!({})).is_none());
    }

    #[test]
    fn unknown_counters_are_unavailable_and_explicit_zero_is_evidence() {
        for usage in [
            serde_json::json!({}),
            serde_json::json!({"input_tokens":3}),
            serde_json::json!({"total_tokens":8}),
            serde_json::json!({"input_tokens":null,"output_tokens":0}),
        ] {
            assert!(extract_usage(&serde_json::json!({"usage":usage})).is_none());
        }
        let zero =
            extract_usage(&serde_json::json!({"usage":{"input_tokens":0,"output_tokens":0}}))
                .unwrap();
        assert_eq!(zero.total_tokens, 0);
    }
}

#[cfg(test)]
mod sdk_tests {
    use super::*;
    use rig::test_utils::{MockStreamingClient, RecordingHttpClient};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    fn request() -> CompletionRequest {
        CompletionRequest::new("hello").max_tokens(64)
    }

    fn sse(frames: Vec<serde_json::Value>) -> String {
        frames
            .into_iter()
            .map(|frame| format!("data: {frame}\n\n"))
            .collect()
    }

    #[tokio::test]
    async fn sdk_chat_wire_preserves_route_history_and_usage() {
        let http = RecordingHttpClient::new(json!({"id":"response-1","object":"chat.completion","created":1,"model":"test-model","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}).to_string());
        let model = openai::OpenAIConfig::new("test-key")
            .with_base_url("https://inference.example/v1")
            .connect(http.clone())
            .chat("test-model")
            .erase();
        let response = complete_with(
            &model,
            request().messages([Message::system("policy")]),
            "openai_compatible",
        )
        .await
        .unwrap();
        assert_eq!(response.assistant_message.content.as_deref(), Some("OK"));
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert_eq!(
            extract_usage(&response.raw_payload).unwrap().total_tokens,
            5
        );
        assert_eq!(response.raw_payload["response_id"], "response-1");
        let captured = http.requests();
        assert_eq!(
            captured[0].uri,
            "https://inference.example/v1/chat/completions"
        );
        let body: serde_json::Value = serde_json::from_slice(&captured[0].body).unwrap();
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["content"], "hello");
    }

    #[tokio::test]
    async fn sdk_stream_assembles_fragmented_tool_arguments_once() {
        let mut bytes = sse(vec![
            json!({"id":"reply","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"lookup","arguments":"{\"id\":"}}]}}]}),
            json!({"id":"reply","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"7}"}}]}}]}),
            json!({"id":"reply","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}),
        ]);
        bytes.push_str("data: [DONE]\n\n");
        let model = openai::OpenAIConfig::new("test-key")
            .connect(MockStreamingClient {
                sse_bytes: bytes.into(),
            })
            .chat("test-model")
            .erase();
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let emitter = ProviderStreamEmitter::new(move |event| captured.lock().unwrap().push(event));
        let response = stream_with(&model, request(), "openai_compatible", Some(emitter))
            .await
            .unwrap();
        assert_eq!(response.assistant_message.tool_calls.len(), 1);
        assert_eq!(
            response.assistant_message.tool_calls[0].arguments,
            json!({"id":7})
        );
        let events = events.lock().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, crate::ProviderStreamEvent::ToolCall(_)))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, crate::ProviderStreamEvent::Usage(_)))
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn sdk_stream_rejects_eof_without_provider_completion() {
        let bytes = sse(vec![
            json!({"choices":[{"index":0,"delta":{"content":"partial"}}]}),
        ]);
        let model = openai::OpenAIConfig::new("test-key")
            .connect(MockStreamingClient {
                sse_bytes: bytes.into(),
            })
            .chat("test-model")
            .erase();
        assert!(
            stream_with(&model, request(), "openai_compatible", None)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn sdk_native_streams_normalize_anthropic_and_gemini_usage() {
        let bytes = sse(vec![
            json!({"type":"message_start","message":{"id":"msg-1","type":"message","role":"assistant","model":"test-model","content":[],"usage":{"input_tokens":3,"output_tokens":0}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"OK"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}),
            json!({"type":"message_stop"}),
        ]);
        let anthropic = anthropic::AnthropicConfig::new("test-key")
            .connect(MockStreamingClient {
                sse_bytes: bytes.into(),
            })
            .completion("test-model")
            .erase();
        let bytes = sse(vec![
            json!({"candidates":[{"index":0,"content":{"role":"model","parts":[{"text":"OK"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":2,"totalTokenCount":5}}),
        ]);
        let gemini = gemini::GeminiConfig::new("test-key")
            .connect(MockStreamingClient {
                sse_bytes: bytes.into(),
            })
            .completion("test-model")
            .erase();
        for (model, provider) in [(anthropic, "anthropic"), (gemini, "gemini")] {
            let response = stream_with(&model, request(), provider, None)
                .await
                .unwrap();
            assert_eq!(response.assistant_message.content.as_deref(), Some("OK"));
            assert_eq!(
                extract_usage(&response.raw_payload).unwrap().total_tokens,
                5
            );
        }
    }

    #[test]
    fn projection_preserves_all_tool_results_and_rejects_argument_coercion() {
        let rig = Message::tool_results(vec![
            rig::message::ToolResult {
                call: CallId::from_wire("a"),
                name: tool_name("lookup".to_string()).unwrap(),
                content: vec![rig::message::ToolResultContent::text("first")],
                is_error: true,
            },
            rig::message::ToolResult {
                call: CallId::from_wire("b"),
                name: tool_name("lookup".to_string()).unwrap(),
                content: vec![rig::message::ToolResultContent::text("second")],
                is_error: false,
            },
        ]);
        let messages = map_rig_messages(rig).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("b"));
        for (index, message) in messages.into_iter().enumerate() {
            let Message::User { content } = map_message(message).unwrap() else {panic!("tool result must map to a user message")};
            let UserContent::ToolResult(result) = &content[0] else {panic!("tool result must survive projection")};
            assert_eq!(result.is_error,index==0);
        }
        for invalid in [json!(null), json!("{}"), json!([])] {
            let function =
                strict_tool_function(tool_name("lookup".to_string()).unwrap(), invalid.clone())
                    .unwrap();
            assert_eq!(rig_arguments(&function), invalid);
            assert!(function.invalid_arguments.is_some());
        }
    }

    #[test]
    fn local_endpoint_has_one_version_suffix() {
        assert_eq!(
            llamacpp_base_url("http://localhost:8080/"),
            "http://localhost:8080/v1"
        );
        assert_eq!(
            llamacpp_base_url("http://localhost:8080/v1/"),
            "http://localhost:8080/v1"
        );
    }
}

#[cfg(test)]
mod live_connectivity_tests {
    use super::inference_for_slug;
    use crate::model::AiProviderConfig;
    use rustok_secrets::{EnvResolver, SecretAccessPolicy, SecretResolverRegistry};

    /// Deployment-only connectivity probe. It is ignored by default because it
    /// makes real network calls and resolves only explicitly prefixed env keys.
    #[tokio::test]
    #[ignore = "requires deployment-owned RUSTOK_AI_LIVE_PROVIDER_CONFIGS_JSON and provider credentials"]
    async fn probes_each_declared_live_provider_target() {
        let raw = std::env::var("RUSTOK_AI_LIVE_PROVIDER_CONFIGS_JSON").expect(
            "set RUSTOK_AI_LIVE_PROVIDER_CONFIGS_JSON to a non-empty JSON array of AiProviderConfig values",
        );
        let configs: Vec<AiProviderConfig> = serde_json::from_str(&raw)
            .expect("live provider configs must be valid AiProviderConfig JSON");
        assert!(
            !configs.is_empty(),
            "at least one deployment-owned live target is required"
        );
        let secrets = SecretResolverRegistry::builder()
            .resolver(
                "env",
                EnvResolver,
                SecretAccessPolicy::Prefix(vec!["RUSTOK_AI_LIVE_".to_string()]),
            )
            .build();

        for config in configs {
            let provider = inference_for_slug(&config.provider_slug, &config, &secrets)
                .await
                .unwrap_or_else(|error| {
                    panic!("{} live factory failed: {error}", config.provider_slug)
                });
            let result = provider
                .test_connection(&config)
                .await
                .unwrap_or_else(|error| {
                    panic!("{} live connectivity failed: {error}", config.provider_slug)
                });
            assert!(
                result.ok,
                "{} live connectivity was not accepted",
                config.provider_slug
            );
        }
    }
}

fn map_request(request: ProviderChatRequest) -> AiResult<CompletionRequest> {
    let mut messages = request
        .messages
        .into_iter()
        .map(map_message)
        .collect::<AiResult<Vec<_>>>()?;
    let prompt = messages.pop().ok_or_else(|| {
        AiError::Validation("provider request requires at least one message".to_string())
    })?;
    let tools = request
        .tools
        .into_iter()
        .map(|tool| {
            Ok(RigToolDefinition::new(
                tool_name(tool.name)?,
                tool.description,
                tool.input_schema,
            ))
        })
        .collect::<AiResult<Vec<_>>>()?;
    Ok(CompletionRequest::new(prompt)
        .messages(messages)
        .model(request.model)
        .tools(tools)
        .temperature(request.temperature.map(f64::from))
        .max_tokens(request.max_tokens.map(u64::from)))
}

fn tool_name(value: String) -> AiResult<ToolName> {
    ToolName::new(value).map_err(|error| AiError::Validation(error.to_string()))
}

pub(crate) fn map_message(message: ChatMessage) -> AiResult<Message> {
    Ok(match message.role {
        ChatMessageRole::System => Message::system(message.content.unwrap_or_default()),
        ChatMessageRole::User => Message::user(message.content.unwrap_or_default()),
        ChatMessageRole::Tool => {
            let tool_call_id = message.tool_call_id.ok_or_else(|| {
                AiError::Validation("tool message requires tool_call_id".to_string())
            })?;
            Message::tool_results(vec![rig::message::ToolResult {
                call: CallId::from_wire(tool_call_id),
                name: tool_name(message.name.unwrap_or_default())?,
                content: vec![rig::message::ToolResultContent::text(message.content.unwrap_or_default())],
                is_error: message.metadata.get("skipped").and_then(serde_json::Value::as_bool).unwrap_or(false),
            }])
        }
        ChatMessageRole::Assistant => {
            let mut content = Vec::new();
            if let Some(text) = message.content.filter(|value| !value.is_empty()) {
                content.push(AssistantContent::text(text));
            }
            for call in message.tool_calls {
                content.push(AssistantContent::ToolCall(RigToolCall::from_wire(
                    call.id,
                    strict_tool_function(tool_name(call.name)?, call.arguments)?,
                )));
            }
            if content.is_empty() {
                content.push(AssistantContent::text(""));
            }
            Message::Assistant(AssistantMessage::new(content))
        }
    })
}

fn strict_tool_function(name: ToolName, arguments: serde_json::Value) -> AiResult<ToolFunction> {
    let mut function = ToolFunction::new(name, arguments.clone());
    if !arguments.is_object() {
        function.arguments.clear();
        function.invalid_arguments = Some(serde_json::to_string(&arguments)?);
    }
    Ok(function)
}

fn rig_arguments(function: &ToolFunction) -> serde_json::Value {
    match &function.invalid_arguments {
        Some(raw) => {
            serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.clone()))
        }
        None => function.arguments_value(),
    }
}

pub(crate) fn map_rig_messages(message: Message) -> AiResult<Vec<ChatMessage>> {
    let base = |role, content| ChatMessage {
        role,
        content,
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
        metadata: serde_json::json!({"engine":"rig_0_46"}),
    };
    Ok(match message {
        Message::System { content } => vec![base(ChatMessageRole::System, Some(content))],
        Message::User { content } => {
            let mut messages = Vec::new();
            for item in content {
                match item {
                    UserContent::Text(value) => {
                        messages.push(base(ChatMessageRole::User, Some(value.text)))
                    }
                    UserContent::ToolResult(result) => {
                        let mut text = String::new();
                        for item in result.content {
                            match item {
                                rig::message::ToolResultContent::Text(value) => {
                                    text.push_str(&value.text)
                                }
                                rig::message::ToolResultContent::Json { value } => {
                                    text.push_str(&serde_json::to_string(&value)?)
                                }
                                _ => {
                                    return Err(AiError::Validation(
                                        "agent text context cannot contain tool media".to_string(),
                                    ));
                                }
                            }
                        }
                        let mut message = base(ChatMessageRole::Tool, Some(text));
                        message.name = Some(result.name.to_string());
                        message.tool_call_id = Some(result.call.to_string());
                        message.metadata["skipped"] = serde_json::json!(result.is_error);
                        message.metadata["untrusted_context"] =
                            serde_json::json!("mcp_tool_result");
                        messages.push(message);
                    }
                    _ => {
                        return Err(AiError::Validation(
                            "agent text context cannot contain user media".to_string(),
                        ));
                    }
                }
            }
            messages
        }
        Message::Assistant(assistant) => vec![
            map_response(
                assistant.content,
                assistant.origin.and_then(|origin| origin.response_id),
                serde_json::Value::Null,
                "rig",
            )?
            .assistant_message,
        ],
    })
}

pub(crate) fn assistant_choice(message: &ChatMessage) -> AiResult<Vec<AssistantContent>> {
    let Message::Assistant(assistant) = map_message(message.clone())? else {
        return Err(AiError::Validation(
            "Rig model turn must be an assistant message".to_string(),
        ));
    };
    Ok(assistant.content)
}

fn map_response(
    content: Vec<AssistantContent>,
    response_id: Option<String>,
    raw_payload: serde_json::Value,
    provider: &str,
) -> AiResult<ProviderChatResponse> {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for item in content {
        match item {
            AssistantContent::Text(value) => text.push_str(&value.text),
            AssistantContent::ToolCall(call) => tool_calls.push(ToolCall {
                id: call.id.to_string(),
                name: call.function.name.to_string(),
                arguments: rig_arguments(&call.function),
            }),
            AssistantContent::Reasoning(_)
            | AssistantContent::Image(_)
            | AssistantContent::Opaque(_) => {}
            _ => return Err(AiError::Provider("unsupported Rig assistant content".to_string())),
        }
    }
    Ok(ProviderChatResponse {
        assistant_message: ChatMessage {
            role: ChatMessageRole::Assistant,
            content: (!text.is_empty()).then_some(text),
            name: None,
            tool_call_id: None,
            tool_calls,
            metadata: serde_json::json!({
                "provider": provider,
                "response_id": response_id,
                "engine": "rig_0_46"
            }),
        },
        finish_reason: None,
        raw_payload,
    })
}
