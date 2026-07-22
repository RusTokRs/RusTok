use anyhow::Context;
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rustok_api::HostRuntimeContext;
use rustok_api::{Action, Permission, Resource};
use rustok_api::{AuthContext, RequestContext, TenantContext, has_any_effective_permission};
use rustok_channel::ChannelService;
use rustok_outbox::TransactionalEventBus;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::services::{MENU_LOCALE_NOT_FOUND_ERROR_CODE, MENU_TRANSLATION_INTEGRITY_ERROR_CODE};
use crate::{
    CANNOT_DELETE_PUBLISHED_ERROR_CODE, CreateMenuInput, CreatePageInput, MenuResponse,
    MenuService, PAGE_DOCUMENT_REVISION_CONFLICT, PAGE_PUBLISHED_DOCUMENT_IMMUTABLE,
    PageBuilderArtifactService, PageCacheScope, PageResponse, PageService, PagesCacheReadRuntime,
    PagesError, PatchPageMetadataInput, PublishedLandingArtifact, SavePageDocumentInput,
    page_cache_key,
};

const ARTIFACT_VARY: &str = "X-Tenant-ID, X-Channel-Slug, X-Channel-ID";
const ARTIFACT_CACHE_CONTROL: &str = "public, max-age=60, stale-while-revalidate=300";

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct GetPageParams {
    pub slug: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct GetPageArtifactParams {
    pub locale: String,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct GetMenuParams {
    pub locale: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct CreateMenuParams {
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct CachedPublishedLandingArtifact {
    locale: String,
    artifact_hash: String,
    document_html: String,
    css: String,
}

impl From<PublishedLandingArtifact> for CachedPublishedLandingArtifact {
    fn from(artifact: PublishedLandingArtifact) -> Self {
        Self {
            locale: artifact.locale,
            artifact_hash: artifact.artifact_hash,
            document_html: artifact.document_html,
            css: artifact.css,
        }
    }
}

#[derive(Clone)]
pub struct PagesHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    cache: Option<PagesCacheReadRuntime>,
}

impl PagesHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }

    fn cache(&self) -> Option<&PagesCacheReadRuntime> {
        self.cache.as_ref()
    }

    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .context("pages HTTP routes require TransactionalEventBus in HostRuntimeContext")?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
            cache: runtime.shared_get::<PagesCacheReadRuntime>(),
        })
    }
}

fn page_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

#[utoipa::path(
    get,
    path = "/api/pages",
    tag = "pages",
    params(GetPageParams),
    responses(
        (status = 200, description = "Page content", body = PageResponse),
        (status = 404, description = "Page not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_page(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<GetPageParams>,
) -> HttpResult<Json<PageResponse>> {
    ensure_pages_permission(&auth, Permission::PAGES_READ)?;

    let slug = params.slug.unwrap_or_else(|| "home".to_string());
    let locale = params
        .locale
        .unwrap_or_else(|| request_context.locale.clone());

    let service = PageService::new(runtime.db_clone(), runtime.event_bus());
    let page = service
        .get_by_slug_with_locale_fallback(
            tenant.id,
            page_security(&auth),
            &locale,
            &slug,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(map_pages_error)?;

    match page {
        Some(page) => Ok(Json(page)),
        None => Err(HttpError::not_found("page_not_found", "Page not found")),
    }
}

#[utoipa::path(
    get,
    path = "/api/menus/{id}",
    tag = "pages",
    params(
        ("id" = Uuid, Path, description = "Menu ID"),
        GetMenuParams
    ),
    responses(
        (status = 200, description = "Exact-locale menu", body = MenuResponse),
        (status = 404, description = "Menu or localized menu copy not found"),
        (status = 500, description = "Menu translation integrity failure")
    )
)]
pub async fn get_menu(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<GetMenuParams>,
) -> HttpResult<Json<MenuResponse>> {
    ensure_menu_module_enabled_for_channel(&runtime, &request_context).await?;
    let effective_locale = params
        .locale
        .unwrap_or_else(|| request_context.locale.clone());

    MenuService::new(runtime.db_clone(), runtime.event_bus())
        .get(
            tenant.id,
            rustok_core::SecurityContext::public_read(),
            id,
            &effective_locale,
        )
        .await
        .map(Json)
        .map_err(map_pages_error)
}

