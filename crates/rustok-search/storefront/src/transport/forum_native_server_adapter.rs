#[cfg(feature = "ssr")]
use crate::model::{SearchFacetBucket, SearchFacetGroup, SearchPreviewResultItem};
use crate::model::{SearchPreviewFilters, SearchPreviewPayload};
use leptos::prelude::{ServerFnError, server};

use super::ApiError;

pub async fn fetch_search(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    forum_storefront_search_native(query, locale, preset_key, filters)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "search/forum-storefront-search")]
async fn forum_storefront_search_native(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ServerFnError> {
    execute_forum_storefront_search_native(
        query,
        locale,
        preset_key,
        filters,
        Vec::new(),
        Vec::new(),
        None,
        None,
        None,
        None,
    )
    .await
}

async fn execute_forum_storefront_search_native(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
    published_from: Option<String>,
    published_to: Option<String>,
    current_channel_only: Option<bool>,
) -> Result<SearchPreviewPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{HostRuntimeContext, OptionalAuthContext, RequestContext, TenantContext};
        use rustok_search::{
            ForumStorefrontSearchAttributeFilter, ForumStorefrontSearchRequest,
            SharedStorefrontSearchCategoryScopePort, SharedStorefrontSearchResultEligibilityPort,
            StorefrontSearchTransport, execute_forum_storefront_search,
            resolve_trusted_storefront_channel_input,
        };

        let runtime = expect_context::<HostRuntimeContext>();
        let db = runtime.db_clone();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let trusted_channel = resolve_trusted_storefront_channel_input(
            &request_context,
            tenant.id,
            filters.channel_id.as_deref(),
        )
        .map_err(|error| ServerFnError::new(error.to_string()))?;
        let auth = leptos_axum::extract::<OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?
            .0;
        let category_scope_port = runtime.shared_get::<SharedStorefrontSearchCategoryScopePort>();
        let result_eligibility_port =
            runtime.shared_get::<SharedStorefrontSearchResultEligibilityPort>();
        let request = ForumStorefrontSearchRequest {
            tenant_id: tenant.id,
            query,
            locale,
            fallback_locale: tenant.default_locale,
            channel_id: trusted_channel.channel_id.map(|value| value.to_string()),
            current_channel_only,
            limit: Some(12),
            offset: Some(0),
            ranking_profile: None,
            preset_key,
            entity_types: filters.entity_types,
            source_modules: filters.source_modules,
            statuses: filters.statuses,
            category_ids: filters.category_ids,
            author_ids,
            tags,
            solved,
            published_from,
            published_to,
            attribute_filters: filters
                .attribute_filters
                .into_iter()
                .map(|filter| ForumStorefrontSearchAttributeFilter {
                    attribute_code: filter.attribute_code,
                    values: filter.values,
                    min: filter.min,
                    max: filter.max,
                })
                .collect(),
            sort_attribute_code: filters.sort_attribute_code,
            sort_desc: filters.sort_desc,
            auth,
            request_context: Some(request_context),
            transport: StorefrontSearchTransport::NativeServer,
        };
        let execution = execute_forum_storefront_search(
            &db,
            category_scope_port,
            result_eligibility_port,
            request,
        )
        .await
        .map_err(|error| ServerFnError::new(error.public_message()))?;

        Ok(map_search_result(
            execution.result,
            execution.query_log_id.map(|value| value.to_string()),
            execution.preset_key,
            execution.elapsed_ms,
        ))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            query,
            locale,
            preset_key,
            filters,
            author_ids,
            tags,
            solved,
            published_from,
            published_to,
            current_channel_only,
        );
        Err(ServerFnError::new(
            "Forum storefront Search requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_search_result(
    value: rustok_search::SearchResult,
    query_log_id: Option<String>,
    preset_key: Option<String>,
    elapsed_ms: u64,
) -> SearchPreviewPayload {
    SearchPreviewPayload {
        query_log_id,
        preset_key,
        total: value.total,
        took_ms: value.took_ms.max(elapsed_ms),
        engine: value.engine.as_str().to_string(),
        ranking_profile: value.ranking_profile.as_str().to_string(),
        items: value.items.into_iter().map(map_result_item).collect(),
        facets: value.facets.into_iter().map(map_facet_group).collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_result_item(value: rustok_search::SearchResultItem) -> SearchPreviewResultItem {
    let url = rustok_search::canonical_search_result_url(&value);
    SearchPreviewResultItem {
        id: value.id.to_string(),
        entity_type: value.entity_type,
        source_module: value.source_module,
        title: value.title,
        snippet: value.snippet,
        score: value.score,
        locale: value.locale,
        url,
        payload: serde_json::to_string(&value.payload).unwrap_or_else(|_| "{}".to_string()),
    }
}

#[cfg(feature = "ssr")]
fn map_facet_group(value: rustok_search::engine::SearchFacetGroup) -> SearchFacetGroup {
    SearchFacetGroup {
        name: value.name,
        buckets: value.buckets.into_iter().map(map_facet_bucket).collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_facet_bucket(value: rustok_search::engine::SearchFacetBucket) -> SearchFacetBucket {
    SearchFacetBucket {
        value: value.value,
        label: value.label,
        count: value.count,
    }
}
