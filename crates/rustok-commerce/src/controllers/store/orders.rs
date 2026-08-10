use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{
    PortActor, PortContext, PortError, PortErrorKind, RequestContext, TenantContext,
};
use rustok_customer::dto::CustomerResponse;
use rustok_customer::{CustomerUserProjectionRequest, in_process_customer_read_port};
use rustok_order::{
    CreateOrderReturnRequest, ListOrderChangeProjectionsRequest, ListOrderReturnProjectionsRequest,
    ReadOrderProjectionRequest,
};
use rustok_payment::{PaymentService, error::PaymentError};
use rustok_web::{HttpError, HttpResult, port_error_to_http_error};
use uuid::Uuid;

use super::{
    super::{
        CommerceHttpRuntime,
        common::{PaginatedResponse, PaginationMeta, PaginationParams},
    },
    StoreOrderChangesParams, StoreOrderRefundsParams, StoreOrderReturnsParams,
};
use crate::dto::{
    CreateOrderReturnInput, ListRefundsInput, OrderChangeResponse, OrderResponse,
    OrderReturnResponse, RefundResponse,
};

const STOREFRONT_ORDER_CUSTOMER_OWNER: &str = "rustok_customer";
const STOREFRONT_ORDER_CUSTOMER_OWNER_OPERATION: &str = "read_customer_projection_by_user";
const STOREFRONT_ORDER_CUSTOMER_BOUNDARY: &str = "commerce_storefront_order_http";
const STOREFRONT_ORDER_OWNER: &str = "rustok_order.storefront_orders";
const STOREFRONT_ORDER_DETAIL_OPERATION: &str = "read_order_projection";
const STOREFRONT_ORDER_RETURN_LIST_OPERATION: &str = "list_order_return_projections";
const STOREFRONT_ORDER_CHANGE_LIST_OPERATION: &str = "list_order_change_projections";
const STOREFRONT_ORDER_RETURN_COMMAND_OPERATION: &str = "create_return";
const STOREFRONT_ORDER_PAYMENT_OWNER: &str = "rustok_payment.storefront_order_refunds";
const STOREFRONT_ORDER_PAYMENT_BOUNDARY: &str = "commerce_storefront_order_http";

type StorefrontOrderPaymentHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

#[derive(Clone, Copy)]
struct StorefrontOrderPaymentErrorContext<'a> {
    tenant_id: Uuid,
    actor_id: Uuid,
    customer_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Option<Uuid>,
    refund_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    channel_slug: Option<&'a str>,
    locale: &'a str,
    operation: &'static str,
}

impl<'a> StorefrontOrderPaymentErrorContext<'a> {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        customer_id: Uuid,
        order_id: Uuid,
        request_context: &'a RequestContext,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            customer_id,
            order_id,
            payment_collection_id: None,
            refund_id: None,
            channel_id: request_context.channel_id,
            channel_slug: request_context.channel_slug.as_deref(),
            locale: request_context.locale.as_str(),
            operation,
        }
    }
}

fn map_storefront_customer_port_error(
    error: PortError,
    context: &PortContext,
    user_id: Uuid,
    consumer_operation: &'static str,
) -> HttpError {
    let public = port_error_to_http_error(error.clone());
    match &error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation => {
            tracing::error!(
                error = ?error,
                owner = STOREFRONT_ORDER_CUSTOMER_OWNER,
                owner_operation = STOREFRONT_ORDER_CUSTOMER_OWNER_OPERATION,
                consumer_operation,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                user_id = %user_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                internal_code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                retryable = error.retryable,
                public_code = %public.code,
                status = %public.status,
                boundary = STOREFRONT_ORDER_CUSTOMER_BOUNDARY,
                "storefront customer read failed"
            );
        }
        _ => {
            tracing::warn!(
                error = ?error,
                owner = STOREFRONT_ORDER_CUSTOMER_OWNER,
                owner_operation = STOREFRONT_ORDER_CUSTOMER_OWNER_OPERATION,
                consumer_operation,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                user_id = %user_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                internal_code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                retryable = error.retryable,
                public_code = %public.code,
                status = %public.status,
                boundary = STOREFRONT_ORDER_CUSTOMER_BOUNDARY,
                "storefront customer read was rejected"
            );
        }
    }
    public
}

