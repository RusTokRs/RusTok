use axum::{
    extract::{Path, Query, State},
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{loco::transactional_event_bus_from_context, AuthContext, TenantContext};
use rustok_core::Permission;
use uuid::Uuid;

use crate::{
    dto::{
        CancelOrderInput, DeliverOrderInput, MarkPaidOrderInput, OrderResponse, ShipOrderInput,
    },
    FulfillmentService, OrderService, PaymentService,
};
use super::{
    super::common::{ensure_permissions, PaginatedResponse},
    AdminOrderDetailResponse, ListOrdersParams,
};

/// Show admin ecommerce order
#[utoipa::path(
    get,
    path = "/admin/orders",
    tag = "admin",
    params(ListOrdersParams),
    responses(
        (status = 200, description = "Orders", body = PaginatedResponse<OrderResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_orders(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Query(params): Query<ListOrdersParams>,
) -> Result<Json<PaginatedResponse<OrderResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_LIST],
        "Permission denied: orders:list required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (orders, total) =
        OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
            .list_orders_with_locale_fallback(
                tenant.id,
                rustok_order::dto::ListOrdersInput {
                    page: pagination.page,
                    per_page: pagination.limit(),
                    status: params.status,
                    customer_id: params.customer_id,
                },
                request_context.locale.as_str(),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(PaginatedResponse {
        data: orders,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

#[utoipa::path(
    get,
    path = "/admin/orders/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    responses(
        (status = 200, description = "Order details", body = AdminOrderDetailResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn show_order(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<AdminOrderDetailResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .get_order_with_locale_fallback(
            tenant.id,
            id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| match err {
            rustok_order::error::OrderError::OrderNotFound(_)
            | rustok_order::error::OrderError::OrderReturnNotFound(_)
            | rustok_order::error::OrderError::OrderChangeNotFound(_) => Error::NotFound,
            other => Error::BadRequest(other.to_string()),
        })?;
    let payment_collection = PaymentService::new(ctx.db.clone())
        .find_latest_collection_by_order(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .find_by_order(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(AdminOrderDetailResponse {
        order,
        payment_collection,
        fulfillment,
    }))
}

/// Mark admin ecommerce order as paid
#[utoipa::path(
    post,
    path = "/admin/orders/{id}/mark-paid",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = MarkPaidOrderInput,
    responses(
        (status = 200, description = "Order marked paid", body = OrderResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn mark_order_paid(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<MarkPaidOrderInput>,
) -> Result<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .mark_paid(
            tenant.id,
            auth.user_id,
            id,
            input.payment_id,
            input.payment_method,
        )
        .await
        .map_err(super::map_order_error)?;

    Ok(Json(order))
}

/// Ship admin ecommerce order
#[utoipa::path(
    post,
    path = "/admin/orders/{id}/ship",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = ShipOrderInput,
    responses(
        (status = 200, description = "Order shipped", body = OrderResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn ship_order(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ShipOrderInput>,
) -> Result<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .ship_order(
            tenant.id,
            auth.user_id,
            id,
            input.tracking_number,
            input.carrier,
        )
        .await
        .map_err(super::map_order_error)?;

    Ok(Json(order))
}

/// Deliver admin ecommerce order
#[utoipa::path(
    post,
    path = "/admin/orders/{id}/deliver",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = DeliverOrderInput,
    responses(
        (status = 200, description = "Order delivered", body = OrderResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn deliver_order(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<DeliverOrderInput>,
) -> Result<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .deliver_order(tenant.id, auth.user_id, id, input.delivered_signature)
        .await
        .map_err(super::map_order_error)?;

    Ok(Json(order))
}

/// Cancel admin ecommerce order
#[utoipa::path(
    post,
    path = "/admin/orders/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = CancelOrderInput,
    responses(
        (status = 200, description = "Order cancelled", body = OrderResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn cancel_order(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderInput>,
) -> Result<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .cancel_order(tenant.id, auth.user_id, id, input.reason)
        .await
        .map_err(super::map_order_error)?;

    Ok(Json(order))
}
