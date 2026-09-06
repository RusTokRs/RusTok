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
use uuid::Uuid;

use super::{
    super::{
        CommerceHttpRuntime,
        common::{PaginatedResponse, ensure_permissions},
    },
    AdminCompleteOrderReturnInput, ListOrderReturnsParams,
};
use crate::{
    CompleteReturnClaimInput, CompleteReturnExchangeInput, CompleteReturnRefundInput,
    CompleteReturnResolutionInput, CreateReturnDecisionInput, PaymentOrchestrationError,
    PostOrderOrchestrationError, ReturnCompletionOrchestrationService,
    ReturnDecisionOwnerOrchestrationError, ReturnDecisionOwnerOrchestrationService,
    ReturnDecisionResponse,
    dto::{
        CancelOrderReturnInput, CreateOrderReturnInput, ListOrderReturnsInput, OrderReturnResponse,
    },
};

const ADMIN_ORDER_RETURN_OWNER: &str = "rustok_order.admin_returns";
const ADMIN_ORDER_RETURN_ORCHESTRATION_OWNER: &str =
    "rustok_commerce.admin_order_return_orchestration";
const ADMIN_ORDER_RETURN_BOUNDARY: &str = "commerce_admin_order_return_http";

type AdminOrderReturnHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

struct AdminOrderReturnErrorContext {
    tenant_id: Uuid,
    order_id: Option<Uuid>,
    return_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminOrderReturnErrorContext {
    fn new(
        tenant_id: Uuid,
        order_id: Option<Uuid>,
        return_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            order_id,
            return_id,
            operation,
        }
    }
}

struct AdminOrderReturnOrchestrationErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    order_id: Option<Uuid>,
    return_id: Option<Uuid>,
    refund_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminOrderReturnOrchestrationErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Option<Uuid>,
        return_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            order_id,
            return_id,
            refund_id: None,
            operation,
        }
    }
}

fn admin_return_decision_order_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    order_id: Uuid,
) -> PortContext {
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-return-decision:{order_id}"),
    )
    // This legacy route has no caller idempotency header. The generated root is
    // write-admission metadata only; the orchestration derives a distinct identity
    // for each Order owner operation without claiming durable request replay.
    .with_idempotency_key(Uuid::new_v4().to_string())
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn admin_order_error_policy(error: &OrderError) -> AdminOrderReturnHttpPolicy {
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

fn admin_order_port_error_policy(error: &PortError) -> AdminOrderReturnHttpPolicy {
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

fn admin_payment_port_error_policy(error: &PortError) -> AdminOrderReturnHttpPolicy {
    match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_payment_invalid",
            "Payment request is invalid",
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
            "commerce_admin_payment_state_conflict",
            "Payment operation conflicts with the current state",
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
            "commerce_admin_payment_storage_unavailable",
            "Payment storage is temporarily unavailable",
            "temporarily_unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_payment_failed",
            "Payment state could not be read safely",
            "invariant_violation",
        ),
    }
}

fn admin_payment_error_policy(error: &PaymentError) -> AdminOrderReturnHttpPolicy {
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

fn admin_reserved_refund_error_policy(error: &PaymentError) -> AdminOrderReturnHttpPolicy {
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
        error => admin_payment_error_policy(error),
    }
}

fn map_admin_order_return_error(
    context: AdminOrderReturnErrorContext,
    error: OrderError,
) -> HttpError {
    let (status, code, message, error_kind) = admin_order_error_policy(&error);
    tracing::error!(
        error = ?error,
        owner = ADMIN_ORDER_RETURN_OWNER,
        tenant_id = %context.tenant_id,
        order_id = ?context.order_id,
        return_id = ?context.return_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_RETURN_BOUNDARY,
        "commerce admin order return owner operation failed"
    );
    HttpError::new(status, code, message)
}

fn map_admin_return_decision_order_port_error(
    tenant_id: Uuid,
    actor_id: Uuid,
    order_id: Uuid,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = admin_order_port_error_policy(&error);
    tracing::error!(
        owner = "rustok_order.post_order_command",
        consumer_operation = "create_return_decision",
        correlation_id = %context.correlation_id,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        order_id_non_nil = !order_id.is_nil(),
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_RETURN_BOUNDARY,
        "commerce admin return-decision Order owner command failed with bounded diagnostics"
    );
    HttpError::new(status, code, message)
}

