use std::sync::{Arc, Mutex};

use async_graphql::{EmptySubscription, Request as GraphqlRequest, Schema};
use async_trait::async_trait;
use axum::{
    Extension, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use rust_decimal::Decimal;
use rustok_api::{
    AuthContext, AuthContextExtension, HostRuntimeContext, Permission, PortActorKind, PortContext,
    PortError, PortErrorKind, RequestContext, TenantContext, TenantContextExtension,
    graphql::GraphqlRuntimeInputs,
};
#[cfg(feature = "marketplace-financial")]
use rustok_commerce::MarketplaceFinancialRuntime;
use rustok_commerce::graphql::{CommerceMutation, CommerceQuery};
use rustok_commerce::graphql_runtime::{
    CommerceFulfillmentLifecycleReadRuntime, CommerceOrderReadRuntime,
    CommerceShippingOptionReadRuntime, CommerceShippingOptionReadScope,
};
use rustok_fulfillment::{
    FindLatestFulfillmentByOrderProjectionRequest, FulfillmentProjectionPage, FulfillmentReadPort,
    FulfillmentResponse, ListFulfillmentProjectionsRequest, ReadFulfillmentProjectionRequest,
};
use rustok_order::{
    OrderService,
    dto::{CreateOrderInput, CreateOrderLineItemInput},
};
use rustok_test_utils::{db::setup_test_db, mock_transactional_event_bus};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "support/mod.rs"]
mod support;

type CommerceSchema = Schema<CommerceQuery, CommerceMutation, EmptySubscription>;

const OWNER_SENTINEL: &str = "secret owner connector detail must not escape";

#[derive(Clone)]
struct ScriptedFulfillmentReadPort {
    read_error: Option<PortError>,
    calls: Arc<Mutex<Vec<RecordedCall>>>,
}

#[derive(Clone, Debug)]
struct RecordedCall {
    operation: &'static str,
    context: PortContext,
}

impl ScriptedFulfillmentReadPort {
    fn failing(error: PortError) -> Self {
        Self {
            read_error: Some(error),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn recording() -> Self {
        Self {
            read_error: None,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record(&self, operation: &'static str, context: PortContext) {
        self.calls
            .lock()
            .expect("scripted fulfillment call lock")
            .push(RecordedCall { operation, context });
    }

    fn calls(&self) -> Vec<RecordedCall> {
        self.calls
            .lock()
            .expect("scripted fulfillment call lock")
            .clone()
    }
}

#[async_trait]
impl FulfillmentReadPort for ScriptedFulfillmentReadPort {
    async fn read_fulfillment_projection(
        &self,
        context: PortContext,
        _request: ReadFulfillmentProjectionRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        self.record("read_fulfillment_projection", context);
        Err(self
            .read_error
            .clone()
            .unwrap_or_else(|| PortError::not_found("test.fulfillment_not_found", OWNER_SENTINEL)))
    }

    async fn list_fulfillment_projections(
        &self,
        context: PortContext,
        _request: ListFulfillmentProjectionsRequest,
    ) -> Result<FulfillmentProjectionPage, PortError> {
        self.record("list_fulfillment_projections", context);
        match self.read_error.clone() {
            Some(error) => Err(error),
            None => Ok(FulfillmentProjectionPage {
                items: Vec::new(),
                total: 0,
            }),
        }
    }

    async fn find_latest_fulfillment_by_order_projection(
        &self,
        context: PortContext,
        _request: FindLatestFulfillmentByOrderProjectionRequest,
    ) -> Result<Option<FulfillmentResponse>, PortError> {
        self.record("find_latest_fulfillment_by_order_projection", context);
        match self.read_error.clone() {
            Some(error) => Err(error),
            None => Ok(None),
        }
    }
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Fulfillment failure contract".to_string(),
        slug: format!("fulfillment-failure-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "ru-RU".to_string(),
        is_active: true,
    }
}

fn request_context(tenant_id: Uuid) -> RequestContext {
    RequestContext {
        tenant_id,
        user_id: None,
        channel_id: None,
        channel_slug: None,
        channel_resolution_source: None,
        locale: "ru-RU".to_string(),
    }
}

fn auth_context(tenant_id: Uuid, user_id: Uuid) -> AuthContext {
    AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![
            Permission::FULFILLMENTS_READ,
            Permission::ORDERS_READ,
            Permission::ORDERS_LIST,
        ],
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    }
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            "Fulfillment failure contract".into(),
            format!("fulfillment-failure-{tenant_id}").into(),
            sea_orm::Value::String(None),
            json!({}).to_string().into(),
            "ru-RU".into(),
            true.into(),
        ],
    ))
    .await
    .expect("tenant fixture should be inserted");

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
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
    .expect("commerce module should be enabled");
}

