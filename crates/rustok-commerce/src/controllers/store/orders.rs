use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{RequestContext, TenantContext};
use rustok_customer::dto::CustomerResponse;
use rustok_customer::{CustomerUserProjectionRequest, in_process_customer_read_port};
use rustok_order::OrderService;
use rustok_payment::PaymentService;
use rustok_web::{HttpError, HttpResult};
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
        .map_err(|error| HttpError::bad_request("commerce_operation_failed", error.message))?;
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

    let customer_id = super::current_customer_id_for_db(runtime.db(), tenant.id, Some(&auth))
        .await?
        .ok_or_else(|| {
            HttpError::unauthorized(
                "commerce_permission_denied",
                "Customer account required".to_string(),
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
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    if order.customer_id != Some(customer_id) {
        return Err(HttpError::unauthorized(
            "commerce_permission_denied",
            "Order does not belong to the current customer".to_string(),
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

    super::ensure_customer_owns_order_for_db(
        runtime.db(),
        runtime.event_bus(),
        tenant.id,
        Some(&auth),
        id,
    )
    .await?;

    let created = OrderService::new(runtime.db_clone(), runtime.event_bus())
        .create_return(tenant.id, id, input)
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

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

    super::ensure_customer_owns_order_for_db(
        runtime.db(),
        runtime.event_bus(),
        tenant.id,
        Some(&auth),
        id,
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
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

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

    let customer_id = super::current_customer_id_for_db(runtime.db(), tenant.id, Some(&auth))
        .await?
        .ok_or_else(|| {
            HttpError::unauthorized(
                "commerce_permission_denied",
                "Customer account required".to_string(),
            )
        })?;
    let order_service = OrderService::new(runtime.db_clone(), runtime.event_bus());
    let order = order_service
        .get_order(tenant.id, id)
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;
    if order.customer_id != Some(customer_id) {
        return Err(HttpError::unauthorized(
            "commerce_permission_denied",
            "Order does not belong to the current customer".to_string(),
        ));
    }

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
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

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

    super::ensure_customer_owns_order_for_db(
        runtime.db(),
        runtime.event_bus(),
        tenant.id,
        Some(&auth),
        id,
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
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    Ok(Json(PaginatedResponse {
        data: items,
        meta: PaginationMeta::new(params.pagination.page, params.pagination.limit(), total),
    }))
}
