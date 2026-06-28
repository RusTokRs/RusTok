use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{loco::transactional_event_bus_from_context, RequestContext, TenantContext};
use uuid::Uuid;

use super::{
    super::common::{PaginatedResponse, PaginationMeta, PaginationParams},
    StoreOrderChangesParams, StoreOrderRefundsParams, StoreOrderReturnsParams,
};
use crate::{
    dto::{
        CreateOrderReturnInput, CustomerResponse, ListOrderChangesInput, ListOrderReturnsInput,
        ListRefundsInput, OrderChangeResponse, OrderResponse, OrderReturnResponse, RefundResponse,
    },
    CustomerService, OrderService, PaymentService,
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
) -> Result<Json<CustomerResponse>> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let service = CustomerService::new(ctx.db.clone());
    let customer = service
        .get_customer_by_user(tenant.id, auth.user_id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<OrderResponse>> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = super::current_customer_id(&ctx, tenant.id, Some(&auth))
        .await?
        .ok_or_else(|| Error::Unauthorized("Customer account required".to_string()))?;
    let service = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let order = service
        .get_order_with_locale_fallback(
            tenant.id,
            id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    if order.customer_id != Some(customer_id) {
        return Err(Error::Unauthorized(
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateOrderReturnInput>,
) -> Result<(StatusCode, Json<OrderReturnResponse>)> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    super::ensure_customer_owns_order(&ctx, tenant.id, Some(&auth), id).await?;

    let created = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .create_return(tenant.id, id, input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
    Query(params): Query<StoreOrderReturnsParams>,
) -> Result<Json<PaginatedResponse<OrderReturnResponse>>> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    super::ensure_customer_owns_order(&ctx, tenant.id, Some(&auth), id).await?;

    let (items, total) =
        OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
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
            .map_err(|err| Error::BadRequest(err.to_string()))?;

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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
    Query(params): Query<StoreOrderRefundsParams>,
) -> Result<Json<PaginatedResponse<RefundResponse>>> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = super::current_customer_id(&ctx, tenant.id, Some(&auth))
        .await?
        .ok_or_else(|| Error::Unauthorized("Customer account required".to_string()))?;
    let order_service =
        OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let order = order_service
        .get_order(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    if order.customer_id != Some(customer_id) {
        return Err(Error::Unauthorized(
            "Order does not belong to the current customer".to_string(),
        ));
    }

    let payment_service = PaymentService::new(ctx.db.clone());
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
        .map_err(|err| Error::BadRequest(err.to_string()))?;

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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
    Query(params): Query<StoreOrderChangesParams>,
) -> Result<Json<PaginatedResponse<OrderChangeResponse>>> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    super::ensure_customer_owns_order(&ctx, tenant.id, Some(&auth), id).await?;

    let (items, total) =
        OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
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
            .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(PaginatedResponse {
        data: items,
        meta: PaginationMeta::new(params.pagination.page, params.pagination.limit(), total),
    }))
}
