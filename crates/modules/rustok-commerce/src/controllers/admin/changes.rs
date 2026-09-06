use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
};
use rustok_order::OrderService;
use rustok_order::error::OrderError;
use rustok_payment::error::PaymentError;
use rustok_web::{HttpError, HttpResult};
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
    ApplyOrderChangeResult, ExchangeDifferenceRefundInput, OrderChangeOrchestrationError,
    PaymentOrchestrationError, PostOrderOrchestrationError,
    dto::{
        CancelOrderChangeInput, CreateOrderChangeInput, ListOrderChangesInput, OrderChangeResponse,
    },
};

const ADMIN_ORDER_CHANGE_OWNER: &str = "rustok_order.admin_changes";
const ADMIN_ORDER_CHANGE_ORCHESTRATION_OWNER: &str =
    "rustok_commerce.admin_order_change_orchestration";
const ADMIN_ORDER_CHANGE_BOUNDARY: &str = "commerce_admin_order_change_http";

type AdminOrderChangeHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

struct AdminOrderChangeErrorContext {
    tenant_id: Uuid,
    order_id: Option<Uuid>,
    order_change_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminOrderChangeErrorContext {
    fn new(
        tenant_id: Uuid,
        order_id: Option<Uuid>,
        order_change_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            order_id,
            order_change_id,
            operation,
        }
    }
}

struct AdminOrderChangeOrchestrationErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    order_id: Option<Uuid>,
    order_change_id: Option<Uuid>,
    payment_collection_id: Option<Uuid>,
    payment_id: Option<Uuid>,
    refund_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminOrderChangeOrchestrationErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        order_change_id: Uuid,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            order_id: None,
            order_change_id: Some(order_change_id),
            payment_collection_id: None,
            payment_id: None,
            refund_id: None,
            operation,
        }
    }
}

fn admin_order_change_read_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    change_id: Uuid,
) -> PortContext {
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-order-change:read:{change_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn admin_order_change_apply_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    change_id: Uuid,
) -> PortContext {
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-order-change:apply:{change_id}"),
    )
    // This legacy route has no caller idempotency header. The fresh identity only
    // satisfies owner write admission and does not claim durable replay semantics.
    .with_idempotency_key(Uuid::new_v4().to_string())
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn admin_order_change_order_error_policy(error: &OrderError) -> AdminOrderChangeHttpPolicy {
    match error {
        OrderError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_order_invalid",
            "Order request is invalid",
            "validation",
        ),
        OrderError::OrderNotFound(_)
        | OrderError::OrderReturnNotFound(_)
        | OrderError::OrderChangeNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        OrderError::InvalidTransition { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_order_state_conflict",
            "Order operation conflicts with the current state",
            "state_conflict",
        ),
        OrderError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_order_storage_unavailable",
            "Order storage is temporarily unavailable",
            "database",
        ),
        OrderError::Core(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_order_failed",
            "Order operation could not be completed safely",
            "core",
        ),
    }
}

fn admin_order_change_port_error_policy(error: &PortError) -> AdminOrderChangeHttpPolicy {
    match &error.kind {
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
    }
}

fn admin_order_change_payment_error_policy(error: &PaymentError) -> AdminOrderChangeHttpPolicy {
    match error {
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PaymentError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_payment_invalid",
            "Payment request is invalid",
            "validation",
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_state_conflict",
            "Payment operation conflicts with the current state",
            "state_conflict",
        ),
        PaymentError::ProviderUnavailable { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_provider_unavailable",
            "Payment provider is temporarily unavailable",
            "provider_unavailable",
        ),
        PaymentError::ProviderInvalidResponse { .. } => (
            StatusCode::BAD_GATEWAY,
            "commerce_admin_payment_provider_invalid_response",
            "Payment provider returned an invalid response; reconciliation may be required",
            "provider_invalid_response",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_reconciliation_required",
            "Payment provider outcome is unknown and requires reconciliation",
            "provider_outcome_unknown",
        ),
        PaymentError::ProviderConfiguration { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_provider_not_configured",
            "Payment provider is not configured for this tenant",
            "provider_configuration",
        ),
        PaymentError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_storage_unavailable",
            "Payment storage is temporarily unavailable",
            "database",
        ),
    }
}

