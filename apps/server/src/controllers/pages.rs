use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use loco_rs::Error;
use loco_rs::Result;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use rustok_pages::{
    BlockResponse, BlockService, CreateBlockInput, CreatePageInput, PageResponse, PageService,
    UpdateBlockInput, UpdatePageInput,
};

use crate::extractors::rbac::{
    RequirePagesCreate, RequirePagesDelete, RequirePagesRead, RequirePagesUpdate,
};
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::{common::RequestContext, context::TenantContext};

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct GetPageParams {
    pub slug: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderBlocksInput {
    pub block_ids: Vec<Uuid>,
}

/// Get a page by slug
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequirePagesRead(user): RequirePagesRead,
    request_context: RequestContext,
    Query(params): Query<GetPageParams>,
) -> Result<Json<PageResponse>> {
    let slug = params.slug.unwrap_or_else(|| "home".to_string());
    let locale = params
        .locale
        .unwrap_or_else(|| request_context.locale.clone());

    let service = PageService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let page = service
        .get_by_slug_with_locale_fallback(
            tenant.id,
            user.security_context(),
            &locale,
            &slug,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    match page {
        Some(page) => Ok(Json(page)),
        None => Err(Error::NotFound),
    }
}

/// Create a new page
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequirePagesCreate(user): RequirePagesCreate,
    Json(input): Json<CreatePageInput>,
) -> Result<(StatusCode, Json<PageResponse>)> {
    let service = PageService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let page = service
        .create(tenant.id, user.security_context(), input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequirePagesUpdate(user): RequirePagesUpdate,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdatePageInput>,
) -> Result<Json<PageResponse>> {
    let service = PageService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let page = service
        .update(tenant.id, user.security_context(), id, input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequirePagesDelete(user): RequirePagesDelete,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let service = PageService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .delete(tenant.id, user.security_context(), id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequirePagesUpdate(user): RequirePagesUpdate,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateBlockInput>,
) -> Result<(StatusCode, Json<BlockResponse>)> {
    let service = BlockService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let block = service
        .create(tenant.id, user.security_context(), id, input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequirePagesUpdate(user): RequirePagesUpdate,
    Path(path): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateBlockInput>,
) -> Result<Json<BlockResponse>> {
    let (_, block_id) = path;
    let service = BlockService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let block = service
        .update(tenant.id, user.security_context(), block_id, input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequirePagesDelete(user): RequirePagesDelete,
    Path(path): Path<(Uuid, Uuid)>,
) -> Result<StatusCode> {
    let (_, block_id) = path;
    let service = BlockService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .delete(tenant.id, user.security_context(), block_id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequirePagesUpdate(user): RequirePagesUpdate,
    Path(id): Path<Uuid>,
    Json(input): Json<ReorderBlocksInput>,
) -> Result<StatusCode> {
    let service = BlockService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .reorder(tenant.id, user.security_context(), id, input.block_ids)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api")
        .add("/pages", axum::routing::get(get_page))
        .add("/admin/pages", axum::routing::post(create_page))
        .add(
            "/admin/pages/{id}",
            axum::routing::put(update_page).delete(delete_page),
        )
        .add(
            "/admin/pages/{id}/blocks",
            axum::routing::post(create_block),
        )
        .add(
            "/admin/pages/{page_id}/blocks/{block_id}",
            axum::routing::put(update_block).delete(delete_block),
        )
        .add(
            "/admin/pages/{id}/blocks/reorder",
            axum::routing::post(reorder_blocks),
        )
}
