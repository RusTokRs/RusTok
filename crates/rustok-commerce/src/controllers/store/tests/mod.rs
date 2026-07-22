use super::{
    MODULE_SLUG, RequestedCartContext, StoreAddCartLineItemInput, StoreCartContextPatch,
    StoreLineItemResolution, cart_context_metadata, checkout_actor_id, ensure_store_cart_access,
    merge_metadata, requested_cart_context, resolve_store_line_item_input,
};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::Response;
use rust_decimal::Decimal;
use rustok_api::Permission;
use rustok_api::RequestContext;
use rustok_api::context::ChannelResolutionSource;
use rustok_api::{AuthContext, ChannelContext, TenantContext};
pub use rustok_api::{AuthContextExtension, ChannelContextExtension, TenantContextExtension};
use rustok_cart::dto::SetCartAdjustmentInput;
use rustok_pricing::{PriceResolutionContext, PricingService};
use rustok_region::dto::{CreateRegionInput, RegionResponse, RegionTranslationInput};
use rustok_region::services::RegionService;
use rustok_test_utils::db::setup_test_db;
use rustok_test_utils::mock_transactional_event_bus;
pub use sea_orm::ConnectionTrait;
use sea_orm::{DatabaseBackend, Statement};
use serde_json::json;
pub use std::str::FromStr;
pub use tower::util::ServiceExt;
use uuid::Uuid;

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll};

use crate::dto::{
    AddCartLineItemInput, CartResponse, CreateCartInput, CreateProductInput,
    CreateShippingOptionInput, CreateVariantInput, PriceInput, ProductTranslationInput,
    ShippingOptionTranslationInput, StoreContextResponse,
};
use rustok_cart::CartService;
use rustok_customer::CustomerService;
use rustok_customer::dto::CreateCustomerInput;
use rustok_fulfillment::FulfillmentService;
use rustok_product::CatalogService;

mod support {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support.rs"));
}

fn sample_cart(customer_id: Option<Uuid>) -> CartResponse {
    CartResponse {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        channel_id: None,
        channel_slug: None,
        customer_id,
        email: Some("buyer@example.com".to_string()),
        region_id: None,
        country_code: None,
        locale_code: None,
        selected_shipping_option_id: None,
        status: "active".to_string(),
        currency_code: "USD".to_string(),
        subtotal_amount: Decimal::ZERO,
        adjustment_total: Decimal::ZERO,
        shipping_total: Decimal::ZERO,
        total_amount: Decimal::ZERO,
        tax_total: Decimal::ZERO,
        metadata: serde_json::json!({}),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        completed_at: None,
        line_items: Vec::new(),
        adjustments: Vec::new(),
        tax_lines: Vec::new(),
        delivery_groups: Vec::new(),
    }
}

fn pricing_context(currency_code: &str, quantity: i32) -> PriceResolutionContext {
    PriceResolutionContext {
        currency_code: currency_code.to_ascii_uppercase(),
        region_id: None,
        price_list_id: None,
        channel_id: None,
        channel_slug: None,
        quantity: Some(quantity),
    }
}

fn sample_request_context(locale: &str) -> RequestContext {
    RequestContext {
        tenant_id: Uuid::new_v4(),
        user_id: None,
        channel_id: None,
        channel_slug: None,
        channel_resolution_source: None,
        locale: locale.to_string(),
    }
}

fn sample_channel_context(slug: &str) -> ChannelContext {
    ChannelContext {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        slug: slug.to_string(),
        name: format!("Channel {slug}"),
        is_active: true,
        status: "active".to_string(),
        target_type: Some("web_domain".to_string()),
        target_value: Some(format!("{slug}.example.test")),
        settings: json!({}),
        resolution_source: ChannelResolutionSource::Host,
        resolution_trace: Vec::new(),
    }
}

