use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    MediaListItem, MediaListPayload, MediaTranslationPayload, MediaUsageSnapshot,
    UpsertTranslationPayload,
};

const MEDIA_QUERY: &str = "query MediaLibrary($tenantId: UUID!, $pagination: PaginationInput!) { media(tenantId: $tenantId, pagination: $pagination) { items { id tenantId uploadedBy filename originalName mimeType size storageDriver publicUrl width height createdAt } total } }";
const MEDIA_ITEM_QUERY: &str = "query MediaItem($tenantId: UUID!, $id: UUID!) { mediaItem(tenantId: $tenantId, id: $id) { id tenantId uploadedBy filename originalName mimeType size storageDriver publicUrl width height createdAt } }";
const MEDIA_TRANSLATIONS_QUERY: &str = "query MediaTranslations($tenantId: UUID!, $mediaId: UUID!) { mediaTranslations(tenantId: $tenantId, mediaId: $mediaId) { id mediaId locale title altText caption } }";
const MEDIA_USAGE_QUERY: &str = "query MediaUsage($tenantId: UUID!) { mediaUsage(tenantId: $tenantId) { tenantId fileCount totalBytes } }";
const UPSERT_MEDIA_TRANSLATION_MUTATION: &str = "mutation UpsertMediaTranslation($tenantId: UUID!, $mediaId: UUID!, $input: UpsertMediaTranslationInput!) { upsertMediaTranslation(tenantId: $tenantId, mediaId: $mediaId, input: $input) { id mediaId locale title altText caption } }";
const DELETE_MEDIA_MUTATION: &str = "mutation DeleteMedia($tenantId: UUID!, $id: UUID!) { deleteMedia(tenantId: $tenantId, id: $id) }";

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

#[derive(Debug, Deserialize)]
struct MediaLibraryResponse {
    media: MediaLibraryConnection,
}

#[derive(Debug, Deserialize)]
struct MediaLibraryConnection {
    items: Vec<GraphqlMediaItem>,
    total: i64,
}

#[derive(Debug, Deserialize)]
struct MediaItemResponse {
    #[serde(rename = "mediaItem")]
    media_item: Option<GraphqlMediaItem>,
}

#[derive(Debug, Deserialize)]
struct MediaTranslationsResponse {
    #[serde(rename = "mediaTranslations")]
    media_translations: Vec<GraphqlMediaTranslation>,
}

#[derive(Debug, Deserialize)]
struct MediaUsageResponse {
    #[serde(rename = "mediaUsage")]
    media_usage: GraphqlMediaUsage,
}

#[derive(Debug, Deserialize)]
struct UpsertMediaTranslationResponse {
    #[serde(rename = "upsertMediaTranslation")]
    upsert_media_translation: GraphqlMediaTranslation,
}

#[derive(Debug, Deserialize)]
struct DeleteMediaResponse {
    #[serde(rename = "deleteMedia")]
    delete_media: bool,
}

#[derive(Debug, Deserialize)]
struct GraphqlMediaItem {
    id: String,
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(rename = "uploadedBy")]
    uploaded_by: Option<String>,
    filename: String,
    #[serde(rename = "originalName")]
    original_name: String,
    #[serde(rename = "mimeType")]
    mime_type: String,
    size: i64,
    #[serde(rename = "storageDriver")]
    storage_driver: String,
    #[serde(rename = "publicUrl")]
    public_url: String,
    width: Option<i32>,
    height: Option<i32>,
    #[serde(rename = "createdAt")]
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct GraphqlMediaTranslation {
    id: String,
    #[serde(rename = "mediaId")]
    media_id: String,
    locale: String,
    title: Option<String>,
    #[serde(rename = "altText")]
    alt_text: Option<String>,
    caption: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphqlMediaUsage {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(rename = "fileCount")]
    file_count: i64,
    #[serde(rename = "totalBytes")]
    total_bytes: i64,
}

#[derive(Debug, Deserialize)]
struct RestMediaItem {
    id: String,
    tenant_id: String,
    uploaded_by: Option<String>,
    filename: String,
    original_name: String,
    mime_type: String,
    size: i64,
    storage_driver: String,
    public_url: String,
    width: Option<i32>,
    height: Option<i32>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct RestApiErrorPayload {
    error: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct MediaLibraryVariables {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    pagination: PaginationInput,
}

#[derive(Debug, Serialize)]
struct PaginationInput {
    first: i32,
    after: Option<String>,
}

#[derive(Debug, Serialize)]
struct MediaItemVariables {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    id: String,
}

#[derive(Debug, Serialize)]
struct MediaTranslationsVariables {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(rename = "mediaId")]
    media_id: String,
}

#[derive(Debug, Serialize)]
struct MediaUsageVariables {
    #[serde(rename = "tenantId")]
    tenant_id: String,
}

#[derive(Debug, Serialize)]
struct UpsertMediaTranslationVariables {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(rename = "mediaId")]
    media_id: String,
    input: UpsertMediaTranslationInput,
}

