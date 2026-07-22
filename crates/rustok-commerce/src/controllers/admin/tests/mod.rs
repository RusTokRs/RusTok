use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{HeaderValue, Method, Request, StatusCode};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::Response;
use rust_decimal::Decimal;
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext};
pub use rustok_api::{AuthContextExtension, TenantContextExtension};
use rustok_test_utils::db::setup_test_db;
use rustok_test_utils::mock_transactional_event_bus;
pub use sea_orm::ConnectionTrait;
use serde_json::json;
use std::ops::Deref;
pub use std::str::FromStr;
pub use tower::util::ServiceExt;
use uuid::Uuid;

use crate::ShippingProfileService;
use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
    CompleteRefundInput, CreateFulfillmentInput, CreateFulfillmentItemInput, CreateOrderInput,
    CreateOrderLineItemInput, CreateOrderTaxLineInput, CreatePaymentCollectionInput,
    CreateRefundInput, DeliverFulfillmentInput, FulfillmentItemQuantityInput, RefundResponse,
    ShipFulfillmentInput, UpdateShippingOptionInput,
};
use rustok_fulfillment::FulfillmentService;
use rustok_order::OrderService;
use rustok_payment::{PaymentRefundCreationService, PaymentService as DomainPaymentService};

mod support {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support.rs"));
}

/// Compatibility wrapper for older controller fixtures. It deliberately does not
/// restore a production refund API: all refund creation is routed through the
/// owner idempotent service with a fixture-owned creation identity.
pub(crate) struct PaymentService {
    inner: DomainPaymentService,
    db: sea_orm::DatabaseConnection,
}

impl PaymentService {
    pub(crate) fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            inner: DomainPaymentService::new(db.clone()),
            db,
        }
    }

    pub(crate) async fn create_refund(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        input: CreateRefundInput,
    ) -> rustok_payment::PaymentResult<RefundResponse> {
        let collection = self.inner.get_collection(tenant_id, collection_id).await?;
        match collection.status.as_str() {
            "pending" => {
                self.inner
                    .authorize_collection(
                        tenant_id,
                        collection_id,
                        AuthorizePaymentInput {
                            provider_id: Some("manual".to_string()),
                            provider_payment_id: None,
                            amount: Some(collection.amount),
                            metadata: json!({"source": "controller-refund-fixture"}),
                        },
                    )
                    .await?;
                self.inner
                    .capture_collection(
                        tenant_id,
                        collection_id,
                        CapturePaymentInput {
                            amount: Some(collection.amount),
                            metadata: json!({"source": "controller-refund-fixture"}),
                        },
                    )
                    .await?;
            }
            "authorized" => {
                self.inner
                    .capture_collection(
                        tenant_id,
                        collection_id,
                        CapturePaymentInput {
                            amount: Some(collection.authorized_amount),
                            metadata: json!({"source": "controller-refund-fixture"}),
                        },
                    )
                    .await?;
            }
            "captured" => {}
            status => {
                return Err(rustok_payment::PaymentError::InvalidTransition {
                    from: status.to_string(),
                    to: "pending".to_string(),
                });
            }
        }

        PaymentRefundCreationService::new(self.db.clone())
            .create_or_replay(
                tenant_id,
                collection_id,
                format!("controller-fixture:{collection_id}:{}", Uuid::new_v4()),
                input,
            )
            .await
    }
}

impl Deref for PaymentService {
    type Target = DomainPaymentService;

    fn deref(&self) -> &Self::Target {
        &self.inner
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

    let path = req.uri().path();
    if req.method() == Method::POST
        && path.starts_with("/admin/payment-collections/")
        && path.ends_with("/refunds")
        && !req.headers().contains_key("idempotency-key")
    {
        let value =
            HeaderValue::from_str(format!("controller-http-fixture:{}", Uuid::new_v4()).as_str())
                .expect("fixture idempotency key must be a valid header");
        req.headers_mut().insert("idempotency-key", value);
    }

    next.run(req).await
}

pub(crate) fn admin_transport_router(
    ctx: crate::controllers::CommerceHttpRuntime,
    tenant: TenantContext,
    auth: AuthContext,
) -> Router {
    let router = Router::new()
        .nest("/admin", crate::controllers::admin::axum_router())
        .with_state(ctx);

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