async fn seed_channel_binding(
    db: &sea_orm::DatabaseConnection,
    channel: &ChannelContext,
    module_slug: &str,
    is_enabled: bool,
) {
    db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO channels (id, tenant_id, slug, name, is_active, is_default, status, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                channel.id.into(),
                channel.tenant_id.into(),
                channel.slug.clone().into(),
                channel.name.clone().into(),
                channel.is_active.into(),
                false.into(),
                channel.status.clone().into(),
                channel.settings.to_string().into(),
            ],
        ))
        .await
        .expect("channel should be inserted for test");

    db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO channel_module_bindings (id, channel_id, module_slug, is_enabled, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                channel.id.into(),
                module_slug.into(),
                is_enabled.into(),
                json!({}).to_string().into(),
            ],
        ))
        .await
        .expect("channel module binding should be inserted for test");
}

async fn set_stock_location_channel_visibility(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    allowed_channel_slugs: &[&str],
) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE stock_locations SET metadata = ? WHERE tenant_id = ?",
        vec![
            json!({
                "channel_visibility": {
                    "allowed_channel_slugs": allowed_channel_slugs
                }
            })
            .to_string()
            .into(),
            tenant_id.into(),
        ],
    ))
    .await
    .expect("stock location visibility should be updated");
}

pub(crate) fn storefront_product_input() -> CreateProductInput {
    CreateProductInput {
        translations: vec![
            ProductTranslationInput {
                locale: "en".to_string(),
                title: "Storefront Product".to_string(),
                description: Some("English description".to_string()),
                handle: Some("storefront-product-en".to_string()),
                meta_title: None,
                meta_description: None,
            },
            ProductTranslationInput {
                locale: "de".to_string(),
                title: "Storefront Produkt".to_string(),
                description: Some("German description".to_string()),
                handle: Some("storefront-product-de".to_string()),
                meta_title: None,
                meta_description: None,
            },
        ],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some("STOREFRONT-SKU-1".to_string()),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "EUR".to_string(),
                channel_id: None,
                channel_slug: None,
                amount: Decimal::from_str("19.99").expect("valid decimal"),
                compare_at_amount: None,
            }],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: None,
            weight_unit: None,
        }],
        seller_id: None,
        vendor: Some("Storefront Vendor".to_string()),
        product_type: Some("physical".to_string()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: vec![],
        publish: false,
        metadata: json!({}),
    }
}

pub(crate) fn test_app_context(
    db: sea_orm::DatabaseConnection,
) -> crate::controllers::CommerceHttpRuntime {
    let marketplace_financial_runtime = crate::MarketplaceFinancialRuntime::in_process(db.clone());
    crate::controllers::CommerceHttpRuntime {
        db,
        event_bus: mock_transactional_event_bus(),
        payment_provider_registry:
            rustok_payment::providers::PaymentProviderRegistry::with_manual_provider(),
        fulfillment_provider_registry:
            rustok_fulfillment::providers::FulfillmentProviderRegistry::with_manual_provider(),
        marketplace_financial_runtime,
    }
}

pub(crate) async fn seed_store_tenant_context(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
    db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                tenant_id.into(),
                "Store Test Tenant".into(),
                format!("store-test-{tenant_id}").into(),
                sea_orm::Value::String(None),
                json!({}).to_string().into(),
                "en".into(),
                true.into(),
            ],
        ))
        .await
        .expect("tenant should be inserted");

    for (locale, name, native_name, is_default) in [
        ("en", "English", "English", true),
        ("de", "German", "Deutsch", false),
    ] {
        db.execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
                vec![
                    Uuid::new_v4().into(),
                    tenant_id.into(),
                    locale.into(),
                    name.into(),
                    native_name.into(),
                    is_default.into(),
                    true.into(),
                    sea_orm::Value::String(None),
                ],
            ))
            .await
            .expect("tenant locale should be inserted");
    }

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