fn host_runtime(
    db: &DatabaseConnection,
    fulfillment_port: Arc<dyn FulfillmentReadPort>,
) -> HostRuntimeContext {
    let event_bus = mock_transactional_event_bus();
    let host = HostRuntimeContext::new(db.clone()).with_shared_value(event_bus.clone());
    #[cfg(feature = "marketplace-financial")]
    let host = host.with_shared_value(MarketplaceFinancialRuntime::in_process(db.clone()));
    host.with_shared_value(CommerceShippingOptionReadRuntime::in_process(db.clone()))
        .with_shared_value(rustok_fulfillment::ShippingOptionAdminCommandRuntime::in_process(db.clone()))
        .with_shared_value(CommerceFulfillmentLifecycleReadRuntime::new(
            fulfillment_port,
        ))
        .with_shared_value(CommerceOrderReadRuntime::in_process(db.clone(), event_bus.clone()))
        .with_shared_value(rustok_order::OrderAdminCommandRuntime::in_process(db.clone(), event_bus.clone()))
        .with_shared_value(rustok_order::OrderPostOrderCommandRuntime::in_process(db.clone(), event_bus.clone()))
        .with_shared_value(rustok_payment::PaymentOrderReadRuntime::in_process(db.clone()))
        .with_shared_value(rustok_payment::PaymentCartReadRuntime::in_process(db.clone()))
        .with_shared_value(rustok_payment::PaymentCollectionRuntime::in_process(db.clone()))
        .with_shared_value(rustok_product::ProductCatalogReadRuntime::in_process(db.clone(), event_bus.clone()))
        .with_shared_value(rustok_product::ProductCatalogCommandRuntime::in_process(db.clone(), event_bus))
}

fn graphql_schema(
    db: &DatabaseConnection,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: AuthContext,
    fulfillment_port: Arc<dyn FulfillmentReadPort>,
) -> CommerceSchema {
    let runtime_inputs = GraphqlRuntimeInputs::new(host_runtime(db, fulfillment_port));
    let runtime_data = rustok_commerce::graphql_runtime::attach_schema_data(&runtime_inputs)
        .expect("commerce GraphQL runtime data should compose");

    Schema::build(
        CommerceQuery,
        CommerceMutation::default(),
        EmptySubscription,
    )
    .extension(CommerceShippingOptionReadScope)
    .data(db.clone())
    .data(mock_transactional_event_bus())
    .data(tenant)
    .data(request_context)
    .data(auth)
    .data(runtime_data)
    .finish()
}

fn rest_router(
    db: &DatabaseConnection,
    tenant: TenantContext,
    auth: AuthContext,
    fulfillment_port: Arc<dyn FulfillmentReadPort>,
) -> Router {
    rustok_commerce::controllers::axum_router(&host_runtime(db, fulfillment_port))
        .expect("commerce HTTP runtime should compose")
        .layer(Extension(AuthContextExtension(auth)))
        .layer(Extension(TenantContextExtension(tenant)))
}

fn response_json(response: &async_graphql::Response) -> Value {
    serde_json::to_value(response).expect("GraphQL response should serialize")
}

fn port_error(kind: PortErrorKind, code: &'static str, retryable: bool) -> PortError {
    PortError::new(kind, code, OWNER_SENTINEL, retryable)
}

