use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
};
use rustok_fulfillment::FindLatestFulfillmentByOrderProjectionRequest;
use rustok_order::{
    CancelOrderRequest as OwnerCancelOrderRequest, DeliverOrderRequest as OwnerDeliverOrderRequest,
    ListOrderProjectionsRequest, MarkOrderPaidRequest as OwnerMarkOrderPaidRequest,
    ReadOrderProjectionRequest, ShipOrderRequest as OwnerShipOrderRequest,
};
use rustok_payment::LatestPaymentCollectionByOrderRequest;
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime,
    super::common::{PaginatedResponse, ensure_permissions},
    AdminOrderDetailResponse, ListOrdersParams,
};
use crate::dto::{
    CancelOrderInput, DeliverOrderInput, MarkPaidOrderInput, OrderResponse, ShipOrderInput,
};

const ADMIN_ORDER_OWNER: &str = "rustok_order.admin_orders";
const ADMIN_ORDER_BOUNDARY: &str = "commerce_admin_order_http";
const ADMIN_ORDER_DETAIL_PAYMENT_OWNER: &str = "rustok_payment.admin_order_detail";
const ADMIN_ORDER_DETAIL_FULFILLMENT_OWNER: &str = "rustok_fulfillment.admin_order_detail";
const ADMIN_ORDER_DETAIL_BOUNDARY: &str = "commerce_admin_order_detail_http";

fn admin_order_port_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    order_id: Option<Uuid>,
    operation: &'static str,
) -> PortContext {
    let resource_id = order_id.unwrap_or(tenant.id);
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-order:{operation}:{resource_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
    .with_idempotency_key(Uuid::new_v4().to_string());
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn port_error_kind(error: &PortError) -> &'static str {
    match &error.kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    }
}

fn map_order_port_error(
    tenant_id: Uuid,
    actor_id: Uuid,
    order_id: Option<Uuid>,
    operation: &'static str,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_order_invalid",
            "Order request is invalid",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_admin_order_state_conflict",
            "Order operation conflicts with the current state",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::UNAUTHORIZED,
            "commerce_permission_denied",
            "Permission denied",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_order_storage_unavailable",
            "Order storage is temporarily unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_order_failed",
            "Order operation could not be completed safely",
        ),
    };
    let error_kind = port_error_kind(&error);
    tracing::error!(
        owner = ADMIN_ORDER_OWNER,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        order_id_present = order_id.is_some(),
        order_id_non_nil = order_id.map(|value| !value.is_nil()).unwrap_or(false),
        operation,
        correlation_id = %context.correlation_id,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_BOUNDARY,
        "commerce admin order owner port failed"
    );
    HttpError::new(status, code, message)
}

fn map_payment_detail_port_error(
    tenant_id: Uuid,
    order_id: Uuid,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_payment_invalid",
            "Payment request is invalid",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_state_conflict",
            "Payment operation conflicts with the current state",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::UNAUTHORIZED,
            "commerce_permission_denied",
            "Permission denied",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_storage_unavailable",
            "Payment storage is temporarily unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_reconciliation_required",
            "Payment state requires reconciliation before it can be read safely",
        ),
    };
    let error_kind = port_error_kind(&error);
    tracing::error!(
        owner = ADMIN_ORDER_DETAIL_PAYMENT_OWNER,
        tenant_id_non_nil = !tenant_id.is_nil(),
        order_id_non_nil = !order_id.is_nil(),
        operation = "find_latest_payment_collection_by_order",
        correlation_id = %context.correlation_id,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_DETAIL_BOUNDARY,
        "commerce admin order detail payment owner read failed"
    );
    HttpError::new(status, code, message)
}

