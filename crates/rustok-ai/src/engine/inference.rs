use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use futures_util::StreamExt;
use rig::prelude::ImageGenerationClient;
use rig::{
    OneOrMany,
    client::CompletionClient,
    completion::{
        CompletionModel, CompletionRequest, Message, ToolDefinition as RigToolDefinition,
    },
    image_generation::ImageGenerationModel,
    message::{AssistantContent, ToolCall as RigToolCall, ToolFunction, UserContent},
    providers::{
        anthropic, azure, chatgpt, cohere, copilot, deepseek, gemini, groq, huggingface,
        hyperbolic, llamafile, minimax, mira, mistral, moonshot, ollama, openai, openrouter,
        perplexity, together, xai, xiaomimimo, zai,
    },
    streaming::{StreamedAssistantContent, ToolCallDeltaContent},
};
use secrecy::ExposeSecret;

use crate::{
    AiError, AiResult,
    model::{
        AiProviderConfig, ChatMessage, ChatMessageRole, ProviderChatRequest, ProviderChatResponse,
        ProviderImageRequest, ProviderImageResponse, ProviderStreamEmitter,
        ProviderStructuredRequest, ProviderTestResult, ProviderUsage, ToolCall,
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
    ) -> AiResult<serde_json::Value>;
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
    image: Option<Arc<dyn ImageInference>>,
}

#[async_trait]
trait ImageInference: Send + Sync {
    async fn generate(&self, request: ProviderImageRequest) -> AiResult<ProviderImageResponse>;
}

#[derive(Clone)]
struct RigImageInference<I> {
    model: I,
    provider: String,
}

#[async_trait]
impl<I> ImageInference for RigImageInference<I>
where
    I: ImageGenerationModel + 'static,
{
    async fn generate(&self, request: ProviderImageRequest) -> AiResult<ProviderImageResponse> {
        let (width, height) = parse_image_size(request.size.as_deref())?;
        let mut builder = self
            .model
            .image_generation_request()
            .prompt(&request.prompt)
            .width(width)
            .height(height);
        if let Some(negative_prompt) = request.negative_prompt {
            builder = builder.additional_params(serde_json::json!({
                "negative_prompt": negative_prompt
            }));
        }
        let response = builder
            .send()
            .await
            .map_err(|error| AiError::Provider(error.to_string()))?;
        Ok(ProviderImageResponse {
            bytes: response.image,
            mime_type: "image/png".to_string(),
            revised_prompt: None,
            raw_payload: serde_json::json!({"provider": self.provider}),
        })
    }
}

fn rig_engine<M>(model: M, provider: &ProviderSlug) -> Box<dyn InferenceEngine>
where
    M: CompletionModel + 'static,
{
    Box::new(RigInferenceEngine {
        model,
        provider: provider.to_string(),
        image: None,
    })
}

