use super::inference::{bedrock_runtime, gemini_grpc_runtime, llamacpp_base_url, openai_config};
use rig::{
    DynModel,
    operation::{Embedding, Rerank},
    providers::{cohere, copilot, gemini, ollama, openai, voyageai},
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

use crate::{AiError, AiProviderConfig, AiResult};

use super::catalog::ProviderIntegration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub documents: Vec<String>,
    pub dimensions: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub documents: Vec<String>,
    pub vectors: Vec<Vec<f64>>,
    pub input_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankRequest {
    pub model: String,
    pub query: String,
    pub documents: Vec<String>,
    pub top_n: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankItem {
    pub index: usize,
    pub document: Option<String>,
    pub relevance_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResponse {
    pub model: String,
    pub items: Vec<RerankItem>,
    pub input_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

pub async fn embed(
    config: &AiProviderConfig,
    secrets: &rustok_secrets::SecretResolverRegistry,
    request: EmbeddingRequest,
) -> AiResult<EmbeddingResponse> {
    if request.documents.is_empty() {
        return Err(AiError::Validation(
            "embedding request requires at least one document".to_string(),
        ));
    }
    let credential = resolve_primary_credential(config, secrets).await?;
    let api_key = credential
        .as_ref()
        .map(ExposeSecret::expose_secret)
        .unwrap_or("");
    let base_url = setting_str(config, "base_url").unwrap_or("");

    if request.dimensions == Some(0) {
        return Err(AiError::Validation(
            "embedding dimensions must be positive".to_string(),
        ));
    }
    macro_rules! keyed_embed {
        ($dialect:expr) => {{
            let client = openai_config($dialect, api_key, base_url).client();
            embed_with(
                client.embedding(request.model, request.dimensions).erase(),
                request.documents,
            )
            .await
        }};
    }
    let integration = ProviderIntegration::from_slug(&config.provider_slug).ok_or_else(|| {
        AiError::InvalidConfig(format!("unknown provider `{}`", config.provider_slug))
    })?;
    match integration {
        #[cfg(feature = "fastembed")]
        ProviderIntegration::Fastembed => {
            let kind = fastembed_model(&request.model)?;
            let client = rig_fastembed::Fastembed::load(&kind)
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            let model = client
                .embedding(&kind, request.dimensions)
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            embed_with(model.erase(), request.documents).await
        }
        ProviderIntegration::OpenAi | ProviderIntegration::OpenAiCompatible => {
            keyed_embed!(&openai::wire::OPENAI)
        }
        ProviderIntegration::AzureOpenAi => {
            let version = setting_str(config, "api_version").ok_or_else(|| {
                AiError::InvalidConfig("Azure API version is required".to_string())
            })?;
            let client = openai_config(&openai::wire::AZURE, api_key, base_url)
                .with_api_version(version.to_owned())
                .client();
            embed_with(
                client.embedding(request.model, request.dimensions).erase(),
                request.documents,
            )
            .await
        }
        ProviderIntegration::GithubCopilot => keyed_embed!(&copilot::DIALECT),
        ProviderIntegration::Cohere => {
            let mut target = cohere::CohereConfig::new(api_key.to_owned());
            if !base_url.is_empty() {
                target = target.with_base_url(base_url.to_owned());
            }
            let mut model = target.client().embedding(request.model, request.dimensions);
            model.wire = model
                .wire
                .with_input_type(setting_str(config, "input_type").unwrap_or("search_document"));
            embed_with(model.erase(), request.documents).await
        }
        ProviderIntegration::Gemini => {
            let mut target = gemini::GeminiConfig::new(api_key.to_owned());
            if !base_url.is_empty() {
                target = target.with_base_url(base_url.to_owned());
            }
            embed_with(
                target
                    .client()
                    .embedding(request.model, request.dimensions)
                    .erase(),
                request.documents,
            )
            .await
        }
        ProviderIntegration::Mistral => keyed_embed!(&openai::wire::MISTRAL),
        ProviderIntegration::Ollama => {
            let mut target = ollama::OllamaConfig::new();
            if !base_url.is_empty() {
                target = target.with_base_url(base_url.to_owned());
            }
            embed_with(
                target
                    .client()
                    .embedding(request.model, request.dimensions)
                    .erase(),
                request.documents,
            )
            .await
        }
        ProviderIntegration::OpenRouter => keyed_embed!(&openai::wire::OPENROUTER),
        ProviderIntegration::Together => keyed_embed!(&openai::wire::TOGETHER),
        ProviderIntegration::VoyageAi => {
            let mut target = voyageai::VoyageAiConfig::new(api_key.to_owned());
            if !base_url.is_empty() {
                target = target.with_base_url(base_url.to_owned());
            }
            let mut model = target.client().embedding(request.model, request.dimensions);
            if let Some(dimensions) = request.dimensions {
                model.wire = model.wire.with_output_dimension(dimensions);
            }
            embed_with(model.erase(), request.documents).await
        }
        ProviderIntegration::AwsBedrock => {
            let client = bedrock_runtime(config)?;
            embed_with(
                client.embedding(request.model, request.dimensions).erase(),
                request.documents,
            )
            .await
        }
        ProviderIntegration::GeminiGrpc => {
            let client = gemini_grpc_runtime(api_key, base_url).await?;
            embed_with(
                client.embedding(request.model, request.dimensions).erase(),
                request.documents,
            )
            .await
        }
        ProviderIntegration::Llamafile => {
            let base = if base_url.is_empty() {
                String::new()
            } else {
                llamacpp_base_url(base_url)
            };
            let client = openai_config(&openai::wire::LLAMACPP, api_key, &base).client();
            embed_with(
                client.embedding(request.model, request.dimensions).erase(),
                request.documents,
            )
            .await
        }
        _ => Err(AiError::InvalidConfig(format!(
            "Rig provider `{}` does not expose embeddings through this entrypoint",
            config.provider_slug
        ))),
    }
}

#[cfg(feature = "fastembed")]
fn fastembed_model(value: &str) -> AiResult<rig_fastembed::FastembedModel> {
    use rig_fastembed::FastembedModel;
    match value.trim().to_ascii_lowercase().as_str() {
        "all-minilm-l6-v2" | "sentence-transformers/all-minilm-l6-v2" => {
            Ok(FastembedModel::AllMiniLML6V2)
        }
        "all-minilm-l6-v2-q" => Ok(FastembedModel::AllMiniLML6V2Q),
        "all-minilm-l12-v2" => Ok(FastembedModel::AllMiniLML12V2),
        "bge-small-en-v1.5" | "baai/bge-small-en-v1.5" => Ok(FastembedModel::BGESmallENV15),
        "bge-base-en-v1.5" | "baai/bge-base-en-v1.5" => Ok(FastembedModel::BGEBaseENV15),
        "bge-large-en-v1.5" | "baai/bge-large-en-v1.5" => Ok(FastembedModel::BGELargeENV15),
        "nomic-embed-text-v1.5" | "nomic-ai/nomic-embed-text-v1.5" => {
            Ok(FastembedModel::NomicEmbedTextV15)
        }
        other => Err(AiError::InvalidConfig(format!(
            "unsupported FastEmbed model `{other}`"
        ))),
    }
}

pub async fn rerank(
    config: &AiProviderConfig,
    secrets: &rustok_secrets::SecretResolverRegistry,
    request: RerankRequest,
) -> AiResult<RerankResponse> {
    if request.documents.is_empty() {
        return Err(AiError::Validation(
            "rerank request requires at least one document".to_string(),
        ));
    }
    let credential = resolve_primary_credential(config, secrets).await?;
    let api_key = credential
        .as_ref()
        .map(ExposeSecret::expose_secret)
        .unwrap_or("");
    let base_url = setting_str(config, "base_url").unwrap_or("");
    let integration = ProviderIntegration::from_slug(&config.provider_slug).ok_or_else(|| {
        AiError::InvalidConfig(format!("unknown provider `{}`", config.provider_slug))
    })?;
    let response = match integration {
        ProviderIntegration::VoyageAi => {
            let mut target = voyageai::VoyageAiConfig::new(api_key.to_owned());
            if !base_url.is_empty() {
                target = target.with_base_url(base_url.to_owned());
            }
            rerank_with(
                target.client().rerank(request.model.clone()).erase(),
                request.model,
                request.query,
                request.documents,
            )
            .await?
        }
        _ => {
            return Err(AiError::InvalidConfig(format!(
                "Rig provider `{}` does not expose reranking",
                config.provider_slug
            )));
        }
    };
    let mut items = response.items;
    if let Some(top_n) = request.top_n {
        items.truncate(top_n);
    }
    Ok(RerankResponse { items, ..response })
}

async fn embed_with(
    model: DynModel<Embedding>,
    documents: Vec<String>,
) -> AiResult<EmbeddingResponse> {
    let response = model
        .call(documents)
        .await
        .map_err(|error| AiError::Provider(error.to_string()))?;
    Ok(EmbeddingResponse {
        documents: response
            .embeddings
            .iter()
            .map(|embedding| embedding.document.clone())
            .collect(),
        vectors: response
            .embeddings
            .into_iter()
            .map(|embedding| embedding.vec)
            .collect(),
        input_tokens: response.usage.input_tokens,
        total_tokens: response.usage.total_tokens,
    })
}

async fn rerank_with(
    model: DynModel<Rerank>,
    requested_model: String,
    query: String,
    documents: Vec<String>,
) -> AiResult<RerankResponse> {
    let response = model
        .call(rig::operation::RerankRequest { query, documents })
        .await
        .map_err(|error| AiError::Provider(error.to_string()))?;
    Ok(RerankResponse {
        model: response.model.unwrap_or(requested_model),
        items: response
            .results
            .into_iter()
            .map(|item| RerankItem {
                index: item.index,
                document: item.document,
                relevance_score: item.relevance_score,
            })
            .collect(),
        input_tokens: response.usage.input_tokens,
        total_tokens: response.usage.total_tokens,
    })
}

async fn resolve_primary_credential(
    config: &AiProviderConfig,
    secrets: &rustok_secrets::SecretResolverRegistry,
) -> AiResult<Option<secrecy::SecretString>> {
    if !matches!(config.target_auth, crate::ProviderTargetAuth::SecretRefs) {
        return Ok(None);
    }
    let descriptor = super::provider_catalog_entry(&config.provider_slug).ok_or_else(|| {
        AiError::InvalidConfig(format!("unknown provider `{}`", config.provider_slug))
    })?;
    let Some(field) = descriptor.credentials.first() else {
        return Ok(None);
    };
    let reference = config.credential_refs.get(field.key).ok_or_else(|| {
        AiError::InvalidConfig(format!("credential reference `{}` is required", field.key))
    })?;
    secrets
        .resolve_for_tenant(config.tenant_id, reference)
        .await
        .map(Some)
        .map_err(|error| AiError::InvalidConfig(error.to_string()))
}

fn setting_str<'a>(config: &'a AiProviderConfig, key: &str) -> Option<&'a str> {
    config.settings.get(key).and_then(serde_json::Value::as_str)
}