#[derive(Debug, Serialize)]
struct UpsertMediaTranslationInput {
    locale: String,
    title: Option<String>,
    #[serde(rename = "altText")]
    alt_text: Option<String>,
    caption: Option<String>,
}

#[derive(Debug, Serialize)]
struct DeleteMediaVariables {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    id: String,
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

fn api_base_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.trim_end_matches("/api/graphql").to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string())
    }
}

async fn graphql_request<V, T>(
    query: &str,
    variables: Option<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, variables),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(ApiError::from)
}

fn map_media_item(item: GraphqlMediaItem) -> MediaListItem {
    MediaListItem {
        id: item.id,
        tenant_id: item.tenant_id,
        uploaded_by: item.uploaded_by,
        filename: item.filename,
        original_name: item.original_name,
        mime_type: item.mime_type,
        size: item.size,
        storage_driver: item.storage_driver,
        public_url: item.public_url,
        width: item.width,
        height: item.height,
        created_at: item.created_at,
    }
}

fn map_rest_media_item(item: RestMediaItem) -> MediaListItem {
    MediaListItem {
        id: item.id,
        tenant_id: item.tenant_id,
        uploaded_by: item.uploaded_by,
        filename: item.filename,
        original_name: item.original_name,
        mime_type: item.mime_type,
        size: item.size,
        storage_driver: item.storage_driver,
        public_url: item.public_url,
        width: item.width,
        height: item.height,
        created_at: item.created_at,
    }
}

fn map_translation(item: GraphqlMediaTranslation) -> MediaTranslationPayload {
    MediaTranslationPayload {
        id: item.id,
        media_id: item.media_id,
        locale: item.locale,
        title: item.title,
        alt_text: item.alt_text,
        caption: item.caption,
    }
}

fn cursor_for_page(page: i32, limit: i32) -> Option<String> {
    if page <= 1 {
        None
    } else {
        Some(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            ((page - 1) * limit).to_string(),
        ))
    }
}

