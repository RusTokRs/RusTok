pub mod graphql_adapter;
pub mod native_server_adapter;

use crate::model::{
    SearchFilterPreset, SearchPreviewFilters, SearchPreviewPayload, SearchSuggestion,
    TrackSearchClickPayload,
};
use leptos::prelude::ServerFnError;
use rustok_graphql::GraphqlHttpError;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

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

pub(crate) fn configured_tenant_slug() -> Option<String> {
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
