use anyhow::Context;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    Json,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rustok_api::HostRuntimeContext;
use rustok_api::{has_any_effective_permission, AuthContext, RequestContext, TenantContext};
use rustok_api::{Action, Permission, Resource};
use rustok_channel::ChannelService;
use rustok_outbox::TransactionalEventBus;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    BlockResponse, BlockService, CreateBlockInput, CreatePageInput, PageBuilderArtifactService,
    PageResponse, PageService, PagesError, UpdateBlockInput, UpdatePageInput,
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderBlocksInput {
    pub block_ids: Vec<Uuid>,
}

#[derive(Clone)]
pub struct PagesHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl PagesHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }
}

impl PagesHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .context("pages HTTP routes require TransactionalEventBus in HostRuntimeContext")?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
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
    let artifact = PageBuilderArtifactService::new(runtime.db_clone())
        .load_public_bound_artifact_with_fallback(
            tenant.id,
            id,
            &locale,
            Some(tenant.default_locale.as_str()),
            request_context.channel_slug.as_deref(),
        )
        .await
        .map_err(map_pages_error)?
        .ok_or_else(|| {
            HttpError::not_found(
                "page_artifact_not_found",
                "Published page artifact was not found",
            )
        })?;

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

    let service = PageService::new(runtime.db_clone(), runtime.event_bus());
    let page = service
        .create(tenant.id, page_security(&auth), input)
        .await
        .map_err(map_pages_error)?;
    Ok((StatusCode::CREATED, Json(page)))
}

#[utoipa::path(
    put,
    path = "/api/admin/pages/{id}",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    request_body = UpdatePageInput,
    responses(
        (status = 200, description = "Page updated", body = PageResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_page(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdatePageInput>,
) -> HttpResult<Json<PageResponse>> {
    ensure_pages_permission(&auth, Permission::PAGES_UPDATE)?;
    if input.status.is_some() {
        ensure_pages_permission(&auth, Permission::new(Resource::Pages, Action::Publish))?;
    }

    let service = PageService::new(runtime.db_clone(), runtime.event_bus());
    let page = service
        .update(tenant.id, page_security(&auth), id, input)
        .await
        .map_err(map_pages_error)?;
    Ok(Json(page))
}

#[utoipa::path(
    delete,
    path = "/api/admin/pages/{id}",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    responses(
        (status = 204, description = "Page deleted"),
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

    let service = PageService::new(runtime.db_clone(), runtime.event_bus());
    service
        .delete(tenant.id, page_security(&auth), id)
        .await
        .map_err(map_pages_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/admin/pages/{id}/blocks",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    request_body = CreateBlockInput,
    responses(
        (status = 201, description = "Block created", body = BlockResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_block(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateBlockInput>,
) -> HttpResult<(StatusCode, Json<BlockResponse>)> {
    ensure_pages_permission(&auth, Permission::PAGES_UPDATE)?;

    let service = BlockService::new(runtime.db_clone(), runtime.event_bus());
    let block = service
        .create(tenant.id, page_security(&auth), id, input)
        .await
        .map_err(map_pages_error)?;
    Ok((StatusCode::CREATED, Json(block)))
}

#[utoipa::path(
    put,
    path = "/api/admin/pages/{page_id}/blocks/{block_id}",
    tag = "pages",
    params(
        ("page_id" = Uuid, Path, description = "Page ID"),
        ("block_id" = Uuid, Path, description = "Block ID")
    ),
    request_body = UpdateBlockInput,
    responses(
        (status = 200, description = "Block updated", body = BlockResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_block(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(path): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateBlockInput>,
) -> HttpResult<Json<BlockResponse>> {
    ensure_pages_permission(&auth, Permission::PAGES_UPDATE)?;

    let (_, block_id) = path;
    let service = BlockService::new(runtime.db_clone(), runtime.event_bus());
    let block = service
        .update(tenant.id, page_security(&auth), block_id, input)
        .await
        .map_err(map_pages_error)?;
    Ok(Json(block))
}

#[utoipa::path(
    delete,
    path = "/api/admin/pages/{page_id}/blocks/{block_id}",
    tag = "pages",
    params(
        ("page_id" = Uuid, Path, description = "Page ID"),
        ("block_id" = Uuid, Path, description = "Block ID")
    ),
    responses(
        (status = 204, description = "Block deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_block(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(path): Path<(Uuid, Uuid)>,
) -> HttpResult<StatusCode> {
    ensure_pages_permission(&auth, Permission::PAGES_DELETE)?;

    let (_, block_id) = path;
    let service = BlockService::new(runtime.db_clone(), runtime.event_bus());
    service
        .delete(tenant.id, page_security(&auth), block_id)
        .await
        .map_err(map_pages_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/admin/pages/{id}/blocks/reorder",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    request_body = ReorderBlocksInput,
    responses(
        (status = 204, description = "Blocks reordered"),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn reorder_blocks(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ReorderBlocksInput>,
) -> HttpResult<StatusCode> {
    ensure_pages_permission(&auth, Permission::PAGES_UPDATE)?;

    let service = BlockService::new(runtime.db_clone(), runtime.event_bus());
    service
        .reorder(tenant.id, page_security(&auth), id, input.block_ids)
        .await
        .map_err(map_pages_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = PagesHttpRuntime::from_host(runtime)?;
    Ok(axum::Router::new()
        .route("/api/pages", axum::routing::get(get_page))
        .route(
            "/api/pages/{id}/artifact",
            axum::routing::get(get_page_artifact),
        )
        .route("/api/admin/pages", axum::routing::post(create_page))
        .route(
            "/api/admin/pages/{id}",
            axum::routing::put(update_page).delete(delete_page),
        )
        .route(
            "/api/admin/pages/{id}/blocks",
            axum::routing::post(create_block),
        )
        .route(
            "/api/admin/pages/{page_id}/blocks/{block_id}",
            axum::routing::put(update_block).delete(delete_block),
        )
        .route(
            "/api/admin/pages/{id}/blocks/reorder",
            axum::routing::post(reorder_blocks),
        )
        .with_state(state))
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
        PagesError::VersionConflict { .. } => {
            HttpError::new(StatusCode::CONFLICT, "page_version_conflict", message)
        }
        PagesError::PageNotFound(_) => HttpError::not_found("page_not_found", message),
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
    use super::{artifact_content_security_policy, etag_matches, ARTIFACT_VARY};

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
    }

    #[test]
    fn etag_matching_accepts_lists_and_wildcard() {
        assert!(etag_matches(r#""a", "b""#, r#""b""#));
        assert!(etag_matches("*", r#""b""#));
        assert!(!etag_matches(r#""a""#, r#""b""#));
    }
}