fn rig_engine_with_image<M, I>(
    model: M,
    image_model: I,
    provider: &ProviderSlug,
) -> Box<dyn InferenceEngine>
where
    M: CompletionModel + 'static,
    I: ImageGenerationModel + 'static,
{
    Box::new(RigInferenceEngine {
        model,
        provider: provider.to_string(),
        image: Some(Arc::new(RigImageInference {
            model: image_model,
            provider: provider.to_string(),
        })),
    })
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
    let engine = match integration {
        ProviderIntegration::OpenAi | ProviderIntegration::OpenAiCompatible => {
            let mut builder = openai::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine_with_image(
                client.completion_model(config.model.clone()),
                client.image_generation_model(image_model),
                slug,
            )
        }
        ProviderIntegration::Anthropic => {
            let mut builder = anthropic::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine(client.completion_model(config.model.clone()), slug)
        }
        ProviderIntegration::Gemini => {
            let mut builder = gemini::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine_with_image(
                client.completion_model(config.model.clone()),
                client.image_generation_model(image_model),
                slug,
            )
        }
        ProviderIntegration::AzureOpenAi => {
            let endpoint = base_url;
            let api_version = config
                .settings
                .get("api_version")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    AiError::InvalidConfig("Azure API version is required".to_string())
                })?;
            let client = azure::Client::builder()
                .api_key(api_key)
                .azure_endpoint(endpoint.to_string())
                .api_version(api_version)
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine(client.completion_model(config.model.clone()), slug)
        }
        ProviderIntegration::ChatGpt => keyed_engine!(chatgpt),
        ProviderIntegration::GithubCopilot => keyed_engine!(copilot),
        ProviderIntegration::Cohere => keyed_engine!(cohere),
        ProviderIntegration::DeepSeek => keyed_engine!(deepseek),
        ProviderIntegration::Galadriel => {
            return Err(AiError::Provider(
                "Galadriel is not supported by the configured Rig provider runtime".to_string(),
            ));
        }
        ProviderIntegration::Groq => keyed_engine!(groq),
        ProviderIntegration::HuggingFace => {
            let mut builder = huggingface::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine_with_image(
                client.completion_model(config.model.clone()),
                client.image_generation_model(image_model),
                slug,
            )
        }
        ProviderIntegration::Hyperbolic => keyed_engine!(hyperbolic),
        ProviderIntegration::Llamafile => {
            let mut builder = llamafile::Client::builder().api_key(rig::client::Nothing);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine(client.completion_model(config.model.clone()), slug)
        }
        ProviderIntegration::MiniMax => keyed_engine!(minimax),
        ProviderIntegration::Mira => keyed_engine!(mira),
        ProviderIntegration::Mistral => keyed_engine!(mistral),
        ProviderIntegration::Moonshot => keyed_engine!(moonshot),
        ProviderIntegration::Ollama => keyed_engine!(ollama),
        ProviderIntegration::OpenRouter => keyed_engine!(openrouter),
        ProviderIntegration::Perplexity => keyed_engine!(perplexity),
        ProviderIntegration::Together => keyed_engine!(together),
        ProviderIntegration::Xai => {
            let mut builder = xai::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine_with_image(
                client.completion_model(config.model.clone()),
                client.image_generation_model(image_model),
                slug,
            )
        }
        ProviderIntegration::XiaomiMimo => keyed_engine!(xiaomimimo),
        ProviderIntegration::Zai => keyed_engine!(zai),
        ProviderIntegration::AwsBedrock => {
            let region = config
                .settings
                .get("region")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(rig_bedrock::client::DEFAULT_AWS_REGION);
            let client = if let Some(profile) = config
                .settings
                .get("profile")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                rig_bedrock::client::Client::with_profile_name(profile)
            } else {
                rig_bedrock::client::ClientBuilder::default()
                    .region(region)
                    .build()
                    .await
            };
            rig_engine_with_image(
                client.completion_model(config.model.clone()),
                client.image_generation_model(image_model),
                slug,
            )
        }
        ProviderIntegration::VertexAi => {
            let project = config
                .settings
                .get("project")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| AiError::InvalidConfig("Vertex project is required".to_string()))?;
            let mut builder = rig_vertexai::Client::builder().with_project(project);
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
            rig_engine(client.completion_model(config.model.clone()), slug)
        }
        ProviderIntegration::GeminiGrpc => {
            let client = rig_gemini_grpc::Client::new(api_key.to_string())
                .await
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rig_engine(client.completion_model(config.model.clone()), slug)
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

    async fn complete_structured(
        &self,
        request: ProviderStructuredRequest,
    ) -> AiResult<serde_json::Value> {
        let mut completion = map_request(request.request)?;
        completion.output_schema = Some(
            schemars::Schema::try_from(request.output_schema)
                .map_err(|error| AiError::Validation(error.to_string()))?,
        );
        let response = complete_with(&self.model, completion, &self.provider).await?;
        let content = response.assistant_message.content.ok_or_else(|| {
            AiError::Provider("Rig structured output returned empty content".to_string())
        })?;
        serde_json::from_str(content.trim()).map_err(AiError::Json)
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
        image.generate(request).await
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
    let mut assembled_tool_calls = BTreeMap::<String, (String, String, String)>::new();
    let mut emitted_tool_call_ids = HashSet::new();
    while let Some(item) = stream.next().await {
        match item.map_err(|error| AiError::Provider(error.to_string()))? {
            StreamedAssistantContent::Text(text) => {
                if let Some(emitter) = &emitter {
                    emitter.emit_text_delta(text.text);
                }
            }
            StreamedAssistantContent::ToolCall { tool_call, .. } => {
                emitted_tool_call_ids.insert(tool_call.id.clone());
                if let Some(emitter) = &emitter {
                    emitter.emit_tool_call(ToolCall {
                        id: tool_call.id,
                        name: tool_call.function.name,
                        arguments: tool_call.function.arguments,
                    });
                }
            }
            StreamedAssistantContent::ToolCallDelta {
                id,
                internal_call_id,
                content,
            } => {
                let entry = assembled_tool_calls
                    .entry(internal_call_id)
                    .or_insert_with(|| (id, String::new(), String::new()));
                match content {
                    ToolCallDeltaContent::Name(name) => entry.1 = name,
                    ToolCallDeltaContent::Delta(delta) => entry.2.push_str(&delta),
                }
            }
            StreamedAssistantContent::Reasoning(_)
            | StreamedAssistantContent::ReasoningDelta { .. }
            | StreamedAssistantContent::Final(_) => {}
            StreamedAssistantContent::Unknown(_) => {
                tracing::debug!(provider, "received an unmodeled provider streaming item");
            }
        }
    }
    if let Some(emitter) = &emitter {
        for (_internal_id, (id, name, arguments)) in assembled_tool_calls {
            if name.is_empty() || emitted_tool_call_ids.contains(&id) {
                continue;
            }
            let arguments = serde_json::from_str(&arguments)
                .unwrap_or_else(|_| serde_json::Value::String(arguments));
            emitter.emit_tool_call(ToolCall {
                id,
                name,
                arguments,
            });
        }
    }
    let raw_payload = stream
        .response
        .as_ref()
        .and_then(|value| serde_json::to_value(value).ok())
        .unwrap_or_else(|| serde_json::json!({"provider": provider, "streaming": true}));
    if let Some(usage) = extract_usage(&raw_payload) {
        if let Some(emitter) = &emitter {
            emitter.emit_usage(usage);
        }
    }
    Ok(map_response(
        stream.choice.into_iter().collect(),
        stream.message_id,
        raw_payload,
        provider,
    ))
}

fn extract_usage(payload: &serde_json::Value) -> Option<ProviderUsage> {
    let usage = payload.get("usage")?;
    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(serde_json::Value::as_u64);
    (input_tokens > 0 || output_tokens > 0 || total_tokens.unwrap_or(0) > 0)
        .then(|| ProviderUsage::normalized(input_tokens, output_tokens, total_tokens))
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