fn map_admin_return_decision_payment_port_error(
    tenant_id: Uuid,
    actor_id: Uuid,
    order_id: Uuid,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = admin_payment_port_error_policy(&error);
    tracing::error!(
        owner = "rustok_payment.admin_read",
        owner_operation = "list_payment_collection_projections",
        consumer_operation = "create_return_decision",
        correlation_id = %context.correlation_id,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        order_id_non_nil = !order_id.is_nil(),
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_RETURN_BOUNDARY,
        "commerce admin return-decision Payment owner read failed with bounded diagnostics"
    );
    HttpError::new(status, code, message)
}

fn map_admin_order_return_orchestration_error(
    mut context: AdminOrderReturnOrchestrationErrorContext,
    error: PostOrderOrchestrationError,
) -> HttpError {
    let (status, code, message, error_kind, source_owner) = match &error {
        PostOrderOrchestrationError::Order(source) => {
            match source {
                OrderError::OrderNotFound(id) => context.order_id = Some(*id),
                OrderError::OrderReturnNotFound(id) => context.return_id = Some(*id),
                _ => {}
            }
            let (status, code, message, error_kind) = admin_order_error_policy(source);
            (status, code, message, error_kind, "rustok_order")
        }
        PostOrderOrchestrationError::Payment(source) => {
            if let PaymentError::RefundNotFound(id) = source {
                context.refund_id = Some(*id);
            }
            let (status, code, message, error_kind) = admin_payment_error_policy(source);
            (status, code, message, error_kind, "rustok_payment")
        }
        PostOrderOrchestrationError::PaymentOrchestration(source) => match source {
            PaymentOrchestrationError::Provider(source)
            | PaymentOrchestrationError::Payment(source) => {
                if let PaymentError::RefundNotFound(id) = source {
                    context.refund_id = Some(*id);
                }
                let (status, code, message, error_kind) = admin_payment_error_policy(source);
                (status, code, message, error_kind, "rustok_payment")
            }
            PaymentOrchestrationError::ProviderAfterRefundReservation { refund_id, source } => {
                context.refund_id = Some(*refund_id);
                let (status, code, message, error_kind) =
                    admin_reserved_refund_error_policy(source);
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
        owner = ADMIN_ORDER_RETURN_ORCHESTRATION_OWNER,
        source_owner,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        order_id = ?context.order_id,
        return_id = ?context.return_id,
        refund_id = ?context.refund_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_ORDER_RETURN_BOUNDARY,
        "commerce admin order return orchestration failed"
    );
    HttpError::new(status, code, message)
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
    Path(id): Path<Uuid>,
    Json(input): Json<CreateOrderReturnInput>,
) -> HttpResult<(StatusCode, Json<OrderReturnResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;
    let created = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .create_return(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_order_return_error(
                AdminOrderReturnErrorContext::new(tenant.id, Some(id), None, "create_return"),
                error,
            )
        })?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    post,
    path = "/admin/orders/{id}/returns/decision",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = CreateReturnDecisionInput,
    responses(
        (status = 201, description = "Return decision created", body = ReturnDecisionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn create_order_return_decision(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateReturnDecisionInput>,
) -> HttpResult<(StatusCode, Json<ReturnDecisionResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;
    if super::decision_requires_payments_update(
        input.decision.action.as_str(),
        input.decision.refund.is_some(),
    ) {
        ensure_permissions(
            &auth,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;
    }

    let context = admin_return_decision_order_context(&tenant, &auth, &request_context, id);
    let service = ReturnDecisionOwnerOrchestrationService::new(
        runtime.db_clone(),
        runtime.order_post_order_command_port(),
        runtime.payment_admin_read_port(),
    )
    .with_payment_provider_registry(runtime.payment_provider_registry());
    let decision = service
        .create_return_decision(context.clone(), tenant.id, id, input)
        .await
        .map_err(|error| match error {
            ReturnDecisionOwnerOrchestrationError::OrderCommand(error) => {
                map_admin_return_decision_order_port_error(
                    tenant.id,
                    auth.user_id,
                    id,
                    &context,
                    error,
                )
            }
            ReturnDecisionOwnerOrchestrationError::PaymentRead(error) => {
                map_admin_return_decision_payment_port_error(
                    tenant.id,
                    auth.user_id,
                    id,
                    &context,
                    error,
                )
            }
            ReturnDecisionOwnerOrchestrationError::PostOrder(error) => {
                map_admin_order_return_orchestration_error(
                    AdminOrderReturnOrchestrationErrorContext::new(
                        tenant.id,
                        auth.user_id,
                        Some(id),
                        None,
                        "create_return_decision",
                    ),
                    error,
                )
            }
        })?;
    Ok((StatusCode::CREATED, Json(decision)))
}

#[utoipa::path(
    get,
    path = "/admin/returns",
    tag = "admin",
    params(ListOrderReturnsParams),
    responses(
        (status = 200, description = "Returns", body = PaginatedResponse<OrderReturnResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_order_returns(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListOrderReturnsParams>,
) -> HttpResult<Json<PaginatedResponse<OrderReturnResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;
    let pagination = params.pagination.unwrap_or_default();
    let order_id = params.order_id;
    let (items, total) = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .list_returns(
            tenant.id,
            ListOrderReturnsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                order_id,
                status: params.status,
            },
        )
        .await
        .map_err(|error| {
            map_admin_order_return_error(
                AdminOrderReturnErrorContext::new(tenant.id, order_id, None, "list_returns"),
                error,
            )
        })?;
    Ok(Json(PaginatedResponse {
        data: items,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

#[utoipa::path(
    get,
    path = "/admin/returns/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Return ID")),
    responses(
        (status = 200, description = "Return details", body = OrderReturnResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Return not found")
    )
)]
pub async fn show_order_return(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<OrderReturnResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;
    let item = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .get_return(tenant.id, id)
        .await
        .map_err(|error| {
            map_admin_order_return_error(
                AdminOrderReturnErrorContext::new(tenant.id, None, Some(id), "get_return"),
                error,
            )
        })?;
    Ok(Json(item))
}

#[utoipa::path(
    post,
    path = "/admin/returns/{id}/complete",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Return ID")),
    request_body = AdminCompleteOrderReturnInput,
    responses(
        (status = 200, description = "Return completed", body = OrderReturnResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Return not found")
    )
)]
pub async fn complete_order_return(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AdminCompleteOrderReturnInput>,
) -> HttpResult<Json<OrderReturnResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;
    if input.refund.is_some() {
        ensure_permissions(
            &auth,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;
    }

    let command = CompleteReturnResolutionInput {
        resolution_type: input.resolution_type,
        refund_id: input.refund_id,
        order_change_id: input.order_change_id,
        refund: input.refund.map(|refund| CompleteReturnRefundInput {
            payment_collection_id: refund.payment_collection_id,
            amount: refund.amount,
            reason: refund.reason,
            metadata: refund.metadata,
            complete: refund.complete,
        }),
        exchange: input.exchange.map(|exchange| CompleteReturnExchangeInput {
            description: exchange.description,
            preview: exchange.preview,
            metadata: exchange.metadata,
        }),
        claim: input.claim.map(|claim| CompleteReturnClaimInput {
            description: claim.description,
            preview: claim.preview,
            metadata: claim.metadata,
        }),
        metadata: input.metadata,
    };
    let item = ReturnCompletionOrchestrationService::new(runtime.db_clone(), runtime.event_bus())
        .with_payment_provider_registry(runtime.payment_provider_registry())
        .complete_return(tenant.id, auth.user_id, id, command)
        .await
        .map_err(|error| {
            map_admin_order_return_orchestration_error(
                AdminOrderReturnOrchestrationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    None,
                    Some(id),
                    "complete_return",
                ),
                error,
            )
        })?;

    Ok(Json(item))
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
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderReturnInput>,
) -> HttpResult<Json<OrderReturnResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;
    let item = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .cancel_return(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_order_return_error(
                AdminOrderReturnErrorContext::new(tenant.id, None, Some(id), "cancel_return"),
                error,
            )
        })?;
    Ok(Json(item))
}
