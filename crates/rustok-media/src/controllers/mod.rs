use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, controller::Routes, Error, Result};
use rustok_api::{AuthContext, TenantContext};
use rustok_storage::StorageService;
use rustok_telemetry::metrics;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    dto::{MediaItem, MediaTranslationItem, UpsertTranslationInput},
    MediaError, MediaService, UploadInput,
};

fn storage_from_ctx(ctx: &AppContext) -> Result<StorageService> {
    ctx.shared_store
        .get::<StorageService>()
        .ok_or(Error::InternalServerError)
}

fn media_error(error: MediaError) -> Error {
    match error {
        MediaError::NotFound(_) => Error::NotFound,
        MediaError::Forbidden => Error::Unauthorized("Access denied".to_string()),
        MediaError::UnsupportedMimeType(content_type) => {
            Error::BadRequest(format!("Unsupported media type: {content_type}"))
        }
        MediaError::FileTooLarge { size, max } => {
            Error::BadRequest(format!("File too large: {size} bytes (max {max} bytes)"))
        }
        MediaError::Storage(error) => Error::Message(error.to_string()),
        MediaError::Db(error) => Error::Message(error.to_string()),
        MediaError::InvalidLocale(locale) => Error::BadRequest(format!("Invalid locale: {locale}")),
    }
}

#[derive(Deserialize)]
pub struct ListParams {
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default)]
    pub offset: u64,
}

fn default_limit() -> u64 {
    20
}

#[derive(Serialize)]
pub struct MediaListResponse {
    pub items: Vec<MediaItem>,
    pub total: u64,
}

/// Upload a media file using multipart/form-data with a `file` field.
pub async fn upload(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<MediaItem>)> {
    let storage = storage_from_ctx(&ctx)?;
    let service = MediaService::new(ctx.db.clone(), storage);

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| Error::BadRequest(format!("Multipart error: {error}")))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name != "file" {
            continue;
        }

        let file_name = field.file_name().unwrap_or("upload.bin").to_string();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        let data = field
            .bytes()
            .await
            .map_err(|error| Error::BadRequest(format!("Failed to read upload: {error}")))?;

        let item = service
            .upload(UploadInput {
                tenant_id: tenant.id,
                uploaded_by: Some(auth.user_id),
                original_name: file_name,
                content_type,
                data,
            })
            .await
            .map_err(media_error)?;

        metrics::record_media_upload(&tenant.id.to_string(), &item.mime_type, item.size as u64);
        return Ok((StatusCode::CREATED, Json(item)));
    }

    Err(Error::BadRequest(
        "No `file` field found in multipart body".to_string(),
    ))
}

/// List media assets for the current tenant.
pub async fn list(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _auth: AuthContext,
    Query(params): Query<ListParams>,
) -> Result<Json<MediaListResponse>> {
    let storage = storage_from_ctx(&ctx)?;
    let service = MediaService::new(ctx.db.clone(), storage);
    let limit = params.limit.clamp(1, 100);
    let (items, total) = service
        .list(tenant.id, limit, params.offset)
        .await
        .map_err(media_error)?;

    Ok(Json(MediaListResponse { items, total }))
}

/// Get a single media asset by ID.
pub async fn get_media(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<MediaItem>> {
    let storage = storage_from_ctx(&ctx)?;
    let service = MediaService::new(ctx.db.clone(), storage);
    let item = service.get(tenant.id, id).await.map_err(media_error)?;
    Ok(Json(item))
}

/// Delete a media asset.
pub async fn delete_media(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let storage = storage_from_ctx(&ctx)?;
    let service = MediaService::new(ctx.db.clone(), storage);
    service.delete(tenant.id, id).await.map_err(media_error)?;
    metrics::record_media_delete(&tenant.id.to_string());
    Ok(StatusCode::NO_CONTENT)
}

/// Upsert localized media metadata for a locale.
pub async fn upsert_translation(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _auth: AuthContext,
    Path((id, locale)): Path<(Uuid, String)>,
    Json(body): Json<UpsertTranslationInput>,
) -> Result<Json<MediaTranslationItem>> {
    let storage = storage_from_ctx(&ctx)?;
    let service = MediaService::new(ctx.db.clone(), storage);
    let translation = service
        .upsert_translation(tenant.id, id, UpsertTranslationInput { locale, ..body })
        .await
        .map_err(media_error)?;

    Ok(Json(translation))
}

pub fn routes() -> Routes {
    use axum::routing::{get, put};

    Routes::new()
        .prefix("api/media")
        .add("/", get(list).post(upload))
        .add("/{id}", get(get_media).delete(delete_media))
        .add("/{id}/translations/{locale}", put(upsert_translation))
}
