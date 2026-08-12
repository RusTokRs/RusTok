use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
};
use rustok_order::{
    CancelOrderChangeRequest as OwnerCancelOrderChangeRequest,
    CancelOrderReturnRequest as OwnerCancelOrderReturnRequest,
    CreateOrderChangeRequest as OwnerCreateOrderChangeRequest,
    CreateOrderReturnRequest as OwnerCreateOrderReturnRequest,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::super::{CommerceHttpRuntime, common::ensure_permissions};
use crate::dto::{
    CancelOrderChangeInput, CancelOrderReturnInput, CreateOrderChangeInput, CreateOrderReturnInput,
    OrderChangeResponse, OrderReturnResponse,
};

const ADMIN_POST_ORDER_OWNER: &str = "rustok_order.post_order_command";
const ADMIN_POST_ORDER_BOUNDARY: &str = "commerce_admin_post_order_command_http";

fn admin_post_order_command_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    resource_id: Uuid,
    operation: &'static str,
) -> PortContext {
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-post-order:{operation}:{resource_id}"),
    )
    // The owner write policy requires an idempotency identity. These legacy REST
    // endpoints do not expose a caller idempotency key, so this value is admission
    // metadata only and does not claim durable replay/exactly-once semantics.
    .with_idempotency_key(Uuid::new_v4().to_string())
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn map_admin_post_order_port_error(
    tenant_id: Uuid,
    actor_id: Uuid,
    resource_id: Uuid,
    operation: &'static str,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_order_invalid",
            "Order request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_admin_order_state_conflict",
            "Order operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::UNAUTHORIZED,
            "commerce_permission_denied",
            "Permission denied",
            "forbidden",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_order_storage_unavailable",
            "Order storage is temporarily unavailable",
            "temporarily_unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_order_failed",
            "Order operation could not be completed safely",
            "invariant_violation",
        ),
    };

    tracing::error!(
        owner = ADMIN_POST_ORDER_OWNER,
        operation,
        correlation_id = %context.correlation_id,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        resource_id_non_nil = !resource_id.is_nil(),
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_POST_ORDER_BOUNDARY,
        "commerce admin post-order owner command failed"
    );

    HttpError::new(status, code, message)
}

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
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateOrderChangeInput>,
) -> HttpResult<(StatusCode, Json<OrderChangeResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let context =
        admin_post_order_command_context(&tenant, &auth, &request_context, id, "create_change");
    let created = runtime
        .order_post_order_command_port()
        .create_change(
            context.clone(),
            OwnerCreateOrderChangeRequest {
                order_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_admin_post_order_port_error(
                tenant.id,
                auth.user_id,
                id,
                "create_change",
                &context,
                error,
            )
        })?;

    Ok((StatusCode::CREATED, Json(created)))
}

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
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderChangeInput>,
) -> HttpResult<Json<OrderChangeResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let context =
        admin_post_order_command_context(&tenant, &auth, &request_context, id, "cancel_change");
    let item = runtime
        .order_post_order_command_port()
        .cancel_change(
            context.clone(),
            OwnerCancelOrderChangeRequest {
                change_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_admin_post_order_port_error(
                tenant.id,
                auth.user_id,
                id,
                "cancel_change",
                &context,
                error,
            )
        })?;

    Ok(Json(item))
}

#[utoipa::path(
    post,
    path = "/admin/orders/{id}/returns",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = CreateOrderReturnInput,
    responses(
        (status = 201, description = "Return created", body = OrderReturnResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn create_order_return(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateOrderReturnInput>,
) -> HttpResult<(StatusCode, Json<OrderReturnResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let context =
        admin_post_order_command_context(&tenant, &auth, &request_context, id, "create_return");
    let created = runtime
        .order_post_order_command_port()
        .create_return(
            context.clone(),
            OwnerCreateOrderReturnRequest {
                order_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_admin_post_order_port_error(
                tenant.id,
                auth.user_id,
                id,
                "create_return",
                &context,
                error,
            )
        })?;

    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    post,
    path = "/admin/returns/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Return ID")),
    request_body = CancelOrderReturnInput,
    responses(
        (status = 200, description = "Return cancelled", body = OrderReturnResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Return not found")
    )
)]
pub async fn cancel_order_return(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderReturnInput>,
) -> HttpResult<Json<OrderReturnResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let context =
        admin_post_order_command_context(&tenant, &auth, &request_context, id, "cancel_return");
    let item = runtime
        .order_post_order_command_port()
        .cancel_return(
            context.clone(),
            OwnerCancelOrderReturnRequest {
                return_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_admin_post_order_port_error(
                tenant.id,
                auth.user_id,
                id,
                "cancel_return",
                &context,
                error,
            )
        })?;

    Ok(Json(item))
}