pub async fn fetch_media_library(
    page: i32,
    per_page: i32,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaListPayload, ApiError> {
    match media_library_native(page, per_page).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let tenant_id = tenant_slug
                .clone()
                .ok_or_else(|| ApiError::Validation("tenant slug is required".to_string()))?;
            let response: MediaLibraryResponse = graphql_request(
                MEDIA_QUERY,
                Some(MediaLibraryVariables {
                    tenant_id,
                    pagination: PaginationInput {
                        first: per_page.clamp(1, 100),
                        after: cursor_for_page(page.max(1), per_page.clamp(1, 100)),
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;
            Ok(MediaListPayload {
                items: response
                    .media
                    .items
                    .into_iter()
                    .map(map_media_item)
                    .collect(),
                total: response.media.total.max(0) as u64,
            })
        }
    }
}

pub async fn fetch_media_detail(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<MediaListItem>, ApiError> {
    match media_detail_native(media_id.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let tenant_id = tenant_slug
                .clone()
                .ok_or_else(|| ApiError::Validation("tenant slug is required".to_string()))?;
            let response: MediaItemResponse = graphql_request(
                MEDIA_ITEM_QUERY,
                Some(MediaItemVariables {
                    tenant_id,
                    id: media_id,
                }),
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.media_item.map(map_media_item))
        }
    }
}

pub async fn fetch_media_translations(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<MediaTranslationPayload>, ApiError> {
    match media_translations_native(media_id.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let tenant_id = tenant_slug
                .clone()
                .ok_or_else(|| ApiError::Validation("tenant slug is required".to_string()))?;
            let response: MediaTranslationsResponse = graphql_request(
                MEDIA_TRANSLATIONS_QUERY,
                Some(MediaTranslationsVariables {
                    tenant_id,
                    media_id,
                }),
                token,
                tenant_slug,
            )
            .await?;
            Ok(response
                .media_translations
                .into_iter()
                .map(map_translation)
                .collect())
        }
    }
}

pub async fn fetch_media_usage(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaUsageSnapshot, ApiError> {
    match media_usage_native().await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let tenant_id = tenant_slug
                .clone()
                .ok_or_else(|| ApiError::Validation("tenant slug is required".to_string()))?;
            let response: MediaUsageResponse = graphql_request(
                MEDIA_USAGE_QUERY,
                Some(MediaUsageVariables { tenant_id }),
                token,
                tenant_slug,
            )
            .await?;
            Ok(MediaUsageSnapshot {
                tenant_id: response.media_usage.tenant_id,
                file_count: response.media_usage.file_count,
                total_bytes: response.media_usage.total_bytes,
            })
        }
    }
}

pub async fn upsert_translation(
    media_id: String,
    payload: UpsertTranslationPayload,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaTranslationPayload, ApiError> {
    match media_upsert_translation_native(media_id.clone(), payload.clone()).await {
        Ok(result) => Ok(result),
        Err(_) => {
            let tenant_id = tenant_slug
                .clone()
                .ok_or_else(|| ApiError::Validation("tenant slug is required".to_string()))?;
            let response: UpsertMediaTranslationResponse = graphql_request(
                UPSERT_MEDIA_TRANSLATION_MUTATION,
                Some(UpsertMediaTranslationVariables {
                    tenant_id,
                    media_id,
                    input: UpsertMediaTranslationInput {
                        locale: payload.locale,
                        title: payload.title,
                        alt_text: payload.alt_text,
                        caption: payload.caption,
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;
            Ok(map_translation(response.upsert_media_translation))
        }
    }
}

pub async fn delete_media(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<bool, ApiError> {
    match media_delete_native(media_id.clone()).await {
        Ok(result) => Ok(result),
        Err(_) => {
            let tenant_id = tenant_slug
                .clone()
                .ok_or_else(|| ApiError::Validation("tenant slug is required".to_string()))?;
            let response: DeleteMediaResponse = graphql_request(
                DELETE_MEDIA_MUTATION,
                Some(DeleteMediaVariables {
                    tenant_id,
                    id: media_id,
                }),
                token,
                tenant_slug,
            )
            .await?;
            Ok(response.delete_media)
        }
    }
}

pub async fn upload_media(
    file_name: String,
    content_type: String,
    data: Vec<u8>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaListItem, ApiError> {
    let part = reqwest::multipart::Part::bytes(data)
        .file_name(file_name)
        .mime_str(&content_type)
        .map_err(|err| ApiError::Rest(format!("invalid content type: {err}")))?;
    let form = reqwest::multipart::Form::new().part("file", part);
    let client = reqwest::Client::new();
    let mut request = client
        .post(format!("{}/api/media", api_base_url()))
        .multipart(form);
    if let Some(token) = token {
        request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(tenant_slug) = tenant_slug {
        request = request.header("X-Tenant-ID", tenant_slug);
    }

    let response = request
        .send()
        .await
        .map_err(|err| ApiError::Rest(format!("upload failed: {err}")))?;
    if !response.status().is_success() {
        return Err(ApiError::Rest(extract_rest_error(response).await));
    }

    response
        .json::<RestMediaItem>()
        .await
        .map(map_rest_media_item)
        .map_err(|err| ApiError::Rest(format!("invalid upload response payload: {err}")))
}

async fn extract_rest_error(response: reqwest::Response) -> String {
    match response.json::<RestApiErrorPayload>().await {
        Ok(payload) => payload
            .message
            .or(payload.error)
            .unwrap_or_else(|| "request failed".to_string()),
        Err(_) => "request failed".to_string(),
    }
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value).map_err(|err| ServerFnError::new(err.to_string()))
}

#[cfg(feature = "ssr")]
fn require_permission(
    auth: &rustok_api::AuthContext,
    permission: rustok_core::Permission,
) -> Result<(), ServerFnError> {
    if rustok_api::has_effective_permission(&auth.permissions, &permission) {
        Ok(())
    } else {
        Err(ServerFnError::new(format!("{permission} required")))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/library")]
async fn media_library_native(page: i32, per_page: i32) -> Result<MediaListPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_core::{Action, Permission, Resource};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::List))?;
        let storage = app_ctx
            .shared_store
            .get::<rustok_storage::StorageService>()
            .ok_or_else(|| ServerFnError::new("StorageService not available"))?;
        let service = rustok_media::MediaService::new(app_ctx.db.clone(), storage);
        let limit = per_page.clamp(1, 100) as u64;
        let offset = (page.max(1) - 1) as u64 * limit;
        let (items, total) = service
            .list(tenant.id, limit, offset)
            .await
            .map_err(ServerFnError::new)?;

        Ok(MediaListPayload {
            items: items
                .into_iter()
                .map(|item| MediaListItem {
                    id: item.id.to_string(),
                    tenant_id: item.tenant_id.to_string(),
                    uploaded_by: item.uploaded_by.map(|value| value.to_string()),
                    filename: item.filename,
                    original_name: item.original_name,
                    mime_type: item.mime_type,
                    size: item.size,
                    storage_driver: item.storage_driver,
                    public_url: item.public_url,
                    width: item.width,
                    height: item.height,
                    created_at: item.created_at.to_rfc3339(),
                })
                .collect(),
            total,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (page, per_page);
        Err(ServerFnError::new(
            "media/library requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/detail")]
async fn media_detail_native(media_id: String) -> Result<Option<MediaListItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_core::{Action, Permission, Resource};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::Read))?;
        let storage = app_ctx
            .shared_store
            .get::<rustok_storage::StorageService>()
            .ok_or_else(|| ServerFnError::new("StorageService not available"))?;
        let service = rustok_media::MediaService::new(app_ctx.db.clone(), storage);
        match service.get(tenant.id, parse_uuid(&media_id)?).await {
            Ok(item) => Ok(Some(MediaListItem {
                id: item.id.to_string(),
                tenant_id: item.tenant_id.to_string(),
                uploaded_by: item.uploaded_by.map(|value| value.to_string()),
                filename: item.filename,
                original_name: item.original_name,
                mime_type: item.mime_type,
                size: item.size,
                storage_driver: item.storage_driver,
                public_url: item.public_url,
                width: item.width,
                height: item.height,
                created_at: item.created_at.to_rfc3339(),
            })),
            Err(rustok_media::MediaError::NotFound(_)) => Ok(None),
            Err(err) => Err(ServerFnError::new(err.to_string())),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = media_id;
        Err(ServerFnError::new(
            "media/detail requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/translations")]
async fn media_translations_native(
    media_id: String,
) -> Result<Vec<MediaTranslationPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_core::{Action, Permission, Resource};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::Read))?;
        let storage = app_ctx
            .shared_store
            .get::<rustok_storage::StorageService>()
            .ok_or_else(|| ServerFnError::new("StorageService not available"))?;
        let service = rustok_media::MediaService::new(app_ctx.db.clone(), storage);
        let items = service
            .get_translations(tenant.id, parse_uuid(&media_id)?)
            .await
            .map_err(ServerFnError::new)?;
        Ok(items
            .into_iter()
            .map(|item| MediaTranslationPayload {
                id: item.id.to_string(),
                media_id: item.media_id.to_string(),
                locale: item.locale,
                title: item.title,
                alt_text: item.alt_text,
                caption: item.caption,
            })
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = media_id;
        Err(ServerFnError::new(
            "media/translations requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/upsert-translation")]
async fn media_upsert_translation_native(
    media_id: String,
    payload: UpsertTranslationPayload,
) -> Result<MediaTranslationPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_core::{Action, Permission, Resource};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::Update))?;
        let storage = app_ctx
            .shared_store
            .get::<rustok_storage::StorageService>()
            .ok_or_else(|| ServerFnError::new("StorageService not available"))?;
        let service = rustok_media::MediaService::new(app_ctx.db.clone(), storage);
        let item = service
            .upsert_translation(
                tenant.id,
                parse_uuid(&media_id)?,
                rustok_media::UpsertTranslationInput {
                    locale: payload.locale,
                    title: payload.title,
                    alt_text: payload.alt_text,
                    caption: payload.caption,
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(MediaTranslationPayload {
            id: item.id.to_string(),
            media_id: item.media_id.to_string(),
            locale: item.locale,
            title: item.title,
            alt_text: item.alt_text,
            caption: item.caption,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (media_id, payload);
        Err(ServerFnError::new(
            "media/upsert-translation requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/delete")]
async fn media_delete_native(media_id: String) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_core::{Action, Permission, Resource};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::Delete))?;
        let storage = app_ctx
            .shared_store
            .get::<rustok_storage::StorageService>()
            .ok_or_else(|| ServerFnError::new("StorageService not available"))?;
        let service = rustok_media::MediaService::new(app_ctx.db.clone(), storage);
        service
            .delete(tenant.id, parse_uuid(&media_id)?)
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = media_id;
        Err(ServerFnError::new(
            "media/delete requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/usage")]
async fn media_usage_native() -> Result<MediaUsageSnapshot, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_core::{Action, Permission, Resource};
        use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::List))?;

        let file_count = rustok_media::entities::media::Entity::find()
            .filter(rustok_media::entities::media::Column::TenantId.eq(tenant.id))
            .count(&app_ctx.db)
            .await
            .map_err(ServerFnError::new)? as i64;

        let total_bytes = rustok_media::entities::media::Entity::find()
            .filter(rustok_media::entities::media::Column::TenantId.eq(tenant.id))
            .select_only()
            .column_as(
                sea_orm::sea_query::Expr::col(rustok_media::entities::media::Column::Size).sum(),
                "total",
            )
            .into_tuple::<Option<i64>>()
            .one(&app_ctx.db)
            .await
            .map_err(ServerFnError::new)?
            .flatten()
            .unwrap_or(0);

        Ok(MediaUsageSnapshot {
            tenant_id: tenant.id.to_string(),
            file_count,
            total_bytes,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new("media/usage requires the `ssr` feature"))
    }
}
