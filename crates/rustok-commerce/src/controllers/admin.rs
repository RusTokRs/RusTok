use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, controller::Routes, Error, Result};
use rustok_api::{loco::transactional_event_bus_from_context, AuthContext, TenantContext};
use rustok_core::Permission;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    dto::{
        AuthorizePaymentInput, CancelFulfillmentInput, CancelOrderInput, CancelPaymentInput,
        CancelRefundInput, CapturePaymentInput, CompleteRefundInput, CreateFulfillmentInput,
        CreateProductInput, CreateRefundInput, CreateShippingOptionInput,
        CreateShippingProfileInput, DeliverFulfillmentInput, DeliverOrderInput,
        FulfillmentResponse, ListFulfillmentsInput, ListPaymentCollectionsInput, ListRefundsInput,
        ListShippingProfilesInput, MarkPaidOrderInput, OrderResponse, PaymentCollectionResponse,
        ProductResponse, RefundResponse, ReopenFulfillmentInput, ReshipFulfillmentInput,
        ShipFulfillmentInput, ShipOrderInput, ShippingOptionResponse, ShippingProfileResponse,
        UpdateProductInput, UpdateShippingOptionInput, UpdateShippingProfileInput,
    },
    storefront_shipping::normalize_shipping_profile_slug,
    CatalogService, FulfillmentOrchestrationError, FulfillmentOrchestrationService,
    FulfillmentService, OrderService, PaymentService, ShippingProfileService,
};

use super::{
    common::{ensure_permissions, PaginatedResponse},
    products::{ListProductsParams, ProductListItem},
};

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/products",
            axum::routing::get(list_products).post(create_product),
        )
        .add(
            "/products/{id}",
            axum::routing::get(show_product)
                .post(update_product)
                .delete(delete_product),
        )
        .add(
            "/products/{id}/publish",
            axum::routing::post(publish_product),
        )
        .add(
            "/products/{id}/unpublish",
            axum::routing::post(unpublish_product),
        )
        .add("/orders", axum::routing::get(list_orders))
        .add("/orders/{id}", axum::routing::get(show_order))
        .add(
            "/orders/{id}/mark-paid",
            axum::routing::post(mark_order_paid),
        )
        .add("/orders/{id}/ship", axum::routing::post(ship_order))
        .add("/orders/{id}/deliver", axum::routing::post(deliver_order))
        .add("/orders/{id}/cancel", axum::routing::post(cancel_order))
        .add(
            "/payment-collections",
            axum::routing::get(list_payment_collections),
        )
        .add(
            "/payment-collections/{id}",
            axum::routing::get(show_payment_collection),
        )
        .add(
            "/payment-collections/{id}/authorize",
            axum::routing::post(authorize_payment_collection),
        )
        .add(
            "/payment-collections/{id}/capture",
            axum::routing::post(capture_payment_collection),
        )
        .add(
            "/payment-collections/{id}/cancel",
            axum::routing::post(cancel_payment_collection),
        )
        .add(
            "/payment-collections/{id}/refunds",
            axum::routing::post(create_refund),
        )
        .add("/refunds", axum::routing::get(list_refunds))
        .add("/refunds/{id}", axum::routing::get(show_refund))
        .add(
            "/refunds/{id}/complete",
            axum::routing::post(complete_refund),
        )
        .add("/refunds/{id}/cancel", axum::routing::post(cancel_refund))
        .add(
            "/shipping-profiles",
            axum::routing::get(list_shipping_profiles).post(create_shipping_profile),
        )
        .add(
            "/shipping-profiles/{id}",
            axum::routing::get(show_shipping_profile).post(update_shipping_profile),
        )
        .add(
            "/shipping-profiles/{id}/deactivate",
            axum::routing::post(deactivate_shipping_profile),
        )
        .add(
            "/shipping-profiles/{id}/reactivate",
            axum::routing::post(reactivate_shipping_profile),
        )
        .add(
            "/shipping-options",
            axum::routing::get(list_shipping_options).post(create_shipping_option),
        )
        .add(
            "/shipping-options/{id}",
            axum::routing::get(show_shipping_option).post(update_shipping_option),
        )
        .add(
            "/shipping-options/{id}/deactivate",
            axum::routing::post(deactivate_shipping_option),
        )
        .add(
            "/shipping-options/{id}/reactivate",
            axum::routing::post(reactivate_shipping_option),
        )
        .add(
            "/fulfillments",
            axum::routing::get(list_fulfillments).post(create_fulfillment),
        )
        .add("/fulfillments/{id}", axum::routing::get(show_fulfillment))
        .add(
            "/fulfillments/{id}/ship",
            axum::routing::post(ship_fulfillment),
        )
        .add(
            "/fulfillments/{id}/deliver",
            axum::routing::post(deliver_fulfillment),
        )
        .add(
            "/fulfillments/{id}/reopen",
            axum::routing::post(reopen_fulfillment),
        )
        .add(
            "/fulfillments/{id}/reship",
            axum::routing::post(reship_fulfillment),
        )
        .add(
            "/fulfillments/{id}/cancel",
            axum::routing::post(cancel_fulfillment),
        )
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminOrderDetailResponse {
    pub order: OrderResponse,
    pub payment_collection: Option<PaymentCollectionResponse>,
    pub fulfillment: Option<FulfillmentResponse>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListOrdersParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub status: Option<String>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListPaymentCollectionsParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub cart_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListFulfillmentsParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListRefundsParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub payment_collection_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListShippingOptionsParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub currency_code: Option<String>,
    pub provider_id: Option<String>,
    pub search: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListShippingProfilesParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub search: Option<String>,
    pub active: Option<bool>,
}

/// List admin ecommerce products
#[utoipa::path(
    get,
    path = "/admin/products",
    tag = "admin",
    params(ListProductsParams),
    responses(
        (status = 200, description = "List of products", body = PaginatedResponse<ProductListItem>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_products(
    state: State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    query: Query<ListProductsParams>,
) -> Result<Json<PaginatedResponse<ProductListItem>>> {
    super::products::list_products(state, tenant, auth, request_context, query).await
}

/// Create admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products",
    tag = "admin",
    request_body = CreateProductInput,
    responses(
        (status = 201, description = "Product created successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn create_product(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateProductInput>,
) -> Result<(StatusCode, Json<ProductResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_CREATE],
        "Permission denied: products:create required",
    )?;

    validate_product_shipping_profile_input(
        &ctx.db,
        tenant.id,
        input.shipping_profile_slug.as_deref(),
    )
    .await?;

    let service = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let product = service
        .create_product(tenant.id, auth.user_id, input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok((StatusCode::CREATED, Json(product)))
}

/// Show admin ecommerce product
#[utoipa::path(
    get,
    path = "/admin/products/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Product details", body = ProductResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn show_product(
    state: State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    path: Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    super::products::show_product(state, tenant, auth, request_context, path).await
}

/// Update admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    request_body = UpdateProductInput,
    responses(
        (status = 200, description = "Product updated successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn update_product(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProductInput>,
) -> Result<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    validate_product_shipping_profile_input(
        &ctx.db,
        tenant.id,
        input.shipping_profile_slug.as_deref(),
    )
    .await?;

    let service = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let product = service
        .update_product(tenant.id, auth.user_id, id, input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(product))
}

/// Delete admin ecommerce product
#[utoipa::path(
    delete,
    path = "/admin/products/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 204, description = "Product deleted successfully"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn delete_product(
    state: State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    path: Path<Uuid>,
) -> Result<StatusCode> {
    super::products::delete_product(state, tenant, auth, path).await
}

/// Publish admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}/publish",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Product published successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn publish_product(
    state: State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    path: Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    super::products::publish_product(state, tenant, auth, path).await
}

/// Unpublish admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}/unpublish",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Product unpublished successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn unpublish_product(
    state: State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    path: Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    super::products::unpublish_product(state, tenant, auth, path).await
}

/// Show admin ecommerce order
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Query(params): Query<ListOrdersParams>,
) -> Result<Json<PaginatedResponse<OrderResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_LIST],
        "Permission denied: orders:list required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (orders, total) =
        OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
            .list_orders_with_locale_fallback(
                tenant.id,
                rustok_order::dto::ListOrdersInput {
                    page: pagination.page,
                    per_page: pagination.limit(),
                    status: params.status,
                    customer_id: params.customer_id,
                },
                request_context.locale.as_str(),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(PaginatedResponse {
        data: orders,
        meta: super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Show admin ecommerce order
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<AdminOrderDetailResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .get_order_with_locale_fallback(
            tenant.id,
            id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| match err {
            rustok_order::error::OrderError::OrderNotFound(_) => Error::NotFound,
            other => Error::BadRequest(other.to_string()),
        })?;
    let payment_collection = PaymentService::new(ctx.db.clone())
        .find_latest_collection_by_order(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .find_by_order(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(AdminOrderDetailResponse {
        order,
        payment_collection,
        fulfillment,
    }))
}

/// List admin payment collections
#[utoipa::path(
    get,
    path = "/admin/payment-collections",
    tag = "admin",
    params(ListPaymentCollectionsParams),
    responses(
        (status = 200, description = "Payment collections", body = PaginatedResponse<PaymentCollectionResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_payment_collections(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListPaymentCollectionsParams>,
) -> Result<Json<PaginatedResponse<PaymentCollectionResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (collections, total) = PaymentService::new(ctx.db.clone())
        .list_collections(
            tenant.id,
            ListPaymentCollectionsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status,
                order_id: params.order_id,
                cart_id: params.cart_id,
                customer_id: params.customer_id,
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(PaginatedResponse {
        data: collections,
        meta: super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Mark admin ecommerce order as paid
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<MarkPaidOrderInput>,
) -> Result<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .mark_paid(
            tenant.id,
            auth.user_id,
            id,
            input.payment_id,
            input.payment_method,
        )
        .await
        .map_err(map_order_error)?;

    Ok(Json(order))
}

/// Ship admin ecommerce order
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ShipOrderInput>,
) -> Result<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .ship_order(
            tenant.id,
            auth.user_id,
            id,
            input.tracking_number,
            input.carrier,
        )
        .await
        .map_err(map_order_error)?;

    Ok(Json(order))
}

/// Deliver admin ecommerce order
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<DeliverOrderInput>,
) -> Result<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .deliver_order(tenant.id, auth.user_id, id, input.delivered_signature)
        .await
        .map_err(map_order_error)?;

    Ok(Json(order))
}

/// Cancel admin ecommerce order
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelOrderInput>,
) -> Result<Json<OrderResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_UPDATE],
        "Permission denied: orders:update required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .cancel_order(tenant.id, auth.user_id, id, input.reason)
        .await
        .map_err(map_order_error)?;

    Ok(Json(order))
}

