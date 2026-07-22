use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use std::{collections::BTreeMap, net::IpAddr};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderSlug(String);

impl ProviderSlug {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
        if normalized.is_empty()
            || !normalized
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        {
            return Err("provider slug must be non-empty snake_case ASCII".to_string());
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn openai_compatible() -> Self {
        Self("openai_compatible".to_string())
    }

    pub fn anthropic() -> Self {
        Self("anthropic".to_string())
    }

    pub fn gemini() -> Self {
        Self("gemini".to_string())
    }
}

impl Default for ProviderSlug {
    fn default() -> Self {
        Self::openai_compatible()
    }
}

impl std::str::FromStr for ProviderSlug {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl std::fmt::Display for ProviderSlug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for ProviderSlug {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Closed set of Rig integrations compiled into this capability.
///
/// Provider slugs are persisted/public strings; this enum is the internal
/// dispatch key used by every Rig factory. Adding a slug therefore requires an
/// explicit compiler-checked factory branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderIntegration {
    OpenAi,
    OpenAiCompatible,
    Anthropic,
    AzureOpenAi,
    ChatGpt,
    GithubCopilot,
    Cohere,
    DeepSeek,
    Galadriel,
    Gemini,
    Groq,
    HuggingFace,
    Hyperbolic,
    Llamafile,
    MiniMax,
    Mira,
    Mistral,
    Moonshot,
    Ollama,
    OpenRouter,
    Perplexity,
    Together,
    VoyageAi,
    Xai,
    XiaomiMimo,
    Zai,
    AwsBedrock,
    VertexAi,
    GeminiGrpc,
    Fastembed,
}

impl ProviderIntegration {
    pub(crate) fn from_slug(slug: &ProviderSlug) -> Option<Self> {
        provider_catalog_entry(slug).map(|entry| entry.integration)
    }

