use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{loco::transactional_event_bus_from_context, AuthContext, TenantContext};
use rustok_core::Permission;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    super::common::{ensure_permissions, PaginatedResponse},
    ListOrderChangesParams,
};
use crate::{
    dto::{
        ApplyOrderChangeInput, CancelOrderChangeInput, CreateOrderChangeInput,
        ListOrderChangesInput, OrderChangeResponse,
    },
    ApplyOrderChangeResult, ExchangeDifferenceRefundInput, OrderService,
    PostOrderOrchestrationError, PostOrderOrchestrationService,
};

/// Create admin order change preview
#[utoipa::path(
    post,
    path = "/admin/orders/{id}/changes",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = CreateOrderChangeInput,
    responses(
        (status = 201, description = "Order change created", body = OrderChangeResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn create_order_change(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateOrderChangeInput>,
) -> Result<(StatusCode, Json<OrderChangeResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let actor_id = auth.user_id;
    let created = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .create_order_change(tenant.id, actor_id, id, input)
        .await
        .map_err(super::map_order_error)?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// List admin order changes
#[utoipa::path(
    get,
    path = "/admin/order-changes",
    tag = "admin",
    params(ListOrderChangesParams),
    responses(
        (status = 200, description = "Order changes", body = PaginatedResponse<OrderChangeResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_order_changes(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListOrderChangesParams>,
) -> Result<Json<PaginatedResponse<OrderChangeResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (items, total) =
        OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
            .list_order_changes(
                tenant.id,
                ListOrderChangesInput {
                    page: pagination.page,
                    per_page: pagination.limit(),
                    order_id: params.order_id,
                    status: params.status,
                    change_type: params.change_type,
                },
            )
            .await
            .map_err(super::map_order_error)?;

    Ok(Json(PaginatedResponse {
        data: items,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Show admin order change
#[utoipa::path(
    get,
    path = "/admin/order-changes/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order change ID")),
    responses(
        (status = 200, description = "Order change details", body = OrderChangeResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order change not found")
    )
)]
pub async fn show_order_change(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<OrderChangeResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let item = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .get_order_change(tenant.id, id)
        .await
        .map_err(super::map_order_error)?;

    Ok(Json(item))
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminApplyOrderChangeInput {
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub difference_refund: Option<ExchangeDifferenceRefundInput>,
}

/// Apply admin order change
#[utoipa::path(
    post,
    path = "/admin/order-changes/{id}/apply",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order change ID")),
    request_body = AdminApplyOrderChangeInput,
    responses(
        (status = 200, description = "Order change applied", body = ApplyOrderChangeResult),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order change not found")
    )
)]
pub async fn apply_order_change(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AdminApplyOrderChangeInput>,
) -> Result<Json<ApplyOrderChangeResult>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let db = ctx.db.clone();
    let event_bus = transactional_event_bus_from_context(&ctx);
    let order_service = OrderService::new(db.clone(), event_bus.clone());
    let orchestration_service = PostOrderOrchestrationService::new(db.clone(), event_bus.clone());

    let order_change = order_service
        .get_order_change(tenant.id, id)
        .await
        .map_err(super::map_order_error)?;

    let result = match order_change.change_type.as_str() {
        "exchange" => orchestration_service
            .apply_exchange_order_change(
                tenant.id,
                order_change.order_id,
                id,
                input.difference_refund,
                input.metadata,
            )
            .await
            .map_err(|err| match err {
                PostOrderOrchestrationError::Order(e) => super::map_order_error(e),
                PostOrderOrchestrationError::Payment(e) => super::map_payment_error(e),
                PostOrderOrchestrationError::Validation(msg) => Error::BadRequest(msg),
            })?,
        "claim" => orchestration_service
            .apply_claim_order_change(tenant.id, id, input.metadata)
            .await
            .map_err(|err| match err {
                PostOrderOrchestrationError::Order(e) => super::map_order_error(e),
                PostOrderOrchestrationError::Payment(e) => super::map_payment_error(e),
                PostOrderOrchestrationError::Validation(msg) => Error::BadRequest(msg),
            })?,
        _ => {
            let item = order_service
                .apply_order_change(
                    tenant.id,
                    id,
                    ApplyOrderChangeInput {
                        metadata: input.metadata,
                    },
                )
                .await
                .map_err(super::map_order_error)?;
            ApplyOrderChangeResult {
                order_change: item,
                refund: None,
            }
        }
    };

    Ok(Json(result))
}

/// Cancel admin order change
#[utoipa::path(
    post,
    path = "/admin/order-changes/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order change ID")),
    request_body = CancelOrderChangeInput,
    responses(
        (status = 200, description = "Order change cancelled", body = OrderChangeResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order change not found")
    )
)]
pub async fn cancel_order_change(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderChangeInput>,
) -> Result<Json<OrderChangeResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let item = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .cancel_order_change(tenant.id, id, input)
        .await
        .map_err(super::map_order_error)?;

    Ok(Json(item))
}