/// Show admin payment collection
#[utoipa::path(
    get,
    path = "/admin/payment-collections/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    responses(
        (status = 200, description = "Payment collection details", body = PaymentCollectionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn show_payment_collection(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;

    let collection = PaymentService::new(ctx.db.clone())
        .get_collection(tenant.id, id)
        .await
        .map_err(map_payment_error)?;

    Ok(Json(collection))
}

/// Create admin refund
#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/refunds",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = CreateRefundInput,
    responses(
        (status = 201, description = "Refund created", body = RefundResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn create_refund(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateRefundInput>,
) -> Result<(StatusCode, Json<RefundResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let refund = PaymentService::new(ctx.db.clone())
        .create_refund(tenant.id, id, input)
        .await
        .map_err(map_payment_error)?;

    Ok((StatusCode::CREATED, Json(refund)))
}

/// List admin refunds
#[utoipa::path(
    get,
    path = "/admin/refunds",
    tag = "admin",
    params(ListRefundsParams),
    responses(
        (status = 200, description = "Refunds", body = PaginatedResponse<RefundResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_refunds(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListRefundsParams>,
) -> Result<Json<PaginatedResponse<RefundResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (refunds, total) = PaymentService::new(ctx.db.clone())
        .list_refunds(
            tenant.id,
            ListRefundsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                payment_collection_id: params.payment_collection_id,
                status: params.status,
            },
        )
        .await
        .map_err(map_payment_error)?;

    Ok(Json(PaginatedResponse {
        data: refunds,
        meta: super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Show admin refund
#[utoipa::path(
    get,
    path = "/admin/refunds/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Refund ID")),
    responses(
        (status = 200, description = "Refund details", body = RefundResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Refund not found")
    )
)]
pub async fn show_refund(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )?;

    let refund = PaymentService::new(ctx.db.clone())
        .get_refund(tenant.id, id)
        .await
        .map_err(map_payment_error)?;

    Ok(Json(refund))
}

/// Complete admin refund
#[utoipa::path(
    post,
    path = "/admin/refunds/{id}/complete",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Refund ID")),
    request_body = CompleteRefundInput,
    responses(
        (status = 200, description = "Refund completed", body = RefundResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Refund not found")
    )
)]
pub async fn complete_refund(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CompleteRefundInput>,
) -> Result<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let refund = PaymentService::new(ctx.db.clone())
        .complete_refund(tenant.id, id, input)
        .await
        .map_err(map_payment_error)?;

    Ok(Json(refund))
}

/// Cancel admin refund
#[utoipa::path(
    post,
    path = "/admin/refunds/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Refund ID")),
    request_body = CancelRefundInput,
    responses(
        (status = 200, description = "Refund cancelled", body = RefundResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Refund not found")
    )
)]
pub async fn cancel_refund(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelRefundInput>,
) -> Result<Json<RefundResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let refund = PaymentService::new(ctx.db.clone())
        .cancel_refund(tenant.id, id, input)
        .await
        .map_err(map_payment_error)?;

    Ok(Json(refund))
}

/// List admin shipping options
#[utoipa::path(
    get,
    path = "/admin/shipping-profiles",
    tag = "admin",
    params(ListShippingProfilesParams),
    responses(
        (status = 200, description = "Shipping profiles", body = PaginatedResponse<ShippingProfileResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_shipping_profiles(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Query(params): Query<ListShippingProfilesParams>,
) -> Result<Json<PaginatedResponse<ShippingProfileResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (items, total) = ShippingProfileService::new(ctx.db.clone())
        .list_shipping_profiles(
            tenant.id,
            ListShippingProfilesInput {
                page: pagination.page,
                per_page: pagination.limit(),
                active: params.active,
                search: params.search,
                locale: Some(request_context.locale.clone()),
            },
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(Json(PaginatedResponse {
        data: items,
        meta: super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Create admin shipping profile
#[utoipa::path(
    post,
    path = "/admin/shipping-profiles",
    tag = "admin",
    request_body = CreateShippingProfileInput,
    responses(
        (status = 201, description = "Shipping profile created successfully", body = ShippingProfileResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn create_shipping_profile(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateShippingProfileInput>,
) -> Result<(StatusCode, Json<ShippingProfileResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_CREATE],
        "Permission denied: fulfillments:create required",
    )?;

    let profile = ShippingProfileService::new(ctx.db.clone())
        .create_shipping_profile(tenant.id, input)
        .await
        .map_err(map_shipping_profile_error)?;

    Ok((StatusCode::CREATED, Json(profile)))
}

/// Show admin shipping profile
#[utoipa::path(
    get,
    path = "/admin/shipping-profiles/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping profile ID")),
    responses(
        (status = 200, description = "Shipping profile details", body = ShippingProfileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping profile not found")
    )
)]
pub async fn show_shipping_profile(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ShippingProfileResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let profile = ShippingProfileService::new(ctx.db.clone())
        .get_shipping_profile(
            tenant.id,
            id,
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(Json(profile))
}

/// Update admin shipping profile
#[utoipa::path(
    post,
    path = "/admin/shipping-profiles/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping profile ID")),
    request_body = UpdateShippingProfileInput,
    responses(
        (status = 200, description = "Shipping profile updated successfully", body = ShippingProfileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping profile not found")
    )
)]
pub async fn update_shipping_profile(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateShippingProfileInput>,
) -> Result<Json<ShippingProfileResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let profile = ShippingProfileService::new(ctx.db.clone())
        .update_shipping_profile(tenant.id, id, input)
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(Json(profile))
}

/// Deactivate admin shipping profile
#[utoipa::path(
    post,
    path = "/admin/shipping-profiles/{id}/deactivate",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping profile ID")),
    responses(
        (status = 200, description = "Shipping profile deactivated successfully", body = ShippingProfileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping profile not found")
    )
)]
pub async fn deactivate_shipping_profile(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ShippingProfileResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let profile = ShippingProfileService::new(ctx.db.clone())
        .deactivate_shipping_profile(tenant.id, id)
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(Json(profile))
}

/// Reactivate admin shipping profile
#[utoipa::path(
    post,
    path = "/admin/shipping-profiles/{id}/reactivate",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping profile ID")),
    responses(
        (status = 200, description = "Shipping profile reactivated successfully", body = ShippingProfileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping profile not found")
    )
)]
pub async fn reactivate_shipping_profile(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ShippingProfileResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let profile = ShippingProfileService::new(ctx.db.clone())
        .reactivate_shipping_profile(tenant.id, id)
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(Json(profile))
}

/// List admin shipping options
#[utoipa::path(
    get,
    path = "/admin/shipping-options",
    tag = "admin",
    params(ListShippingOptionsParams),
    responses(
        (status = 200, description = "Shipping options", body = PaginatedResponse<ShippingOptionResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_shipping_options(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Query(params): Query<ListShippingOptionsParams>,
) -> Result<Json<PaginatedResponse<ShippingOptionResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let mut items = FulfillmentService::new(ctx.db.clone())
        .list_all_shipping_options(
            tenant.id,
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    if let Some(active) = params.active {
        items.retain(|option| option.active == active);
    }
    if let Some(currency_code) = params.currency_code.as_deref() {
        items.retain(|option| option.currency_code.eq_ignore_ascii_case(currency_code));
    }
    if let Some(provider_id) = params.provider_id.as_deref() {
        items.retain(|option| option.provider_id.eq_ignore_ascii_case(provider_id));
    }
    if let Some(search) = params.search.as_deref() {
        let search = search.trim().to_ascii_lowercase();
        if !search.is_empty() {
            items.retain(|option| option.name.to_ascii_lowercase().contains(&search));
        }
    }
    let total = items.len() as u64;
    let data = items
        .into_iter()
        .skip(pagination.offset() as usize)
        .take(pagination.limit() as usize)
        .collect();

    Ok(Json(PaginatedResponse {
        data,
        meta: super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Create admin shipping option
#[utoipa::path(
    post,
    path = "/admin/shipping-options",
    tag = "admin",
    request_body = CreateShippingOptionInput,
    responses(
        (status = 201, description = "Shipping option created successfully", body = ShippingOptionResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn create_shipping_option(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateShippingOptionInput>,
) -> Result<(StatusCode, Json<ShippingOptionResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_CREATE],
        "Permission denied: fulfillments:create required",
    )?;

    validate_shipping_option_profile_inputs(
        &ctx.db,
        tenant.id,
        input.allowed_shipping_profile_slugs.as_ref(),
    )
    .await?;

    let option = FulfillmentService::new(ctx.db.clone())
        .create_shipping_option(tenant.id, input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok((StatusCode::CREATED, Json(option)))
}

/// Show admin shipping option
#[utoipa::path(
    get,
    path = "/admin/shipping-options/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping option ID")),
    responses(
        (status = 200, description = "Shipping option details", body = ShippingOptionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping option not found")
    )
)]
pub async fn show_shipping_option(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let option = FulfillmentService::new(ctx.db.clone())
        .get_shipping_option(
            tenant.id,
            id,
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| match err {
            rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_) => {
                Error::NotFound
            }
            other => Error::BadRequest(other.to_string()),
        })?;

    Ok(Json(option))
}

/// Update admin shipping option
#[utoipa::path(
    post,
    path = "/admin/shipping-options/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping option ID")),
    request_body = UpdateShippingOptionInput,
    responses(
        (status = 200, description = "Shipping option updated successfully", body = ShippingOptionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping option not found")
    )
)]
pub async fn update_shipping_option(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateShippingOptionInput>,
) -> Result<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    validate_shipping_option_profile_inputs(
        &ctx.db,
        tenant.id,
        input.allowed_shipping_profile_slugs.as_ref(),
    )
    .await?;

    let option = FulfillmentService::new(ctx.db.clone())
        .update_shipping_option(tenant.id, id, input)
        .await
        .map_err(|err| match err {
            rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_) => {
                Error::NotFound
            }
            other => Error::BadRequest(other.to_string()),
        })?;

    Ok(Json(option))
}

