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
        CapturePaymentInput, CreateProductInput, DeliverFulfillmentInput, DeliverOrderInput,
        FulfillmentResponse, MarkPaidOrderInput, OrderResponse, PaymentCollectionResponse,
        ProductResponse, ShipFulfillmentInput, ShipOrderInput, UpdateProductInput,
    },
    CatalogService, FulfillmentService, OrderService, PaymentService,
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
    path: Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    super::products::show_product(state, tenant, auth, path).await
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
            .list_orders(
                tenant.id,
                rustok_order::dto::ListOrdersInput {
                    page: pagination.page,
                    per_page: pagination.limit(),
                    status: params.status,
                    customer_id: params.customer_id,
                },
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
    Path(id): Path<Uuid>,
) -> Result<Json<AdminOrderDetailResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;

    let order = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx))
        .get_order(tenant.id, id)
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
        rustok_payment::error::PaymentError::PaymentCollectionNotFound(_) => Error::NotFound,
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
        CreateFulfillmentInput, CreateOrderInput, CreateOrderLineItemInput,
        CreatePaymentCollectionInput,
    };
    use crate::{FulfillmentService, OrderService, PaymentService};

    #[path = "../../../../tests/support.rs"]
    mod support;

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
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        sku: Some("ADMIN-ORDER-1".to_string()),
                        title: "Admin Order".to_string(),
                        quantity: 2,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-transport" }),
                    }],
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
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        sku: Some("ADMIN-ORDER-LIST-1".to_string()),
                        title: "Admin List Order 1".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("15.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-list" }),
                    }],
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
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        sku: Some("ADMIN-ORDER-LIST-2".to_string()),
                        title: "Admin List Order 2".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-list" }),
                    }],
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
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        sku: Some("ADMIN-ORDER-LIFECYCLE-1".to_string()),
                        title: "Admin Lifecycle Order".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("30.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-lifecycle" }),
                    }],
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
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        sku: Some("ADMIN-ORDER-CANCEL-1".to_string()),
                        title: "Admin Cancel Order".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("10.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-order-cancel" }),
                    }],
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
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        sku: Some("ADMIN-PAYMENT-1".to_string()),
                        title: "Admin Payment Order".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-payment-transport" }),
                    }],
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
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        sku: Some("ADMIN-FULLFILLMENT-1".to_string()),
                        title: "Admin Fulfillment Order".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-fulfillment-transport" }),
                    }],
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
}