fn storefront_order_read_port_context(
    tenant_id: Uuid,
    auth: &rustok_api::AuthContext,
    request_context: &RequestContext,
    order_id: Uuid,
    operation: &'static str,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-storefront-order:{operation}:{order_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn storefront_order_return_command_context(
    tenant_id: Uuid,
    auth: &rustok_api::AuthContext,
    request_context: &RequestContext,
    order_id: Uuid,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-storefront-order:create-return:{order_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
    .with_idempotency_key(Uuid::new_v4().to_string());
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn map_storefront_order_port_error(
    error: PortError,
    context: &PortContext,
    owner_operation: &'static str,
    consumer_operation: &'static str,
    actor_id: Uuid,
    customer_id: Uuid,
    order_id: Uuid,
) -> HttpError {
    let (status, code, message, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_store_order_invalid",
            "Order request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_store_order_not_found",
            "Order resource was not found",
            "not_found",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_store_order_state_conflict",
            "Order operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::UNAUTHORIZED,
            "commerce_store_order_access_denied",
            "Order does not belong to the current customer",
            "forbidden",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_order_unavailable",
            "Order service is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_order_failed",
            "Order operation could not be completed safely",
            "invariant_violation",
        ),
    };
    tracing::error!(
        error = ?error,
        owner = STOREFRONT_ORDER_OWNER,
        owner_operation,
        consumer_operation,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        actor_id = %actor_id,
        customer_id = %customer_id,
        order_id = %order_id,
        actor = ?context.actor,
        channel = ?context.channel,
        locale = %context.locale,
        deadline_ms = ?context.deadline_ms,
        internal_code = %error.code,
        internal_message = %error.message,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = STOREFRONT_ORDER_CUSTOMER_BOUNDARY,
        "storefront order owner read failed"
    );
    HttpError::new(status, code, message)
}

fn map_storefront_order_command_port_error(
    error: PortError,
    context: &PortContext,
    actor_id: Uuid,
    customer_id: Uuid,
    order_id: Uuid,
) -> HttpError {
    let (status, code, message, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_store_order_invalid",
            "Order request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_store_order_not_found",
            "Order resource was not found",
            "not_found",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_store_order_state_conflict",
            "Order operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::UNAUTHORIZED,
            "commerce_store_order_access_denied",
            "Order does not belong to the current customer",
            "forbidden",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_order_unavailable",
            "Order service is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_order_failed",
            "Order operation could not be completed safely",
            "invariant_violation",
        ),
    };
    tracing::error!(
        owner = "rustok_order",
        owner_operation = STOREFRONT_ORDER_RETURN_COMMAND_OPERATION,
        consumer_operation = "create_order_return",
        correlation_id = %context.correlation_id,
        tenant_id_non_nil = !context.tenant_id.is_empty(),
        actor_id_non_nil = !actor_id.is_nil(),
        customer_id_non_nil = !customer_id.is_nil(),
        order_id_non_nil = !order_id.is_nil(),
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = STOREFRONT_ORDER_CUSTOMER_BOUNDARY,
        "storefront Order return command failed with bounded diagnostics"
    );
    HttpError::new(status, code, message)
}

