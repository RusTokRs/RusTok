use crate::model::{SearchAttributeFilter, SearchPreviewFilters, SearchPreviewPayload};
use rustok_graphql::{GraphqlRequest, execute as execute_graphql, graphql_url};
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

/// The forum Search field takes the richer filters as arguments beside `input`, so every filter
/// family travels in its own additive operation: `ForumStorefrontSearch` above keeps its exact
/// existing document, and a caller that has not landed a filter keeps using it. A shared
/// `SearchPreviewInput` field would instead have promised author, tag, solved and date filtering
/// to every Search surface, including the ones whose field has no such argument, as the three
/// `forum-search` filter contracts record.
const FORUM_STOREFRONT_SEARCH_BY_AUTHORS_QUERY: &str = concat!(
    "query ForumStorefrontSearchByAuthors($input: SearchPreviewInput!, $authorIds: [String!]!) {",
    " forumStorefrontSearch(input: $input, authorIds: $authorIds) {",
    " queryLogId presetKey total tookMs engine rankingProfile",
    " items { id entityType sourceModule title snippet score locale url payload }",
    " facets { name buckets { value label count } }",
    " }",
    " }",
);

const FORUM_STOREFRONT_SEARCH_BY_FILTERS_QUERY: &str = concat!(
    "query ForumStorefrontSearchByFilters(",
    "$input: SearchPreviewInput!, $tags: [String!], $solved: Boolean",
    ") {",
    " forumStorefrontSearch(input: $input, tags: $tags, solved: $solved) {",
    " queryLogId presetKey total tookMs engine rankingProfile",
    " items { id entityType sourceModule title snippet score locale url payload }",
    " facets { name buckets { value label count } }",
    " }",
    " }",
);

const FORUM_STOREFRONT_SEARCH_BY_DATE_WINDOW_QUERY: &str = concat!(
    "query ForumStorefrontSearchByDateWindow(",
    "$input: SearchPreviewInput!, $publishedFrom: String, $publishedTo: String",
    ") {",
    " forumStorefrontSearch(input: $input,",
    " publishedFrom: $publishedFrom, publishedTo: $publishedTo) {",
    " queryLogId presetKey total tookMs engine rankingProfile",
    " items { id entityType sourceModule title snippet score locale url payload }",
    " facets { name buckets { value label count } }",
    " }",
    " }",
);

#[derive(Debug, Serialize)]
struct AuthorSearchPreviewVariables {
    input: SearchPreviewInput,
    #[serde(rename = "authorIds")]
    author_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FilterSearchPreviewVariables {
    input: SearchPreviewInput,
    tags: Vec<String>,
    solved: Option<bool>,
}

const FORUM_STOREFRONT_SEARCH_BY_CURRENT_CHANNEL_QUERY: &str = concat!(
    "query ForumStorefrontSearchByCurrentChannel($input: SearchPreviewInput!) {",
    " forumStorefrontSearch(input: $input, currentChannelOnly: true) {",
    " queryLogId presetKey total tookMs engine rankingProfile",
    " items { id entityType sourceModule title snippet score locale url payload }",
    " facets { name buckets { value label count } }",
    " }",
    " }",
);

#[derive(Debug, Serialize)]
struct DateWindowSearchPreviewVariables {
    input: SearchPreviewInput,
    #[serde(rename = "publishedFrom")]
    published_from: Option<String>,
    #[serde(rename = "publishedTo")]
    published_to: Option<String>,
}

pub async fn fetch_search(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    let input = search_preview_input(query, locale, preset_key, filters);
    execute_forum_search_document(
        FORUM_STOREFRONT_SEARCH_QUERY,
        SearchPreviewVariables { input },
    )
    .await
}

pub async fn fetch_search_with_authors(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
) -> Result<SearchPreviewPayload, ApiError> {
    let input = search_preview_input(query, locale, preset_key, filters);
    execute_forum_search_document(
        FORUM_STOREFRONT_SEARCH_BY_AUTHORS_QUERY,
        AuthorSearchPreviewVariables { input, author_ids },
    )
    .await
}

pub async fn fetch_search_with_filters(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    tags: Vec<String>,
    solved: Option<bool>,
) -> Result<SearchPreviewPayload, ApiError> {
    let input = search_preview_input(query, locale, preset_key, filters);
    execute_forum_search_document(
        FORUM_STOREFRONT_SEARCH_BY_FILTERS_QUERY,
        FilterSearchPreviewVariables {
            input,
            tags,
            solved,
        },
    )
    .await
}

pub async fn fetch_search_with_date_window(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    published_from: Option<String>,
    published_to: Option<String>,
) -> Result<SearchPreviewPayload, ApiError> {
    let input = search_preview_input(query, locale, preset_key, filters);
    execute_forum_search_document(
        FORUM_STOREFRONT_SEARCH_BY_DATE_WINDOW_QUERY,
        DateWindowSearchPreviewVariables {
            input,
            published_from,
            published_to,
        },
    )
    .await
}

/// Trusted current-channel narrowing. The additive document carries the flag as a literal,
/// because the FORUM-23B2F4 contract forbids arbitrary channel-slug input: a caller either asks
/// for the narrowing or travels on the base operation, and an absent or false request keeps
/// existing behavior.
pub async fn fetch_search_with_current_channel(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    current_channel_only: Option<bool>,
) -> Result<SearchPreviewPayload, ApiError> {
    let input = search_preview_input(query, locale, preset_key, filters);
    let document = if current_channel_only == Some(true) {
        FORUM_STOREFRONT_SEARCH_BY_CURRENT_CHANNEL_QUERY
    } else {
        FORUM_STOREFRONT_SEARCH_QUERY
    };
    execute_forum_search_document(document, SearchPreviewVariables { input }).await
}

/// One document, one response shape: every forum Search operation selects the same payload, so the
/// transport differs only in the arguments it sends and must not differ in decoding or error
/// mapping.
async fn execute_forum_search_document<V>(
    document: &'static str,
    variables: V,
) -> Result<SearchPreviewPayload, ApiError>
where
    V: Serialize,
{
    let response: ForumStorefrontSearchResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(document, Some(variables)),
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
