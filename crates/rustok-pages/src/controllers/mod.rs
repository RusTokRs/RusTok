use anyhow::Context;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use rustok_api::HostRuntimeContext;
use rustok_api::{has_any_effective_permission, AuthContext, RequestContext, TenantContext};
use rustok_api::{Action, Permission, Resource};
use rustok_outbox::TransactionalEventBus;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    BlockResponse, BlockService, CreateBlockInput, CreatePageInput, PageResponse, PageService,
    UpdateBlockInput, UpdatePageInput,
};

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct GetPageParams {
    pub slug: Option<String>,
    pub locale: Option<String>,
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
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            &locale,
            &slug,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| HttpError::bad_request("pages_operation_failed", err.to_string()))?;

    match page {
        Some(page) => Ok(Json(page)),
        None => Err(HttpError::not_found("page_not_found", "Page not found")),
    }
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
        .create(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            input,
        )
        .await
        .map_err(|err| HttpError::bad_request("pages_operation_failed", err.to_string()))?;
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
        .update(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            id,
            input,
        )
        .await
        .map_err(|err| HttpError::bad_request("pages_operation_failed", err.to_string()))?;
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
        .delete(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            id,
        )
        .await
        .map_err(|err| HttpError::bad_request("pages_operation_failed", err.to_string()))?;
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
        .create(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            id,
            input,
        )
        .await
        .map_err(|err| HttpError::bad_request("pages_operation_failed", err.to_string()))?;
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
        .update(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            block_id,
            input,
        )
        .await
        .map_err(|err| HttpError::bad_request("pages_operation_failed", err.to_string()))?;
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
        .delete(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            block_id,
        )
        .await
        .map_err(|err| HttpError::bad_request("pages_operation_failed", err.to_string()))?;
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
        .reorder(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            id,
            input.block_ids,
        )
        .await
        .map_err(|err| HttpError::bad_request("pages_operation_failed", err.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = PagesHttpRuntime::from_host(runtime)?;
    Ok(axum::Router::new()
        .route("/api/pages", axum::routing::get(get_page))
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

fn ensure_pages_permission(auth: &AuthContext, permission: Permission) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(HttpError::unauthorized(
            "pages_permission_denied",
            "Permission denied: pages:* required",
        ));
    }

    Ok(())
}