fn storefront_order_payment_error_policy(
    context: &mut StorefrontOrderPaymentErrorContext<'_>,
    error: &PaymentError,
) -> StorefrontOrderPaymentHttpPolicy {
    match error {
        PaymentError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_store_payment_invalid",
            "Payment request is invalid",
            "validation",
        ),
        PaymentError::PaymentCollectionNotFound(payment_collection_id) => {
            context.payment_collection_id = Some(*payment_collection_id);
            (
                StatusCode::NOT_FOUND,
                "commerce_store_payment_not_found",
                "Payment resource was not found",
                "payment_collection_not_found",
            )
        }
        PaymentError::PaymentNotFound(payment_collection_id) => {
            context.payment_collection_id = Some(*payment_collection_id);
            (
                StatusCode::NOT_FOUND,
                "commerce_store_payment_not_found",
                "Payment resource was not found",
                "payment_not_found",
            )
        }
        PaymentError::RefundNotFound(refund_id) => {
            context.refund_id = Some(*refund_id);
            (
                StatusCode::NOT_FOUND,
                "commerce_store_payment_not_found",
                "Payment resource was not found",
                "refund_not_found",
            )
        }
        PaymentError::InvalidTransition { .. } => (
            StatusCode::CONFLICT,
            "commerce_store_payment_state_conflict",
            "Payment operation conflicts with the current state",
            "state_conflict",
        ),
        PaymentError::ProviderRejected { .. } => (
            StatusCode::CONFLICT,
            "commerce_store_payment_state_conflict",
            "Payment operation conflicts with the current state",
            "provider_rejected",
        ),
        PaymentError::ProviderUnavailable { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_payment_provider_unavailable",
            "Payment provider is temporarily unavailable",
            "provider_unavailable",
        ),
        PaymentError::ProviderInvalidResponse { .. } => (
            StatusCode::BAD_GATEWAY,
            "commerce_store_payment_provider_invalid_response",
            "Payment provider returned an invalid response",
            "provider_invalid_response",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => (
            StatusCode::CONFLICT,
            "commerce_store_payment_reconciliation_required",
            "Payment state requires reconciliation",
            "reconciliation_required",
        ),
        PaymentError::ProviderConfiguration { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_payment_provider_not_configured",
            "Payment provider is not configured for this tenant",
            "provider_configuration",
        ),
        PaymentError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_payment_unavailable",
            "Payment service is temporarily unavailable",
            "database",
        ),
    }
}

fn map_storefront_payment_error(
    mut context: StorefrontOrderPaymentErrorContext<'_>,
    error: PaymentError,
) -> HttpError {
    let (status, code, message, error_kind) =
        storefront_order_payment_error_policy(&mut context, &error);
    tracing::error!(
        error = ?error,
        owner = STOREFRONT_ORDER_PAYMENT_OWNER,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        customer_id = %context.customer_id,
        order_id = %context.order_id,
        payment_collection_id = ?context.payment_collection_id,
        refund_id = ?context.refund_id,
        channel_id = ?context.channel_id,
        channel = ?context.channel_slug,
        locale = %context.locale,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = STOREFRONT_ORDER_PAYMENT_BOUNDARY,
        "storefront payment read failed"
    );
    HttpError::new(status, code, message)
}

async fn current_storefront_customer_id(
    runtime: &CommerceHttpRuntime,
    tenant_id: Uuid,
    auth: &rustok_api::AuthContext,
    operation: &'static str,
) -> HttpResult<Option<Uuid>> {
    let customer_context = super::storefront_customer_port_context(tenant_id, auth.user_id);
    match in_process_customer_read_port(runtime.db_clone())
        .read_customer_projection_by_user(
            customer_context.clone(),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None),
        Err(error) => Err(map_storefront_customer_port_error(
            error,
            &customer_context,
            auth.user_id,
            operation,
        )),
    }
}

async fn read_storefront_order_projection(
    runtime: &CommerceHttpRuntime,
    tenant_id: Uuid,
    tenant_default_locale: &str,
    request_context: &RequestContext,
    auth: &rustok_api::AuthContext,
    customer_id: Uuid,
    order_id: Uuid,
    operation: &'static str,
) -> HttpResult<OrderResponse> {
    let read_context =
        storefront_order_read_port_context(tenant_id, auth, request_context, order_id, operation);
    runtime
        .order_read_port()
        .read_order_projection(
            read_context.clone(),
            ReadOrderProjectionRequest {
                order_id,
                tenant_default_locale: Some(tenant_default_locale.to_string()),
            },
        )
        .await
        .map_err(|error| {
            map_storefront_order_port_error(
                error,
                &read_context,
                STOREFRONT_ORDER_DETAIL_OPERATION,
                operation,
                auth.user_id,
                customer_id,
                order_id,
            )
        })
}