#[utoipa::path(
    get,
    path = "/api/pages/{id}/artifact",
    tag = "pages",
    params(
        ("id" = Uuid, Path, description = "Page ID"),
        GetPageArtifactParams
    ),
    responses(
        (status = 200, description = "Published static landing artifact", content_type = "text/html"),
        (status = 304, description = "Artifact has not changed"),
        (status = 404, description = "Published artifact not found")
    )
)]
pub async fn get_page_artifact(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<GetPageArtifactParams>,
    headers: HeaderMap,
) -> HttpResult<Response> {
    ensure_pages_module_enabled_for_channel(&runtime, &request_context).await?;

    let locale = rustok_api::normalize_locale_tag(&params.locale).ok_or_else(|| {
        HttpError::bad_request(
            "invalid_page_artifact_locale",
            "Artifact locale must be a valid normalized locale tag",
        )
    })?;
    let artifact = load_cached_page_artifact(
        &runtime,
        tenant.id,
        id,
        &locale,
        tenant.default_locale.as_str(),
        request_context.channel_slug.as_deref(),
    )
    .await?;

    let etag = format!("\"{}\"", artifact.artifact_hash);
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| etag_matches(value, &etag))
    {
        return Ok(Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(header::ETAG, etag)
            .header(header::CONTENT_LANGUAGE, artifact.locale.as_str())
            .header(header::VARY, ARTIFACT_VARY)
            .header(header::CACHE_CONTROL, ARTIFACT_CACHE_CONTROL)
            .body(Body::empty())
            .expect("artifact 304 response headers are valid"));
    }

    let csp = artifact_content_security_policy(&artifact.css);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CONTENT_LANGUAGE, artifact.locale.as_str())
        .header(header::ETAG, etag)
        .header(header::VARY, ARTIFACT_VARY)
        .header(header::CACHE_CONTROL, ARTIFACT_CACHE_CONTROL)
        .header("content-security-policy", csp)
        .header("referrer-policy", "strict-origin-when-cross-origin")
        .header("x-content-type-options", "nosniff")
        .header("cross-origin-resource-policy", "same-origin")
        .body(Body::from(artifact.document_html))
        .expect("artifact response headers are valid"))
}

async fn load_cached_page_artifact(
    runtime: &PagesHttpRuntime,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    fallback_locale: &str,
    channel_slug: Option<&str>,
) -> HttpResult<CachedPublishedLandingArtifact> {
    let cache_variant = artifact_cache_variant(locale, fallback_locale, channel_slug);
    let cache_key = if let Some(cache) = runtime.cache() {
        match cache.generation_snapshot(tenant_id).await {
            Ok(generations) => match page_cache_key(
                PageCacheScope::Artifact,
                tenant_id,
                page_id,
                generations.artifact,
                cache_variant.as_str(),
            ) {
                Ok(key) => Some(key),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        %tenant_id,
                        %page_id,
                        "Pages artifact cache key rejected"
                    );
                    None
                }
            },
            Err(error) => {
                tracing::warn!(
                    %error,
                    %tenant_id,
                    %page_id,
                    "Pages artifact generation read failed; bypassing cache"
                );
                None
            }
        }
    } else {
        None
    };

    if let (Some(cache), Some(cache_key)) = (runtime.cache(), cache_key.as_ref()) {
        match cache
            .get_json::<CachedPublishedLandingArtifact>(cache_key)
            .await
        {
            Ok(Some(artifact)) => {
                tracing::debug!(%tenant_id, %page_id, "Pages artifact cache hit");
                return Ok(artifact);
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    %error,
                    %tenant_id,
                    %page_id,
                    "Pages artifact cache read failed; loading source artifact"
                );
            }
        }
    }

    let artifact = PageBuilderArtifactService::new(runtime.db_clone())
        .load_public_bound_artifact_with_fallback(
            tenant_id,
            page_id,
            locale,
            Some(fallback_locale),
            channel_slug,
        )
        .await
        .map_err(map_pages_error)?
        .ok_or_else(|| {
            HttpError::not_found(
                "page_artifact_not_found",
                "Published page artifact was not found",
            )
        })?;
    let artifact = CachedPublishedLandingArtifact::from(artifact);

    if let (Some(cache), Some(cache_key)) = (runtime.cache(), cache_key) {
        if let Err(error) = cache.put_json(cache_key, &artifact).await {
            tracing::warn!(
                %error,
                %tenant_id,
                %page_id,
                "Pages artifact cache fill failed"
            );
        }
    }
    Ok(artifact)
}

fn artifact_cache_variant(locale: &str, fallback_locale: &str, channel_slug: Option<&str>) -> String {
    serde_json::to_string(&(
        locale.trim(),
        fallback_locale.trim(),
        channel_slug.unwrap_or_default(),
    ))
    .expect("serializing a tuple of strings cannot fail")
}

