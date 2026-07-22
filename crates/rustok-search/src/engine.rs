use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ranking::SearchRankingProfile;
use rustok_core::Result;

const BLOG_SOURCE_MODULE: &str = "blog";
const BLOG_ENTITY_TYPE: &str = "blog_post";
const BLOG_STOREFRONT_ROUTE: &str = "/modules/blog";
const MAX_BLOG_SLUG_LEN: usize = 200;

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
    #[serde(default)]
    pub channel_id: Option<Uuid>,
    pub original_query: String,
    pub query: String,
    pub ranking_profile: SearchRankingProfile,
    pub preset_key: Option<String>,
    pub limit: usize,
    pub offset: usize,
    pub published_only: bool,
    pub entity_types: Vec<String>,
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
    #[serde(default)]
    pub category_ids: Vec<Uuid>,
    #[serde(default)]
    pub attribute_filters: Vec<SearchAttributeFilter>,
    #[serde(default)]
    pub sort_attribute_code: Option<String>,
    #[serde(default)]
    pub sort_desc: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchAttributeFilter {
    pub attribute_code: String,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub min: Option<String>,
    #[serde(default)]
    pub max: Option<String>,
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

/// Resolves the canonical application URL for a normalized Search result.
///
/// URL ownership lives in the Search contract so GraphQL, native server
/// functions, remote connectors, and future consumers cannot drift. Blog URLs
/// are derived only for the canonical Blog source/entity pair and only from a
/// bounded safe owner-projected slug. Invalid payloads fail closed.
pub fn canonical_search_result_url(value: &SearchResultItem) -> Option<String> {
    match value.entity_type.as_str() {
        "product" => Some(format!("/store/products/{}", value.id)),
        "node" => Some(format!(
            "/modules/content?id={}{}",
            value.id,
            content_kind_query(&value.source_module)
        )),
        BLOG_ENTITY_TYPE if value.source_module == BLOG_SOURCE_MODULE => {
            canonical_blog_result_url(&value.payload)
        }
        _ => None,
    }
}

fn content_kind_query(source_module: &str) -> String {
    if source_module.is_empty() || source_module == "content" {
        return String::new();
    }

    if source_module.len() > 64
        || !source_module
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return String::new();
    }

    format!("&kind={source_module}")
}

fn canonical_blog_result_url(payload: &serde_json::Value) -> Option<String> {
    let slug = payload.get("slug")?.as_str()?.trim();
    if !valid_blog_slug(slug) {
        return None;
    }

    Some(format!("{BLOG_STOREFRONT_ROUTE}?slug={slug}"))
}

fn valid_blog_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= MAX_BLOG_SLUG_LEN
        && slug
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchFacetBucket {
    pub value: String,
    #[serde(default)]
    pub label: Option<String>,
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
    pub ranking_profile: SearchRankingProfile,
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
    use super::{SearchEngineKind, SearchResultItem, canonical_search_result_url};
    use serde_json::json;
    use uuid::Uuid;

    fn item(
        entity_type: &str,
        source_module: &str,
        payload: serde_json::Value,
    ) -> SearchResultItem {
        SearchResultItem {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").expect("valid UUID"),
            entity_type: entity_type.to_string(),
            source_module: source_module.to_string(),
            title: "Result".to_string(),
            snippet: None,
            score: 1.0,
            locale: Some("en".to_string()),
            payload,
        }
    }

    #[test]
    fn try_from_str_rejects_unknown_engines() {
        assert_eq!(
            SearchEngineKind::try_from_str("postgres"),
            Some(SearchEngineKind::Postgres)
        );
        assert_eq!(SearchEngineKind::try_from_str("unknown"), None);
    }

    #[test]
    fn canonical_url_derives_blog_route_from_safe_owner_slug() {
        let value = item("blog_post", "blog", json!({ "slug": "release-notes-2026" }));

        assert_eq!(
            canonical_search_result_url(&value).as_deref(),
            Some("/modules/blog?slug=release-notes-2026")
        );
    }

    #[test]
    fn canonical_url_fails_closed_for_spoofed_or_invalid_blog_payloads() {
        for value in [
            item("blog_post", "content", json!({ "slug": "valid" })),
            item("blog_post", "blog", json!({ "slug": "../admin" })),
            item("blog_post", "blog", json!({ "slug": "hello world" })),
            item("blog_post", "blog", json!({ "slug": 7 })),
            item("blog_post", "blog", json!({})),
        ] {
            assert_eq!(canonical_search_result_url(&value), None);
        }
    }

    #[test]
    fn canonical_url_preserves_product_and_content_contracts() {
        let product = item("product", "commerce", json!({}));
        assert_eq!(
            canonical_search_result_url(&product).as_deref(),
            Some("/store/products/00000000-0000-0000-0000-000000000001")
        );

        let content = item("node", "forum", json!({}));
        assert_eq!(
            canonical_search_result_url(&content).as_deref(),
            Some("/modules/content?id=00000000-0000-0000-0000-000000000001&kind=forum")
        );

        let unsafe_content = item("node", "forum&admin=true", json!({}));
        assert_eq!(
            canonical_search_result_url(&unsafe_content).as_deref(),
            Some("/modules/content?id=00000000-0000-0000-0000-000000000001")
        );
    }
}
