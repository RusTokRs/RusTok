pub mod graphql_adapter;
pub mod native_server_adapter;
mod navigation;

use crate::model::{
    SearchFilterPreset, SearchPreviewFilters, SearchPreviewPayload, SearchSuggestion,
    TrackSearchClickPayload,
};
use leptos::prelude::ServerFnError;
use rustok_ui_transport::{execute_selected_transport, UiTransportError, UiTransportPath};
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

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub type SearchTransportError = UiTransportError;

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
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
) -> Result<SearchPreviewPayload, SearchTransportError> {
    let native_query = query.clone();
    let native_locale = locale.clone();
    let native_preset_key = preset_key.clone();
    let native_filters = filters.clone();
    let mut payload = execute_selected_transport(
        "search",
        selected_transport_path(),
        move || {
            native_server_adapter::fetch_search(
                native_query,
                native_locale,
                native_preset_key,
                native_filters,
            )
        },
        move || graphql_adapter::fetch_search(query, locale, preset_key, filters),
    )
    .await?;
    navigation::enrich_search_result_urls(&mut payload);
    Ok(payload)
}

pub async fn fetch_suggestions(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, SearchTransportError> {
    let native_query = query.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "search",
        selected_transport_path(),
        move || native_server_adapter::fetch_suggestions(native_query, native_locale),
        move || graphql_adapter::fetch_suggestions(query, locale),
    )
    .await
}

pub async fn fetch_filter_presets() -> Result<Vec<SearchFilterPreset>, SearchTransportError> {
    execute_selected_transport(
        "search",
        selected_transport_path(),
        native_server_adapter::fetch_filter_presets,
        graphql_adapter::fetch_filter_presets,
    )
    .await
}

pub async fn track_search_click(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, SearchTransportError> {
    let native_query_log_id = query_log_id.clone();
    let native_document_id = document_id.clone();
    let native_href = href.clone();
    execute_selected_transport(
        "search",
        selected_transport_path(),
        move || {
            native_server_adapter::track_search_click(
                native_query_log_id,
                native_document_id,
                position,
                native_href,
            )
        },
        move || graphql_adapter::track_search_click(query_log_id, document_id, position, href),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }
}
