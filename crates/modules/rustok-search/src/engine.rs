use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ranking::SearchRankingProfile;
use rustok_core::Result;

const BLOG_SOURCE_MODULE: &str = "blog";
const BLOG_ENTITY_TYPE: &str = "blog_post";
const BLOG_STOREFRONT_ROUTE: &str = "/modules/blog";
const MAX_BLOG_SLUG_LEN: usize = 200;
const FORUM_SOURCE_MODULE: &str = "forum";
const FORUM_CATEGORY_ENTITY_TYPE: &str = "forum_category";
const FORUM_TOPIC_ENTITY_TYPE: &str = "forum_topic";
const FORUM_REPLY_ENTITY_TYPE: &str = "forum_reply";
const FORUM_TOPIC_ROUTE_SHORT_ID_LEN: usize = 12;
const MAX_FORUM_ROUTE_LOCALE_LEN: usize = 64;
const MAX_FORUM_ROUTE_SLUG_LEN: usize = 255;

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
/// URL projection remains centralized here so GraphQL, native server functions,
/// remote connectors, and future consumers cannot drift. Forum owns its route
/// identity and projects the exact localized path. Search validates the projected
/// envelope, entity identities, locale, short topic identity, path shape, and
/// optional reply anchor before exposing it. Invalid or stale projections fail
/// closed and require owner reindexing; Search never builds Forum result URLs.
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
        FORUM_CATEGORY_ENTITY_TYPE | FORUM_TOPIC_ENTITY_TYPE | FORUM_REPLY_ENTITY_TYPE
            if value.source_module == FORUM_SOURCE_MODULE =>
        {
            canonical_forum_projected_result_url(value)
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

fn canonical_forum_projected_result_url(value: &SearchResultItem) -> Option<String> {
    let route = value.payload.get("route")?.as_str()?;
    if route != route.trim()
        || route.is_empty()
        || !route.starts_with('/')
        || route.starts_with("//")
        || route.chars().any(char::is_control)
        || route.contains('#')
    {
        return None;
    }

    let locale = exact_forum_locale(value.locale.as_deref()?)?;
    match value.entity_type.as_str() {
        FORUM_CATEGORY_ENTITY_TYPE => {
            let category_id = parse_payload_uuid(&value.payload, "category_id")?;
            if category_id != value.id {
                return None;
            }
            canonical_forum_category_route(route, locale.as_str())?;
        }
        FORUM_TOPIC_ENTITY_TYPE => {
            let topic_id = parse_payload_uuid(&value.payload, "topic_id")?;
            if topic_id != value.id {
                return None;
            }
            canonical_forum_topic_route(route, locale.as_str(), topic_id, None)?;
        }
        FORUM_REPLY_ENTITY_TYPE => {
            let reply_id = parse_payload_uuid(&value.payload, "reply_id")?;
            if reply_id != value.id {
                return None;
            }
            let topic_id = parse_payload_uuid(&value.payload, "topic_id")?;
            canonical_forum_topic_route(route, locale.as_str(), topic_id, Some(reply_id))?;
        }
        _ => return None,
    }

    Some(route.to_string())
}

fn canonical_forum_category_route(route: &str, expected_locale: &str) -> Option<()> {
    let (path, query) = split_forum_route(route)?;
    if query.is_some() {
        return None;
    }
    let segments = forum_route_segments(path)?;
    match segments.as_slice() {
        [locale, "forum", "c", slug] if *locale == expected_locale && valid_forum_slug(slug) => {
            Some(())
        }
        _ => None,
    }
}

fn canonical_forum_topic_route(
    route: &str,
    expected_locale: &str,
    topic_id: Uuid,
    reply_id: Option<Uuid>,
) -> Option<()> {
    let (path, query) = split_forum_route(route)?;
    match reply_id {
        Some(reply_id) => {
            let expected_query = format!("reply={reply_id}");
            if query != Some(expected_query.as_str()) {
                return None;
            }
        }
        None if query.is_none() => {}
        None => return None,
    }

    let segments = forum_route_segments(path)?;
    let expected_short_id = forum_topic_short_identity(topic_id);
    match segments.as_slice() {
        [locale, "forum", "t", short_id, slug]
            if *locale == expected_locale
                && *short_id == expected_short_id.as_str()
                && valid_forum_short_identity(short_id)
                && valid_forum_slug(slug) =>
        {
            Some(())
        }
        _ => None,
    }
}

fn split_forum_route(route: &str) -> Option<(&str, Option<&str>)> {
    let mut parts = route.split('?');
    let path = parts.next()?;
    let query = parts.next();
    if parts.next().is_some() || query.is_some_and(str::is_empty) {
        return None;
    }
    Some((path, query))
}

fn forum_route_segments(path: &str) -> Option<Vec<&str>> {
    let path = path.strip_prefix('/')?;
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.is_empty() || segments.iter().any(|segment| segment.is_empty()) {
        return None;
    }
    Some(segments)
}

fn exact_forum_locale(value: &str) -> Option<String> {
    if value != value.trim()
        || value.is_empty()
        || value.chars().count() > MAX_FORUM_ROUTE_LOCALE_LEN
    {
        return None;
    }
    let normalized = rustok_api::normalize_locale_tag(value)?;
    (normalized == value).then_some(normalized)
}

fn valid_forum_short_identity(value: &str) -> bool {
    value.len() == FORUM_TOPIC_ROUTE_SHORT_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn forum_topic_short_identity(topic_id: Uuid) -> String {
    topic_id.simple().to_string()[..FORUM_TOPIC_ROUTE_SHORT_ID_LEN].to_string()
}

fn valid_forum_slug(value: &str) -> bool {
    if value.is_empty()
        || value.len() > MAX_FORUM_ROUTE_SLUG_LEN
        || value.starts_with('-')
        || value.ends_with('-')
        || value.contains("--")
    {
        return false;
    }
    value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn parse_payload_uuid(payload: &serde_json::Value, key: &str) -> Option<Uuid> {
    let value = payload.get(key)?.as_str()?;
    let value = Uuid::parse_str(value).ok()?;
    (!value.is_nil()).then_some(value)
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

    fn item_with_id(
        id: &str,
        entity_type: &str,
        source_module: &str,
        locale: Option<&str>,
        payload: serde_json::Value,
    ) -> SearchResultItem {
        SearchResultItem {
            id: Uuid::parse_str(id).expect("valid UUID"),
            entity_type: entity_type.to_string(),
            source_module: source_module.to_string(),
            title: "Result".to_string(),
            snippet: None,
            score: 1.0,
            locale: locale.map(ToOwned::to_owned),
            payload,
        }
    }

    fn item(
        entity_type: &str,
        source_module: &str,
        payload: serde_json::Value,
    ) -> SearchResultItem {
        item_with_id(
            "00000000-0000-0000-0000-000000000001",
            entity_type,
            source_module,
            Some("en"),
            payload,
        )
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
    fn canonical_url_accepts_owner_projected_forum_category_topic_and_reply_routes() {
        let category_id = "11111111-1111-4111-8111-111111111111";
        let topic_id = "22222222-2222-4222-8222-222222222222";
        let reply_id = "33333333-3333-4333-8333-333333333333";
        let category = item_with_id(
            category_id,
            "forum_category",
            "forum",
            Some("en"),
            json!({
                "category_id": category_id,
                "route": "/en/forum/c/general"
            }),
        );
        let topic = item_with_id(
            topic_id,
            "forum_topic",
            "forum",
            Some("en"),
            json!({
                "topic_id": topic_id,
                "route": "/en/forum/t/222222222222/welcome"
            }),
        );
        let reply = item_with_id(
            reply_id,
            "forum_reply",
            "forum",
            Some("en"),
            json!({
                "reply_id": reply_id,
                "topic_id": topic_id,
                "route": format!("/en/forum/t/222222222222/welcome?reply={reply_id}")
            }),
        );

        assert_eq!(
            canonical_search_result_url(&category).as_deref(),
            Some("/en/forum/c/general")
        );
        assert_eq!(
            canonical_search_result_url(&topic).as_deref(),
            Some("/en/forum/t/222222222222/welcome")
        );
        assert_eq!(
            canonical_search_result_url(&reply).as_deref(),
            Some("/en/forum/t/222222222222/welcome?reply=33333333-3333-4333-8333-333333333333")
        );
    }

    #[test]
    fn canonical_url_rejects_stale_or_malformed_forum_route_projections() {
        let category_id = "11111111-1111-4111-8111-111111111111";
        let topic_id = "22222222-2222-4222-8222-222222222222";
        let reply_id = "33333333-3333-4333-8333-333333333333";
        let cases = [
            item_with_id(
                category_id,
                "forum_category",
                "content",
                Some("en"),
                json!({
                    "category_id": category_id,
                    "route": "/en/forum/c/general"
                }),
            ),
            item_with_id(
                category_id,
                "forum_category",
                "forum",
                Some("en"),
                json!({
                    "category_id": category_id,
                    "route": format!("/modules/forum?category={category_id}")
                }),
            ),
            item_with_id(
                category_id,
                "forum_category",
                "forum",
                Some("en"),
                json!({
                    "category_id": topic_id,
                    "route": "/en/forum/c/general"
                }),
            ),
            item_with_id(
                category_id,
                "forum_category",
                "forum",
                Some("ru"),
                json!({
                    "category_id": category_id,
                    "route": "/en/forum/c/general"
                }),
            ),
            item_with_id(
                category_id,
                "forum_category",
                "forum",
                Some(" en "),
                json!({
                    "category_id": category_id,
                    "route": "/en/forum/c/general"
                }),
            ),
            item_with_id(
                category_id,
                "forum_category",
                "forum",
                Some("en"),
                json!({
                    "category_id": category_id,
                    "route": "/en/forum/c/general "
                }),
            ),
            item_with_id(
                topic_id,
                "forum_topic",
                "forum",
                Some("en"),
                json!({
                    "topic_id": topic_id,
                    "route": "/en/forum/t/111111111111/welcome"
                }),
            ),
            item_with_id(
                topic_id,
                "forum_topic",
                "forum",
                Some("en"),
                json!({
                    "topic_id": topic_id,
                    "route": "/en/forum/t/222222222222/Welcome"
                }),
            ),
            item_with_id(
                topic_id,
                "forum_topic",
                "forum",
                Some("en"),
                json!({
                    "topic_id": topic_id,
                    "route": "https://example.invalid/en/forum/t/222222222222/welcome"
                }),
            ),
            item_with_id(
                topic_id,
                "forum_topic",
                "forum",
                Some("en"),
                json!({
                    "topic_id": topic_id,
                    "route": "//example.invalid/en/forum/t/222222222222/welcome"
                }),
            ),
            item_with_id(
                topic_id,
                "forum_topic",
                "forum",
                Some("en"),
                json!({
                    "topic_id": topic_id,
                    "route": "/en/forum/t/222222222222/welcome#fragment"
                }),
            ),
            item_with_id(
                reply_id,
                "forum_reply",
                "forum",
                Some("en"),
                json!({
                    "reply_id": reply_id,
                    "topic_id": topic_id,
                    "route": "/en/forum/t/222222222222/welcome"
                }),
            ),
            item_with_id(
                reply_id,
                "forum_reply",
                "forum",
                Some("en"),
                json!({
                    "reply_id": reply_id,
                    "topic_id": topic_id,
                    "route": "/en/forum/t/222222222222/welcome?reply=44444444-4444-4444-8444-444444444444"
                }),
            ),
            item_with_id(
                reply_id,
                "forum_reply",
                "forum",
                None,
                json!({
                    "reply_id": reply_id,
                    "topic_id": topic_id,
                    "route": format!("/en/forum/t/222222222222/welcome?reply={reply_id}")
                }),
            ),
        ];

        for value in cases {
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