fn admin_order_change_reserved_refund_error_policy(
    error: &PaymentError,
) -> AdminOrderChangeHttpPolicy {
    match error {
        PaymentError::ProviderOutcomeUnknown { .. }
        | PaymentError::ProviderInvalidResponse { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_refund_reconciliation_required",
            "Refund remains reserved while the provider outcome is reconciled",
            "refund_reconciliation_required",
        ),
        PaymentError::ProviderUnavailable { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_refund_provider_unavailable",
            "Refund remains reserved and the provider operation may be retried safely",
            "refund_provider_unavailable",
        ),
        error => admin_order_change_payment_error_policy(error),
    }
}

fn adopt_order_change_order_error_identity(
    context: &mut AdminOrderChangeOrchestrationErrorContext,
    error: &OrderError,
) {
    match error {
        OrderError::OrderNotFound(id) => context.order_id = Some(*id),
        OrderError::OrderChangeNotFound(id) => context.order_change_id = Some(*id),
        _ => {}
    }
}

fn adopt_order_change_payment_error_identity(
    context: &mut AdminOrderChangeOrchestrationErrorContext,
    error: &PaymentError,
) {
    match error {
        PaymentError::PaymentCollectionNotFound(id) => context.payment_collection_id = Some(*id),
        PaymentError::PaymentNotFound(id) => context.payment_id = Some(*id),
        PaymentError::RefundNotFound(id) => context.refund_id = Some(*id),
        _ => {}
    }
}

fn map_admin_order_change_error(
    mut context: AdminOrderChangeErrorContext,
    error: OrderError,
) -> HttpError {
    match &error {
        OrderError::OrderNotFound(id) => context.order_id = Some(*id),
        OrderError::OrderChangeNotFound(id) => context.order_change_id = Some(*id),
        _ => {}
    }
    let (status, code, message, error_kind) = admin_order_change_order_error_policy(&error);
    tracing::error!(
        error = ?error,
        owner = ADMIN_ORDER_CHANGE_OWNER,
        tenant_id = %context.tenant_id,
        order_id = ?context.order_id,
        order_change_id = ?context.order_change_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_CHANGE_BOUNDARY,
        "commerce admin order change owner operation failed"
    );
    HttpError::new(status, code, message)
}

fn map_admin_order_change_port_error(
    context: &PortContext,
    actor_id: Uuid,
    order_change_id: Uuid,
    owner_operation: &'static str,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = admin_order_change_port_error_policy(&error);
    tracing::error!(
        owner = "rustok_order",
        owner_operation,
        consumer_operation = "apply_order_change",
        correlation_id = %context.correlation_id,
        tenant_id_non_empty = !context.tenant_id.is_empty(),
        actor_id_non_nil = !actor_id.is_nil(),
        order_change_id_non_nil = !order_change_id.is_nil(),
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_CHANGE_BOUNDARY,
        "commerce admin order-change owner port failed with bounded diagnostics"
    );
    HttpError::new(status, code, message)
}