fn map_fulfillment_detail_port_error(
    tenant_id: Uuid,
    order_id: Uuid,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_fulfillment_invalid",
            "Fulfillment request is invalid",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_admin_fulfillment_state_conflict",
            "Fulfillment operation conflicts with the current state",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::UNAUTHORIZED,
            "commerce_permission_denied",
            "Permission denied",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_fulfillment_storage_unavailable",
            "Fulfillment storage is temporarily unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_fulfillment_failed",
            "Fulfillment state could not be read safely",
        ),
    };
    let error_kind = port_error_kind(&error);
    tracing::error!(
        owner = ADMIN_ORDER_DETAIL_FULFILLMENT_OWNER,
        tenant_id_non_nil = !tenant_id.is_nil(),
        order_id_non_nil = !order_id.is_nil(),
        operation = "find_latest_fulfillment_by_order_projection",
        correlation_id = %context.correlation_id,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_DETAIL_BOUNDARY,
        "commerce admin order detail fulfillment owner read failed"
    );
    HttpError::new(status, code, message)
}

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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<ListOrdersParams>,
) -> HttpResult<Json<PaginatedResponse<OrderResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_LIST],
        "Permission denied: orders:list required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let customer_id = params.customer_id;
    let context = admin_order_port_context(&tenant, &auth, &request_context, None, "list_orders");
    let page = runtime
        .order_read_port()
        .list_order_projections(
            context.clone(),
            ListOrderProjectionsRequest {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status,
                customer_id,
                tenant_default_locale: Some(tenant.default_locale.clone()),
            },
        )
        .await
        .map_err(|error| {
            map_order_port_error(
                tenant.id,
                auth.user_id,
                None,
                "list_orders",
                &context,
                error,
            )
        })?;

    Ok(Json(PaginatedResponse {
        data: page.items,
        meta: super::super::common::PaginationMeta::new(
            pagination.page,
            pagination.limit(),
            page.total,
        ),
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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<AdminOrderDetailResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let order_context =
        admin_order_port_context(&tenant, &auth, &request_context, Some(id), "get_order");
    let order = runtime
        .order_read_port()
        .read_order_projection(
            order_context.clone(),
            ReadOrderProjectionRequest {
                order_id: id,
                tenant_default_locale: Some(tenant.default_locale.clone()),
            },
        )
        .await
        .map_err(|error| {
            map_order_port_error(
                tenant.id,
                auth.user_id,
                Some(id),
                "get_order",
                &order_context,
                error,
            )
        })?;

    let payment_context =
        admin_order_port_context(&tenant, &auth, &request_context, Some(id), "payment_detail");
    let payment_collection = runtime
        .payment_order_read_port()
        .find_latest_collection_by_order(
            payment_context.clone(),
            LatestPaymentCollectionByOrderRequest { order_id: id },
        )
        .await
        .map_err(|error| map_payment_detail_port_error(tenant.id, id, &payment_context, error))?;

    let fulfillment_context = admin_order_port_context(
        &tenant,
        &auth,
        &request_context,
        Some(id),
        "fulfillment_detail",
    );
    let fulfillment = runtime
        .fulfillment_read_port()
        .find_latest_fulfillment_by_order_projection(
            fulfillment_context.clone(),
            FindLatestFulfillmentByOrderProjectionRequest { order_id: id },
        )
        .await
        .map_err(|error| {
            map_fulfillment_detail_port_error(tenant.id, id, &fulfillment_context, error)
        })?;

    Ok(Json(AdminOrderDetailResponse {
        order,
        payment_collection,
        fulfillment,
    }))
}

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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<MarkPaidOrderInput>,
) -> HttpResult<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let context = admin_order_port_context(
        &tenant,
        &auth,
        &request_context,
        Some(id),
        "mark_order_paid",
    );
    let order = runtime
        .order_admin_command_port()
        .mark_paid(
            context.clone(),
            OwnerMarkOrderPaidRequest {
                order_id: id,
                payment_id: input.payment_id,
                payment_method: input.payment_method,
            },
        )
        .await
        .map_err(|error| {
            map_order_port_error(
                tenant.id,
                auth.user_id,
                Some(id),
                "mark_order_paid",
                &context,
                error,
            )
        })?;

    Ok(Json(order))
}

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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ShipOrderInput>,
) -> HttpResult<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let context =
        admin_order_port_context(&tenant, &auth, &request_context, Some(id), "ship_order");
    let order = runtime
        .order_admin_command_port()
        .ship(
            context.clone(),
            OwnerShipOrderRequest {
                order_id: id,
                tracking_number: input.tracking_number,
                carrier: input.carrier,
            },
        )
        .await
        .map_err(|error| {
            map_order_port_error(
                tenant.id,
                auth.user_id,
                Some(id),
                "ship_order",
                &context,
                error,
            )
        })?;

    Ok(Json(order))
}

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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<DeliverOrderInput>,
) -> HttpResult<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let context =
        admin_order_port_context(&tenant, &auth, &request_context, Some(id), "deliver_order");
    let order = runtime
        .order_admin_command_port()
        .deliver(
            context.clone(),
            OwnerDeliverOrderRequest {
                order_id: id,
                delivered_signature: input.delivered_signature,
            },
        )
        .await
        .map_err(|error| {
            map_order_port_error(
                tenant.id,
                auth.user_id,
                Some(id),
                "deliver_order",
                &context,
                error,
            )
        })?;

    Ok(Json(order))
}

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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderInput>,
) -> HttpResult<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let context =
        admin_order_port_context(&tenant, &auth, &request_context, Some(id), "cancel_order");
    let order = runtime
        .order_admin_command_port()
        .cancel(
            context.clone(),
            OwnerCancelOrderRequest {
                order_id: id,
                reason: input.reason,
            },
        )
        .await
        .map_err(|error| {
            map_order_port_error(
                tenant.id,
                auth.user_id,
                Some(id),
                "cancel_order",
                &context,
                error,
            )
        })?;

    Ok(Json(order))
}
