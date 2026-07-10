use serde::{Deserialize, Serialize};

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
pub struct ProviderCatalogEntry {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub features: &'static [ProviderFeature],
    pub settings: &'static [ProviderConfigField],
    pub credentials: &'static [ProviderConfigField],
    pub compiled_in: bool,
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
const CLOUD_SETTINGS: &[ProviderConfigField] = &[
    ProviderConfigField {
        key: "region",
        label: "Region",
        kind: ProviderFieldKind::Text,
        required: true,
    },
    ProviderConfigField {
        key: "project",
        label: "Project or account",
        kind: ProviderFieldKind::Text,
        required: false,
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
const CHAT_MEDIA: &[ProviderFeature] = &[
    ProviderFeature::Chat,
    ProviderFeature::Streaming,
    ProviderFeature::Tools,
    ProviderFeature::StructuredOutput,
    ProviderFeature::Embeddings,
    ProviderFeature::Image,
    ProviderFeature::Audio,
    ProviderFeature::Transcription,
    ProviderFeature::Multimodal,
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
            compiled_in: true,
        }
    };
}

static CATALOG: &[ProviderCatalogEntry] = &[
    entry!("openai", "OpenAI", CHAT_MEDIA, BASE_URL, API_KEY),
    entry!(
        "openai_compatible",
        "OpenAI-compatible",
        CHAT_MEDIA,
        BASE_URL,
        API_KEY
    ),
    entry!("anthropic", "Anthropic", CHAT, BASE_URL, API_KEY),
    entry!(
        "azure_openai",
        "Azure OpenAI",
        CHAT_EMBED,
        CLOUD_SETTINGS,
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
    entry!("gemini", "Google Gemini", CHAT_EMBED, BASE_URL, API_KEY),
    entry!("groq", "Groq", CHAT, BASE_URL, API_KEY),
    entry!(
        "hugging_face",
        "Hugging Face",
        CHAT_MEDIA,
        BASE_URL,
        API_KEY
    ),
    entry!("hyperbolic", "Hyperbolic", CHAT, BASE_URL, API_KEY),
    entry!("llamafile", "Llamafile", CHAT_EMBED, LOCAL_URL, &[]),
    entry!("minimax", "MiniMax", CHAT, BASE_URL, API_KEY),
    entry!("mira", "Mira", CHAT, BASE_URL, API_KEY),
    entry!("mistral", "Mistral", CHAT_EMBED, BASE_URL, API_KEY),
    entry!("moonshot", "Moonshot", CHAT, BASE_URL, API_KEY),
    entry!("ollama", "Ollama", CHAT_EMBED, LOCAL_URL, &[]),
    entry!("openrouter", "OpenRouter", CHAT_MEDIA, BASE_URL, API_KEY),
    entry!("perplexity", "Perplexity", CHAT, BASE_URL, API_KEY),
    entry!("together", "Together AI", CHAT_EMBED, BASE_URL, API_KEY),
    entry!("voyage_ai", "Voyage AI", EMBED_RERANK, BASE_URL, API_KEY),
    entry!("xai", "xAI", CHAT, BASE_URL, API_KEY),
    entry!("xiaomi_mimo", "Xiaomi MiMo", CHAT, BASE_URL, API_KEY),
    entry!("zai", "Z.ai", CHAT, BASE_URL, API_KEY),
    entry!(
        "aws_bedrock",
        "AWS Bedrock",
        CHAT_EMBED,
        CLOUD_SETTINGS,
        &[]
    ),
    entry!(
        "vertex_ai",
        "Google Vertex AI",
        CHAT_EMBED,
        CLOUD_SETTINGS,
        &[]
    ),
    entry!(
        "gemini_grpc",
        "Google Gemini gRPC",
        CHAT_EMBED,
        CLOUD_SETTINGS,
        &[]
    ),
    ProviderCatalogEntry {
        slug: "fastembed",
        display_name: "FastEmbed",
        features: EMBEDDINGS,
        settings: &[],
        credentials: &[],
        compiled_in: cfg!(feature = "fastembed"),
    },
];

pub fn provider_catalog() -> &'static [ProviderCatalogEntry] {
    CATALOG
}

pub fn provider_catalog_entry(slug: &ProviderSlug) -> Option<&'static ProviderCatalogEntry> {
    CATALOG.iter().find(|entry| entry.slug == slug.as_str())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn catalog_slugs_are_unique_and_normalized() {
        let mut seen = HashSet::new();
        for entry in provider_catalog() {
            assert_eq!(ProviderSlug::new(entry.slug).unwrap().as_str(), entry.slug);
            assert!(seen.insert(entry.slug));
        }
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
}
