mod graphql_adapter;
mod native_server_adapter;
mod rest_adapter;

use leptos::prelude::ServerFnError;
use rustok_graphql::GraphqlHttpError;
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

pub async fn fetch_media_library(
    page: i32,
    per_page: i32,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaListPayload, ApiError> {
    match native_server_adapter::media_library_native(page, per_page).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            graphql_adapter::fetch_media_library_graphql(page, per_page, token, tenant_slug).await
        }
    }
}

pub async fn fetch_media_detail(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<MediaListItem>, ApiError> {
    match native_server_adapter::media_detail_native(media_id.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => graphql_adapter::fetch_media_detail_graphql(media_id, token, tenant_slug).await,
    }
}

pub async fn fetch_media_translations(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<MediaTranslationPayload>, ApiError> {
    match native_server_adapter::media_translations_native(media_id.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            graphql_adapter::fetch_media_translations_graphql(media_id, token, tenant_slug).await
        }
    }
}

pub async fn fetch_media_usage(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaUsageSnapshot, ApiError> {
    match native_server_adapter::media_usage_native().await {
        Ok(payload) => Ok(payload),
        Err(_) => graphql_adapter::fetch_media_usage_graphql(token, tenant_slug).await,
    }
}

pub async fn upsert_translation(
    media_id: String,
    payload: UpsertTranslationPayload,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaTranslationPayload, ApiError> {
    match native_server_adapter::media_upsert_translation_native(media_id.clone(), payload.clone())
        .await
    {
        Ok(result) => Ok(result),
        Err(_) => {
            graphql_adapter::upsert_translation_graphql(media_id, payload, token, tenant_slug).await
        }
    }
}

pub async fn delete_media(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<bool, ApiError> {
    match native_server_adapter::media_delete_native(media_id.clone()).await {
        Ok(result) => Ok(result),
        Err(_) => graphql_adapter::delete_media_graphql(media_id, token, tenant_slug).await,
    }
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
