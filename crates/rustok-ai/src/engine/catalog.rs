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
    ($slug:literal, $name:literal, $features:expr, $settings:expr, $credentials:expr) => {
        ProviderCatalogEntry {
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
    entry!("openai", "OpenAI", CHAT_EMBED_IMAGE, BASE_URL, API_KEY),
    entry!(
        "openai_compatible",
        "OpenAI-compatible",
        CHAT_EMBED_IMAGE,
        BASE_URL,
        API_KEY
    ),
    entry!("anthropic", "Anthropic", CHAT, BASE_URL, API_KEY),
    entry!(
        "azure_openai",
        "Azure OpenAI",
        CHAT_EMBED,
        AZURE_OPENAI_SETTINGS,
        API_KEY
    ),
    entry!("chatgpt", "ChatGPT", CHAT, BASE_URL, TOKEN),
    entry!(
        "github_copilot",
        "GitHub Copilot",
        CHAT_EMBED,
        BASE_URL,
        TOKEN
    ),
    entry!("cohere", "Cohere", CHAT_EMBED, BASE_URL, API_KEY),
    entry!("deepseek", "DeepSeek", CHAT, BASE_URL, API_KEY),
    entry!("galadriel", "Galadriel", CHAT, BASE_URL, API_KEY),
    entry!(
        "gemini",
        "Google Gemini",
        CHAT_EMBED_IMAGE,
        BASE_URL,
        API_KEY
    ),
    entry!("groq", "Groq", CHAT, BASE_URL, API_KEY),
    entry!(
        "hugging_face",
        "Hugging Face",
        CHAT_IMAGE,
        BASE_URL,
        API_KEY
    ),
    entry!("hyperbolic", "Hyperbolic", CHAT, BASE_URL, API_KEY),
    entry!("llamafile", "Llamafile", CHAT_EMBED, LOCAL_URL, &[]),
    entry!("minimax", "MiniMax", CHAT, BASE_URL, API_KEY),
    entry!("mira", "Mira", CHAT, BASE_URL, API_KEY),
    entry!("mistral", "Mistral", CHAT_EMBED, BASE_URL, API_KEY),
    entry!("moonshot", "Moonshot", CHAT, BASE_URL, API_KEY),
    ProviderCatalogEntry {
        slug: "ollama",
        display_name: "Ollama",
        features: CHAT_EMBED,
        settings: LOCAL_URL,
        credentials: &[],
        default_settings: OLLAMA_DEFAULTS,
        compiled_in: true,
    },
    entry!("openrouter", "OpenRouter", CHAT_EMBED, BASE_URL, API_KEY),
    entry!("perplexity", "Perplexity", CHAT, BASE_URL, API_KEY),
    entry!("together", "Together AI", CHAT_EMBED, BASE_URL, API_KEY),
    entry!("voyage_ai", "Voyage AI", EMBED_RERANK, BASE_URL, API_KEY),
    entry!("xai", "xAI", CHAT_IMAGE, BASE_URL, API_KEY),
    entry!("xiaomi_mimo", "Xiaomi MiMo", CHAT, BASE_URL, API_KEY),
    entry!("zai", "Z.ai", CHAT, BASE_URL, API_KEY),
    entry!(
        "aws_bedrock",
        "AWS Bedrock",
        CHAT_EMBED_IMAGE,
        AWS_SETTINGS,
        &[]
    ),
    entry!("vertex_ai", "Google Vertex AI", CHAT, VERTEX_SETTINGS, &[]),
    entry!(
        "gemini_grpc",
        "Google Gemini gRPC",
        CHAT_EMBED,
        &[],
        API_KEY
    ),
    ProviderCatalogEntry {
        slug: "fastembed",
        display_name: "FastEmbed",
        features: EMBEDDINGS,
        settings: &[],
        credentials: &[],
        default_settings: &[],
        compiled_in: cfg!(feature = "fastembed"),
    },
];

pub fn provider_catalog() -> &'static [ProviderCatalogEntry] {
    CATALOG
}

pub fn provider_catalog_entry(slug: &ProviderSlug) -> Option<&'static ProviderCatalogEntry> {
    CATALOG.iter().find(|entry| entry.slug == slug.as_str())
}

#[cfg(feature = "server")]
pub fn provider_factory_supports(slug: &ProviderSlug, feature: ProviderFeature) -> bool {
    match feature {
        ProviderFeature::Chat
        | ProviderFeature::Streaming
        | ProviderFeature::Tools
        | ProviderFeature::StructuredOutput => !matches!(slug.as_str(), "voyage_ai" | "fastembed"),
        ProviderFeature::Embeddings => matches!(
            slug.as_str(),
            "openai"
                | "openai_compatible"
                | "azure_openai"
                | "github_copilot"
                | "cohere"
                | "gemini"
                | "llamafile"
                | "mistral"
                | "ollama"
                | "openrouter"
                | "together"
                | "voyage_ai"
                | "aws_bedrock"
                | "gemini_grpc"
                | "fastembed"
        ),
        ProviderFeature::Rerank => slug.as_str() == "voyage_ai",
        ProviderFeature::Image => matches!(
            slug.as_str(),
            "openai" | "openai_compatible" | "gemini" | "hugging_face" | "xai" | "aws_bedrock"
        ),
        ProviderFeature::Audio | ProviderFeature::Transcription | ProviderFeature::Multimodal => {
            false
        }
    }
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
    fn catalog_matches_the_rig_0_39_registry_snapshot() {
        let snapshot: ProviderCatalogSnapshot = serde_json::from_str(include_str!(
            "../../contracts/rig-0.39-provider-catalog.json"
        ))
        .expect("catalog snapshot is valid JSON");
        assert_eq!(snapshot.rig_version, "0.39.0");
        let slugs = provider_catalog()
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
    fn every_compiled_descriptor_has_a_linked_factory_for_each_declared_feature() {
        for entry in provider_catalog().iter().filter(|entry| entry.compiled_in) {
            let slug = ProviderSlug::new(entry.slug).unwrap();
            for feature in entry.features {
                assert!(
                    provider_factory_supports(&slug, *feature),
                    "{}/{} is catalogued without a linked Rig factory",
                    entry.slug,
                    format!("{feature:?}")
                );
            }
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn egress_policy_rejects_private_origins_without_explicit_local_setting() {
        let policy = ProviderEgressPolicy::default();
        assert!(policy
            .validate_egress_url("http://127.0.0.1:11434")
            .is_err());
        assert!(policy
            .validate_egress_url("https://api.openai.com/v1")
            .is_err());
        let policy = ProviderEgressPolicy {
            allowed_origins: vec!["api.openai.com".to_string()],
            allow_local_origins: true,
        };
        assert!(policy
            .validate_egress_url("https://api.openai.com/v1")
            .is_ok());
        assert!(policy.validate_egress_url("http://127.0.0.1:11434").is_ok());
    }
}
