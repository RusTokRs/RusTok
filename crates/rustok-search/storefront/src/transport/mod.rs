pub mod forum_graphql_adapter;
pub mod forum_native_server_adapter;
pub mod graphql_adapter;
pub mod native_server_adapter;

use crate::model::{
    SearchFilterPreset, SearchPreviewFilters, SearchPreviewPayload, SearchSuggestion,
    TrackSearchClickPayload,
};
use leptos::prelude::ServerFnError;
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};
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
    let forum_category_scope = is_explicit_forum_category_scope(&filters);
    let native_query = query.clone();
    let native_locale = locale.clone();
    let native_preset_key = preset_key.clone();
    let native_filters = filters.clone();

    if forum_category_scope {
        return execute_selected_transport(
            "search",
            selected_transport_path(),
            move || {
                forum_native_server_adapter::fetch_search(
                    native_query,
                    native_locale,
                    native_preset_key,
                    native_filters,
                )
            },
            move || forum_graphql_adapter::fetch_search(query, locale, preset_key, filters),
        )
        .await;
    }

    execute_selected_transport(
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
    .await
}

#[allow(dead_code)]
pub async fn fetch_forum_search_by_authors(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
) -> Result<SearchPreviewPayload, SearchTransportError> {
    let native_query = query.clone();
    let native_locale = locale.clone();
    let native_preset_key = preset_key.clone();
    let native_filters = filters.clone();
    let native_author_ids = author_ids.clone();

    execute_selected_transport(
        "search",
        selected_transport_path(),
        move || {
            forum_native_server_adapter::fetch_search_with_authors(
                native_query,
                native_locale,
                native_preset_key,
                native_filters,
                native_author_ids,
            )
        },
        move || {
            forum_graphql_adapter::fetch_search_with_authors(
                query, locale, preset_key, filters, author_ids,
            )
        },
    )
    .await
}

#[allow(dead_code)]
pub async fn fetch_forum_search_with_filters(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
) -> Result<SearchPreviewPayload, SearchTransportError> {
    let native_query = query.clone();
    let native_locale = locale.clone();
    let native_preset_key = preset_key.clone();
    let native_filters = filters.clone();
    let native_author_ids = author_ids.clone();
    let native_tags = tags.clone();

    execute_selected_transport(
        "search",
        selected_transport_path(),
        move || {
            forum_native_server_adapter::fetch_search_with_filters(
                native_query,
                native_locale,
                native_preset_key,
                native_filters,
                native_author_ids,
                native_tags,
                solved,
            )
        },
        move || {
            forum_graphql_adapter::fetch_search_with_filters(
                query, locale, preset_key, filters, author_ids, tags, solved,
            )
        },
    )
    .await
}

#[allow(dead_code)]
pub async fn fetch_forum_search_with_date_window(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
    published_from: Option<String>,
    published_to: Option<String>,
) -> Result<SearchPreviewPayload, SearchTransportError> {
    let native_query = query.clone();
    let native_locale = locale.clone();
    let native_preset_key = preset_key.clone();
    let native_filters = filters.clone();
    let native_author_ids = author_ids.clone();
    let native_tags = tags.clone();
    let native_published_from = published_from.clone();
    let native_published_to = published_to.clone();

    execute_selected_transport(
        "search",
        selected_transport_path(),
        move || {
            forum_native_server_adapter::fetch_search_with_date_window(
                native_query,
                native_locale,
                native_preset_key,
                native_filters,
                native_author_ids,
                native_tags,
                solved,
                native_published_from,
                native_published_to,
            )
        },
        move || {
            forum_graphql_adapter::fetch_search_with_date_window(
                query,
                locale,
                preset_key,
                filters,
                author_ids,
                tags,
                solved,
                published_from,
                published_to,
            )
        },
    )
    .await
}

#[allow(dead_code)]
pub async fn fetch_forum_search_with_current_channel(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
    author_ids: Vec<String>,
    tags: Vec<String>,
    solved: Option<bool>,
    published_from: Option<String>,
    published_to: Option<String>,
) -> Result<SearchPreviewPayload, SearchTransportError> {
    let native_query = query.clone();
    let native_locale = locale.clone();
    let native_preset_key = preset_key.clone();
    let native_filters = filters.clone();
    let native_author_ids = author_ids.clone();
    let native_tags = tags.clone();
    let native_published_from = published_from.clone();
    let native_published_to = published_to.clone();

    execute_selected_transport(
        "search",
        selected_transport_path(),
        move || {
            forum_native_server_adapter::fetch_search_with_current_channel(
                native_query,
                native_locale,
                native_preset_key,
                native_filters,
                native_author_ids,
                native_tags,
                solved,
                native_published_from,
                native_published_to,
            )
        },
        move || {
            forum_graphql_adapter::fetch_search_with_current_channel(
                query,
                locale,
                preset_key,
                filters,
                author_ids,
                tags,
                solved,
                published_from,
                published_to,
            )
        },
    )
    .await
}

fn is_explicit_forum_category_scope(filters: &SearchPreviewFilters) -> bool {
    !filters.category_ids.is_empty()
        && filters.source_modules.len() == 1
        && filters.source_modules[0]
            .trim()
            .eq_ignore_ascii_case("forum")
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
    fn default_test_profile_uses_graphql_transport() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }

    #[test]
    fn only_explicit_forum_category_scope_selects_owner_path() {
        let mut filters = SearchPreviewFilters {
            source_modules: vec!["forum".to_string()],
            category_ids: vec![uuid::Uuid::new_v4().to_string()],
            ..SearchPreviewFilters::default()
        };
        assert!(is_explicit_forum_category_scope(&filters));

        filters.source_modules.push("product".to_string());
        assert!(!is_explicit_forum_category_scope(&filters));

        filters.source_modules = vec!["forum".to_string()];
        filters.category_ids.clear();
        assert!(!is_explicit_forum_category_scope(&filters));
    }
}
