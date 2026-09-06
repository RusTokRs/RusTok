use rig::{
    client::{EmbeddingsClient, RerankingClient},
    embeddings::EmbeddingModel,
    providers::{
        azure, cohere, copilot, gemini, llamafile, mistral, ollama, openai, openrouter, together,
        voyageai,
    },
    rerank::RerankModel,
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
    pub input_tokens: u64,
    pub total_tokens: u64,
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
    pub input_tokens: u64,
    pub total_tokens: u64,
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

    macro_rules! keyed_embed {
        ($provider:ident) => {{
            let mut builder = $provider::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            let model = match request.dimensions {
                Some(dimensions) => {
                    client.embedding_model_with_ndims(request.model.clone(), dimensions)
                }
                None => client.embedding_model(request.model.clone()),
            };
            embed_with(model, request.documents).await
        }};
    }

    let integration = ProviderIntegration::from_slug(&config.provider_slug).ok_or_else(|| {
        AiError::InvalidConfig(format!("unknown provider `{}`", config.provider_slug))
    })?;
    match integration {
        #[cfg(feature = "fastembed")]
        ProviderIntegration::Fastembed => {
            let model = fastembed_model(&request.model)?;
            let model = rig_fastembed::Client::new()
                .embedding_model(&model)
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            embed_with(model, request.documents).await
        }
        ProviderIntegration::OpenAi | ProviderIntegration::OpenAiCompatible => keyed_embed!(openai),
        ProviderIntegration::AzureOpenAi => {
            let api_version = setting_str(config, "api_version").ok_or_else(|| {
                AiError::InvalidConfig("Azure API version is required".to_string())
            })?;
            let client = azure::Client::builder()
                .api_key(api_key)
                .azure_endpoint(base_url.to_string())
                .api_version(api_version)
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            let model = match request.dimensions {
                Some(dimensions) => {
                    client.embedding_model_with_ndims(request.model.clone(), dimensions)
                }
                None => client.embedding_model(request.model.clone()),
            };
            embed_with(model, request.documents).await
        }
        ProviderIntegration::GithubCopilot => keyed_embed!(copilot),
        ProviderIntegration::Cohere => {
            let mut builder = cohere::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            let input_type = setting_str(config, "input_type").unwrap_or("search_document");
            let model = match request.dimensions {
                Some(dimensions) => {
                    client.embedding_model_with_ndims(request.model.clone(), input_type, dimensions)
                }
                None => client.embedding_model(request.model.clone(), input_type),
            };
            embed_with(model, request.documents).await
        }
        ProviderIntegration::Gemini => keyed_embed!(gemini),
        ProviderIntegration::Mistral => keyed_embed!(mistral),
        ProviderIntegration::Ollama => keyed_embed!(ollama),
        ProviderIntegration::OpenRouter => keyed_embed!(openrouter),
        ProviderIntegration::Together => keyed_embed!(together),
        ProviderIntegration::VoyageAi => keyed_embed!(voyageai),
        ProviderIntegration::AwsBedrock => {
            let region =
                setting_str(config, "region").unwrap_or(rig_bedrock::client::DEFAULT_AWS_REGION);
            let client = if let Some(profile) =
                setting_str(config, "profile").filter(|value| !value.trim().is_empty())
            {
                rig_bedrock::client::Client::with_profile_name(profile)
            } else {
                rig_bedrock::client::ClientBuilder::default()
                    .region(region)
                    .build()
                    .await
            };
            let model = match request.dimensions {
                Some(dimensions) => {
                    client.embedding_model_with_ndims(request.model.clone(), dimensions)
                }
                None => client.embedding_model(request.model.clone()),
            };
            embed_with(model, request.documents).await
        }
        ProviderIntegration::GeminiGrpc => {
            let client = rig_gemini_grpc::Client::new(api_key.to_string())
                .await
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            let model = match request.dimensions {
                Some(dimensions) => {
                    client.embedding_model_with_ndims(request.model.clone(), dimensions)
                }
                None => client.embedding_model(request.model.clone()),
            };
            embed_with(model, request.documents).await
        }
        ProviderIntegration::Llamafile => {
            let mut builder = llamafile::Client::builder().api_key(rig::client::Nothing);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            let model = match request.dimensions {
                Some(dimensions) => {
                    client.embedding_model_with_ndims(request.model.clone(), dimensions)
                }
                None => client.embedding_model(request.model.clone()),
            };
            embed_with(model, request.documents).await
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
            let mut builder = voyageai::Client::builder().api_key(api_key);
            if !base_url.is_empty() {
                builder = builder.base_url(base_url);
            }
            let client = builder
                .build()
                .map_err(|error| AiError::InvalidConfig(error.to_string()))?;
            rerank_with(
                client.rerank_model(request.model),
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

async fn embed_with<M: EmbeddingModel>(
    model: M,
    documents: Vec<String>,
) -> AiResult<EmbeddingResponse> {
    let response = model
        .embed_texts_with_usage(documents)
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

async fn rerank_with<M: RerankModel>(
    model: M,
    query: String,
    documents: Vec<String>,
) -> AiResult<RerankResponse> {
    let response = model
        .rerank(&query, documents)
        .await
        .map_err(|error| AiError::Provider(error.to_string()))?;
    Ok(RerankResponse {
        model: response.model,
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
