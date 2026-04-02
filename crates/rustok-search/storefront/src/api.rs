use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    SearchFilterPreset, SearchPreviewFilters, SearchPreviewPayload, SearchSuggestion,
    TrackSearchClickPayload,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

const STOREFRONT_SEARCH_QUERY: &str = "query StorefrontSearch($input: SearchPreviewInput!) { storefrontSearch(input: $input) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value count } } } }";
const STOREFRONT_FILTER_PRESETS_QUERY: &str = "query StorefrontSearchFilterPresets { storefrontSearchFilterPresets { key label entityTypes sourceModules statuses rankingProfile } }";
const STOREFRONT_SEARCH_SUGGESTIONS_QUERY: &str = "query StorefrontSearchSuggestions($input: SearchSuggestionsInput!) { storefrontSearchSuggestions(input: $input) { text kind documentId entityType sourceModule locale url score } }";
const TRACK_SEARCH_CLICK_MUTATION: &str = "mutation TrackSearchClick($input: TrackSearchClickInput!) { trackSearchClick(input: $input) { success tracked } }";

#[derive(Debug, Deserialize)]
struct StorefrontSearchResponse {
    #[serde(rename = "storefrontSearch")]
    storefront_search: SearchPreviewPayload,
}

#[derive(Debug, Deserialize)]
struct StorefrontSearchSuggestionsResponse {
    #[serde(rename = "storefrontSearchSuggestions")]
    storefront_search_suggestions: Vec<SearchSuggestion>,
}

#[derive(Debug, Deserialize)]
struct StorefrontFilterPresetsResponse {
    #[serde(rename = "storefrontSearchFilterPresets")]
    storefront_search_filter_presets: Vec<SearchFilterPreset>,
}

#[derive(Debug, Serialize)]
struct SearchPreviewVariables {
    input: SearchPreviewInput,
}

#[derive(Debug, Serialize)]
struct SearchSuggestionsVariables {
    input: SearchSuggestionsInput,
}

#[derive(Debug, Deserialize)]
struct TrackSearchClickResponse {
    #[serde(rename = "trackSearchClick")]
    track_search_click: TrackSearchClickPayload,
}

#[derive(Debug, Serialize)]
struct TrackSearchClickVariables {
    input: TrackSearchClickInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchPreviewInput {
    query: String,
    locale: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
    #[serde(rename = "presetKey")]
    preset_key: Option<String>,
    #[serde(rename = "entityTypes")]
    entity_types: Option<Vec<String>>,
    #[serde(rename = "sourceModules")]
    source_modules: Option<Vec<String>>,
    statuses: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchSuggestionsInput {
    query: String,
    locale: Option<String>,
    limit: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackSearchClickInput {
    #[serde(rename = "queryLogId")]
    query_log_id: String,
    #[serde(rename = "documentId")]
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(query: &str, variables: V) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_storefront_search(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    match fetch_storefront_search_server(
        query.clone(),
        locale.clone(),
        preset_key.clone(),
        filters.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => fetch_storefront_search_graphql(query, locale, preset_key, filters).await,
    }
}

pub async fn fetch_storefront_filter_presets() -> Result<Vec<SearchFilterPreset>, ApiError> {
    match fetch_storefront_filter_presets_server().await {
        Ok(payload) => Ok(payload),
        Err(_) => fetch_storefront_filter_presets_graphql().await,
    }
}

pub async fn fetch_storefront_suggestions(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ApiError> {
    match fetch_storefront_suggestions_server(query.clone(), locale.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => fetch_storefront_suggestions_graphql(query, locale).await,
    }
}

pub async fn track_search_click(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    match track_search_click_server(
        query_log_id.clone(),
        document_id.clone(),
        position,
        href.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => track_search_click_graphql(query_log_id, document_id, position, href).await,
    }
}

pub async fn fetch_storefront_search_server(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    storefront_search_native(query, locale, preset_key, filters)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_filter_presets_server() -> Result<Vec<SearchFilterPreset>, ApiError> {
    storefront_filter_presets_native()
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_suggestions_server(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ApiError> {
    storefront_search_suggestions_native(query, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn track_search_click_server(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    storefront_track_search_click_native(query_log_id, document_id, position, href)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_search_graphql(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    let response: StorefrontSearchResponse = request(
        STOREFRONT_SEARCH_QUERY,
        SearchPreviewVariables {
            input: SearchPreviewInput {
                query,
                locale,
                limit: Some(12),
                offset: Some(0),
                preset_key,
                entity_types: (!filters.entity_types.is_empty()).then_some(filters.entity_types),
                source_modules: (!filters.source_modules.is_empty())
                    .then_some(filters.source_modules),
                statuses: (!filters.statuses.is_empty()).then_some(filters.statuses),
            },
        },
    )
    .await?;

    Ok(response.storefront_search)
}

pub async fn fetch_storefront_filter_presets_graphql() -> Result<Vec<SearchFilterPreset>, ApiError>
{
    let response: StorefrontFilterPresetsResponse =
        request(STOREFRONT_FILTER_PRESETS_QUERY, ()).await?;

    Ok(response.storefront_search_filter_presets)
}

pub async fn fetch_storefront_suggestions_graphql(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ApiError> {
    let response: StorefrontSearchSuggestionsResponse = request(
        STOREFRONT_SEARCH_SUGGESTIONS_QUERY,
        SearchSuggestionsVariables {
            input: SearchSuggestionsInput {
                query,
                locale,
                limit: Some(6),
            },
        },
    )
    .await?;

    Ok(response.storefront_search_suggestions)
}

pub async fn track_search_click_graphql(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    let response: TrackSearchClickResponse = request(
        TRACK_SEARCH_CLICK_MUTATION,
        TrackSearchClickVariables {
            input: TrackSearchClickInput {
                query_log_id,
                document_id,
                position,
                href,
            },
        },
    )
    .await?;

    Ok(response.track_search_click)
}

#[server(prefix = "/api/fn", endpoint = "search/storefront-search")]
async fn storefront_search_native(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        fetch_storefront_search_graphql(query, locale, preset_key, filters)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query, locale, preset_key, filters);
        Err(ServerFnError::new(
            "search/storefront-search requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/storefront-filter-presets")]
async fn storefront_filter_presets_native() -> Result<Vec<SearchFilterPreset>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        fetch_storefront_filter_presets_graphql()
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "search/storefront-filter-presets requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/storefront-suggestions")]
async fn storefront_search_suggestions_native(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        fetch_storefront_suggestions_graphql(query, locale)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query, locale);
        Err(ServerFnError::new(
            "search/storefront-suggestions requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/storefront-track-click")]
async fn storefront_track_search_click_native(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        track_search_click_graphql(query_log_id, document_id, position, href)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query_log_id, document_id, position, href);
        Err(ServerFnError::new(
            "search/storefront-track-click requires the `ssr` feature",
        ))
    }
}
