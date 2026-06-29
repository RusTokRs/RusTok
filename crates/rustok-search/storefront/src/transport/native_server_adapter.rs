use crate::model::{
    SearchFilterPreset, SearchPreviewFilters, SearchPreviewPayload, SearchSuggestion,
    TrackSearchClickPayload,
};
use leptos::prelude::*;

use super::ApiError;

pub async fn fetch_search(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    fetch_storefront_search_server(query, locale, preset_key, filters).await
}

pub async fn fetch_suggestions(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ApiError> {
    fetch_storefront_suggestions_server(query, locale).await
}

pub async fn fetch_filter_presets() -> Result<Vec<SearchFilterPreset>, ApiError> {
    fetch_storefront_filter_presets_server().await
}

pub async fn track_search_click(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    track_search_click_server(query_log_id, document_id, position, href).await
}

async fn fetch_storefront_search_server(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    storefront_search_native(query, locale, preset_key, filters)
        .await
        .map_err(ApiError::from)
}

async fn fetch_storefront_filter_presets_server() -> Result<Vec<SearchFilterPreset>, ApiError> {
    storefront_filter_presets_native()
        .await
        .map_err(ApiError::from)
}

async fn fetch_storefront_suggestions_server(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ApiError> {
    storefront_search_suggestions_native(query, locale)
        .await
        .map_err(ApiError::from)
}

async fn track_search_click_server(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    storefront_track_search_click_native(query_log_id, document_id, position, href)
        .await
        .map_err(ApiError::from)
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
        super::graphql_adapter::fetch_search(query, locale, preset_key, filters)
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
        super::graphql_adapter::fetch_filter_presets()
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
        super::graphql_adapter::fetch_suggestions(query, locale)
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
        super::graphql_adapter::track_search_click(query_log_id, document_id, position, href)
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
