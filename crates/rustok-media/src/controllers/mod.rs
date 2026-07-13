use anyhow::Context;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use rustok_api::{
    has_effective_permission, Action, AuthContext, HostRuntimeContext, Permission, Resource,
    TenantContext,
};
use rustok_storage::StorageService;
use rustok_telemetry::metrics;
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    dto::{MediaItem, MediaTranslationItem, UpsertTranslationInput, DEFAULT_MAX_SIZE},
    MediaError, MediaService, UploadInput,
};

const MULTIPART_OVERHEAD_BYTES: u64 = 1024 * 1024;

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
        MediaError::InvalidMediaContent { declared, reason } => HttpError::bad_request(
            "invalid_media_content",
            format!("Invalid {declared} upload: {reason}"),
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

fn require_media_permission(
    tenant: &TenantContext,
    auth: &AuthContext,
    action: Action,
) -> HttpResult<()> {
    if auth.tenant_id != tenant.id {
        return Err(HttpError::unauthorized(
            "media_access_denied",
            "Authenticated user is not bound to the current tenant",
        ));
    }

    let permission = Permission::new(Resource::Media, action);
    if !has_effective_permission(&auth.permissions, &permission) {
        return Err(HttpError::unauthorized(
            "media_access_denied",
            format!("Permission required: {permission}"),
        ));
    }

    Ok(())
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
    require_media_permission(&tenant, &auth, Action::Create)?;
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
    auth: AuthContext,
    Query(params): Query<ListParams>,
) -> HttpResult<Json<MediaListResponse>> {
    require_media_permission(&tenant, &auth, Action::List)?;
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
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<MediaItem>> {
    require_media_permission(&tenant, &auth, Action::Read)?;
    let service = MediaService::new(runtime.db_clone(), runtime.storage());
    let item = service.get(tenant.id, id).await.map_err(media_error)?;
    Ok(Json(item))
}

/// Delete a media asset.
pub async fn delete_media(
    State(runtime): State<MediaHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    require_media_permission(&tenant, &auth, Action::Delete)?;
    let service = MediaService::new(runtime.db_clone(), runtime.storage());
    service.delete(tenant.id, id).await.map_err(media_error)?;
    metrics::record_media_delete(&tenant.id.to_string());
    Ok(StatusCode::NO_CONTENT)
}

/// Upsert localized media metadata for a locale.
pub async fn upsert_translation(
    State(runtime): State<MediaHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path((id, locale)): Path<(Uuid, String)>,
    Json(body): Json<UpsertTranslationInput>,
) -> HttpResult<Json<MediaTranslationItem>> {
    require_media_permission(&tenant, &auth, Action::Update)?;
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
    let body_limit = DEFAULT_MAX_SIZE.saturating_add(MULTIPART_OVERHEAD_BYTES) as usize;
    Ok(axum::Router::new()
        .route("/api/media/", get(list).post(upload))
        .route("/api/media/{id}", get(get_media).delete(delete_media))
        .route(
            "/api/media/{id}/translations/{locale}",
            put(upsert_translation),
        )
        .layer(DefaultBodyLimit::max(body_limit))
        .with_state(state))
}

#[cfg(test)]
mod tests {
    use super::require_media_permission;
    use rustok_api::{Action, AuthContext, Permission, Resource, TenantContext};
    use uuid::Uuid;

    fn tenant(id: Uuid) -> TenantContext {
        TenantContext {
            id,
            name: "Tenant".to_string(),
            slug: "tenant".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn auth(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    #[test]
    fn media_rest_requires_effective_permission_and_matching_tenant() {
        let tenant_id = Uuid::new_v4();
        let manage = Permission::new(Resource::Media, Action::Manage);
        assert!(require_media_permission(
            &tenant(tenant_id),
            &auth(tenant_id, vec![manage]),
            Action::Delete,
        )
        .is_ok());
        assert!(require_media_permission(
            &tenant(tenant_id),
            &auth(tenant_id, Vec::new()),
            Action::Read,
        )
        .is_err());
        assert!(require_media_permission(
            &tenant(tenant_id),
            &auth(Uuid::new_v4(), vec![manage]),
            Action::Read,
        )
        .is_err());
    }
}