async fn ensure_customer_owns_order(
    runtime: &CommerceHttpRuntime,
    tenant_id: Uuid,
    tenant_default_locale: &str,
    request_context: &RequestContext,
    auth: &rustok_api::AuthContext,
    order_id: Uuid,
    operation: &'static str,
) -> HttpResult<Uuid> {
    let customer_id = current_storefront_customer_id(runtime, tenant_id, auth, operation)
        .await?
        .ok_or_else(|| {
            HttpError::unauthorized(
                "commerce_store_customer_required",
                "Customer account required",
            )
        })?;
    let order = read_storefront_order_projection(
        runtime,
        tenant_id,
        tenant_default_locale,
        request_context,
        auth,
        customer_id,
        order_id,
        operation,
    )
    .await?;

    if order.customer_id != Some(customer_id) {
        return Err(HttpError::unauthorized(
            "commerce_store_order_access_denied",
            "Order does not belong to the current customer",
        ));
    }

    Ok(customer_id)
}

/// Get current storefront customer
#[utoipa::path(
    get,
    path = "/store/customers/me",
    tag = "store",
    responses(
        (status = 200, description = "Current customer", body = CustomerResponse),
        (status = 401, description = "Authentication required")
    )
)]
pub async fn get_me(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
) -> HttpResult<Json<CustomerResponse>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_context = super::storefront_customer_port_context(tenant.id, auth.user_id);
    let customer = in_process_customer_read_port(runtime.db_clone())
        .read_customer_projection_by_user(
            customer_context.clone(),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
        .map_err(|error| {
            map_storefront_customer_port_error(error, &customer_context, auth.user_id, "get_me")
        })?;
    Ok(Json(customer))
}