fn assert_graphql_call_context(call: &RecordedCall, tenant_id: Uuid, resource_id: Uuid) {
    assert_eq!(call.context.tenant_id, tenant_id.to_string());
    assert_eq!(call.context.deadline_ms, Some(2_000));
    assert_eq!(call.context.actor.kind, PortActorKind::Service);
    assert_eq!(
        call.context.actor.id,
        "rustok-commerce.graphql-query-fulfillments"
    );
    assert_eq!(call.context.locale, "en");
    assert_eq!(call.context.channel, None);
    assert_eq!(
        call.context.correlation_id,
        format!(
            "graphql-fulfillment-lifecycle:fulfillment:read_fulfillment_projection:{resource_id}"
        )
    );
}

#[tokio::test]
async fn graphql_fulfillment_lookup_preserves_typed_port_errors_and_redacts_owner_messages() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let cases = [
        (
            port_error(PortErrorKind::Validation, "owner.validation", false),
            "FULFILLMENT_REQUEST_INVALID",
            false,
        ),
        (
            port_error(PortErrorKind::Conflict, "owner.conflict", false),
            "FULFILLMENT_STATE_CONFLICT",
            false,
        ),
        (
            port_error(PortErrorKind::Forbidden, "owner.forbidden", false),
            "FULFILLMENT_ACCESS_DENIED",
            false,
        ),
        (
            port_error(PortErrorKind::Unavailable, "owner.unavailable", true),
            "FULFILLMENT_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        (
            port_error(PortErrorKind::Timeout, "owner.timeout", true),
            "FULFILLMENT_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        (
            port_error(PortErrorKind::InvariantViolation, "owner.invariant", false),
            "FULFILLMENT_OPERATION_FAILED",
            false,
        ),
    ];

    for (error, expected_code, expected_retryable) in cases {
        let fulfillment_id = Uuid::new_v4();
        let port = Arc::new(ScriptedFulfillmentReadPort::failing(error));
        let schema = graphql_schema(
            &db,
            tenant_context(tenant_id),
            request_context(tenant_id),
            auth_context(tenant_id, user_id),
            port.clone(),
        );
        let response = schema
            .execute(GraphqlRequest::new(format!(
                r#"query {{
                    fulfillment(tenantId: "{tenant_id}", id: "{fulfillment_id}") {{ id }}
                }}"#
            )))
            .await;
        let payload = response_json(&response);
        let error = &payload["errors"][0];

        assert_eq!(error["extensions"]["code"], json!(expected_code));
        assert_eq!(error["extensions"]["retryable"], json!(expected_retryable));
        assert!(
            !payload.to_string().contains(OWNER_SENTINEL),
            "owner message escaped through GraphQL: {payload}"
        );

        let calls = port.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].operation, "read_fulfillment_projection");
        assert_graphql_call_context(&calls[0], tenant_id, fulfillment_id);
    }
}

#[tokio::test]
async fn graphql_fulfillment_lookup_keeps_not_found_optional() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    let port = Arc::new(ScriptedFulfillmentReadPort::failing(port_error(
        PortErrorKind::NotFound,
        "owner.not_found",
        false,
    )));
    let schema = graphql_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id),
        auth_context(tenant_id, user_id),
        port.clone(),
    );

    let response = schema
        .execute(GraphqlRequest::new(format!(
            r#"query {{
                fulfillment(tenantId: "{tenant_id}", id: "{fulfillment_id}") {{ id }}
            }}"#
        )))
        .await;
    let payload = response_json(&response);

    assert!(
        response.errors.is_empty(),
        "optional not-found should not produce GraphQL errors: {:?}",
        response.errors
    );
    assert_eq!(payload["data"]["fulfillment"], Value::Null);
    assert!(!payload.to_string().contains(OWNER_SENTINEL));
    assert_graphql_call_context(&port.calls()[0], tenant_id, fulfillment_id);
}