#[utoipa::path(
    post,
    path = "/api/admin/pages",
    tag = "pages",
    request_body = CreatePageInput,
    responses(
        (status = 201, description = "Page created", body = PageResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_page(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreatePageInput>,
) -> HttpResult<(StatusCode, Json<PageResponse>)> {
    ensure_pages_permission(&auth, Permission::PAGES_CREATE)?;
    if input.publish {
        ensure_pages_permission(&auth, Permission::new(Resource::Pages, Action::Publish))?;
    }

    let page = PageService::new(runtime.db_clone(), runtime.event_bus())
        .create(tenant.id, page_security(&auth), input)
        .await
        .map_err(map_pages_error)?;
    Ok((StatusCode::CREATED, Json(page)))
}

#[utoipa::path(
    post,
    path = "/api/admin/menus",
    tag = "pages",
    params(CreateMenuParams),
    request_body = CreateMenuInput,
    responses(
        (status = 201, description = "Page created", body = MenuResponse),
        (status = 400, description = "Invalid localized menu input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_menu(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<CreateMenuParams>,
    Json(input): Json<CreateMenuInput>,
) -> HttpResult<(StatusCode, Json<MenuResponse>)> {
    ensure_pages_permission(&auth, Permission::PAGES_CREATE)?;
    let effective_locale = params
        .locale
        .unwrap_or_else(|| request_context.locale.clone());

    let menu = MenuService::new(runtime.db_clone(), runtime.event_bus())
        .create(tenant.id, page_security(&auth), &effective_locale, input)
        .await
        .map_err(map_pages_error)?;
    Ok((StatusCode::CREATED, Json(menu)))
}

#[utoipa::path(
    patch,
    path = "/api/admin/pages/{id}/metadata",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    request_body = PatchPageMetadataInput,
    responses(
        (status = 200, description = "Page metadata updated", body = PageResponse),
        (status = 409, description = "Metadata version conflict"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn patch_page_metadata(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<PatchPageMetadataInput>,
) -> HttpResult<Json<PageResponse>> {
    ensure_pages_permission(&auth, Permission::PAGES_UPDATE)?;
    PageService::new(runtime.db_clone(), runtime.event_bus())
        .patch_metadata(tenant.id, page_security(&auth), id, input)
        .await
        .map(Json)
        .map_err(map_pages_error)
}

#[utoipa::path(
    put,
    path = "/api/admin/pages/{id}/document",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    request_body = SavePageDocumentInput,
    responses(
        (status = 200, description = "Page document saved", body = PageResponse),
        (status = 409, description = "Document revision conflict or published document is immutable"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn save_page_document(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<SavePageDocumentInput>,
) -> HttpResult<Json<PageResponse>> {
    ensure_pages_permission(&auth, Permission::PAGES_UPDATE)?;
    PageService::new(runtime.db_clone(), runtime.event_bus())
        .save_document(tenant.id, page_security(&auth), id, input)
        .await
        .map(Json)
        .map_err(map_pages_error)
}

#[utoipa::path(
    delete,
    path = "/api/admin/pages/{id}",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    responses(
        (status = 204, description = "Page deleted"),
        (status = 409, description = "Published page must be unpublished before deletion"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_page(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    ensure_pages_permission(&auth, Permission::PAGES_DELETE)?;
    PageService::new(runtime.db_clone(), runtime.event_bus())
        .delete(tenant.id, page_security(&auth), id)
        .await
        .map_err(map_pages_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = PagesHttpRuntime::from_host(runtime)?;
    Ok(axum::Router::new()
        .route("/api/pages", axum::routing::get(get_page))
        .route("/api/menus/{id}", axum::routing::get(get_menu))
        .route(
            "/api/pages/{id}/artifact",
            axum::routing::get(get_page_artifact),
        )
        .route("/api/admin/pages", axum::routing::post(create_page))
        .route("/api/admin/menus", axum::routing::post(create_menu))
        .route("/api/admin/pages/{id}", axum::routing::delete(delete_page))
        .route(
            "/api/admin/pages/{id}/metadata",
            axum::routing::patch(patch_page_metadata),
        )
        .route(
            "/api/admin/pages/{id}/document",
            axum::routing::put(save_page_document),
        )
        .with_state(state))
}

async fn ensure_menu_module_enabled_for_channel(
    runtime: &PagesHttpRuntime,
    request_context: &RequestContext,
) -> HttpResult<()> {
    let Some(channel_id) = request_context.channel_id else {
        return Ok(());
    };
    let enabled = ChannelService::new(runtime.db_clone())
        .is_module_enabled(channel_id, "pages")
        .await
        .map_err(|error| {
            tracing::error!(
                channel_id = %channel_id,
                error = %error,
                "failed to evaluate Pages module availability for menu delivery"
            );
            HttpError::internal("Unable to evaluate channel availability")
        })?;
    if enabled {
        Ok(())
    } else {
        Err(HttpError::not_found("menu_not_found", "Menu was not found"))
    }
}

async fn ensure_pages_module_enabled_for_channel(
    runtime: &PagesHttpRuntime,
    request_context: &RequestContext,
) -> HttpResult<()> {
    let Some(channel_id) = request_context.channel_id else {
        return Ok(());
    };
    let enabled = ChannelService::new(runtime.db_clone())
        .is_module_enabled(channel_id, "pages")
        .await
        .map_err(|error| {
            tracing::error!(
                channel_id = %channel_id,
                error = %error,
                "failed to evaluate Pages module availability for artifact delivery"
            );
            HttpError::internal("Unable to evaluate channel availability")
        })?;
    if enabled {
        Ok(())
    } else {
        Err(HttpError::not_found(
            "page_artifact_not_found",
            "Published page artifact was not found",
        ))
    }
}

fn etag_matches(if_none_match: &str, etag: &str) -> bool {
    if_none_match
        .split(',')
        .map(str::trim)
        .any(|candidate| candidate == "*" || candidate == etag)
}

fn artifact_content_security_policy(css: &str) -> String {
    let style_hash = BASE64.encode(Sha256::digest(css.as_bytes()));
    format!(
        "default-src 'none'; style-src 'sha256-{style_hash}'; img-src 'self' https: data:; media-src 'self' https:; font-src 'self' https: data:; connect-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'self'"
    )
}

fn map_pages_error(error: PagesError) -> HttpError {
    let message = error.to_string();
    match error {
        PagesError::VersionConflict { .. } => HttpError::new(
            StatusCode::CONFLICT,
            "page_metadata_version_conflict",
            message,
        ),
        PagesError::CannotDeletePublished => HttpError::new(
            StatusCode::CONFLICT,
            CANNOT_DELETE_PUBLISHED_ERROR_CODE.to_ascii_lowercase(),
            message,
        ),
        PagesError::Rich(rich)
            if rich.error_code.as_deref() == Some(PAGE_DOCUMENT_REVISION_CONFLICT) =>
        {
            HttpError::new(
                StatusCode::CONFLICT,
                "page_document_revision_conflict",
                message,
            )
        }
        PagesError::Rich(rich)
            if rich.error_code.as_deref() == Some(PAGE_PUBLISHED_DOCUMENT_IMMUTABLE) =>
        {
            HttpError::new(
                StatusCode::CONFLICT,
                "page_published_document_immutable",
                message,
            )
        }
        PagesError::Rich(rich)
            if rich.error_code.as_deref() == Some(MENU_LOCALE_NOT_FOUND_ERROR_CODE) =>
        {
            HttpError::not_found("menu_locale_not_found", message)
        }
        PagesError::Rich(rich)
            if rich.error_code.as_deref() == Some(MENU_TRANSLATION_INTEGRITY_ERROR_CODE) =>
        {
            HttpError::internal(message)
        }
        PagesError::PageNotFound(_) => HttpError::not_found("page_not_found", message),
        PagesError::MenuNotFound(_) => HttpError::not_found("menu_not_found", message),
        PagesError::Forbidden(_) => HttpError::forbidden("pages_permission_denied", message),
        PagesError::Database(_) | PagesError::Tenant(_) | PagesError::ArtifactIntegrity(_) => {
            HttpError::internal(message)
        }
        _ => HttpError::bad_request("pages_operation_failed", message),
    }
}

fn ensure_pages_permission(auth: &AuthContext, permission: Permission) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(HttpError::forbidden(
            "pages_permission_denied",
            "Permission denied: pages:* required",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ARTIFACT_VARY, artifact_cache_variant, artifact_content_security_policy, etag_matches,
    };

    #[test]
    fn artifact_csp_hashes_exact_css() {
        let first = artifact_content_security_policy("body{margin:0}");
        let second = artifact_content_security_policy("body{margin:1px}");
        assert!(first.contains("style-src 'sha256-"));
        assert_ne!(first, second);
        assert!(!first.contains("unsafe-inline"));
    }

    #[test]
    fn artifact_csp_allows_renderer_media_sources() {
        let csp = artifact_content_security_policy("");
        assert!(csp.contains("media-src 'self' https:"));
    }

    #[test]
    fn artifact_cache_varies_by_tenant_and_channel_context() {
        assert!(ARTIFACT_VARY.contains("X-Tenant-ID"));
        assert!(ARTIFACT_VARY.contains("X-Channel-Slug"));
        assert!(ARTIFACT_VARY.contains("X-Channel-ID"));
        let base = artifact_cache_variant("en", "en", Some("web"));
        assert_ne!(base, artifact_cache_variant("fr", "en", Some("web")));
        assert_ne!(base, artifact_cache_variant("en", "ru", Some("web")));
        assert_ne!(base, artifact_cache_variant("en", "en", Some("mobile")));
    }

    #[test]
    fn etag_matching_accepts_lists_and_wildcard() {
        assert!(etag_matches(r#""a", "b""#, r#""b""#));
        assert!(etag_matches("*", r#""b""#));
        assert!(!etag_matches(r#""a""#, r#""b""#));
    }
}
