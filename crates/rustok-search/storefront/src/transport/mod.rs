pub mod graphql_adapter;
pub mod native_server_adapter;

use crate::api::ApiError;
use crate::model::{
    SearchFilterPreset, SearchPreviewFilters, SearchPreviewPayload, SearchSuggestion,
    TrackSearchClickPayload,
};

pub async fn fetch_search(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    match native_server_adapter::fetch_search(
        query.clone(),
        locale.clone(),
        preset_key.clone(),
        filters.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => graphql_adapter::fetch_search(query, locale, preset_key, filters).await,
    }
}

pub async fn fetch_suggestions(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ApiError> {
    match native_server_adapter::fetch_suggestions(query.clone(), locale.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => graphql_adapter::fetch_suggestions(query, locale).await,
    }
}

pub async fn fetch_filter_presets() -> Result<Vec<SearchFilterPreset>, ApiError> {
    match native_server_adapter::fetch_filter_presets().await {
        Ok(payload) => Ok(payload),
        Err(_) => graphql_adapter::fetch_filter_presets().await,
    }
}

pub async fn track_search_click(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    match native_server_adapter::track_search_click(
        query_log_id.clone(),
        document_id.clone(),
        position,
        href.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            graphql_adapter::track_search_click(query_log_id, document_id, position, href).await
        }
    }
}
