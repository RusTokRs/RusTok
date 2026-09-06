use crate::model::{SearchAttributeFilter, SearchPreviewFilters, SearchPreviewPayload};
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use super::{ApiError, configured_tenant_slug};

const FORUM_STOREFRONT_SEARCH_QUERY: &str = "query ForumStorefrontSearch($input: SearchPreviewInput!) { forumStorefrontSearch(input: $input) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }";

#[derive(Debug, Deserialize)]
struct ForumStorefrontSearchResponse {
    #[serde(rename = "forumStorefrontSearch")]
    forum_storefront_search: SearchPreviewPayload,
}

#[derive(Debug, Serialize)]
struct SearchPreviewVariables {
    input: SearchPreviewInput,
}

#[derive(Debug, Clone, Serialize)]
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
    source_modules: Vec<String>,
    statuses: Option<Vec<String>>,
    #[serde(rename = "categoryIds")]
    category_ids: Vec<String>,
    #[serde(rename = "attributeFilters")]
    attribute_filters: Option<Vec<SearchAttributeFilterInput>>,
    #[serde(rename = "sortAttributeCode")]
    sort_attribute_code: Option<String>,
    #[serde(rename = "sortDesc")]
    sort_desc: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
struct SearchAttributeFilterInput {
    #[serde(rename = "attributeCode")]
    attribute_code: String,
    values: Option<Vec<String>>,
    min: Option<String>,
    max: Option<String>,
}

pub async fn fetch_search(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    let input = search_preview_input(query, locale, preset_key, filters);
    let response: ForumStorefrontSearchResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            FORUM_STOREFRONT_SEARCH_QUERY,
            Some(SearchPreviewVariables { input }),
        ),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))?;

    Ok(response.forum_storefront_search)
}

fn search_preview_input(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> SearchPreviewInput {
    let SearchPreviewFilters {
        channel_id,
        entity_types,
        source_modules,
        statuses,
        category_ids,
        attribute_filters,
        sort_attribute_code,
        sort_desc,
    } = filters;

    SearchPreviewInput {
        query,
        locale,
        channel_id,
        limit: Some(12),
        offset: Some(0),
        preset_key,
        entity_types: (!entity_types.is_empty()).then_some(entity_types),
        source_modules,
        statuses: (!statuses.is_empty()).then_some(statuses),
        category_ids,
        attribute_filters: (!attribute_filters.is_empty())
            .then_some(search_attribute_filter_inputs(attribute_filters)),
        sort_attribute_code,
        sort_desc: sort_desc.then_some(true),
    }
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
