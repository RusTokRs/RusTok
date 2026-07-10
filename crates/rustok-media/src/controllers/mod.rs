use anyhow::Context;
use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
use rustok_storage::StorageService;
use rustok_telemetry::metrics;
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    dto::{MediaItem, MediaTranslationItem, UpsertTranslationInput},
    MediaError, MediaService, UploadInput,
};

#[derive(Clone)]
pub struct MediaHttpRuntime {
    db: sea_orm::DatabaseConnection,
    storage: StorageService,
}

impl MediaHttpRuntime {
    fn db_clone(&self) -> sea_orm::DatabaseConnection {
        self.db.clone()
    }

    fn storage(&self) -> StorageService {
        self.storage.clone()
    }
}

impl MediaHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let storage = runtime
            .shared_get::<StorageService>()
            .context("media HTTP routes require StorageService in HostRuntimeContext")?;
        Ok(Self {
            db: runtime.db_clone(),
            storage,
        })
    }
}

fn media_error(error: MediaError) -> HttpError {
    match error {
        MediaError::NotFound(_) => HttpError::not_found("media_not_found", "Media asset not found"),
        MediaError::Forbidden => HttpError::unauthorized("media_access_denied", "Access denied"),
        MediaError::UnsupportedMimeType(content_type) => HttpError::bad_request(
            "unsupported_media_type",
            format!("Unsupported media type: {content_type}"),
        ),
        MediaError::FileTooLarge { size, max } => HttpError::bad_request(
            "media_file_too_large",
            format!("File too large: {size} bytes (max {max} bytes)"),
        ),
        MediaError::Storage(error) => HttpError::internal(error.to_string()),
        MediaError::Db(error) => HttpError::internal(error.to_string()),
        MediaError::InvalidLocale(locale) => {
            HttpError::bad_request("invalid_media_locale", format!("Invalid locale: {locale}"))
        }
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
    State(runtime): State<MediaHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    mut multipart: Multipart,
) -> HttpResult<(StatusCode, Json<MediaItem>)> {
    let service = MediaService::new(runtime.db_clone(), runtime.storage());

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        HttpError::bad_request("media_bad_request", format!("Multipart error: {error}"))
    })? {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name != "file" {
            continue;
        }

        let file_name = field.file_name().unwrap_or("upload.bin").to_string();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        let data = field.bytes().await.map_err(|error| {
            HttpError::bad_request(
                "media_bad_request",
                format!("Failed to read upload: {error}"),
            )
        })?;

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

    Err(HttpError::bad_request(
        "media_bad_request",
        "No `file` field found in multipart body".to_string(),
    ))
}

/// List media assets for the current tenant.
pub async fn list(
    State(runtime): State<MediaHttpRuntime>,
    tenant: TenantContext,
    _auth: AuthContext,
    Query(params): Query<ListParams>,
) -> HttpResult<Json<MediaListResponse>> {
    let service = MediaService::new(runtime.db_clone(), runtime.storage());
    let limit = params.limit.clamp(1, 100);
    let (items, total) = service
        .list(tenant.id, limit, params.offset)
        .await
        .map_err(media_error)?;

    Ok(Json(MediaListResponse { items, total }))
}

/// Get a single media asset by ID.
pub async fn get_media(
    State(runtime): State<MediaHttpRuntime>,
    tenant: TenantContext,
    _auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<MediaItem>> {
    let service = MediaService::new(runtime.db_clone(), runtime.storage());
    let item = service.get(tenant.id, id).await.map_err(media_error)?;
    Ok(Json(item))
}

/// Delete a media asset.
pub async fn delete_media(
    State(runtime): State<MediaHttpRuntime>,
    tenant: TenantContext,
    _auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    let service = MediaService::new(runtime.db_clone(), runtime.storage());
    service.delete(tenant.id, id).await.map_err(media_error)?;
    metrics::record_media_delete(&tenant.id.to_string());
    Ok(StatusCode::NO_CONTENT)
}

/// Upsert localized media metadata for a locale.
pub async fn upsert_translation(
    State(runtime): State<MediaHttpRuntime>,
    tenant: TenantContext,
    _auth: AuthContext,
    Path((id, locale)): Path<(Uuid, String)>,
    Json(body): Json<UpsertTranslationInput>,
) -> HttpResult<Json<MediaTranslationItem>> {
    let service = MediaService::new(runtime.db_clone(), runtime.storage());
    let translation = service
        .upsert_translation(tenant.id, id, UpsertTranslationInput { locale, ..body })
        .await
        .map_err(media_error)?;

    Ok(Json(translation))
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    use axum::routing::{get, put};

    let state = MediaHttpRuntime::from_host(runtime)?;
    Ok(axum::Router::new()
        .route("/api/media/", get(list).post(upload))
        .route("/api/media/{id}", get(get_media).delete(delete_media))
        .route(
            "/api/media/{id}/translations/{locale}",
            put(upsert_translation),
        )
        .with_state(state))
}