pub(crate) async fn create_customer_for_user(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    email: &str,
) -> Uuid {
    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(user_id),
                email: email.to_string(),
                first_name: Some("Store".to_string()),
                last_name: Some("Customer".to_string()),
                phone: None,
                locale: Some("de".to_string()),
                metadata: json!({}),
            },
        )
        .await
        .expect("customer should be created")
        .id
}

#[derive(Clone)]
pub(crate) struct TransportRequestContext {
    tenant: TenantContext,
    auth: Option<AuthContext>,
    channel: Option<ChannelContext>,
}

#[derive(Clone)]
pub(crate) struct StorefrontTestClient {
    router: Router,
    guest_cart_token: Arc<Mutex<Option<axum::http::HeaderValue>>>,
}

impl tower::Service<Request<Body>> for StorefrontTestClient {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        if !request
            .headers()
            .contains_key(rustok_cart::GUEST_CART_TOKEN_HEADER)
        {
            if let Some(token) = self
                .guest_cart_token
                .lock()
                .expect("guest cart test token lock")
                .clone()
            {
                request
                    .headers_mut()
                    .insert(rustok_cart::GUEST_CART_TOKEN_HEADER, token);
            }
        }

        let router = self.router.clone();
        let guest_cart_token = self.guest_cart_token.clone();
        Box::pin(async move {
            let response = tower::ServiceExt::oneshot(router, request).await?;
            if let Some(token) = response
                .headers()
                .get(rustok_cart::GUEST_CART_TOKEN_HEADER)
                .cloned()
            {
                *guest_cart_token.lock().expect("guest cart test token lock") = Some(token);
            }
            Ok(response)
        })
    }
}

impl StorefrontTestClient {
    pub(crate) fn with_guest_cart_token(self, token: String) -> Self {
        let token = axum::http::HeaderValue::from_str(&token)
            .expect("guest cart test token must be a valid header value");
        *self
            .guest_cart_token
            .lock()
            .expect("guest cart test token lock") = Some(token);
        self
    }
}

pub(crate) async fn inject_transport_context(
    State(context): State<TransportRequestContext>,
    mut req: axum::extract::Request,
    next: Next,
) -> Response {
    req.extensions_mut()
        .insert(TenantContextExtension(context.tenant));
    if let Some(auth) = context.auth {
        req.extensions_mut().insert(AuthContextExtension(auth));
    }
    if let Some(channel) = context.channel {
        req.extensions_mut()
            .insert(ChannelContextExtension(channel));
    }
    next.run(req).await
}

pub(crate) fn commerce_transport_router(
    ctx: crate::controllers::CommerceHttpRuntime,
    tenant: TenantContext,
) -> StorefrontTestClient {
    commerce_transport_router_with_auth(ctx, tenant, None)
}

pub(crate) fn commerce_transport_router_with_auth(
    ctx: crate::controllers::CommerceHttpRuntime,
    tenant: TenantContext,
    auth: Option<AuthContext>,
) -> StorefrontTestClient {
    commerce_transport_router_with_context(ctx, tenant, auth, None)
}

pub(crate) fn commerce_transport_router_with_context(
    ctx: crate::controllers::CommerceHttpRuntime,
    tenant: TenantContext,
    auth: Option<AuthContext>,
    channel: Option<ChannelContext>,
) -> StorefrontTestClient {
    let router = Router::new()
        .nest("/store", crate::controllers::store::axum_router())
        .with_state(ctx);

    let router = router
        .layer(axum::middleware::from_fn(
            rustok_cart::guest_access_http::resolve,
        ))
        .layer(from_fn_with_state(
            TransportRequestContext {
                tenant,
                auth,
                channel,
            },
            inject_transport_context,
        ));

    StorefrontTestClient {
        router,
        guest_cart_token: Arc::new(Mutex::new(None)),
    }
}

pub mod cart_patch;
pub mod carts;
pub mod checkout;
pub mod products;
pub mod shipping;
