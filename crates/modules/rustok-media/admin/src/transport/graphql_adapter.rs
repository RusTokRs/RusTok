#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use super::ApiError;
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
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

fn required_tenant_id(tenant_slug: &Option<String>) -> Result<String, ApiError> {
    tenant_slug
        .clone()
        .ok_or_else(|| ApiError::Validation("tenant slug is required".to_string()))
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

pub(super) async fn fetch_media_library_graphql(
    page: i32,
    per_page: i32,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaListPayload, ApiError> {
    let tenant_id = required_tenant_id(&tenant_slug)?;
    let clamped_per_page = per_page.clamp(1, 100);
    let response: MediaLibraryResponse = graphql_request(
        MEDIA_QUERY,
        Some(MediaLibraryVariables {
            tenant_id,
            pagination: PaginationInput {
                first: clamped_per_page,
                after: cursor_for_page(page.max(1), clamped_per_page),
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

pub(super) async fn fetch_media_detail_graphql(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Option<MediaListItem>, ApiError> {
    let tenant_id = required_tenant_id(&tenant_slug)?;
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

pub(super) async fn fetch_media_translations_graphql(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<MediaTranslationPayload>, ApiError> {
    let tenant_id = required_tenant_id(&tenant_slug)?;
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

pub(super) async fn fetch_media_usage_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaUsageSnapshot, ApiError> {
    let tenant_id = required_tenant_id(&tenant_slug)?;
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

pub(super) async fn upsert_translation_graphql(
    media_id: String,
    payload: UpsertTranslationPayload,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MediaTranslationPayload, ApiError> {
    let tenant_id = required_tenant_id(&tenant_slug)?;
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

pub(super) async fn delete_media_graphql(
    media_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<bool, ApiError> {
    let tenant_id = required_tenant_id(&tenant_slug)?;
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