fn map_admin_order_change_orchestration_error(
    mut context: AdminOrderChangeOrchestrationErrorContext,
    error: PostOrderOrchestrationError,
) -> HttpError {
    let (status, code, message, error_kind, source_owner) = match &error {
        PostOrderOrchestrationError::Order(source) => {
            adopt_order_change_order_error_identity(&mut context, source);
            let (status, code, message, error_kind) = admin_order_change_order_error_policy(source);
            (status, code, message, error_kind, "rustok_order")
        }
        PostOrderOrchestrationError::Payment(source) => {
            adopt_order_change_payment_error_identity(&mut context, source);
            let (status, code, message, error_kind) =
                admin_order_change_payment_error_policy(source);
            (status, code, message, error_kind, "rustok_payment")
        }
        PostOrderOrchestrationError::PaymentOrchestration(source) => match source {
            PaymentOrchestrationError::Provider(source)
            | PaymentOrchestrationError::Payment(source) => {
                adopt_order_change_payment_error_identity(&mut context, source);
                let (status, code, message, error_kind) =
                    admin_order_change_payment_error_policy(source);
                (status, code, message, error_kind, "rustok_payment")
            }
            PaymentOrchestrationError::ProviderAfterRefundReservation { refund_id, source } => {
                context.refund_id = Some(*refund_id);
                let (status, code, message, error_kind) =
                    admin_order_change_reserved_refund_error_policy(source);
                (status, code, message, error_kind, "rustok_payment")
            }
        },
        PostOrderOrchestrationError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_post_order_invalid",
            "Post-order request is invalid",
            "validation",
            "rustok_commerce",
        ),
    };
    tracing::error!(
        error = ?error,
        owner = ADMIN_ORDER_CHANGE_ORCHESTRATION_OWNER,
        source_owner,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        order_id = ?context.order_id,
        order_change_id = ?context.order_change_id,
        payment_collection_id = ?context.payment_collection_id,
        payment_id = ?context.payment_id,
        refund_id = ?context.refund_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_CHANGE_BOUNDARY,
        "commerce admin order change orchestration failed"
    );
    HttpError::new(status, code, message)
}

fn map_admin_order_change_apply_error(
    context: AdminOrderChangeOrchestrationErrorContext,
    read_context: &PortContext,
    command_context: &PortContext,
    error: OrderChangeOrchestrationError,
) -> HttpError {
    match error {
        OrderChangeOrchestrationError::OrderRead(source) => map_admin_order_change_port_error(
            read_context,
            context.actor_id,
            context.order_change_id.unwrap_or_default(),
            "read_order_change_projection",
            source,
        ),
        OrderChangeOrchestrationError::OrderCommand(source) => map_admin_order_change_port_error(
            command_context,
            context.actor_id,
            context.order_change_id.unwrap_or_default(),
            "apply_change",
            source,
        ),
        OrderChangeOrchestrationError::PostOrder(source) => {
            map_admin_order_change_orchestration_error(context, source)
        }
    }
}

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
        .map_err(|error| {
            map_admin_order_change_error(
                AdminOrderChangeErrorContext::new(tenant.id, Some(id), None, "create_order_change"),
                error,
            )
        })?;

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
    let order_id = params.order_id;
    let (items, total) = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .list_order_changes(
            tenant.id,
            ListOrderChangesInput {
                page: pagination.page,
                per_page: pagination.limit(),
                order_id,
                status: params.status,
                change_type: params.change_type,
            },
        )
        .await
        .map_err(|error| {
            map_admin_order_change_error(
                AdminOrderChangeErrorContext::new(tenant.id, order_id, None, "list_order_changes"),
                error,
            )
        })?;

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
        .map_err(|error| {
            map_admin_order_change_error(
                AdminOrderChangeErrorContext::new(tenant.id, None, Some(id), "get_order_change"),
                error,
            )
        })?;

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
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AdminApplyOrderChangeInput>,
) -> HttpResult<Json<ApplyOrderChangeResult>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let actor_id = auth.user_id;
    let read_context = admin_order_change_read_context(&tenant, &auth, &request_context, id);
    let command_context = admin_order_change_apply_context(&tenant, &auth, &request_context, id);
    let result = OrderChangeOrchestrationService::from_order_ports(
        runtime.db_clone(),
        runtime.event_bus(),
        runtime.order_read_port(),
        runtime.order_post_order_command_port(),
    )
    .with_payment_provider_registry(runtime.payment_provider_registry())
    .apply_order_change_with_owner_ports(
        tenant.id,
        id,
        read_context.clone(),
        command_context.clone(),
        input.difference_refund,
        input.metadata,
    )
    .await
    .map_err(|error| {
        map_admin_order_change_apply_error(
            AdminOrderChangeOrchestrationErrorContext::new(
                tenant.id,
                actor_id,
                id,
                "apply_order_change",
            ),
            &read_context,
            &command_context,
            error,
        )
    })?;

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
        .map_err(|error| {
            map_admin_order_change_error(
                AdminOrderChangeErrorContext::new(tenant.id, None, Some(id), "cancel_order_change"),
                error,
            )
        })?;

    Ok(Json(item))
}