#[tokio::test]
async fn graphql_list_and_latest_by_order_apply_the_same_deadline_contract() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            user_id,
            CreateOrderInput {
                customer_id: None,
                currency_code: "RUB".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("FULFILLMENT-READ-FAILURE".to_string()),
                    title: "Fulfillment read failure contract".to_string(),
                    quantity: 1,
                    unit_price: Decimal::ONE,
                    metadata: json!({}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: json!({}),
            },
        )
        .await
        .expect("order fixture should be created");

    let port = Arc::new(ScriptedFulfillmentReadPort::recording());
    let schema = graphql_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id),
        auth_context(tenant_id, user_id),
        port.clone(),
    );
    let response = schema
        .execute(GraphqlRequest::new(format!(
            r#"query {{
                fulfillments(
                    tenantId: "{tenant_id}",
                    filter: {{ orderId: "{}", page: 1, perPage: 5 }}
                ) {{
                    total
                }}
                order(tenantId: "{tenant_id}", id: "{}") {{
                    fulfillment {{ id }}
                }}
            }}"#,
            order.id, order.id
        )))
        .await;

    assert!(
        response.errors.is_empty(),
        "list/latest context query failed: {:?}",
        response.errors
    );
    let calls = port.calls();
    let list = calls
        .iter()
        .find(|call| call.operation == "list_fulfillment_projections")
        .expect("list operation should be recorded");
    let latest = calls
        .iter()
        .find(|call| call.operation == "find_latest_fulfillment_by_order_projection")
        .expect("latest-by-order operation should be recorded");

    for call in [list, latest] {
        assert_eq!(call.context.tenant_id, tenant_id.to_string());
        assert_eq!(call.context.deadline_ms, Some(2_000));
        assert_eq!(call.context.actor.kind, PortActorKind::Service);
        assert_eq!(
            call.context.actor.id,
            "rustok-commerce.graphql-query-fulfillments"
        );
        assert_eq!(call.context.locale, "en");
        assert_eq!(call.context.channel, None);
        assert!(call.context.correlation_id.ends_with(&order.id.to_string()));
    }
}

#[tokio::test]
async fn admin_rest_fulfillment_detail_preserves_typed_errors_and_request_context() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let cases = [
        (
            port_error(PortErrorKind::Validation, "owner.validation", false),
            StatusCode::BAD_REQUEST,
            "commerce_admin_fulfillment_invalid",
        ),
        (
            port_error(PortErrorKind::NotFound, "owner.not_found", false),
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
        ),
        (
            port_error(PortErrorKind::Conflict, "owner.conflict", false),
            StatusCode::CONFLICT,
            "commerce_admin_fulfillment_state_conflict",
        ),
        (
            port_error(PortErrorKind::Forbidden, "owner.forbidden", false),
            StatusCode::UNAUTHORIZED,
            "commerce_permission_denied",
        ),
        (
            port_error(PortErrorKind::Unavailable, "owner.unavailable", true),
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_fulfillment_storage_unavailable",
        ),
        (
            port_error(PortErrorKind::Timeout, "owner.timeout", true),
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_fulfillment_storage_unavailable",
        ),
        (
            port_error(PortErrorKind::InvariantViolation, "owner.invariant", false),
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_fulfillment_failed",
        ),
    ];

    for (error, expected_status, expected_code) in cases {
        let fulfillment_id = Uuid::new_v4();
        let port = Arc::new(ScriptedFulfillmentReadPort::failing(error));
        let response = rest_router(
            &db,
            tenant_context(tenant_id),
            auth_context(tenant_id, user_id),
            port.clone(),
        )
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/admin/fulfillments/{fulfillment_id}"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .header("Accept-Language", "ru-RU,ru;q=0.9")
                .body(Body::empty())
                .expect("REST request should build"),
        )
        .await
        .expect("REST request should complete");

        assert_eq!(response.status(), expected_status);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("REST error body should read");
        let payload: Value = serde_json::from_slice(&body).expect("REST error body should be JSON");
        assert_eq!(payload["code"], json!(expected_code));
        assert!(
            !String::from_utf8_lossy(&body).contains(OWNER_SENTINEL),
            "owner message escaped through REST: {}",
            String::from_utf8_lossy(&body)
        );

        let calls = port.calls();
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.operation, "read_fulfillment_projection");
        assert_eq!(call.context.tenant_id, tenant_id.to_string());
        assert_eq!(call.context.deadline_ms, Some(2_000));
        assert_eq!(call.context.actor.kind, PortActorKind::User);
        assert_eq!(call.context.actor.id, user_id.to_string());
        assert_eq!(call.context.locale, "ru-RU");
        assert_eq!(call.context.channel, None);
        assert_eq!(
            call.context.correlation_id,
            format!("commerce-admin-fulfillment:get_fulfillment:{fulfillment_id}")
        );
    }
}
