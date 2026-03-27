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
        CreateProductInput, FulfillmentResponse, OrderResponse, PaymentCollectionResponse,
        ProductResponse, UpdateProductInput,
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
        .add("/orders/{id}", axum::routing::get(show_order))
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminOrderDetailResponse {
    pub order: OrderResponse,
    pub payment_collection: Option<PaymentCollectionResponse>,
    pub fulfillment: Option<FulfillmentResponse>,
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
        assert_eq!(payload["payment_collection"]["id"], json!(payment_collection.id));
        assert_eq!(payload["payment_collection"]["order_id"], json!(order.id));
        assert_eq!(payload["fulfillment"]["id"], json!(fulfillment.id));
        assert_eq!(payload["fulfillment"]["order_id"], json!(order.id));
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
}
