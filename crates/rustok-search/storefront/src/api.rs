use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{SearchPreviewFilters, SearchPreviewPayload, TrackSearchClickPayload};

pub type ApiError = GraphqlHttpError;

const STOREFRONT_SEARCH_QUERY: &str = "query StorefrontSearch($input: SearchPreviewInput!) { storefrontSearch(input: $input) { queryLogId total tookMs engine items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value count } } } }";
const TRACK_SEARCH_CLICK_MUTATION: &str = "mutation TrackSearchClick($input: TrackSearchClickInput!) { trackSearchClick(input: $input) { success tracked } }";

#[derive(Debug, Deserialize)]
struct StorefrontSearchResponse {
    #[serde(rename = "storefrontSearch")]
    storefront_search: SearchPreviewPayload,
}

#[derive(Debug, Serialize)]
struct SearchPreviewVariables {
    input: SearchPreviewInput,
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

#[derive(Debug, Serialize)]
struct SearchPreviewInput {
    query: String,
    locale: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
    #[serde(rename = "entityTypes")]
    entity_types: Option<Vec<String>>,
    #[serde(rename = "sourceModules")]
    source_modules: Option<Vec<String>>,
    statuses: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
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
}

pub async fn fetch_storefront_search(
    query: String,
    locale: Option<String>,
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

pub async fn track_search_click(
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
