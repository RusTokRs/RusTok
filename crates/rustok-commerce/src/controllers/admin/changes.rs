use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext};
use rustok_order::OrderService;
use rustok_web::HttpResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime,
    super::common::{PaginatedResponse, ensure_permissions},
    ListOrderChangesParams,
};
use crate::services::OrderChangeOrchestrationService;
use crate::{
    ApplyOrderChangeResult, ExchangeDifferenceRefundInput,
    dto::{
        CancelOrderChangeInput, CreateOrderChangeInput, ListOrderChangesInput, OrderChangeResponse,
    },
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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateOrderChangeInput>,
) -> HttpResult<(StatusCode, Json<OrderChangeResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let actor_id = auth.user_id;
    let created = OrderService::new(runtime.db_clone(), runtime.event_bus())
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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListOrderChangesParams>,
) -> HttpResult<Json<PaginatedResponse<OrderChangeResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (items, total) = OrderService::new(runtime.db_clone(), runtime.event_bus())
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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<OrderChangeResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let item = OrderService::new(runtime.db_clone(), runtime.event_bus())
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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AdminApplyOrderChangeInput>,
) -> HttpResult<Json<ApplyOrderChangeResult>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let result = OrderChangeOrchestrationService::new(runtime.db_clone(), runtime.event_bus())
        .with_payment_provider_registry(runtime.payment_provider_registry())
        .apply_order_change(tenant.id, id, input.difference_refund, input.metadata)
        .await
        .map_err(super::map_post_order_orchestration_error)?;

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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderChangeInput>,
) -> HttpResult<Json<OrderChangeResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let item = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .cancel_order_change(tenant.id, id, input)
        .await
        .map_err(super::map_order_error)?;

    Ok(Json(item))
}
