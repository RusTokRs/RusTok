mod graphql_adapter;
mod native_server_adapter;
mod rest_adapter;

use leptos::prelude::ServerFnError;
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    MediaListItem, MediaListPayload, MediaTranslationPayload, MediaUsageSnapshot,
    UpsertTranslationPayload,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    Rest(String),
    ServerFn(String),
    Validation(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::Rest(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
            Self::Validation(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

impl From<UiTransportError> for ApiError {
    fn from(value: UiTransportError) -> Self {
        match value.failed_path {
            UiTransportPath::NativeServer => Self::ServerFn(value.to_string()),
            UiTransportPath::Graphql => Self::Graphql(value.to_string()),
        }
    }
}

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

pub async fn fetch_media_library(
    page: i32,
    per_page: i32,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaListPayload, ApiError> {
    execute_selected_transport(
        "media_admin",
        selected_transport_path(),
        move || native_server_adapter::media_library_native(page, per_page),
        move || graphql_adapter::fetch_media_library_graphql(page, per_page, token, tenant_slug),
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_media_detail(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<MediaListItem>, ApiError> {
    let native_media_id = media_id.clone();
    execute_selected_transport(
        "media_admin",
        selected_transport_path(),
        move || native_server_adapter::media_detail_native(native_media_id),
        move || graphql_adapter::fetch_media_detail_graphql(media_id, token, tenant_slug),
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_media_translations(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<MediaTranslationPayload>, ApiError> {
    let native_media_id = media_id.clone();
    execute_selected_transport(
        "media_admin",
        selected_transport_path(),
        move || native_server_adapter::media_translations_native(native_media_id),
        move || graphql_adapter::fetch_media_translations_graphql(media_id, token, tenant_slug),
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_media_usage(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaUsageSnapshot, ApiError> {
    execute_selected_transport(
        "media_admin",
        selected_transport_path(),
        native_server_adapter::media_usage_native,
        move || graphql_adapter::fetch_media_usage_graphql(token, tenant_slug),
    )
    .await
    .map_err(ApiError::from)
}

pub async fn upsert_translation(
    media_id: String,
    payload: UpsertTranslationPayload,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaTranslationPayload, ApiError> {
    let native_media_id = media_id.clone();
    let native_payload = payload.clone();
    execute_selected_transport(
        "media_admin",
        selected_transport_path(),
        move || {
            native_server_adapter::media_upsert_translation_native(native_media_id, native_payload)
        },
        move || graphql_adapter::upsert_translation_graphql(media_id, payload, token, tenant_slug),
    )
    .await
    .map_err(ApiError::from)
}

pub async fn delete_media(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<bool, ApiError> {
    let native_media_id = media_id.clone();
    execute_selected_transport(
        "media_admin",
        selected_transport_path(),
        move || native_server_adapter::media_delete_native(native_media_id),
        move || graphql_adapter::delete_media_graphql(media_id, token, tenant_slug),
    )
    .await
    .map_err(ApiError::from)
}

pub async fn upload_media(
    file_name: String,
    content_type: String,
    data: Vec<u8>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaListItem, ApiError> {
    rest_adapter::upload_media_rest(file_name, content_type, data, token, tenant_slug).await
}
