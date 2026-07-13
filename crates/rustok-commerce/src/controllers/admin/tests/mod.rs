use axum::body::{to_bytes, Body};
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::Response;
use axum::Router;
use rust_decimal::Decimal;
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext};
pub use rustok_api::{AuthContextExtension, TenantContextExtension};
use rustok_test_utils::db::setup_test_db;
use rustok_test_utils::mock_transactional_event_bus;
pub use sea_orm::ConnectionTrait;
use serde_json::json;
pub use std::str::FromStr;
pub use tower::util::ServiceExt;
use uuid::Uuid;

use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
    CompleteRefundInput, CreateFulfillmentInput, CreateFulfillmentItemInput, CreateOrderInput,
    CreateOrderLineItemInput, CreateOrderTaxLineInput, CreatePaymentCollectionInput,
    CreateRefundInput, DeliverFulfillmentInput, FulfillmentItemQuantityInput, RefundResponse,
    ShipFulfillmentInput, UpdateShippingOptionInput,
};
use crate::ShippingProfileService;
use rustok_fulfillment::FulfillmentService;
use rustok_order::OrderService;
use rustok_payment::PaymentService;

mod support {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support.rs"));
}

pub(crate) fn test_app_context(
    db: sea_orm::DatabaseConnection,
) -> crate::controllers::CommerceHttpRuntime {
    crate::controllers::CommerceHttpRuntime {
        db,
        event_bus: mock_transactional_event_bus(),
        payment_provider_registry:
            rustok_payment::providers::PaymentProviderRegistry::with_manual_provider(),
        fulfillment_provider_registry:
            rustok_fulfillment::providers::FulfillmentProviderRegistry::with_manual_provider(),
    }
}

pub(crate) async fn seed_tenant_context(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
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
pub(crate) struct TransportRequestContext {
    pub tenant: TenantContext,
    pub auth: AuthContext,
}

pub(crate) async fn inject_transport_context(
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

pub(crate) fn admin_transport_router(
    ctx: crate::controllers::CommerceHttpRuntime,
    tenant: TenantContext,
    auth: AuthContext,
) -> Router {
    let router = crate::controllers::admin::axum_router().with_state(ctx);

    router.layer(from_fn_with_state(
        TransportRequestContext { tenant, auth },
        inject_transport_context,
    ))
}

pub mod fulfillments;
pub mod orders;
pub mod payments;
pub mod returns;
pub mod shipping;
