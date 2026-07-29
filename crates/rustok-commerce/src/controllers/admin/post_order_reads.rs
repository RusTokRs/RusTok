use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{
    AuthContext, Permission, PermissionExtractor, PortActor, PortContext, PortError,
    PortErrorKind, RequestContext, TenantContext,
};
use rustok_order::{
    ListOrderChangeProjectionsRequest, ListOrderReturnProjectionsRequest,
    OrderChangeResponse, OrderReturnResponse, ReadOrderChangeProjectionRequest,
    ReadOrderReturnProjectionRequest,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::{
        CommerceHttpRuntime,
        common::{PaginatedResponse, PaginationMeta, ensure_permissions},
    },
    ListOrderChangesParams, ListOrderReturnsParams,
};

const ADMIN_POST_ORDER_OWNER: &str = "rustok_order.admin_post_order_reads";
const ADMIN_POST_ORDER_BOUNDARY: &str = "commerce_admin_post_order_http";

fn admin_post_order_read_context(
    tenant_id: Uuid,
    auth: &AuthContext,
    request_context: &RequestContext,
    resource_id: Option<Uuid>,
    operation: &'static str,
) -> PortContext {
    let resource_id = resource_id.unwrap_or(tenant_id);
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-post-order:{operation}:{resource_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

#[allow(clippy::too_many_arguments)]
fn map_admin_post_order_port_error(
    error: PortError,
    port_context: &PortContext,
    owner_operation: &'static str,
    consumer_operation: &'static str,
    actor_id: Uuid,
    return_id: Option<Uuid>,
    change_id: Option<Uuid>,
    order_id: Option<Uuid>,
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
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_order_failed",
            "Order operation could not be completed safely",
            "invariant_violation",
        ),
    };
    tracing::error!(
        error = ?error,
        owner = ADMIN_POST_ORDER_OWNER,
        owner_operation,
        consumer_operation,
        correlation_id = %port_context.correlation_id,
        tenant_id = %port_context.tenant_id,
        actor_id = %actor_id,
        return_id = ?return_id,
        change_id = ?change_id,
        order_id = ?order_id,
        actor = ?port_context.actor,
        channel = ?port_context.channel,
        locale = %port_context.locale,
        deadline_ms = ?port_context.deadline_ms,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_POST_ORDER_BOUNDARY,
        "commerce admin post-order owner read failed"
    );
    HttpError::new(status, code, message)
}

#[utoipa::path(
    get,
    path = "/admin/returns",
    tag = "admin",
    params(ListOrderReturnsParams),
    responses(
        (status = 200, description = "Returns", body = PaginatedResponse<OrderReturnResponse>),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Permission denied"),
        (status = 503, description = "Order storage unavailable")
    )
)]
pub async fn list_order_returns(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: AuthContext,
    PermissionExtractor(permissions): PermissionExtractor,
    Query(params): Query<ListOrderReturnsParams>,
) -> HttpResult<Json<PaginatedResponse<OrderReturnResponse>>> {
    ensure_permissions(&permissions, &[Permission::ORDER_RETURNS_LIST])?;
    let pagination = params.pagination.unwrap_or_default();
    let order_id = params.order_id;
    let context = admin_post_order_read_context(
        tenant.id,
        &auth,
        &request_context,
        order_id,
        "list_order_returns",
    );
    let page = runtime
        .order_read_port()
        .list_order_return_projections(
            context.clone(),
            ListOrderReturnProjectionsRequest {
                page: pagination.page,
                per_page: pagination.per_page,
                order_id,
                status: params.status,
            },
        )
        .await
        .map_err(|error| {
            map_admin_post_order_port_error(
                error,
                &context,
                "list_order_return_projections",
                "list_order_returns",
                auth.user_id,
                None,
                None,
                order_id,
            )
        })?;

    Ok(Json(PaginatedResponse {
        data: page.items,
        meta: PaginationMeta::new(pagination.page, pagination.limit(), page.total),
    }))
}

#[utoipa::path(
    get,
    path = "/admin/returns/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Return ID")),
    responses(
        (status = 200, description = "Return", body = OrderReturnResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Return not found"),
        (status = 503, description = "Order storage unavailable")
    )
)]
pub async fn show_order_return(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: AuthContext,
    PermissionExtractor(permissions): PermissionExtractor,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<OrderReturnResponse>> {
    ensure_permissions(&permissions, &[Permission::ORDER_RETURNS_READ])?;
    let context = admin_post_order_read_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "show_order_return",
    );
    let item = runtime
        .order_read_port()
        .read_order_return_projection(
            context.clone(),
            ReadOrderReturnProjectionRequest { return_id: id },
        )
        .await
        .map_err(|error| {
            map_admin_post_order_port_error(
                error,
                &context,
                "read_order_return_projection",
                "show_order_return",
                auth.user_id,
                Some(id),
                None,
                None,
            )
        })?;
    Ok(Json(item))
}

#[utoipa::path(
    get,
    path = "/admin/order-changes",
    tag = "admin",
    params(ListOrderChangesParams),
    responses(
        (status = 200, description = "Order changes", body = PaginatedResponse<OrderChangeResponse>),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Permission denied"),
        (status = 503, description = "Order storage unavailable")
    )
)]
pub async fn list_order_changes(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: AuthContext,
    PermissionExtractor(permissions): PermissionExtractor,
    Query(params): Query<ListOrderChangesParams>,
) -> HttpResult<Json<PaginatedResponse<OrderChangeResponse>>> {
    ensure_permissions(&permissions, &[Permission::ORDER_CHANGES_LIST])?;
    let pagination = params.pagination.unwrap_or_default();
    let order_id = params.order_id;
    let context = admin_post_order_read_context(
        tenant.id,
        &auth,
        &request_context,
        order_id,
        "list_order_changes",
    );
    let page = runtime
        .order_read_port()
        .list_order_change_projections(
            context.clone(),
            ListOrderChangeProjectionsRequest {
                page: pagination.page,
                per_page: pagination.per_page,
                order_id,
                status: params.status,
                change_type: params.change_type,
            },
        )
        .await
        .map_err(|error| {
            map_admin_post_order_port_error(
                error,
                &context,
                "list_order_change_projections",
                "list_order_changes",
                auth.user_id,
                None,
                None,
                order_id,
            )
        })?;

    Ok(Json(PaginatedResponse {
        data: page.items,
        meta: PaginationMeta::new(pagination.page, pagination.limit(), page.total),
    }))
}

#[utoipa::path(
    get,
    path = "/admin/order-changes/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order change ID")),
    responses(
        (status = 200, description = "Order change", body = OrderChangeResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Order change not found"),
        (status = 503, description = "Order storage unavailable")
    )
)]
pub async fn show_order_change(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: AuthContext,
    PermissionExtractor(permissions): PermissionExtractor,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<OrderChangeResponse>> {
    ensure_permissions(&permissions, &[Permission::ORDER_CHANGES_READ])?;
    let context = admin_post_order_read_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "show_order_change",
    );
    let item = runtime
        .order_read_port()
        .read_order_change_projection(
            context.clone(),
            ReadOrderChangeProjectionRequest { change_id: id },
        )
        .await
        .map_err(|error| {
            map_admin_post_order_port_error(
                error,
                &context,
                "read_order_change_projection",
                "show_order_change",
                auth.user_id,
                None,
                Some(id),
                None,
            )
        })?;
    Ok(Json(item))
}