    /// The capabilities for which this integration has a concrete Rig factory
    /// in `inference.rs` or `vectors.rs`. This deliberately does not consult
    /// the public descriptor: it is the compiler-exhaustive counterpart to
    /// the catalog declaration, so a descriptor cannot advertise a feature
    /// merely because it listed it itself.
    #[cfg(feature = "server")]
    const fn factory_supports(self, feature: ProviderFeature) -> bool {
        use ProviderFeature::{
            Chat, Embeddings, Image, Rerank, Streaming, StructuredOutput, Tools,
        };
        match self {
            Self::OpenAi | Self::OpenAiCompatible => matches!(
                feature,
                Chat | Streaming | Tools | StructuredOutput | Embeddings | Image
            ),
            Self::Anthropic
            | Self::ChatGpt
            | Self::DeepSeek
            | Self::Galadriel
            | Self::Groq
            | Self::Hyperbolic
            | Self::MiniMax
            | Self::Mira
            | Self::Moonshot
            | Self::Perplexity
            | Self::XiaomiMimo
            | Self::Zai
            | Self::VertexAi => matches!(feature, Chat | Streaming | Tools | StructuredOutput),
            Self::AzureOpenAi
            | Self::GithubCopilot
            | Self::Cohere
            | Self::Mistral
            | Self::Ollama
            | Self::OpenRouter
            | Self::Together
            | Self::GeminiGrpc
            | Self::Llamafile => matches!(
                feature,
                Chat | Streaming | Tools | StructuredOutput | Embeddings
            ),
            Self::Gemini | Self::AwsBedrock => matches!(
                feature,
                Chat | Streaming | Tools | StructuredOutput | Embeddings | Image
            ),
            Self::HuggingFace | Self::Xai => {
                matches!(feature, Chat | Streaming | Tools | StructuredOutput | Image)
            }
            Self::VoyageAi => matches!(feature, Embeddings | Rerank),
            Self::Fastembed => matches!(feature, Embeddings) && cfg!(feature = "fastembed"),
        }
    }
}

/// Stable deployment-owned connection identifier.
///
/// Tenant records may select this value, but never supply the endpoint or
/// cloud coordinates represented by the target.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderTargetId(String);

impl ProviderTargetId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into().trim().to_ascii_lowercase();
        if value.is_empty()
            || !value
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
        {
            return Err("provider target id must be non-empty lowercase ASCII".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderTargetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(feature = "server")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderTarget {
    pub id: ProviderTargetId,
    pub provider_slug: ProviderSlug,
    pub display_name: String,
    #[serde(default)]
    pub auth: ProviderTargetAuth,
    #[serde(default)]
    pub settings: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTargetAuth {
    #[default]
    SecretRefs,
    WorkloadIdentity,
    None,
}

#[cfg(feature = "server")]
#[derive(Debug, Clone, Default)]
pub struct AiProviderTargetCatalog {
    targets: BTreeMap<ProviderTargetId, AiProviderTarget>,
}

#[cfg(feature = "server")]
impl AiProviderTargetCatalog {
    pub fn new(targets: Vec<AiProviderTarget>) -> Result<Self, String> {
        Self::new_with_egress_policy(targets, &ProviderEgressPolicy::default())
    }

    /// Constructs the deployment-owned target catalog only after every target
    /// has passed the same descriptor schema and egress checks used during
    /// runtime materialization. Callers that expose local gateways must pass
    /// their explicit deployment policy here; a later tenant profile cannot
    /// make an invalid target usable.
    pub fn new_with_egress_policy(
        targets: Vec<AiProviderTarget>,
        egress_policy: &ProviderEgressPolicy,
    ) -> Result<Self, String> {
        let mut values = BTreeMap::new();
        for target in targets {
            validate_provider_target(&target, egress_policy)?;
            if values.insert(target.id.clone(), target).is_some() {
                return Err("provider target ids must be unique".to_string());
            }
        }
        Ok(Self { targets: values })
    }

    /// Reads deployment configuration only. Values in this JSON must never be
    /// accepted through tenant-facing transports.
    pub fn from_environment() -> Result<Self, String> {
        Self::from_environment_with_egress_policy(&ProviderEgressPolicy::default())
    }

    /// Reads and validates deployment configuration against its server-owned
    /// egress policy. This must run before targets are exposed to profiles/UI.
    pub fn from_environment_with_egress_policy(
        egress_policy: &ProviderEgressPolicy,
    ) -> Result<Self, String> {
        let Some(raw) = std::env::var_os("RUSTOK_AI_PROVIDER_TARGETS_JSON") else {
            return Ok(Self::default());
        };
        let targets = serde_json::from_str::<Vec<AiProviderTarget>>(&raw.to_string_lossy())
            .map_err(|error| format!("invalid RUSTOK_AI_PROVIDER_TARGETS_JSON: {error}"))?;
        Self::new_with_egress_policy(targets, egress_policy)
    }

    pub fn get(&self, id: &ProviderTargetId) -> Option<&AiProviderTarget> {
        self.targets.get(id)
    }

    pub fn entries(&self) -> impl Iterator<Item = &AiProviderTarget> {
        self.targets.values()
    }
}

#[cfg(feature = "server")]
fn validate_provider_target(
    target: &AiProviderTarget,
    egress_policy: &ProviderEgressPolicy,
) -> Result<(), String> {
    let descriptor = provider_catalog_entry(&target.provider_slug).ok_or_else(|| {
        format!(
            "provider target `{}` references unknown integration `{}`",
            target.id, target.provider_slug
        )
    })?;
    if !descriptor.compiled_in {
        return Err(format!(
            "provider target `{}` references integration `{}` which is not compiled into this deployment",
            target.id, target.provider_slug
        ));
    }
    if target.display_name.trim().is_empty() {
        return Err(format!(
            "provider target `{}` requires a display name",
            target.id
        ));
    }
    egress_policy.validate_settings(descriptor, &target.settings)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFeature {
    Chat,
    Streaming,
    Tools,
    StructuredOutput,
    Embeddings,
    Rerank,
    Image,
    Audio,
    Transcription,
    Multimodal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFieldKind {
    Text,
    Url,
    Integer,
    Boolean,
    SecretRef,
}

impl ProviderFieldKind {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Url => "url",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::SecretRef => "secret_ref",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfigField {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: ProviderFieldKind,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProviderDefaultSetting {
    pub key: &'static str,
    pub value: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProviderCatalogEntry {
    #[serde(skip)]
    pub(crate) integration: ProviderIntegration,
    pub slug: &'static str,
    pub display_name: &'static str,
    pub features: &'static [ProviderFeature],
    pub settings: &'static [ProviderConfigField],
    pub credentials: &'static [ProviderConfigField],
    pub default_settings: &'static [ProviderDefaultSetting],
    pub compiled_in: bool,
}

#[cfg(feature = "server")]
#[derive(Debug, Clone, Default)]
pub struct ProviderEgressPolicy {
    pub allowed_origins: Vec<String>,
    pub allow_local_origins: bool,
}

#[cfg(feature = "server")]
impl ProviderEgressPolicy {
    pub fn validate_settings(
        &self,
        descriptor: &ProviderCatalogEntry,
        settings: &BTreeMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        for field in descriptor.settings {
            if field.required && !settings.contains_key(field.key) {
                return Err(format!("provider setting `{}` is required", field.key));
            }
        }
        for (key, value) in settings {
            let field = descriptor
                .settings
                .iter()
                .find(|field| field.key == key)
                .ok_or_else(|| format!("unknown provider setting `{key}`"))?;
            match field.kind {
                ProviderFieldKind::Text | ProviderFieldKind::Url => {
                    if !value.is_string() {
                        return Err(format!("provider setting `{key}` must be text"));
                    }
                }
                ProviderFieldKind::Integer if value.as_i64().is_none() => {
                    return Err(format!("provider setting `{key}` must be an integer"));
                }
                ProviderFieldKind::Boolean if !value.is_boolean() => {
                    return Err(format!("provider setting `{key}` must be boolean"));
                }
                ProviderFieldKind::SecretRef => {
                    return Err(format!("provider setting `{key}` must be a credential ref"));
                }
                _ => {}
            }
            if matches!(field.kind, ProviderFieldKind::Url) {
                self.validate_egress_url(value.as_str().unwrap_or_default())?;
            }
        }
        Ok(())
    }

    pub fn validate_egress_url(&self, value: &str) -> Result<(), String> {
        let url =
            url::Url::parse(value).map_err(|error| format!("invalid provider URL: {error}"))?;
        if !matches!(url.scheme(), "https" | "http") {
            return Err("provider URL must use HTTP(S)".to_string());
        }
        let host = url
            .host_str()
            .ok_or_else(|| "provider URL requires a host".to_string())?
            .to_ascii_lowercase();
        let local = host == "localhost"
            || host.ends_with(".localhost")
            || host.parse::<IpAddr>().is_ok_and(is_local_or_private_ip);
        if local {
            if self.allow_local_origins {
                return Ok(());
            }
            return Err(
                "loopback and private provider origins are disabled by server policy".to_string(),
            );
        }
        if url.scheme() != "https" {
            return Err("non-local provider URLs must use HTTPS".to_string());
        }
        if self.allowed_origins.iter().any(|origin| origin == &host) {
            Ok(())
        } else {
            Err(format!(
                "provider origin `{host}` is not in the server egress allowlist"
            ))
        }
    }
}

#[cfg(feature = "server")]
fn is_local_or_private_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            address.is_loopback() || address.is_private() || address.is_unspecified()
        }
        IpAddr::V6(address) => {
            address.is_loopback()
                || address.is_unspecified()
                || (address.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

const API_KEY: &[ProviderConfigField] = &[ProviderConfigField {
    key: "api_key",
    label: "API key",
    kind: ProviderFieldKind::SecretRef,
    required: true,
}];
const TOKEN: &[ProviderConfigField] = &[ProviderConfigField {
    key: "access_token",
    label: "Access token",
    kind: ProviderFieldKind::SecretRef,
    required: true,
}];
const BASE_URL: &[ProviderConfigField] = &[ProviderConfigField {
    key: "base_url",
    label: "Base URL",
    kind: ProviderFieldKind::Url,
    required: false,
}];
const LOCAL_URL: &[ProviderConfigField] = &[ProviderConfigField {
    key: "base_url",
    label: "Local runtime URL",
    kind: ProviderFieldKind::Url,
    required: true,
}];
const OLLAMA_DEFAULTS: &[ProviderDefaultSetting] = &[ProviderDefaultSetting {
    key: "base_url",
    value: "http://localhost:11434",
}];
const AWS_SETTINGS: &[ProviderConfigField] = &[
    ProviderConfigField {
        key: "region",
        label: "Region",
        kind: ProviderFieldKind::Text,
        required: true,
    },
    ProviderConfigField {
        key: "profile",
        label: "AWS profile",
        kind: ProviderFieldKind::Text,
        required: false,
    },
];
const VERTEX_SETTINGS: &[ProviderConfigField] = &[
    ProviderConfigField {
        key: "project",
        label: "Google Cloud project",
        kind: ProviderFieldKind::Text,
        required: true,
    },
    ProviderConfigField {
        key: "location",
        label: "Google Cloud location",
        kind: ProviderFieldKind::Text,
        required: false,
    },
];
const AZURE_OPENAI_SETTINGS: &[ProviderConfigField] = &[
    ProviderConfigField {
        key: "base_url",
        label: "Azure OpenAI endpoint",
        kind: ProviderFieldKind::Url,
        required: true,
    },
    ProviderConfigField {
        key: "api_version",
        label: "API version",
        kind: ProviderFieldKind::Text,
        required: true,
    },
];

const CHAT: &[ProviderFeature] = &[
    ProviderFeature::Chat,
    ProviderFeature::Streaming,
    ProviderFeature::Tools,
    ProviderFeature::StructuredOutput,
];
const CHAT_EMBED: &[ProviderFeature] = &[
    ProviderFeature::Chat,
    ProviderFeature::Streaming,
    ProviderFeature::Tools,
    ProviderFeature::StructuredOutput,
    ProviderFeature::Embeddings,
];
const CHAT_IMAGE: &[ProviderFeature] = &[
    ProviderFeature::Chat,
    ProviderFeature::Streaming,
    ProviderFeature::Tools,
    ProviderFeature::StructuredOutput,
    ProviderFeature::Image,
];
const CHAT_EMBED_IMAGE: &[ProviderFeature] = &[
    ProviderFeature::Chat,
    ProviderFeature::Streaming,
    ProviderFeature::Tools,
    ProviderFeature::StructuredOutput,
    ProviderFeature::Embeddings,
    ProviderFeature::Image,
];
const EMBED_RERANK: &[ProviderFeature] = &[ProviderFeature::Embeddings, ProviderFeature::Rerank];
const EMBEDDINGS: &[ProviderFeature] = &[ProviderFeature::Embeddings];

macro_rules! entry {
    ($integration:path, $slug:literal, $name:literal, $features:expr, $settings:expr, $credentials:expr) => {
        ProviderCatalogEntry {
            integration: $integration,
            slug: $slug,
            display_name: $name,
            features: $features,
            settings: $settings,
            credentials: $credentials,
            default_settings: &[],
            compiled_in: true,
        }
    };
}

static CATALOG: &[ProviderCatalogEntry] = &[
    entry!(
        ProviderIntegration::OpenAi,
        "openai",
        "OpenAI",
        CHAT_EMBED_IMAGE,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::OpenAiCompatible,
        "openai_compatible",
        "OpenAI-compatible",
        CHAT_EMBED_IMAGE,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Anthropic,
        "anthropic",
        "Anthropic",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::AzureOpenAi,
        "azure_openai",
        "Azure OpenAI",
        CHAT_EMBED,
        AZURE_OPENAI_SETTINGS,
        API_KEY
    ),
    entry!(
        ProviderIntegration::ChatGpt,
        "chatgpt",
        "ChatGPT",
        CHAT,
        BASE_URL,
        TOKEN
    ),
    entry!(
        ProviderIntegration::GithubCopilot,
        "github_copilot",
        "GitHub Copilot",
        CHAT_EMBED,
        BASE_URL,
        TOKEN
    ),
    entry!(
        ProviderIntegration::Cohere,
        "cohere",
        "Cohere",
        CHAT_EMBED,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::DeepSeek,
        "deepseek",
        "DeepSeek",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Galadriel,
        "galadriel",
        "Galadriel",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Gemini,
        "gemini",
        "Google Gemini",
        CHAT_EMBED_IMAGE,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Groq,
        "groq",
        "Groq",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::HuggingFace,
        "hugging_face",
        "Hugging Face",
        CHAT_IMAGE,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Hyperbolic,
        "hyperbolic",
        "Hyperbolic",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Llamafile,
        "llamafile",
        "Llamafile",
        CHAT_EMBED,
        LOCAL_URL,
        &[]
    ),
    entry!(
        ProviderIntegration::MiniMax,
        "minimax",
        "MiniMax",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Mira,
        "mira",
        "Mira",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Mistral,
        "mistral",
        "Mistral",
        CHAT_EMBED,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Moonshot,
        "moonshot",
        "Moonshot",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    ProviderCatalogEntry {
        integration: ProviderIntegration::Ollama,
        slug: "ollama",
        display_name: "Ollama",
        features: CHAT_EMBED,
        settings: LOCAL_URL,
        credentials: &[],
        default_settings: OLLAMA_DEFAULTS,
        compiled_in: true,
    },
    entry!(
        ProviderIntegration::OpenRouter,
        "openrouter",
        "OpenRouter",
        CHAT_EMBED,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Perplexity,
        "perplexity",
        "Perplexity",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Together,
        "together",
        "Together AI",
        CHAT_EMBED,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::VoyageAi,
        "voyage_ai",
        "Voyage AI",
        EMBED_RERANK,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Xai,
        "xai",
        "xAI",
        CHAT_IMAGE,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::XiaomiMimo,
        "xiaomi_mimo",
        "Xiaomi MiMo",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::Zai,
        "zai",
        "Z.ai",
        CHAT,
        BASE_URL,
        API_KEY
    ),
    entry!(
        ProviderIntegration::AwsBedrock,
        "aws_bedrock",
        "AWS Bedrock",
        CHAT_EMBED_IMAGE,
        AWS_SETTINGS,
        &[]
    ),
    entry!(
        ProviderIntegration::VertexAi,
        "vertex_ai",
        "Google Vertex AI",
        CHAT,
        VERTEX_SETTINGS,
        &[]
    ),
    entry!(
        ProviderIntegration::GeminiGrpc,
        "gemini_grpc",
        "Google Gemini gRPC",
        CHAT_EMBED,
        &[],
        API_KEY
    ),
    ProviderCatalogEntry {
        integration: ProviderIntegration::Fastembed,
        slug: "fastembed",
        display_name: "FastEmbed",
        features: EMBEDDINGS,
        settings: &[],
        credentials: &[],
        default_settings: &[],
        compiled_in: cfg!(feature = "fastembed"),
    },
];

/// Descriptors that are actually available in this binary.
///
/// The backing registry also contains optional integrations so its pinned Rig
/// snapshot remains reviewable, but uncompiled integrations must never leak
/// into tenant-facing catalog responses or be selectable as deployment
/// targets.
pub fn provider_catalog() -> impl Iterator<Item = &'static ProviderCatalogEntry> {
    CATALOG.iter().filter(|entry| entry.compiled_in)
}

pub fn provider_catalog_entry(slug: &ProviderSlug) -> Option<&'static ProviderCatalogEntry> {
    provider_catalog().find(|entry| entry.slug == slug.as_str())
}

#[cfg(feature = "server")]
pub fn provider_factory_supports(slug: &ProviderSlug, feature: ProviderFeature) -> bool {
    provider_catalog_entry(slug).is_some_and(|entry| entry.integration.factory_supports(feature))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct ProviderCatalogSnapshot {
        rig_version: String,
        provider_slugs: Vec<String>,
    }

    #[test]
    fn catalog_slugs_are_unique_and_normalized() {
        let mut seen = HashSet::new();
        for entry in provider_catalog() {
            assert_eq!(ProviderSlug::new(entry.slug).unwrap().as_str(), entry.slug);
            assert!(seen.insert(entry.slug));
        }
    }

    #[test]
    fn provider_field_kinds_have_transport_stable_slugs() {
        assert_eq!(ProviderFieldKind::Text.slug(), "text");
        assert_eq!(ProviderFieldKind::Url.slug(), "url");
        assert_eq!(ProviderFieldKind::Integer.slug(), "integer");
        assert_eq!(ProviderFieldKind::Boolean.slug(), "boolean");
        assert_eq!(ProviderFieldKind::SecretRef.slug(), "secret_ref");
    }

    #[cfg(feature = "server")]
    #[test]
    fn provider_targets_reject_unknown_integrations_and_duplicate_ids() {
        let known = AiProviderTarget {
            id: ProviderTargetId::new("openai_primary").unwrap(),
            provider_slug: ProviderSlug::new("openai").unwrap(),
            display_name: "OpenAI primary".to_string(),
            auth: ProviderTargetAuth::SecretRefs,
            settings: BTreeMap::new(),
        };
        assert!(AiProviderTargetCatalog::new(vec![known.clone()]).is_ok());
        assert!(AiProviderTargetCatalog::new(vec![known.clone(), known]).is_err());
        let unknown = AiProviderTarget {
            id: ProviderTargetId::new("unknown").unwrap(),
            provider_slug: ProviderSlug::new("not_a_provider").unwrap(),
            display_name: "Unknown".to_string(),
            auth: ProviderTargetAuth::SecretRefs,
            settings: BTreeMap::new(),
        };
        assert!(AiProviderTargetCatalog::new(vec![unknown]).is_err());
    }

    #[cfg(feature = "server")]
    #[test]
    fn provider_targets_validate_schema_egress_and_build_availability_at_load_time() {
        let local = AiProviderTarget {
            id: ProviderTargetId::new("local_ollama").unwrap(),
            provider_slug: ProviderSlug::new("ollama").unwrap(),
            display_name: "Local Ollama".to_string(),
            auth: ProviderTargetAuth::None,
            settings: BTreeMap::from([(
                "base_url".to_string(),
                serde_json::json!("http://127.0.0.1:11434"),
            )]),
        };
        assert!(AiProviderTargetCatalog::new(vec![local.clone()]).is_err());
        assert!(
            AiProviderTargetCatalog::new_with_egress_policy(
                vec![local],
                &ProviderEgressPolicy {
                    allowed_origins: Vec::new(),
                    allow_local_origins: true,
                },
            )
            .is_ok()
        );

        let malformed_vertex = AiProviderTarget {
            id: ProviderTargetId::new("vertex").unwrap(),
            provider_slug: ProviderSlug::new("vertex_ai").unwrap(),
            display_name: "Vertex".to_string(),
            auth: ProviderTargetAuth::WorkloadIdentity,
            settings: BTreeMap::new(),
        };
        assert!(AiProviderTargetCatalog::new(vec![malformed_vertex]).is_err());

        let fastembed = AiProviderTarget {
            id: ProviderTargetId::new("fastembed").unwrap(),
            provider_slug: ProviderSlug::new("fastembed").unwrap(),
            display_name: "FastEmbed".to_string(),
            auth: ProviderTargetAuth::None,
            settings: BTreeMap::new(),
        };
        assert_eq!(
            AiProviderTargetCatalog::new(vec![fastembed]).is_ok(),
            cfg!(feature = "fastembed")
        );
    }

    #[test]
    fn catalog_matches_the_rig_0_39_registry_snapshot() {
        let snapshot: ProviderCatalogSnapshot = serde_json::from_str(include_str!(
            "../../contracts/rig-0.39-provider-catalog.json"
        ))
        .expect("catalog snapshot is valid JSON");
        assert_eq!(snapshot.rig_version, "0.39.0");
        let slugs = CATALOG
            .iter()
            .map(|entry| entry.slug.to_string())
            .collect::<Vec<_>>();
        assert_eq!(slugs, snapshot.provider_slugs);
    }

    #[test]
    fn every_remote_provider_declares_credentials_or_workload_settings() {
        for entry in provider_catalog() {
            if matches!(entry.slug, "ollama" | "llamafile" | "fastembed") {
                continue;
            }
            assert!(!entry.credentials.is_empty() || !entry.settings.is_empty());
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn compiled_catalog_and_rig_factories_have_exact_feature_parity() {
        const ALL_FEATURES: &[ProviderFeature] = &[
            ProviderFeature::Chat,
            ProviderFeature::Streaming,
            ProviderFeature::Tools,
            ProviderFeature::StructuredOutput,
            ProviderFeature::Embeddings,
            ProviderFeature::Rerank,
            ProviderFeature::Image,
            ProviderFeature::Audio,
            ProviderFeature::Transcription,
            ProviderFeature::Multimodal,
        ];

        for entry in provider_catalog() {
            let slug = ProviderSlug::new(entry.slug).unwrap();
            assert!(
                ProviderIntegration::from_slug(&slug) == Some(entry.integration),
                "{} is catalogued without a typed integration dispatch key",
                entry.slug
            );
            for feature in ALL_FEATURES {
                assert_eq!(
                    entry.features.contains(feature),
                    provider_factory_supports(&slug, *feature),
                    "{}/{:?} catalog descriptor and concrete Rig factory disagree",
                    entry.slug,
                    feature
                );
            }
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn uncompiled_integrations_cannot_be_selected_as_deployment_targets() {
        if cfg!(feature = "fastembed") {
            return;
        }
        let target = AiProviderTarget {
            id: ProviderTargetId::new("fastembed_local").unwrap(),
            provider_slug: ProviderSlug::new("fastembed").unwrap(),
            display_name: "FastEmbed local".to_string(),
            auth: ProviderTargetAuth::None,
            settings: BTreeMap::new(),
        };
        assert!(AiProviderTargetCatalog::new(vec![target]).is_err());
        assert!(provider_catalog().all(|entry| entry.slug != "fastembed"));
    }

    #[cfg(feature = "server")]
    #[test]
    fn egress_policy_rejects_private_origins_without_explicit_local_setting() {
        let policy = ProviderEgressPolicy::default();
        assert!(
            policy
                .validate_egress_url("http://127.0.0.1:11434")
                .is_err()
        );
        assert!(
            policy
                .validate_egress_url("https://api.openai.com/v1")
                .is_err()
        );
        let policy = ProviderEgressPolicy {
            allowed_origins: vec!["api.openai.com".to_string()],
            allow_local_origins: true,
        };
        assert!(
            policy
                .validate_egress_url("https://api.openai.com/v1")
                .is_ok()
        );
        assert!(policy.validate_egress_url("http://127.0.0.1:11434").is_ok());
    }
}
