use crate::model::{
    SearchAttributeFilter, SearchFilterPreset, SearchPreviewFilters, SearchPreviewPayload,
    SearchSuggestion, TrackSearchClickPayload,
};
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use super::{ApiError, configured_tenant_slug};

const STOREFRONT_SEARCH_QUERY: &str = "query StorefrontSearch($input: SearchPreviewInput!) { storefrontSearch(input: $input) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }";
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
    #[serde(rename = "channelId")]
    channel_id: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
    #[serde(rename = "presetKey")]
    preset_key: Option<String>,
    #[serde(rename = "entityTypes")]
    entity_types: Option<Vec<String>>,
    #[serde(rename = "sourceModules")]
    source_modules: Option<Vec<String>>,
    statuses: Option<Vec<String>>,
    #[serde(rename = "categoryIds")]
    category_ids: Option<Vec<String>>,
    #[serde(rename = "attributeFilters")]
    attribute_filters: Option<Vec<SearchAttributeFilterInput>>,
    #[serde(rename = "sortAttributeCode")]
    sort_attribute_code: Option<String>,
    #[serde(rename = "sortDesc")]
    sort_desc: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchAttributeFilterInput {
    #[serde(rename = "attributeCode")]
    attribute_code: String,
    values: Option<Vec<String>>,
    min: Option<String>,
    max: Option<String>,
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
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn fetch_search(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    fetch_storefront_search_graphql(query, locale, preset_key, filters).await
}

async fn fetch_storefront_search_graphql(
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
                channel_id: filters.channel_id,
                limit: Some(12),
                offset: Some(0),
                preset_key,
                entity_types: (!filters.entity_types.is_empty()).then_some(filters.entity_types),
                source_modules: (!filters.source_modules.is_empty())
                    .then_some(filters.source_modules),
                statuses: (!filters.statuses.is_empty()).then_some(filters.statuses),
                category_ids: (!filters.category_ids.is_empty()).then_some(filters.category_ids),
                attribute_filters: (!filters.attribute_filters.is_empty())
                    .then_some(search_attribute_filter_inputs(filters.attribute_filters)),
                sort_attribute_code: filters.sort_attribute_code,
                sort_desc: filters.sort_desc.then_some(true),
            },
        },
    )
    .await?;

    Ok(response.storefront_search)
}

fn search_attribute_filter_inputs(
    filters: Vec<SearchAttributeFilter>,
) -> Vec<SearchAttributeFilterInput> {
    filters
        .into_iter()
        .map(|filter| SearchAttributeFilterInput {
            attribute_code: filter.attribute_code,
            values: (!filter.values.is_empty()).then_some(filter.values),
            min: filter.min,
            max: filter.max,
        })
        .collect()
}

pub async fn fetch_suggestions(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ApiError> {
    fetch_storefront_suggestions_graphql(query, locale).await
}

async fn fetch_storefront_suggestions_graphql(
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

pub async fn fetch_filter_presets() -> Result<Vec<SearchFilterPreset>, ApiError> {
    fetch_storefront_filter_presets_graphql().await
}

async fn fetch_storefront_filter_presets_graphql() -> Result<Vec<SearchFilterPreset>, ApiError> {
    let response: StorefrontFilterPresetsResponse =
        request(STOREFRONT_FILTER_PRESETS_QUERY, ()).await?;

    Ok(response.storefront_search_filter_presets)
}

pub async fn track_search_click(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    track_search_click_graphql(query_log_id, document_id, position, href).await
}

async fn track_search_click_graphql(
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