/// Deactivate admin shipping option
#[utoipa::path(
    post,
    path = "/admin/shipping-options/{id}/deactivate",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping option ID")),
    responses(
        (status = 200, description = "Shipping option deactivated successfully", body = ShippingOptionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping option not found")
    )
)]
pub async fn deactivate_shipping_option(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let option = FulfillmentService::new(ctx.db.clone())
        .deactivate_shipping_option(tenant.id, id)
        .await
        .map_err(|err| match err {
            rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_) => {
                Error::NotFound
            }
            other => Error::BadRequest(other.to_string()),
        })?;

    Ok(Json(option))
}

/// Reactivate admin shipping option
#[utoipa::path(
    post,
    path = "/admin/shipping-options/{id}/reactivate",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping option ID")),
    responses(
        (status = 200, description = "Shipping option reactivated successfully", body = ShippingOptionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping option not found")
    )
)]
pub async fn reactivate_shipping_option(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let option = FulfillmentService::new(ctx.db.clone())
        .reactivate_shipping_option(tenant.id, id)
        .await
        .map_err(|err| match err {
            rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_) => {
                Error::NotFound
            }
            other => Error::BadRequest(other.to_string()),
        })?;

    Ok(Json(option))
}

/// List admin fulfillments
#[utoipa::path(
    get,
    path = "/admin/fulfillments",
    tag = "admin",
    params(ListFulfillmentsParams),
    responses(
        (status = 200, description = "Fulfillments", body = PaginatedResponse<FulfillmentResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_fulfillments(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListFulfillmentsParams>,
) -> Result<Json<PaginatedResponse<FulfillmentResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (fulfillments, total) = FulfillmentService::new(ctx.db.clone())
        .list_fulfillments(
            tenant.id,
            ListFulfillmentsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status,
                order_id: params.order_id,
                customer_id: params.customer_id,
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(PaginatedResponse {
        data: fulfillments,
        meta: super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Authorize admin payment collection
#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/authorize",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = AuthorizePaymentInput,
    responses(
        (status = 200, description = "Payment collection authorized", body = PaymentCollectionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn authorize_payment_collection(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AuthorizePaymentInput>,
) -> Result<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let collection = PaymentService::new(ctx.db.clone())
        .authorize_collection(tenant.id, id, input)
        .await
        .map_err(map_payment_error)?;

    Ok(Json(collection))
}

/// Capture admin payment collection
#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/capture",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = CapturePaymentInput,
    responses(
        (status = 200, description = "Payment collection captured", body = PaymentCollectionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn capture_payment_collection(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CapturePaymentInput>,
) -> Result<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let collection = PaymentService::new(ctx.db.clone())
        .capture_collection(tenant.id, id, input)
        .await
        .map_err(map_payment_error)?;

    Ok(Json(collection))
}

/// Cancel admin payment collection
#[utoipa::path(
    post,
    path = "/admin/payment-collections/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Payment collection ID")),
    request_body = CancelPaymentInput,
    responses(
        (status = 200, description = "Payment collection cancelled", body = PaymentCollectionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Payment collection not found")
    )
)]
pub async fn cancel_payment_collection(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelPaymentInput>,
) -> Result<Json<PaymentCollectionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PAYMENTS_UPDATE],
        "Permission denied: payments:update required",
    )?;

    let collection = PaymentService::new(ctx.db.clone())
        .cancel_collection(tenant.id, id, input)
        .await
        .map_err(map_payment_error)?;

    Ok(Json(collection))
}

/// Show admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments",
    tag = "admin",
    request_body = CreateFulfillmentInput,
    responses(
        (status = 201, description = "Fulfillment created", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn create_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateFulfillmentInput>,
) -> Result<(StatusCode, Json<FulfillmentResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_CREATE],
        "Permission denied: fulfillments:create required",
    )?;

    let fulfillment = FulfillmentOrchestrationService::new(ctx.db.clone())
        .create_manual_fulfillment(tenant.id, input)
        .await
        .map_err(map_fulfillment_orchestration_error)?;

    Ok((StatusCode::CREATED, Json(fulfillment)))
}