/// Get customer-owned storefront order
#[utoipa::path(
    get,
    path = "/store/orders/{id}",
    tag = "store",
    params(("id" = Uuid, Path, description = "Order ID")),
    responses(
        (status = 200, description = "Order details", body = OrderResponse),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn get_order(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<OrderResponse>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id = current_storefront_customer_id(&runtime, tenant.id, &auth, "get_order")
        .await?
        .ok_or_else(|| {
            HttpError::unauthorized(
                "commerce_store_customer_required",
                "Customer account required",
            )
        })?;
    let order = read_storefront_order_projection(
        &runtime,
        tenant.id,
        tenant.default_locale.as_str(),
        &request_context,
        &auth,
        customer_id,
        id,
        "get_order",
    )
    .await?;

    if order.customer_id != Some(customer_id) {
        return Err(HttpError::unauthorized(
            "commerce_store_order_access_denied",
            "Order does not belong to the current customer",
        ));
    }

    Ok(Json(order))
}

/// Create a return request for the current customer's order.
#[utoipa::path(
    post,
    path = "/store/orders/{id}/returns",
    tag = "store",
    params(("id" = Uuid, Path, description = "Order ID")),
    request_body = CreateOrderReturnInput,
    responses(
        (status = 201, description = "Return created", body = OrderReturnResponse),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn create_order_return(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateOrderReturnInput>,
) -> HttpResult<(StatusCode, Json<OrderReturnResponse>)> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id = ensure_customer_owns_order(
        &runtime,
        tenant.id,
        tenant.default_locale.as_str(),
        &request_context,
        &auth,
        id,
        "create_order_return_access",
    )
    .await?;

    let command_context =
        storefront_order_return_command_context(tenant.id, &auth, &request_context, id);
    let created = runtime
        .order_post_order_command_port()
        .create_return(
            command_context.clone(),
            CreateOrderReturnRequest {
                order_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_storefront_order_command_port_error(
                error,
                &command_context,
                auth.user_id,
                customer_id,
                id,
            )
        })?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// List return requests for the current customer's order.
#[utoipa::path(
    get,
    path = "/store/orders/{id}/returns",
    tag = "store",
    params(
        ("id" = Uuid, Path, description = "Order ID"),
        PaginationParams,
        ("status" = Option<String>, Query, description = "Optional return status filter")
    ),
    responses(
        (status = 200, description = "Order returns", body = PaginatedResponse<OrderReturnResponse>),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn list_order_returns(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
    Query(params): Query<StoreOrderReturnsParams>,
) -> HttpResult<Json<PaginatedResponse<OrderReturnResponse>>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id = ensure_customer_owns_order(
        &runtime,
        tenant.id,
        tenant.default_locale.as_str(),
        &request_context,
        &auth,
        id,
        "list_order_returns_access",
    )
    .await?;

    let read_context = storefront_order_read_port_context(
        tenant.id,
        &auth,
        &request_context,
        id,
        "list_order_returns",
    );
    let page = runtime
        .order_read_port()
        .list_order_return_projections(
            read_context.clone(),
            ListOrderReturnProjectionsRequest {
                page: params.pagination.page,
                per_page: params.pagination.per_page,
                order_id: Some(id),
                status: params.status,
            },
        )
        .await
        .map_err(|error| {
            map_storefront_order_port_error(
                error,
                &read_context,
                STOREFRONT_ORDER_RETURN_LIST_OPERATION,
                "list_order_returns",
                auth.user_id,
                customer_id,
                id,
            )
        })?;

    Ok(Json(PaginatedResponse {
        data: page.items,
        meta: PaginationMeta::new(params.pagination.page, params.pagination.limit(), page.total),
    }))
}

/// List refunds for the current customer's order
#[utoipa::path(
    get,
    path = "/store/orders/{id}/refunds",
    tag = "store",
    params(
        ("id" = Uuid, Path, description = "Order ID"),
        PaginationParams,
        ("status" = Option<String>, Query, description = "Optional refund status filter")
    ),
    responses(
        (status = 200, description = "Order refunds", body = PaginatedResponse<RefundResponse>),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn list_order_refunds(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
    Query(params): Query<StoreOrderRefundsParams>,
) -> HttpResult<Json<PaginatedResponse<RefundResponse>>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id = ensure_customer_owns_order(
        &runtime,
        tenant.id,
        tenant.default_locale.as_str(),
        &request_context,
        &auth,
        id,
        "list_order_refunds_access",
    )
    .await?;

    let payment_service = PaymentService::new(runtime.db_clone());
    let (items, total) = payment_service
        .list_refunds(
            tenant.id,
            ListRefundsInput {
                page: params.pagination.page,
                per_page: params.pagination.per_page,
                payment_collection_id: None,
                order_id: Some(id),
                status: params.status,
            },
        )
        .await
        .map_err(|error| {
            map_storefront_payment_error(
                StorefrontOrderPaymentErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    customer_id,
                    id,
                    &request_context,
                    "list_order_refunds",
                ),
                error,
            )
        })?;

    Ok(Json(PaginatedResponse {
        data: items,
        meta: PaginationMeta::new(params.pagination.page, params.pagination.limit(), total),
    }))
}

/// List order changes for the current customer's order
#[utoipa::path(
    get,
    path = "/store/orders/{id}/changes",
    tag = "store",
    params(
        ("id" = Uuid, Path, description = "Order ID"),
        PaginationParams,
        ("status" = Option<String>, Query, description = "Optional order change status filter"),
        ("change_type" = Option<String>, Query, description = "Optional change type filter")
    ),
    responses(
        (status = 200, description = "Order changes", body = PaginatedResponse<OrderChangeResponse>),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn list_order_changes(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
    Query(params): Query<StoreOrderChangesParams>,
) -> HttpResult<Json<PaginatedResponse<OrderChangeResponse>>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id = ensure_customer_owns_order(
        &runtime,
        tenant.id,
        tenant.default_locale.as_str(),
        &request_context,
        &auth,
        id,
        "list_order_changes_access",
    )
    .await?;

    let read_context = storefront_order_read_port_context(
        tenant.id,
        &auth,
        &request_context,
        id,
        "list_order_changes",
    );
    let page = runtime
        .order_read_port()
        .list_order_change_projections(
            read_context.clone(),
            ListOrderChangeProjectionsRequest {
                page: params.pagination.page,
                per_page: params.pagination.per_page,
                order_id: Some(id),
                status: params.status,
                change_type: params.change_type,
            },
        )
        .await
        .map_err(|error| {
            map_storefront_order_port_error(
                error,
                &read_context,
                STOREFRONT_ORDER_CHANGE_LIST_OPERATION,
                "list_order_changes",
                auth.user_id,
                customer_id,
                id,
            )
        })?;

    Ok(Json(PaginatedResponse {
        data: page.items,
        meta: PaginationMeta::new(params.pagination.page, params.pagination.limit(), page.total),
    }))
}
