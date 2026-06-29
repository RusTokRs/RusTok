use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{AuthContext, TenantContext};
use rustok_core::Permission;
use rustok_fulfillment::FulfillmentService;
use uuid::Uuid;

use super::{
    super::common::{ensure_permissions, PaginatedResponse},
    ListFulfillmentsParams,
};
use crate::{
    dto::{
        CancelFulfillmentInput, CreateFulfillmentInput, DeliverFulfillmentInput,
        FulfillmentResponse, ListFulfillmentsInput, ReopenFulfillmentInput, ReshipFulfillmentInput,
        ShipFulfillmentInput,
    },
    FulfillmentOrchestrationService,
};

/// List admin fulfillments
#[utoipa::path(
    get,
    path = "/admin/fulfillments",
    tag = "admin",
    params(ListFulfillmentsParams),
    responses(
        (status = 200, description = "Fulfillments", body = PaginatedResponse<FulfillmentResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_fulfillments(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListFulfillmentsParams>,
) -> Result<Json<PaginatedResponse<FulfillmentResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (fulfillments, total) = FulfillmentService::new(ctx.db.clone())
        .list_fulfillments(
            tenant.id,
            ListFulfillmentsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status,
                order_id: params.order_id,
                customer_id: params.customer_id,
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(PaginatedResponse {
        data: fulfillments,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Create admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments",
    tag = "admin",
    request_body = CreateFulfillmentInput,
    responses(
        (status = 201, description = "Fulfillment created", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn create_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateFulfillmentInput>,
) -> Result<(StatusCode, Json<FulfillmentResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_CREATE],
        "Permission denied: fulfillments:create required",
    )?;

    let fulfillment = FulfillmentOrchestrationService::new(ctx.db.clone())
        .create_manual_fulfillment(tenant.id, input)
        .await
        .map_err(super::map_fulfillment_orchestration_error)?;

    Ok((StatusCode::CREATED, Json(fulfillment)))
}

/// Show admin fulfillment
#[utoipa::path(
    get,
    path = "/admin/fulfillments/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    responses(
        (status = 200, description = "Fulfillment details", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn show_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .get_fulfillment(tenant.id, id)
        .await
        .map_err(super::map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

/// Ship admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/ship",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = ShipFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment shipped", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn ship_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ShipFulfillmentInput>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .ship_fulfillment(tenant.id, id, input)
        .await
        .map_err(super::map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

/// Deliver admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/deliver",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = DeliverFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment delivered", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn deliver_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<DeliverFulfillmentInput>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .deliver_fulfillment(tenant.id, id, input)
        .await
        .map_err(super::map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

/// Reopen admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/reopen",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = ReopenFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment reopened", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn reopen_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ReopenFulfillmentInput>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .reopen_fulfillment(tenant.id, id, input)
        .await
        .map_err(super::map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

/// Reship admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/reship",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = ReshipFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment marked for reship", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn reship_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ReshipFulfillmentInput>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .reship_fulfillment(tenant.id, id, input)
        .await
        .map_err(super::map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

/// Cancel admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = CancelFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment cancelled", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn cancel_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelFulfillmentInput>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .cancel_fulfillment(tenant.id, id, input)
        .await
        .map_err(super::map_fulfillment_error)?;

    Ok(Json(fulfillment))
}
