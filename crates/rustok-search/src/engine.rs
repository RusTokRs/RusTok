use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rustok_core::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEngineKind {
    Postgres,
    Meilisearch,
    Typesense,
    Algolia,
}

impl SearchEngineKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Meilisearch => "meilisearch",
            Self::Typesense => "typesense",
            Self::Algolia => "algolia",
        }
    }

    pub fn try_from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "postgres" => Some(Self::Postgres),
            "meilisearch" => Some(Self::Meilisearch),
            "typesense" => Some(Self::Typesense),
            "algolia" => Some(Self::Algolia),
            _ => None,
        }
    }

    pub fn from_db_value(value: &str) -> Self {
        Self::try_from_str(value).unwrap_or(Self::Postgres)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchConnectorDescriptor {
    pub kind: SearchEngineKind,
    pub label: String,
    pub provided_by: String,
    pub enabled: bool,
    pub default_engine: bool,
}

impl SearchConnectorDescriptor {
    pub fn postgres_default() -> Self {
        Self {
            kind: SearchEngineKind::Postgres,
            label: "PostgreSQL".to_string(),
            provided_by: "rustok-search".to_string(),
            enabled: true,
            default_engine: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub tenant_id: Option<Uuid>,
    pub locale: Option<String>,
    pub original_query: String,
    pub query: String,
    pub limit: usize,
    pub offset: usize,
    pub published_only: bool,
    pub entity_types: Vec<String>,
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub id: Uuid,
    pub entity_type: String,
    pub source_module: String,
    pub title: String,
    pub snippet: Option<String>,
    pub score: f64,
    pub locale: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchFacetBucket {
    pub value: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchFacetGroup {
    pub name: String,
    pub buckets: Vec<SearchFacetBucket>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub items: Vec<SearchResultItem>,
    pub total: u64,
    pub took_ms: u64,
    pub engine: SearchEngineKind,
    pub facets: Vec<SearchFacetGroup>,
}

#[async_trait]
pub trait SearchEngine: Send + Sync {
    fn kind(&self) -> SearchEngineKind;

    fn descriptor(&self) -> SearchConnectorDescriptor;

    async fn search(&self, query: SearchQuery) -> Result<SearchResult>;
}

#[cfg(test)]
mod tests {
    use super::SearchEngineKind;

    #[test]
    fn try_from_str_rejects_unknown_engines() {
        assert_eq!(
            SearchEngineKind::try_from_str("postgres"),
            Some(SearchEngineKind::Postgres)
        );
        assert_eq!(SearchEngineKind::try_from_str("unknown"), None);
    }
}
