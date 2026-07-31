use crate::model::{SearchAttributeFilter, SearchPreviewFilters, SearchPreviewPayload};
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use super::{ApiError, configured_tenant_slug};

const FORUM_STOREFRONT_SEARCH_QUERY: &str = "query ForumStorefrontSearch($input: SearchPreviewInput!) { forumStorefrontSearch(input: $input) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }";
const FORUM_STOREFRONT_SEARCH_BY_AUTHORS_QUERY: &str = "query ForumStorefrontSearchByAuthors($input: SearchPreviewInput!, $authorIds: [String!]!) { forumStorefrontSearch(input: $input, authorIds: $authorIds) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }";
const FORUM_STOREFRONT_SEARCH_BY_FILTERS_QUERY: &str = "query ForumStorefrontSearchByFilters($input: SearchPreviewInput!, $authorIds: [String!], $tags: [String!], $solved: Boolean) { forumStorefrontSearch(input: $input, authorIds: $authorIds, tags: $tags, solved: $solved) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }";
const FORUM_STOREFRONT_SEARCH_BY_DATE_WINDOW_QUERY: &str = "query ForumStorefrontSearchByDateWindow($input: SearchPreviewInput!, $authorIds: [String!], $tags: [String!], $solved: Boolean, $publishedFrom: String, $publishedTo: String) { forumStorefrontSearch(input: $input, authorIds: $authorIds, tags: $tags, solved: $solved, publishedFrom: $publishedFrom, publishedTo: $publishedTo) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value label count } } } }";

#[derive(Debug, Deserialize)]
struct ForumStorefrontSearchResponse {
    #[serde(rename = "forumStorefrontSearch")]
    forum_storefront_search: SearchPreviewPayload,
}

#[derive(Debug, Serialize)]
struct SearchPreviewVariables {
    input: SearchPreviewInput,
}

#[derive(Debug, Serialize)]
struct AuthorSearchPreviewVariables {
    input: SearchPreviewInput,
    #[serde(rename = "authorIds")]
    author_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FilterSearchPreviewVariables {
    input: SearchPreviewInput,
    #[serde(rename = "authorIds")]
    author_ids: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    solved: Option<bool>,
}

#[derive(Debug, Serialize)]
struct DateWindowSearchPreviewVariables {
    input: SearchPreviewInput,
    #[serde(rename = "authorIds")]
    author_ids: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    solved: Option<bool>,
    #[serde(rename = "publishedFrom")]
    published_from: Option<String>,
    #[serde(rename = "publishedTo")]
    published_to: Option<String>,
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

pub async fn fetch_search_with_authors(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
) -> Result<SearchPreviewPayload, ApiError> {
    let input = search_preview_input(query, locale, preset_key, filters);
    let response: ForumStorefrontSearchResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            FORUM_STOREFRONT_SEARCH_BY_AUTHORS_QUERY,
            Some(AuthorSearchPreviewVariables { input, author_ids }),
        ),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))?;

    Ok(response.forum_storefront_search)
}

pub async fn fetch_search_with_filters(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
) -> Result<SearchPreviewPayload, ApiError> {
    let input = search_preview_input(query, locale, preset_key, filters);
    let response: ForumStorefrontSearchResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            FORUM_STOREFRONT_SEARCH_BY_FILTERS_QUERY,
            Some(FilterSearchPreviewVariables {
                input,
                author_ids: (!author_ids.is_empty()).then_some(author_ids),
                tags: (!tags.is_empty()).then_some(tags),
                solved,
            }),
        ),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))?;

    Ok(response.forum_storefront_search)
}

pub async fn fetch_search_with_date_window(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
    published_from: Option<String>,
    published_to: Option<String>,
) -> Result<SearchPreviewPayload, ApiError> {
    let input = search_preview_input(query, locale, preset_key, filters);
    let response: ForumStorefrontSearchResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            FORUM_STOREFRONT_SEARCH_BY_DATE_WINDOW_QUERY,
            Some(DateWindowSearchPreviewVariables {
                input,
                author_ids: (!author_ids.is_empty()).then_some(author_ids),
                tags: (!tags.is_empty()).then_some(tags),
                solved,
                published_from,
                published_to,
            }),
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
