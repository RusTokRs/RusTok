use futures_util::StreamExt;
use rig::{
    DynModel, Model,
    driver::{Exchange, Opening, Transport},
    error::ProviderError,
    http_client::DynHttpClient,
    operation::{Embedding, Rerank},
    providers::{cohere, copilot, gemini, ollama, openai, voyageai},
    wire::{Encoded, WireFrame},
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

use crate::{AiError, AiProviderConfig, AiResult};

use super::{
    catalog::ProviderIntegration,
    inference::{
        OPENAI_COMPATIBLE_DIALECT, bedrock_runtime, gemini_grpc_runtime, llamacpp_base_url,
        openai_config,
    },
    resolve_primary_credential,
};

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
    /// `None` means the provider did not report this counter.
    pub input_tokens: Option<u64>,
    /// `None` means the provider did not report this counter.
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
    /// `None` means the provider did not report this counter.
    pub input_tokens: Option<u64>,
    /// `None` means the provider did not report this counter.
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
    if request.dimensions == Some(0) {
        return Err(AiError::Validation(
            "embedding dimensions must be positive".to_string(),
        ));
    }
    let credential = resolve_primary_credential(config, secrets).await?;
    let api_key = credential
        .as_ref()
        .map(ExposeSecret::expose_secret)
        .unwrap_or("");
    let base_url = setting_str(config, "base_url").unwrap_or("").trim();
    macro_rules! keyed_embed {
        ($dialect:expr) => {{
            let client = openai_config($dialect, api_key, base_url).client();
            embed_with(
                ordered_openai_embeddings(client.embedding(request.model, request.dimensions)),
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
        ProviderIntegration::OpenAi => keyed_embed!(&openai::wire::OPENAI),
        ProviderIntegration::OpenAiCompatible => keyed_embed!(&OPENAI_COMPATIBLE_DIALECT),
        ProviderIntegration::AzureOpenAi => {
            let version = setting_str(config, "api_version").ok_or_else(|| {
                AiError::InvalidConfig("Azure API version is required".to_string())
            })?;
            let client = openai_config(&openai::wire::AZURE, api_key, base_url)
                .with_api_version(version.to_owned())
                .client();
            embed_with(
                ordered_openai_embeddings(client.embedding(request.model, request.dimensions)),
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
            let client = gemini_grpc_runtime(api_key).await?;
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
                ordered_openai_embeddings(client.embedding(request.model, request.dimensions)),
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

#[derive(Clone)]
struct OrderedOpenAiEmbeddingTransport(DynHttpClient);

impl Transport<openai::wire::Embeddings> for OrderedOpenAiEmbeddingTransport {
    fn send(&self, payload: Encoded, exchange: Exchange) -> Opening<WireFrame> {
        let opening = Transport::<openai::wire::Embeddings>::send(&self.0, payload, exchange);
        Opening::new(async move {
            Ok(opening
                .await?
                .map_frames(|frames| frames.map(|frame| frame.and_then(order_embedding_frame))))
        })
    }
}

fn ordered_openai_embeddings(model: Model<openai::wire::Embeddings>) -> DynModel<Embedding> {
    Model::new(model.wire, OrderedOpenAiEmbeddingTransport(model.transport)).erase()
}

// Rig 0.46 pairs vectors with inputs by frame order. Normalize explicit
// response indices before that fold, while leaving protocol decoding to Rig.
fn order_embedding_frame(frame: WireFrame) -> Result<WireFrame, ProviderError> {
    let Ok(mut payload) = serde_json::from_str::<serde_json::Value>(&frame.as_str()) else {
        return Ok(frame);
    };
    let Some(data) = payload
        .get_mut("data")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(frame);
    };
    if data.iter().all(|row| row.get("index").is_none()) {
        return Ok(frame);
    }
    let index = |row: &serde_json::Value| {
        row.get("index")
            .and_then(serde_json::Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
    };
    if data
        .iter()
        .enumerate()
        .all(|(position, row)| index(row) == Some(position))
    {
        return Ok(frame);
    }
    let invalid_indices = || {
        ProviderError::Response(
            "embedding reply indices do not identify each input exactly once".to_string(),
        )
    };
    let mut ordered = vec![serde_json::Value::Null; data.len()];
    for row in data.drain(..) {
        let slot = index(&row)
            .and_then(|index| ordered.get_mut(index))
            .filter(|slot| slot.is_null())
            .ok_or_else(invalid_indices)?;
        *slot = row;
    }
    *data = ordered;
    Ok(WireFrame::Text(payload.to_string().into()))
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
    let base_url = setting_str(config, "base_url").unwrap_or("").trim();
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

fn setting_str<'a>(config: &'a AiProviderConfig, key: &str) -> Option<&'a str> {
    config.settings.get(key).and_then(serde_json::Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig::test_utils::RecordingHttpClient;
    use serde_json::json;

    #[tokio::test]
    async fn embedding_keeps_document_alignment_and_unreported_usage() {
        let http = RecordingHttpClient::new(
            json!({
                "object": "list",
                "model": "text-embedding-3-small",
                "data": [
                    {"object": "embedding", "index": 1, "embedding": [0.0, 1.0]},
                    {"object": "embedding", "index": 0, "embedding": [1.0, 0.0]}
                ]
            })
            .to_string(),
        );
        let model = openai_config(
            &OPENAI_COMPATIBLE_DIALECT,
            "test-key",
            "https://inference.example/v1",
        )
        .connect(http.clone())
        .embedding("text-embedding-3-small", Some(2));
        let response = embed_with(
            ordered_openai_embeddings(model),
            vec!["first".to_string(), "second".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(response.documents, ["first", "second"]);
        assert_eq!(response.vectors, [vec![1.0, 0.0], vec![0.0, 1.0]]);
        assert_eq!(response.input_tokens, None);
        assert_eq!(response.total_tokens, None);
        let requests = http.requests();
        let request: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(request["dimensions"], 2);
    }

    #[test]
    fn partially_indexed_and_duplicate_embedding_replies_fail_closed() {
        for data in [
            json!([{"index":0},{}]),
            json!([{"index":0},{"index":0}]),
            json!([{"index":2},{"index":0}]),
        ] {
            let frame = WireFrame::Text(json!({"data": data}).to_string().into());
            assert!(order_embedding_frame(frame).is_err());
        }
    }
}
