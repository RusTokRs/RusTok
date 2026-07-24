use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{PortError, RequestContext, TenantContext};
use rustok_customer::dto::CustomerResponse;
use rustok_customer::{CustomerUserProjectionRequest, in_process_customer_read_port};
use rustok_order::{OrderService, error::OrderError};
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
    CreateOrderReturnInput, ListOrderChangesInput, ListOrderReturnsInput, ListRefundsInput,
    OrderChangeResponse, OrderResponse, OrderReturnResponse, RefundResponse,
};

fn map_storefront_customer_port_error(
    error: PortError,
    operation: &'static str,
    tenant_id: Uuid,
) -> HttpError {
    tracing::error!(
        error = ?error,
        operation,
        tenant_id = %tenant_id,
        boundary = "commerce_storefront_order_http",
        "storefront customer read failed"
    );
    port_error_to_http_error(error)
}

fn map_storefront_order_error(
    error: OrderError,
    operation: &'static str,
    tenant_id: Uuid,
    order_id: Uuid,
) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        OrderError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_store_order_invalid",
            "Order request is invalid",
            "validation",
        ),
        OrderError::OrderNotFound(_)
        | OrderError::OrderReturnNotFound(_)
        | OrderError::OrderChangeNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_store_order_not_found",
            "Order resource was not found",
            "not_found",
        ),
        OrderError::InvalidTransition { .. } => (
            StatusCode::CONFLICT,
            "commerce_store_order_state_conflict",
            "Order operation conflicts with the current state",
            "state_conflict",
        ),
        OrderError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_order_unavailable",
            "Order service is temporarily unavailable",
            "database",
        ),
        OrderError::Core(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_order_failed",
            "Order operation could not be completed safely",
            "core",
        ),
    };
    tracing::error!(
        error = ?error,
        operation,
        tenant_id = %tenant_id,
        order_id = %order_id,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_order_http",
        "storefront order operation failed"
    );
    HttpError::new(status, code, message)
}

fn map_storefront_payment_error(
    error: PaymentError,
    operation: &'static str,
    tenant_id: Uuid,
    order_id: Uuid,
) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        PaymentError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_store_payment_invalid",
            "Payment request is invalid",
            "validation",
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_store_payment_not_found",
            "Payment resource was not found",
            "not_found",
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
            StatusCode::CONFLICT,
            "commerce_store_payment_state_conflict",
            "Payment operation conflicts with the current state",
            "state_conflict",
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
    };
    tracing::error!(
        error = ?error,
        operation,
        tenant_id = %tenant_id,
        order_id = %order_id,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_order_http",
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
    match in_process_customer_read_port(runtime.db_clone())
        .read_customer_projection_by_user(
            super::storefront_customer_port_context(tenant_id, auth.user_id),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None),
        Err(error) => Err(map_storefront_customer_port_error(
            error, operation, tenant_id,
        )),
    }
}

async fn ensure_customer_owns_order(
    runtime: &CommerceHttpRuntime,
    tenant_id: Uuid,
    auth: &rustok_api::AuthContext,
    order_id: Uuid,
    operation: &'static str,
) -> HttpResult<()> {
    let customer_id = current_storefront_customer_id(runtime, tenant_id, auth, operation)
        .await?
        .ok_or_else(|| {
            HttpError::unauthorized(
                "commerce_store_customer_required",
                "Customer account required",
            )
        })?;
    let order = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .get_order(tenant_id, order_id)
        .await
        .map_err(|error| {
            map_storefront_order_error(error, operation, tenant_id, order_id)
        })?;

    if order.customer_id != Some(customer_id) {
        return Err(HttpError::unauthorized(
            "commerce_store_order_access_denied",
            "Order does not belong to the current customer",
        ));
    }

    Ok(())
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

    let customer = in_process_customer_read_port(runtime.db_clone())
        .read_customer_projection_by_user(
            super::storefront_customer_port_context(tenant.id, auth.user_id),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
        .map_err(|error| {
            map_storefront_customer_port_error(error, "get_me", tenant.id)
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
    let service = OrderService::new(runtime.db_clone(), runtime.event_bus());
    let order = service
        .get_order_with_locale_fallback(
            tenant.id,
            id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|error| map_storefront_order_error(error, "get_order", tenant.id, id))?;

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

    ensure_customer_owns_order(
        &runtime,
        tenant.id,
        &auth,
        id,
        "create_order_return_access",
    )
    .await?;

    let created = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .create_return(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_storefront_order_error(error, "create_order_return", tenant.id, id)
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

    ensure_customer_owns_order(
        &runtime,
        tenant.id,
        &auth,
        id,
        "list_order_returns_access",
    )
    .await?;

    let (items, total) = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .list_returns(
            tenant.id,
            ListOrderReturnsInput {
                page: params.pagination.page,
                per_page: params.pagination.per_page,
                order_id: Some(id),
                status: params.status,
            },
        )
        .await
        .map_err(|error| {
            map_storefront_order_error(error, "list_order_returns", tenant.id, id)
        })?;

    Ok(Json(PaginatedResponse {
        data: items,
        meta: PaginationMeta::new(params.pagination.page, params.pagination.limit(), total),
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

    ensure_customer_owns_order(
        &runtime,
        tenant.id,
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
            map_storefront_payment_error(error, "list_order_refunds", tenant.id, id)
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

    ensure_customer_owns_order(
        &runtime,
        tenant.id,
        &auth,
        id,
        "list_order_changes_access",
    )
    .await?;

    let (items, total) = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .list_order_changes(
            tenant.id,
            ListOrderChangesInput {
                page: params.pagination.page,
                per_page: params.pagination.per_page,
                order_id: Some(id),
                status: params.status,
                change_type: params.change_type,
            },
        )
        .await
        .map_err(|error| {
            map_storefront_order_error(error, "list_order_changes", tenant.id, id)
        })?;

    Ok(Json(PaginatedResponse {
        data: items,
        meta: PaginationMeta::new(params.pagination.page, params.pagination.limit(), total),
    }))
}