/// Show admin fulfillment
#[utoipa::path(
    get,
    path = "/admin/fulfillments/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    responses(
        (status = 200, description = "Fulfillment details", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn show_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .get_fulfillment(tenant.id, id)
        .await
        .map_err(map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

/// Ship admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/ship",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = ShipFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment shipped", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn ship_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ShipFulfillmentInput>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .ship_fulfillment(tenant.id, id, input)
        .await
        .map_err(map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

/// Deliver admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/deliver",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = DeliverFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment delivered", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn deliver_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<DeliverFulfillmentInput>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .deliver_fulfillment(tenant.id, id, input)
        .await
        .map_err(map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

/// Reopen admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/reopen",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = ReopenFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment reopened", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn reopen_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ReopenFulfillmentInput>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .reopen_fulfillment(tenant.id, id, input)
        .await
        .map_err(map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

/// Reship admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/reship",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = ReshipFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment marked for reship", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn reship_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ReshipFulfillmentInput>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .reship_fulfillment(tenant.id, id, input)
        .await
        .map_err(map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

/// Cancel admin fulfillment
#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = CancelFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment cancelled", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn cancel_fulfillment(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelFulfillmentInput>,
) -> Result<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(ctx.db.clone())
        .cancel_fulfillment(tenant.id, id, input)
        .await
        .map_err(map_fulfillment_error)?;

    Ok(Json(fulfillment))
}

fn map_payment_error(error: rustok_payment::error::PaymentError) -> Error {
    match error {
        rustok_payment::error::PaymentError::PaymentCollectionNotFound(_)
        | rustok_payment::error::PaymentError::RefundNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

fn map_order_error(error: rustok_order::error::OrderError) -> Error {
    match error {
        rustok_order::error::OrderError::OrderNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

fn map_fulfillment_error(error: rustok_fulfillment::error::FulfillmentError) -> Error {
    match error {
        rustok_fulfillment::error::FulfillmentError::FulfillmentNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

fn map_fulfillment_orchestration_error(error: FulfillmentOrchestrationError) -> Error {
    match error {
        FulfillmentOrchestrationError::OrderNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

fn map_shipping_profile_error(error: crate::CommerceError) -> Error {
    match error {
        crate::CommerceError::ShippingProfileNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

async fn validate_product_shipping_profile_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    shipping_profile_slug: Option<&str>,
) -> Result<()> {
    let Some(slug) = shipping_profile_slug.and_then(normalize_shipping_profile_slug) else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slug_exists(tenant_id, &slug)
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(())
}

async fn validate_shipping_option_profile_inputs(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    allowed_shipping_profile_slugs: Option<&Vec<String>>,
) -> Result<()> {
    let Some(slugs) = allowed_shipping_profile_slugs else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::body::{to_bytes, Body};
    use axum::extract::State;
    use axum::http::{Request, StatusCode};
    use axum::middleware::{from_fn_with_state, Next};
    use axum::response::Response;
    use axum::Router;
    use loco_rs::app::{AppContext, SharedStore};
    use loco_rs::cache;
    use loco_rs::environment::Environment;
    use loco_rs::storage::{self, Storage};
    use loco_rs::tests_cfg::config::test_config;
    use rust_decimal::Decimal;
    use rustok_api::{AuthContext, AuthContextExtension, TenantContext, TenantContextExtension};
    use rustok_core::events::EventTransport;
    use rustok_core::Permission;
    use rustok_test_utils::db::setup_test_db;
    use rustok_test_utils::{mock_transactional_event_bus, MockEventTransport};
    use sea_orm::ConnectionTrait;
    use serde_json::json;
    use std::str::FromStr;
    use std::sync::Arc;
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::dto::{
        AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
        CompleteRefundInput, CreateFulfillmentInput, CreateFulfillmentItemInput, CreateOrderInput,
        CreateOrderLineItemInput, CreatePaymentCollectionInput, CreateRefundInput,
        DeliverFulfillmentInput, FulfillmentItemQuantityInput, RefundResponse,
        ShipFulfillmentInput, UpdateShippingOptionInput,
    };
    use crate::{FulfillmentService, OrderService, PaymentService, ShippingProfileService};

    mod support {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support.rs"));
    }

    fn test_app_context(db: sea_orm::DatabaseConnection) -> AppContext {
        let shared_store = Arc::new(SharedStore::default());
        let event_transport: Arc<dyn EventTransport> = Arc::new(MockEventTransport::new());
        shared_store.insert(event_transport);

        AppContext {
            environment: Environment::Test,
            db,
            queue_provider: None,
            config: test_config(),
            mailer: None,
            storage: Storage::single(storage::drivers::mem::new()).into(),
            cache: Arc::new(cache::Cache::new(cache::drivers::null::new())),
            shared_store,
        }
    }

    async fn seed_tenant_context(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                tenant_id.into(),
                "Admin Test Tenant".into(),
                format!("admin-test-{tenant_id}").into(),
                sea_orm::Value::String(None),
                json!({}).to_string().into(),
                "en".into(),
                true.into(),
            ],
        ))
        .await
        .expect("tenant should be inserted");

        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                "commerce".into(),
                true.into(),
                json!({}).to_string().into(),
            ],
        ))
        .await
        .expect("commerce module should be enabled for tenant");
    }

    #[derive(Clone)]
    struct TransportRequestContext {
        tenant: TenantContext,
        auth: AuthContext,
    }

    async fn inject_transport_context(
        State(context): State<TransportRequestContext>,
        mut req: axum::extract::Request,
        next: Next,
    ) -> Response {
        req.extensions_mut()
            .insert(TenantContextExtension(context.tenant));
        req.extensions_mut()
            .insert(AuthContextExtension(context.auth));
        next.run(req).await
    }

    fn admin_transport_router(ctx: AppContext, tenant: TenantContext, auth: AuthContext) -> Router {
        let routes = crate::controllers::routes();
        let mut router = Router::new();
        for handler in routes.handlers {
            router = router.route(&handler.uri, handler.method.with_state(ctx.clone()));
        }

        router.layer(from_fn_with_state(
            TransportRequestContext { tenant, auth },
            inject_transport_context,
        ))
    }

    #[tokio::test]
    async fn admin_order_transport_returns_order_with_payment_and_fulfillment() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-ORDER-1".to_string()),
                        title: "Admin Order".to_string(),
                        quantity: 2,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-transport" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-order-transport" }),
                },
            )
            .await
            .expect("order should be created");
        let payment_collection = PaymentService::new(db.clone())
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(order.id),
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    amount: order.total_amount,
                    metadata: json!({ "source": "admin-order-payment" }),
                },
            )
            .await
            .expect("payment collection should be created");
        let fulfillment = FulfillmentService::new(db.clone())
            .create_fulfillment(
                tenant_id,
                CreateFulfillmentInput {
                    order_id: order.id,
                    shipping_option_id: None,
                    customer_id: Some(customer_id),
                    carrier: Some("manual".to_string()),
                    tracking_number: Some("TRACK-123".to_string()),
                    items: None,
                    metadata: json!({ "source": "admin-order-fulfillment" }),
                },
            )
            .await
            .expect("fulfillment should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/orders/{}", order.id))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected admin order body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be JSON");
        assert_eq!(payload["order"]["id"], json!(order.id));
        assert_eq!(payload["order"]["customer_id"], json!(customer_id));
        assert_eq!(
            payload["payment_collection"]["id"],
            json!(payment_collection.id)
        );
        assert_eq!(payload["payment_collection"]["order_id"], json!(order.id));
        assert_eq!(payload["fulfillment"]["id"], json!(fulfillment.id));
        assert_eq!(payload["fulfillment"]["order_id"], json!(order.id));
    }

    #[tokio::test]
    async fn admin_order_transport_returns_typed_adjustments_and_totals() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-ORDER-ADJUSTMENT-1".to_string()),
                        title: "Admin Adjusted Order".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-adjustment-transport" }),
                    }],
                    adjustments: vec![rustok_order::dto::CreateOrderAdjustmentInput {
                        line_item_index: Some(0),
                        source_type: "Promotion".to_string(),
                        source_id: Some("promo-admin".to_string()),
                        amount: Decimal::from_str("5.00").expect("valid decimal"),
                        metadata: json!({
                            "rule_code": "admin-adjustment",
                            "display_label": "Admin promotion"
                        }),
                    }],
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-order-adjustment-transport" }),
                },
            )
            .await
            .expect("order should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/orders/{}", order.id))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected admin order adjustment body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be JSON");
        assert_eq!(payload["order"]["subtotal_amount"], json!("25"));
        assert_eq!(payload["order"]["adjustment_total"], json!("5"));
        assert_eq!(payload["order"]["total_amount"], json!("20"));
        assert_eq!(
            payload["order"]["adjustments"][0]["line_item_id"],
            payload["order"]["line_items"][0]["id"]
        );
        assert_eq!(
            payload["order"]["adjustments"][0]["source_type"],
            json!("promotion")
        );
        assert_eq!(
            payload["order"]["adjustments"][0]["source_id"],
            json!("promo-admin")
        );
        assert_eq!(payload["order"]["adjustments"][0]["amount"], json!("5"));
        assert_eq!(
            payload["order"]["adjustments"][0]["currency_code"],
            json!("EUR")
        );
        assert_eq!(
            payload["order"]["adjustments"][0]["metadata"],
            json!({ "rule_code": "admin-adjustment" })
        );
    }

    #[tokio::test]
    async fn admin_order_transport_returns_shipping_total_and_shipping_scoped_adjustments() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::from_str("9.99").expect("valid decimal"),
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-ORDER-SHIPPING-ADJUSTMENT-1".to_string()),
                        title: "Admin Shipping Adjusted Order".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-shipping-adjustment-transport" }),
                    }],
                    adjustments: vec![rustok_order::dto::CreateOrderAdjustmentInput {
                        line_item_index: None,
                        source_type: "Promotion".to_string(),
                        source_id: Some("promo-shipping-admin".to_string()),
                        amount: Decimal::from_str("4.99").expect("valid decimal"),
                        metadata: json!({
                            "rule_code": "admin-shipping-adjustment",
                            "scope": "shipping",
                            "display_label": "Admin shipping promotion"
                        }),
                    }],
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-order-shipping-adjustment-transport" }),
                },
            )
            .await
            .expect("order should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/orders/{}", order.id))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected admin shipping adjustment body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be JSON");
        assert_eq!(payload["order"]["shipping_total"], json!("9.99"));
        assert_eq!(payload["order"]["adjustment_total"], json!("4.99"));
        assert_eq!(payload["order"]["total_amount"], json!("30"));
        assert_eq!(
            payload["order"]["adjustments"][0]["line_item_id"],
            json!(null)
        );
        assert_eq!(
            payload["order"]["adjustments"][0]["source_type"],
            json!("promotion")
        );
        assert_eq!(
            payload["order"]["adjustments"][0]["source_id"],
            json!("promo-shipping-admin")
        );
        assert_eq!(payload["order"]["adjustments"][0]["amount"], json!("4.99"));
        assert_eq!(
            payload["order"]["adjustments"][0]["currency_code"],
            json!("EUR")
        );
        assert_eq!(
            payload["order"]["adjustments"][0]["metadata"],
            json!({ "rule_code": "admin-shipping-adjustment", "scope": "shipping" })
        );
    }

    #[tokio::test]
    async fn admin_orders_transport_lists_orders_with_pagination_and_status_filter() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_a = Uuid::new_v4();
        let customer_b = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_LIST],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let service = OrderService::new(db.clone(), mock_transactional_event_bus());
        let first_order = service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_a),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-ORDER-LIST-1".to_string()),
                        title: "Admin List Order 1".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("15.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-list" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-order-list" }),
                },
            )
            .await
            .expect("first order should be created");
        let second_order = service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_b),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-ORDER-LIST-2".to_string()),
                        title: "Admin List Order 2".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-list" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-order-list" }),
                },
            )
            .await
            .expect("second order should be created");
        service
            .cancel_order(
                tenant_id,
                actor_id,
                second_order.id,
                Some("filtered".to_string()),
            )
            .await
            .expect("second order should be cancelled");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/admin/orders?status=cancelled&customer_id={}&page=1&per_page=1",
                        customer_b
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected admin orders body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be JSON");
        let data = payload["data"].as_array().expect("data should be array");
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["id"], json!(second_order.id));
        assert_eq!(data[0]["status"], json!("cancelled"));
        assert_eq!(payload["meta"]["total"], json!(1));
        assert_eq!(payload["meta"]["page"], json!(1));
        assert_eq!(payload["meta"]["per_page"], json!(1));
        assert_ne!(data[0]["id"], json!(first_order.id));
    }

    #[tokio::test]
    async fn admin_payment_collections_transport_lists_collections_with_pagination_and_filters() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_a = Uuid::new_v4();
        let customer_b = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PAYMENTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
        let payment_service = PaymentService::new(db.clone());
        let first_order = order_service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_a),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-PAYMENT-LIST-1".to_string()),
                        title: "Admin Payment List 1".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("15.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-payment-list" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-payment-list" }),
                },
            )
            .await
            .expect("first order should be created");
        let second_order = order_service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_b),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-PAYMENT-LIST-2".to_string()),
                        title: "Admin Payment List 2".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-payment-list" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-payment-list" }),
                },
            )
            .await
            .expect("second order should be created");
        let first_collection = payment_service
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(first_order.id),
                    customer_id: Some(customer_a),
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("15.00").expect("valid decimal"),
                    metadata: json!({ "source": "admin-payment-list" }),
                },
            )
            .await
            .expect("first collection should be created");
        let second_collection = payment_service
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(second_order.id),
                    customer_id: Some(customer_b),
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("20.00").expect("valid decimal"),
                    metadata: json!({ "source": "admin-payment-list" }),
                },
            )
            .await
            .expect("second collection should be created");
        payment_service
            .cancel_collection(
                tenant_id,
                second_collection.id,
                CancelPaymentInput {
                    reason: Some("filtered".to_string()),
                    metadata: json!({ "source": "admin-payment-list" }),
                },
            )
            .await
            .expect("second collection should be cancelled");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/admin/payment-collections?status=cancelled&customer_id={}&page=1&per_page=1",
                        customer_b
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected admin payment collections body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be JSON");
        let data = payload["data"].as_array().expect("data should be array");
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["id"], json!(second_collection.id));
        assert_eq!(data[0]["status"], json!("cancelled"));
        assert_eq!(payload["meta"]["total"], json!(1));
        assert_ne!(data[0]["id"], json!(first_collection.id));
    }

    #[tokio::test]
    async fn admin_refunds_transport_creates_completes_cancels_and_lists_refunds() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PAYMENTS_READ, Permission::PAYMENTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
        let payment_service = PaymentService::new(db.clone());
        let order = order_service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-REFUND-LIFECYCLE-1".to_string()),
                        title: "Admin Refund Lifecycle".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-refund-lifecycle" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-refund-lifecycle" }),
                },
            )
            .await
            .expect("order should be created");
        let order = order_service
            .confirm_order(tenant_id, actor_id, order.id)
            .await
            .expect("order should be confirmed");
        let collection = payment_service
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(order.id),
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("25.00").expect("valid decimal"),
                    metadata: json!({ "source": "admin-refund-lifecycle" }),
                },
            )
            .await
            .expect("collection should be created");
        payment_service
            .authorize_collection(
                tenant_id,
                collection.id,
                AuthorizePaymentInput {
                    provider_id: Some("manual".to_string()),
                    provider_payment_id: Some("admin-refund-1".to_string()),
                    amount: None,
                    metadata: json!({ "step": "authorized" }),
                },
            )
            .await
            .expect("collection should be authorized");
        payment_service
            .capture_collection(
                tenant_id,
                collection.id,
                CapturePaymentInput {
                    amount: Some(Decimal::from_str("25.00").expect("valid decimal")),
                    metadata: json!({ "step": "captured" }),
                },
            )
            .await
            .expect("collection should be captured");

        let app = admin_transport_router(test_app_context(db), tenant, auth);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/admin/payment-collections/{}/refunds",
                        collection.id
                    ))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        serde_json::to_vec(&CreateRefundInput {
                            amount: Decimal::from_str("10.00").expect("valid decimal"),
                            reason: Some("customer-request".to_string()),
                            metadata: json!({ "step": "create-1" }),
                        })
                        .expect("create refund body"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        let created_refund: RefundResponse =
            serde_json::from_slice(&create_body).expect("refund response should parse");
        assert_eq!(created_refund.status, "pending");

        let complete_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/refunds/{}/complete", created_refund.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        serde_json::to_vec(&CompleteRefundInput {
                            metadata: json!({ "step": "complete-1" }),
                        })
                        .expect("complete refund body"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(complete_response.status(), StatusCode::OK);

        let second_create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/admin/payment-collections/{}/refunds",
                        collection.id
                    ))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        serde_json::to_vec(&CreateRefundInput {
                            amount: Decimal::from_str("5.00").expect("valid decimal"),
                            reason: Some("ops-review".to_string()),
                            metadata: json!({ "step": "create-2" }),
                        })
                        .expect("create refund body"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        let second_create_body = to_bytes(second_create_response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        let second_refund: RefundResponse =
            serde_json::from_slice(&second_create_body).expect("refund response should parse");

        let cancel_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/refunds/{}/cancel", second_refund.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        serde_json::to_vec(&CancelRefundInput {
                            reason: Some("review-failed".to_string()),
                            metadata: json!({ "step": "cancel-2" }),
                        })
                        .expect("cancel refund body"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(cancel_response.status(), StatusCode::OK);

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/admin/refunds?payment_collection_id={}",
                        collection.id
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        let list_body = to_bytes(list_response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        let list_payload: serde_json::Value =
            serde_json::from_slice(&list_body).expect("response should be JSON");
        assert_eq!(list_payload["meta"]["total"], json!(2));
        let listed_ids = list_payload["data"]
            .as_array()
            .expect("data should be array")
            .iter()
            .filter_map(|item| item["id"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(listed_ids.len(), 2);
        assert!(listed_ids.contains(&second_refund.id.to_string().as_str()));
        assert!(listed_ids.contains(&created_refund.id.to_string().as_str()));

        let detail_response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/payment-collections/{}", collection.id))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        let detail_body = to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        let detail_payload: serde_json::Value =
            serde_json::from_slice(&detail_body).expect("response should be JSON");
        assert_eq!(detail_payload["refunded_amount"], json!("10"));
        assert_eq!(
            detail_payload["refunds"]
                .as_array()
                .expect("refunds should be array")
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn admin_shipping_profiles_transport_supports_create_update_and_list() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![
                Permission::FULFILLMENTS_READ,
                Permission::FULFILLMENTS_CREATE,
                Permission::FULFILLMENTS_UPDATE,
            ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/shipping-profiles")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "slug": " bulky-freight ",
                            "name": "Bulky Freight",
                            "description": "Large parcel handling",
                            "metadata": { "source": "admin-shipping-profiles" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create request should succeed");
        let create_status = create_response.status();
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create response should read");
        assert_eq!(
            create_status,
            StatusCode::CREATED,
            "unexpected create body: {}",
            String::from_utf8_lossy(&create_body)
        );

        let created: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create response should be JSON");
        let profile_id = created["id"]
            .as_str()
            .expect("created shipping profile id should be present");
        assert_eq!(created["slug"], json!("bulky-freight"));

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/shipping-profiles?search=bulky&page=1&per_page=10")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("list request should succeed");
        let list_status = list_response.status();
        let list_body = to_bytes(list_response.into_body(), usize::MAX)
            .await
            .expect("list response should read");
        assert_eq!(
            list_status,
            StatusCode::OK,
            "unexpected list body: {}",
            String::from_utf8_lossy(&list_body)
        );
        let listed: serde_json::Value =
            serde_json::from_slice(&list_body).expect("list response should be JSON");
        assert_eq!(listed["meta"]["total"], json!(1));
        assert_eq!(listed["data"][0]["id"], json!(profile_id));

        let update_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/shipping-profiles/{profile_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        serde_json::to_string(&crate::dto::UpdateShippingProfileInput {
                            slug: None,
                            translations: Some(vec![crate::dto::ShippingProfileTranslationInput {
                                locale: "en".to_string(),
                                name: "Oversize Freight".to_string(),
                                description: Some("Updated profile".to_string()),
                            }]),
                            metadata: Some(json!({ "updated": true })),
                        })
                        .expect("update payload should serialize"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update request should succeed");
        let update_status = update_response.status();
        let update_body = to_bytes(update_response.into_body(), usize::MAX)
            .await
            .expect("update response should read");
        assert_eq!(
            update_status,
            StatusCode::OK,
            "unexpected update body: {}",
            String::from_utf8_lossy(&update_body)
        );
        let updated: serde_json::Value =
            serde_json::from_slice(&update_body).expect("update response should be JSON");
        assert_eq!(updated["name"], json!("Oversize Freight"));
        assert_eq!(updated["metadata"]["updated"], json!(true));

        let show_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/shipping-profiles/{profile_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("show request should succeed");
        let show_status = show_response.status();
        let show_body = to_bytes(show_response.into_body(), usize::MAX)
            .await
            .expect("show response should read");
        assert_eq!(
            show_status,
            StatusCode::OK,
            "unexpected show body: {}",
            String::from_utf8_lossy(&show_body)
        );
        let shown: serde_json::Value =
            serde_json::from_slice(&show_body).expect("show response should be JSON");
        assert_eq!(shown["id"], json!(profile_id));
        assert_eq!(shown["slug"], json!("bulky-freight"));

        let deactivate_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/shipping-profiles/{profile_id}/deactivate"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("deactivate request should succeed");
        let deactivate_body = to_bytes(deactivate_response.into_body(), usize::MAX)
            .await
            .expect("deactivate response should read");
        let deactivated: serde_json::Value =
            serde_json::from_slice(&deactivate_body).expect("deactivate response should be JSON");
        assert_eq!(deactivated["active"], json!(false));

        let reactivate_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/shipping-profiles/{profile_id}/reactivate"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("reactivate request should succeed");
        let reactivate_body = to_bytes(reactivate_response.into_body(), usize::MAX)
            .await
            .expect("reactivate response should read");
        let reactivated: serde_json::Value =
            serde_json::from_slice(&reactivate_body).expect("reactivate response should be JSON");
        assert_eq!(reactivated["active"], json!(true));
    }

    #[tokio::test]
    async fn admin_shipping_options_transport_supports_create_update_and_list() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        ShippingProfileService::new(db.clone())
            .create_shipping_profile(
                tenant_id,
                crate::dto::CreateShippingProfileInput {
                    slug: "bulky".to_string(),
                    translations: vec![crate::dto::ShippingProfileTranslationInput {
                        locale: "en".to_string(),
                        name: "Bulky".to_string(),
                        description: None,
                    }],
                    metadata: json!({}),
                },
            )
            .await
            .expect("bulky profile should be created");
        ShippingProfileService::new(db.clone())
            .create_shipping_profile(
                tenant_id,
                crate::dto::CreateShippingProfileInput {
                    slug: "cold-chain".to_string(),
                    translations: vec![crate::dto::ShippingProfileTranslationInput {
                        locale: "en".to_string(),
                        name: "Cold Chain".to_string(),
                        description: None,
                    }],
                    metadata: json!({}),
                },
            )
            .await
            .expect("cold-chain profile should be created");
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![
                Permission::FULFILLMENTS_READ,
                Permission::FULFILLMENTS_CREATE,
                Permission::FULFILLMENTS_UPDATE,
            ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/shipping-options")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "name": "Bulky Freight",
                            "currency_code": "eur",
                            "amount": "29.99",
                            "provider_id": " manual ",
                            "allowed_shipping_profile_slugs": [" bulky ", "cold-chain", "bulky"],
                            "metadata": { "source": "admin-shipping-options" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create request should succeed");
        let create_status = create_response.status();
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create response should read");
        assert_eq!(
            create_status,
            StatusCode::CREATED,
            "unexpected create body: {}",
            String::from_utf8_lossy(&create_body)
        );

        let created: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create response should be JSON");
        let option_id = created["id"]
            .as_str()
            .expect("created shipping option id should be present");
        assert_eq!(
            created["allowed_shipping_profile_slugs"],
            json!(["bulky", "cold-chain"])
        );

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/shipping-options?search=freight&page=1&per_page=10")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("list request should succeed");
        let list_status = list_response.status();
        let list_body = to_bytes(list_response.into_body(), usize::MAX)
            .await
            .expect("list response should read");
        assert_eq!(
            list_status,
            StatusCode::OK,
            "unexpected list body: {}",
            String::from_utf8_lossy(&list_body)
        );
        let listed: serde_json::Value =
            serde_json::from_slice(&list_body).expect("list response should be JSON");
        assert_eq!(listed["meta"]["total"], json!(1));
        assert_eq!(listed["data"][0]["id"], json!(option_id));

        let update_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/shipping-options/{option_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        serde_json::to_string(&UpdateShippingOptionInput {
                            translations: Some(vec![crate::dto::ShippingOptionTranslationInput {
                                locale: "en".to_string(),
                                name: "Cold Chain Freight".to_string(),
                            }]),
                            currency_code: Some("usd".to_string()),
                            amount: Some(Decimal::from_str("39.99").expect("valid decimal")),
                            provider_id: Some("custom-provider".to_string()),
                            allowed_shipping_profile_slugs: Some(vec!["cold-chain".to_string()]),
                            metadata: Some(json!({ "updated": true })),
                        })
                        .expect("update payload should serialize"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update request should succeed");
        let update_status = update_response.status();
        let update_body = to_bytes(update_response.into_body(), usize::MAX)
            .await
            .expect("update response should read");
        assert_eq!(
            update_status,
            StatusCode::OK,
            "unexpected update body: {}",
            String::from_utf8_lossy(&update_body)
        );
        let updated: serde_json::Value =
            serde_json::from_slice(&update_body).expect("update response should be JSON");
        assert_eq!(updated["name"], json!("Cold Chain Freight"));
        assert_eq!(updated["currency_code"], json!("USD"));
        assert_eq!(updated["provider_id"], json!("custom-provider"));
        assert_eq!(
            updated["allowed_shipping_profile_slugs"],
            json!(["cold-chain"])
        );

        let show_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/shipping-options/{option_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("show request should succeed");
        let show_status = show_response.status();
        let show_body = to_bytes(show_response.into_body(), usize::MAX)
            .await
            .expect("show response should read");
        assert_eq!(
            show_status,
            StatusCode::OK,
            "unexpected show body: {}",
            String::from_utf8_lossy(&show_body)
        );
        let shown: serde_json::Value =
            serde_json::from_slice(&show_body).expect("show response should be JSON");
        assert_eq!(shown["id"], json!(option_id));
        assert_eq!(shown["metadata"]["updated"], json!(true));
        assert_eq!(
            shown["metadata"]["shipping_profiles"]["allowed_slugs"],
            json!(["cold-chain"])
        );

        let deactivate_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/shipping-options/{option_id}/deactivate"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("deactivate request should succeed");
        let deactivate_body = to_bytes(deactivate_response.into_body(), usize::MAX)
            .await
            .expect("deactivate response should read");
        let deactivated: serde_json::Value =
            serde_json::from_slice(&deactivate_body).expect("deactivate response should be JSON");
        assert_eq!(deactivated["active"], json!(false));

        let inactive_list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/shipping-options?active=false&page=1&per_page=10")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("inactive list request should succeed");
        let inactive_list_body = to_bytes(inactive_list_response.into_body(), usize::MAX)
            .await
            .expect("inactive list response should read");
        let inactive_listed: serde_json::Value =
            serde_json::from_slice(&inactive_list_body).expect("inactive list should be JSON");
        assert_eq!(inactive_listed["meta"]["total"], json!(1));
        assert_eq!(inactive_listed["data"][0]["id"], json!(option_id));
        assert_eq!(inactive_listed["data"][0]["active"], json!(false));

        let reactivate_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/shipping-options/{option_id}/reactivate"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("reactivate request should succeed");
        let reactivate_body = to_bytes(reactivate_response.into_body(), usize::MAX)
            .await
            .expect("reactivate response should read");
        let reactivated: serde_json::Value =
            serde_json::from_slice(&reactivate_body).expect("reactivate response should be JSON");
        assert_eq!(reactivated["active"], json!(true));
    }

    #[tokio::test]
    async fn admin_fulfillments_transport_lists_fulfillments_with_pagination_and_filters() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_a = Uuid::new_v4();
        let customer_b = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::FULFILLMENTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
        let fulfillment_service = FulfillmentService::new(db.clone());
        let first_order = order_service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_a),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-FULFILLMENT-LIST-1".to_string()),
                        title: "Admin Fulfillment List 1".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("15.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-fulfillment-list" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-fulfillment-list" }),
                },
            )
            .await
            .expect("first order should be created");
        let second_order = order_service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_b),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-FULFILLMENT-LIST-2".to_string()),
                        title: "Admin Fulfillment List 2".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-fulfillment-list" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-fulfillment-list" }),
                },
            )
            .await
            .expect("second order should be created");
        let first_fulfillment = fulfillment_service
            .create_fulfillment(
                tenant_id,
                CreateFulfillmentInput {
                    order_id: first_order.id,
                    shipping_option_id: None,
                    customer_id: Some(customer_a),
                    carrier: None,
                    tracking_number: None,
                    items: None,
                    metadata: json!({ "source": "admin-fulfillment-list" }),
                },
            )
            .await
            .expect("first fulfillment should be created");
        let second_fulfillment = fulfillment_service
            .create_fulfillment(
                tenant_id,
                CreateFulfillmentInput {
                    order_id: second_order.id,
                    shipping_option_id: None,
                    customer_id: Some(customer_b),
                    carrier: None,
                    tracking_number: None,
                    items: None,
                    metadata: json!({ "source": "admin-fulfillment-list" }),
                },
            )
            .await
            .expect("second fulfillment should be created");
        fulfillment_service
            .ship_fulfillment(
                tenant_id,
                second_fulfillment.id,
                ShipFulfillmentInput {
                    carrier: "manual".to_string(),
                    tracking_number: "TRACK-FILTERED".to_string(),
                    items: None,
                    metadata: json!({ "source": "admin-fulfillment-list" }),
                },
            )
            .await
            .expect("second fulfillment should be shipped");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/admin/fulfillments?status=shipped&customer_id={}&page=1&per_page=1",
                        customer_b
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected admin fulfillments body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be JSON");
        let data = payload["data"].as_array().expect("data should be array");
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["id"], json!(second_fulfillment.id));
        assert_eq!(data[0]["status"], json!("shipped"));
        assert_eq!(payload["meta"]["total"], json!(1));
        assert_ne!(data[0]["id"], json!(first_fulfillment.id));
    }

    #[tokio::test]
    async fn admin_orders_transport_requires_orders_list_permission() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/orders")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_order_lifecycle_transport_marks_paid_ships_delivers_and_reads_detail() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_UPDATE, Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let service = OrderService::new(db.clone(), mock_transactional_event_bus());
        let order = service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-ORDER-LIFECYCLE-1".to_string()),
                        title: "Admin Lifecycle Order".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("30.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-lifecycle" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-order-lifecycle" }),
                },
            )
            .await
            .expect("order should be created");
        service
            .confirm_order(tenant_id, actor_id, order.id)
            .await
            .expect("order should be confirmed");

        let app = admin_transport_router(test_app_context(db), tenant, auth);

        let mark_paid_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/orders/{}/mark-paid", order.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "payment_id": "manual-payment-1",
                            "payment_method": "manual"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("mark paid request should succeed");
        assert_eq!(mark_paid_response.status(), StatusCode::OK);

        let ship_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/orders/{}/ship", order.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "tracking_number": "TRACK-ORDER-1",
                            "carrier": "manual"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("ship request should succeed");
        assert_eq!(ship_response.status(), StatusCode::OK);

        let deliver_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/orders/{}/deliver", order.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "delivered_signature": "signed-by-admin"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("deliver request should succeed");
        let deliver_status = deliver_response.status();
        let deliver_body = to_bytes(deliver_response.into_body(), usize::MAX)
            .await
            .expect("deliver body should read");
        assert_eq!(
            deliver_status,
            StatusCode::OK,
            "unexpected deliver body: {}",
            String::from_utf8_lossy(&deliver_body)
        );
        let delivered: serde_json::Value =
            serde_json::from_slice(&deliver_body).expect("deliver response should be JSON");
        assert_eq!(delivered["status"], json!("delivered"));
        assert_eq!(delivered["carrier"], json!("manual"));
        assert_eq!(delivered["tracking_number"], json!("TRACK-ORDER-1"));
        assert_eq!(delivered["delivered_signature"], json!("signed-by-admin"));

        let detail_response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/orders/{}", order.id))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("detail request should succeed");
        let detail_body = to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .expect("detail body should read");
        let detail: serde_json::Value =
            serde_json::from_slice(&detail_body).expect("detail response should be JSON");
        assert_eq!(detail["order"]["status"], json!("delivered"));
        assert_eq!(detail["order"]["payment_id"], json!("manual-payment-1"));
        assert_eq!(detail["order"]["payment_method"], json!("manual"));
    }

    #[tokio::test]
    async fn admin_order_lifecycle_transport_cancels_order() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let service = OrderService::new(db.clone(), mock_transactional_event_bus());
        let order = service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-ORDER-CANCEL-1".to_string()),
                        title: "Admin Cancel Order".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("10.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-cancel" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-order-cancel" }),
                },
            )
            .await
            .expect("order should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let cancel_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/orders/{}/cancel", order.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "reason": "customer-requested"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("cancel request should succeed");
        let cancel_status = cancel_response.status();
        let cancel_body = to_bytes(cancel_response.into_body(), usize::MAX)
            .await
            .expect("cancel body should read");
        assert_eq!(
            cancel_status,
            StatusCode::OK,
            "unexpected cancel body: {}",
            String::from_utf8_lossy(&cancel_body)
        );
        let cancelled: serde_json::Value =
            serde_json::from_slice(&cancel_body).expect("cancel response should be JSON");
        assert_eq!(cancelled["status"], json!("cancelled"));
        assert_eq!(
            cancelled["cancellation_reason"],
            json!("customer-requested")
        );
    }

    #[tokio::test]
    async fn admin_order_transport_requires_orders_read_permission() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/orders/{}", Uuid::new_v4()))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_payment_collection_transport_authorizes_captures_and_reads_detail() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PAYMENTS_READ, Permission::PAYMENTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-PAYMENT-1".to_string()),
                        title: "Admin Payment Order".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-payment-transport" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-payment-transport" }),
                },
            )
            .await
            .expect("order should be created");
        let payment_collection = PaymentService::new(db.clone())
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(order.id),
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    amount: order.total_amount,
                    metadata: json!({ "source": "admin-payment-transport" }),
                },
            )
            .await
            .expect("payment collection should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);

        let authorize_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/admin/payment-collections/{}/authorize",
                        payment_collection.id
                    ))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "provider_id": null,
                            "provider_payment_id": null,
                            "amount": "25.00",
                            "metadata": { "source": "admin-payment-authorize" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("authorize request should succeed");
        let authorize_status = authorize_response.status();
        let authorize_body = to_bytes(authorize_response.into_body(), usize::MAX)
            .await
            .expect("authorize body should read");
        assert_eq!(
            authorize_status,
            StatusCode::OK,
            "unexpected authorize body: {}",
            String::from_utf8_lossy(&authorize_body)
        );
        let authorized: serde_json::Value =
            serde_json::from_slice(&authorize_body).expect("authorize response should be JSON");
        assert_eq!(authorized["status"], json!("authorized"));

        let capture_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/admin/payment-collections/{}/capture",
                        payment_collection.id
                    ))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "amount": "25.00",
                            "metadata": { "source": "admin-payment-capture" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("capture request should succeed");
        let capture_status = capture_response.status();
        let capture_body = to_bytes(capture_response.into_body(), usize::MAX)
            .await
            .expect("capture body should read");
        assert_eq!(
            capture_status,
            StatusCode::OK,
            "unexpected capture body: {}",
            String::from_utf8_lossy(&capture_body)
        );
        let captured: serde_json::Value =
            serde_json::from_slice(&capture_body).expect("capture response should be JSON");
        assert_eq!(captured["status"], json!("captured"));
        assert_eq!(captured["captured_amount"], json!("25"));

        let detail_response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/admin/payment-collections/{}",
                        payment_collection.id
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("detail request should succeed");
        let detail_status = detail_response.status();
        let detail_body = to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .expect("detail body should read");
        assert_eq!(
            detail_status,
            StatusCode::OK,
            "unexpected payment detail body: {}",
            String::from_utf8_lossy(&detail_body)
        );
        let detail: serde_json::Value =
            serde_json::from_slice(&detail_body).expect("detail response should be JSON");
        assert_eq!(detail["id"], json!(payment_collection.id));
        assert_eq!(detail["status"], json!("captured"));
        assert_eq!(detail["order_id"], json!(order.id));
    }

    #[tokio::test]
    async fn admin_fulfillment_transport_creates_manual_fulfillment_with_typed_items() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![
                Permission::FULFILLMENTS_CREATE,
                Permission::FULFILLMENTS_READ,
            ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-FULLFILLMENT-CREATE-1".to_string()),
                        title: "Admin Fulfillment Create Order".to_string(),
                        quantity: 3,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({
                            "source": "admin-fulfillment-create",
                            "seller": {
                                "scope": "merchant-alpha",
                                "label": "Merchant Alpha"
                            }
                        }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-fulfillment-create" }),
                },
            )
            .await
            .expect("order should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/fulfillments")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "order_id": order.id,
                            "shipping_option_id": null,
                            "customer_id": null,
                            "carrier": null,
                            "tracking_number": null,
                            "items": [
                                {
                                    "order_line_item_id": order.line_items[0].id,
                                    "quantity": 2,
                                    "metadata": { "source": "admin-manual-fulfillment" }
                                }
                            ],
                            "metadata": { "source": "admin-manual-fulfillment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create request should succeed");
        let create_status = create_response.status();
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create body should read");
        assert_eq!(
            create_status,
            StatusCode::CREATED,
            "unexpected create body: {}",
            String::from_utf8_lossy(&create_body)
        );
        let created: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create response should be JSON");
        assert_eq!(created["order_id"], json!(order.id));
        assert_eq!(created["customer_id"], json!(customer_id));
        assert_eq!(
            created["items"][0]["order_line_item_id"],
            json!(order.line_items[0].id)
        );
        assert_eq!(created["items"][0]["quantity"], json!(2));
        assert_eq!(
            created["metadata"]["delivery_group"]["shipping_profile_slug"],
            json!("default")
        );
        assert_eq!(
            created["metadata"]["delivery_group"]["seller_scope"],
            json!("merchant-alpha")
        );
        assert_eq!(created["metadata"]["post_order"]["manual"], json!(true));
    }

    #[tokio::test]
    async fn admin_fulfillment_transport_rejects_overfulfillment_for_order_line_item() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::FULFILLMENTS_CREATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-FULLFILLMENT-OVER-1".to_string()),
                        title: "Admin Fulfillment Over Order".to_string(),
                        quantity: 2,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-fulfillment-over" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-fulfillment-over" }),
                },
            )
            .await
            .expect("order should be created");
        FulfillmentService::new(db.clone())
            .create_fulfillment(
                tenant_id,
                CreateFulfillmentInput {
                    order_id: order.id,
                    shipping_option_id: None,
                    customer_id: Some(customer_id),
                    carrier: None,
                    tracking_number: None,
                    items: Some(vec![CreateFulfillmentItemInput {
                        order_line_item_id: order.line_items[0].id,
                        quantity: 2,
                        metadata: json!({ "source": "existing-fulfillment" }),
                    }]),
                    metadata: json!({ "source": "existing-fulfillment" }),
                },
            )
            .await
            .expect("existing fulfillment should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let create_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/fulfillments")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "order_id": order.id,
                            "shipping_option_id": null,
                            "customer_id": customer_id,
                            "carrier": null,
                            "tracking_number": null,
                            "items": [
                                {
                                    "order_line_item_id": order.line_items[0].id,
                                    "quantity": 1,
                                    "metadata": {}
                                }
                            ],
                            "metadata": { "source": "admin-overfulfillment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create request should complete");
        let create_status = create_response.status();
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create body should read");
        assert_eq!(
            create_status,
            StatusCode::BAD_REQUEST,
            "unexpected overfulfillment body: {}",
            String::from_utf8_lossy(&create_body)
        );
        assert!(
            String::from_utf8_lossy(&create_body).contains("no remaining quantity"),
            "unexpected overfulfillment body: {}",
            String::from_utf8_lossy(&create_body)
        );
    }

    #[tokio::test]
    async fn admin_fulfillment_transport_ships_delivers_and_reads_detail() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![
                Permission::FULFILLMENTS_READ,
                Permission::FULFILLMENTS_UPDATE,
            ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-FULLFILLMENT-1".to_string()),
                        title: "Admin Fulfillment Order".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-fulfillment-transport" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-fulfillment-transport" }),
                },
            )
            .await
            .expect("order should be created");
        let fulfillment = FulfillmentService::new(db.clone())
            .create_fulfillment(
                tenant_id,
                CreateFulfillmentInput {
                    order_id: order.id,
                    shipping_option_id: None,
                    customer_id: Some(customer_id),
                    carrier: None,
                    tracking_number: None,
                    items: None,
                    metadata: json!({ "source": "admin-fulfillment-transport" }),
                },
            )
            .await
            .expect("fulfillment should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);

        let ship_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/fulfillments/{}/ship", fulfillment.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "carrier": "manual",
                            "tracking_number": "TRACK-456",
                            "metadata": { "source": "admin-fulfillment-ship" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("ship request should succeed");
        let ship_status = ship_response.status();
        let ship_body = to_bytes(ship_response.into_body(), usize::MAX)
            .await
            .expect("ship body should read");
        assert_eq!(
            ship_status,
            StatusCode::OK,
            "unexpected ship body: {}",
            String::from_utf8_lossy(&ship_body)
        );
        let shipped: serde_json::Value =
            serde_json::from_slice(&ship_body).expect("ship response should be JSON");
        assert_eq!(shipped["status"], json!("shipped"));
        assert_eq!(shipped["tracking_number"], json!("TRACK-456"));

        let deliver_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/fulfillments/{}/deliver", fulfillment.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "delivered_note": "Left at front desk",
                            "metadata": { "source": "admin-fulfillment-deliver" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("deliver request should succeed");
        let deliver_status = deliver_response.status();
        let deliver_body = to_bytes(deliver_response.into_body(), usize::MAX)
            .await
            .expect("deliver body should read");
        assert_eq!(
            deliver_status,
            StatusCode::OK,
            "unexpected deliver body: {}",
            String::from_utf8_lossy(&deliver_body)
        );
        let delivered: serde_json::Value =
            serde_json::from_slice(&deliver_body).expect("deliver response should be JSON");
        assert_eq!(delivered["status"], json!("delivered"));
        assert_eq!(delivered["delivered_note"], json!("Left at front desk"));

        let detail_response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/fulfillments/{}", fulfillment.id))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("detail request should succeed");
        let detail_status = detail_response.status();
        let detail_body = to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .expect("detail body should read");
        assert_eq!(
            detail_status,
            StatusCode::OK,
            "unexpected fulfillment detail body: {}",
            String::from_utf8_lossy(&detail_body)
        );
        let detail: serde_json::Value =
            serde_json::from_slice(&detail_body).expect("detail response should be JSON");
        assert_eq!(detail["id"], json!(fulfillment.id));
        assert_eq!(detail["status"], json!("delivered"));
        assert_eq!(detail["order_id"], json!(order.id));
    }

    #[tokio::test]
    async fn admin_fulfillment_transport_supports_partial_item_ship_and_deliver() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![
                Permission::FULFILLMENTS_READ,
                Permission::FULFILLMENTS_CREATE,
                Permission::FULFILLMENTS_UPDATE,
            ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-FULLFILLMENT-PARTIAL-1".to_string()),
                        title: "Admin Fulfillment Partial Order".to_string(),
                        quantity: 3,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-fulfillment-partial" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-fulfillment-partial" }),
                },
            )
            .await
            .expect("order should be created");
        let fulfillment = FulfillmentService::new(db.clone())
            .create_fulfillment(
                tenant_id,
                CreateFulfillmentInput {
                    order_id: order.id,
                    shipping_option_id: None,
                    customer_id: Some(customer_id),
                    carrier: None,
                    tracking_number: None,
                    items: Some(vec![CreateFulfillmentItemInput {
                        order_line_item_id: order.line_items[0].id,
                        quantity: 3,
                        metadata: json!({ "source": "admin-fulfillment-partial" }),
                    }]),
                    metadata: json!({ "source": "admin-fulfillment-partial" }),
                },
            )
            .await
            .expect("fulfillment should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let item_id = fulfillment.items[0].id;

        let ship_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/fulfillments/{}/ship", fulfillment.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "carrier": "manual",
                            "tracking_number": "TRACK-PARTIAL",
                            "items": [
                                {
                                    "fulfillment_item_id": item_id,
                                    "quantity": 2
                                }
                            ],
                            "metadata": { "source": "admin-fulfillment-partial-ship" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("partial ship request should succeed");
        let ship_body = to_bytes(ship_response.into_body(), usize::MAX)
            .await
            .expect("ship body should read");
        let shipped: serde_json::Value =
            serde_json::from_slice(&ship_body).expect("ship response should be JSON");
        assert_eq!(shipped["status"], json!("shipped"));
        assert_eq!(shipped["items"][0]["shipped_quantity"], json!(2));
        assert_eq!(shipped["items"][0]["delivered_quantity"], json!(0));

        let deliver_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/fulfillments/{}/deliver", fulfillment.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "delivered_note": "partial delivered",
                            "items": [
                                {
                                    "fulfillment_item_id": item_id,
                                    "quantity": 1
                                }
                            ],
                            "metadata": { "source": "admin-fulfillment-partial-deliver" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("partial deliver request should succeed");
        let deliver_body = to_bytes(deliver_response.into_body(), usize::MAX)
            .await
            .expect("deliver body should read");
        let delivered: serde_json::Value =
            serde_json::from_slice(&deliver_body).expect("deliver response should be JSON");
        assert_eq!(delivered["status"], json!("shipped"));
        assert_eq!(delivered["items"][0]["shipped_quantity"], json!(2));
        assert_eq!(delivered["items"][0]["delivered_quantity"], json!(1));
        assert_eq!(
            delivered["metadata"]["audit"]["events"][1]["type"],
            json!("deliver")
        );
    }

    #[tokio::test]
    async fn admin_fulfillment_transport_supports_reopen_and_reship() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Admin Test Tenant".to_string(),
            slug: format!("admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![
                Permission::FULFILLMENTS_READ,
                Permission::FULFILLMENTS_CREATE,
                Permission::FULFILLMENTS_UPDATE,
            ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(customer_id),
                    currency_code: "eur".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-FULLFILLMENT-REOPEN-1".to_string()),
                        title: "Admin Fulfillment Reopen Order".to_string(),
                        quantity: 2,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-fulfillment-reopen" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-fulfillment-reopen" }),
                },
            )
            .await
            .expect("order should be created");
        let fulfillment = FulfillmentService::new(db.clone())
            .create_fulfillment(
                tenant_id,
                CreateFulfillmentInput {
                    order_id: order.id,
                    shipping_option_id: None,
                    customer_id: Some(customer_id),
                    carrier: None,
                    tracking_number: None,
                    items: Some(vec![CreateFulfillmentItemInput {
                        order_line_item_id: order.line_items[0].id,
                        quantity: 2,
                        metadata: json!({ "source": "admin-fulfillment-reopen" }),
                    }]),
                    metadata: json!({ "source": "admin-fulfillment-reopen" }),
                },
            )
            .await
            .expect("fulfillment should be created");

        let app = admin_transport_router(test_app_context(db.clone()), tenant, auth);
        let item_id = fulfillment.items[0].id;

        FulfillmentService::new(db.clone())
            .ship_fulfillment(
                tenant_id,
                fulfillment.id,
                ShipFulfillmentInput {
                    carrier: "manual".to_string(),
                    tracking_number: "ADMIN-REOPEN-OLD".to_string(),
                    items: Some(vec![FulfillmentItemQuantityInput {
                        fulfillment_item_id: item_id,
                        quantity: 2,
                    }]),
                    metadata: json!({ "source": "admin-fulfillment-reopen-ship" }),
                },
            )
            .await
            .expect("fulfillment should ship");
        FulfillmentService::new(db.clone())
            .deliver_fulfillment(
                tenant_id,
                fulfillment.id,
                DeliverFulfillmentInput {
                    delivered_note: Some("done".to_string()),
                    items: Some(vec![FulfillmentItemQuantityInput {
                        fulfillment_item_id: item_id,
                        quantity: 2,
                    }]),
                    metadata: json!({ "source": "admin-fulfillment-reopen-deliver" }),
                },
            )
            .await
            .expect("fulfillment should deliver");

        let reopen_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/fulfillments/{}/reopen", fulfillment.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "items": [
                                {
                                    "fulfillment_item_id": item_id,
                                    "quantity": 1
                                }
                            ],
                            "metadata": { "source": "admin-fulfillment-reopen-step" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("reopen request should succeed");
        let reopen_body = to_bytes(reopen_response.into_body(), usize::MAX)
            .await
            .expect("reopen body should read");
        let reopened: serde_json::Value =
            serde_json::from_slice(&reopen_body).expect("reopen response should be JSON");
        assert_eq!(reopened["status"], json!("shipped"));
        assert_eq!(reopened["items"][0]["delivered_quantity"], json!(1));
        assert_eq!(reopened["delivered_note"], serde_json::Value::Null);

        let deliver_again = FulfillmentService::new(db.clone())
            .deliver_fulfillment(
                tenant_id,
                fulfillment.id,
                DeliverFulfillmentInput {
                    delivered_note: Some("done-again".to_string()),
                    items: Some(vec![FulfillmentItemQuantityInput {
                        fulfillment_item_id: item_id,
                        quantity: 1,
                    }]),
                    metadata: json!({ "source": "admin-fulfillment-redeliver" }),
                },
            )
            .await
            .expect("fulfillment should be delivered again");
        assert_eq!(deliver_again.status, "delivered");

        let reship_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/fulfillments/{}/reship", fulfillment.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "carrier": "manual",
                            "tracking_number": "ADMIN-REOPEN-NEW",
                            "items": [
                                {
                                    "fulfillment_item_id": item_id,
                                    "quantity": 2
                                }
                            ],
                            "metadata": { "source": "admin-fulfillment-reship-step" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("reship request should succeed");
        let reship_body = to_bytes(reship_response.into_body(), usize::MAX)
            .await
            .expect("reship body should read");
        let reshipped: serde_json::Value =
            serde_json::from_slice(&reship_body).expect("reship response should be JSON");
        assert_eq!(reshipped["status"], json!("shipped"));
        assert_eq!(reshipped["tracking_number"], json!("ADMIN-REOPEN-NEW"));
        assert_eq!(reshipped["items"][0]["delivered_quantity"], json!(0));
        assert_eq!(
            reshipped["metadata"]["audit"]["events"][4]["type"],
            json!("reship")
        );
    }
}
