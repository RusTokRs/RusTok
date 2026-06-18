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
        CreateOrderLineItemInput, CreateOrderTaxLineInput, CreatePaymentCollectionInput,
        CreateRefundInput, DeliverFulfillmentInput, FulfillmentItemQuantityInput, RefundResponse,
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
                    tax_lines: vec![
                        CreateOrderTaxLineInput {
                            line_item_index: Some(0),
                            shipping_option_id: None,
                            rate: Decimal::from_str("19.00").expect("valid decimal"),
                            amount: Decimal::from_str("9.50").expect("valid decimal"),
                            description: Some("VAT line item".to_string()),
                            currency_code: "eur".to_string(),
                            provider_id: "region_default".to_string(),
                            metadata: json!({
                                "tax_included": false,
                                "scope": "line_item"
                            }),
                        },
                        CreateOrderTaxLineInput {
                            line_item_index: None,
                            shipping_option_id: None,
                            rate: Decimal::from_str("19.00").expect("valid decimal"),
                            amount: Decimal::from_str("1.00").expect("valid decimal"),
                            description: Some("VAT shipping".to_string()),
                            currency_code: "eur".to_string(),
                            provider_id: "region_default".to_string(),
                            metadata: json!({
                                "tax_included": false,
                                "scope": "shipping"
                            }),
                        },
                        CreateOrderTaxLineInput {
                            line_item_index: None,
                            shipping_option_id: None,
                            rate: Decimal::from_str("19.00").expect("valid decimal"),
                            amount: Decimal::from_str("0.50").expect("valid decimal"),
                            description: Some("VAT order".to_string()),
                            currency_code: "eur".to_string(),
                            provider_id: "region_default".to_string(),
                            metadata: json!({
                                "tax_included": false,
                                "scope": "order"
                            }),
                        },
                    ],
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
        assert_eq!(payload["order"]["tax_total"], json!("11"));
        assert_eq!(payload["order"]["tax_included"], json!(false));
        assert_eq!(payload["order"]["tax_lines"].as_array().unwrap().len(), 3);
        assert_eq!(
            payload["order"]["tax_lines"][0]["provider_id"],
            json!("region_default")
        );
        assert_eq!(
            payload["order"]["tax_lines"][0]["line_item_id"].is_string(),
            true
        );
        assert_eq!(
            payload["order"]["tax_lines"][1]["shipping_option_id"].is_string(),
            true
        );
        assert_eq!(
            payload["order"]["tax_lines"][2]["line_item_id"],
            json!(null)
        );
        assert_eq!(
            payload["order"]["tax_lines"][2]["shipping_option_id"],
            json!(null)
        );
        assert_eq!(
            payload["payment_collection"]["id"],
            json!(payment_collection.id)
        );
        assert_eq!(payload["payment_collection"]["order_id"], json!(order.id));
        assert_eq!(
            payload["payment_collection"]["amount"],
            payload["order"]["total_amount"]
        );
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
                    tax_lines: vec![CreateOrderTaxLineInput {
                        line_item_index: Some(0),
                        shipping_option_id: None,
                        rate: Decimal::from_str("10.00").expect("valid decimal"),
                        amount: Decimal::from_str("2.00").expect("valid decimal"),
                        description: Some("VAT".to_string()),
                        currency_code: "usd".to_string(),
                        provider_id: "region_default".to_string(),
                        metadata: json!({ "tax_included": false }),
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
        assert_eq!(data[0]["subtotal_amount"], json!("20"));
        assert_eq!(data[0]["total_amount"], json!("22"));
        assert_eq!(data[0]["tax_total"], json!("2"));
        assert_eq!(data[0]["tax_included"], json!(false));
        assert_eq!(
            data[0]["tax_lines"][0]["provider_id"],
            json!("region_default")
        );
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
    async fn admin_refund_transport_hides_foreign_tenant_refund() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let foreign_tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        seed_tenant_context(&db, foreign_tenant_id).await;

        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "usd".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("REFUND-FOREIGN-1".to_string()),
                        title: "Refund Foreign".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("10.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-refund-foreign" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-refund-foreign" }),
                },
            )
            .await
            .expect("order should be created");

        let collection = PaymentService::new(db.clone())
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(order.id),
                    customer_id: order.customer_id,
                    currency_code: "USD".to_string(),
                    amount: order.total_amount,
                    metadata: json!({ "source": "admin-refund-foreign" }),
                },
            )
            .await
            .expect("collection should be created");

        let refund = PaymentService::new(db.clone())
            .create_refund(
                tenant_id,
                collection.id,
                CreateRefundInput {
                    amount: Decimal::from_str("4.00").expect("valid decimal"),
                    reason: Some("test".to_string()),
                    metadata: json!({ "source": "admin-refund-foreign" }),
                },
            )
            .await
            .expect("refund should be created");

        let foreign_tenant = TenantContext {
            id: foreign_tenant_id,
            name: "Foreign Tenant".to_string(),
            slug: format!("foreign-{foreign_tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let foreign_auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id: foreign_tenant_id,
            permissions: vec![Permission::PAYMENTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        let app = admin_transport_router(test_app_context(db), foreign_tenant, foreign_auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/refunds/{}", refund.id))
                    .header("X-Tenant-ID", foreign_tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn admin_refunds_transport_list_ignores_foreign_tenant_payment_collection_filter() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let foreign_tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        seed_tenant_context(&db, foreign_tenant_id).await;

        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "usd".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("REFUND-LIST-FOREIGN-1".to_string()),
                        title: "Refund list foreign".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("12.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-refund-list-foreign" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-refund-list-foreign" }),
                },
            )
            .await
            .expect("order should be created");

        let collection = PaymentService::new(db.clone())
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(order.id),
                    customer_id: order.customer_id,
                    currency_code: "USD".to_string(),
                    amount: order.total_amount,
                    metadata: json!({ "source": "admin-refund-list-foreign" }),
                },
            )
            .await
            .expect("collection should be created");

        PaymentService::new(db.clone())
            .create_refund(
                tenant_id,
                collection.id,
                CreateRefundInput {
                    amount: Decimal::from_str("3.00").expect("valid decimal"),
                    reason: Some("test".to_string()),
                    metadata: json!({ "source": "admin-refund-list-foreign" }),
                },
            )
            .await
            .expect("refund should be created");

        let foreign_tenant = TenantContext {
            id: foreign_tenant_id,
            name: "Foreign Tenant".to_string(),
            slug: format!("foreign-{foreign_tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let foreign_auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id: foreign_tenant_id,
            permissions: vec![Permission::PAYMENTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        let app = admin_transport_router(test_app_context(db), foreign_tenant, foreign_auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/admin/refunds?payment_collection_id={}",
                        collection.id
                    ))
                    .header("X-Tenant-ID", foreign_tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be JSON");
        assert_eq!(payload["data"], json!([]));
        assert_eq!(payload["total"], json!(0));
    }

    #[tokio::test]
    async fn admin_refunds_transport_create_rejects_foreign_tenant_payment_collection() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_a).await;
        seed_tenant_context(&db, tenant_b).await;

        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_a,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "usd".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("REFUND-CREATE-FOREIGN-1".to_string()),
                        title: "Refund create foreign".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("14.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-refund-create-foreign" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-refund-create-foreign" }),
                },
            )
            .await
            .expect("order should be created");

        let collection = PaymentService::new(db.clone())
            .create_collection(
                tenant_a,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(order.id),
                    customer_id: order.customer_id,
                    currency_code: "USD".to_string(),
                    amount: order.total_amount,
                    metadata: json!({ "source": "admin-refund-create-foreign" }),
                },
            )
            .await
            .expect("collection should be created");

        let tenant = TenantContext {
            id: tenant_b,
            name: "Tenant B".to_string(),
            slug: format!("tenant-b-{tenant_b}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id: tenant_b,
            permissions: vec![Permission::PAYMENTS_CREATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/admin/payment-collections/{}/refunds",
                        collection.id
                    ))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_b.to_string())
                    .body(Body::from(
                        json!({
                            "amount": "2.00",
                            "reason": "test",
                            "metadata": { "source": "admin-refund-create-foreign" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn admin_refunds_transport_rejects_invalid_status_filter() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Tenant".to_string(),
            slug: format!("tenant-{tenant_id}"),
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

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/refunds?status=processing")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn admin_refunds_transport_accepts_case_insensitive_status_filter() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "usd".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("REFUND-LIST-UPPER-1".to_string()),
                        title: "Refund list uppercase".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("11.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-refund-list-uppercase" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-refund-list-uppercase" }),
                },
            )
            .await
            .expect("order should be created");

        let collection = PaymentService::new(db.clone())
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(order.id),
                    customer_id: order.customer_id,
                    currency_code: "USD".to_string(),
                    amount: order.total_amount,
                    metadata: json!({ "source": "admin-refund-list-uppercase" }),
                },
            )
            .await
            .expect("collection should be created");

        PaymentService::new(db.clone())
            .create_refund(
                tenant_id,
                collection.id,
                CreateRefundInput {
                    amount: Decimal::from_str("3.00").expect("valid decimal"),
                    reason: Some("test".to_string()),
                    metadata: json!({ "source": "admin-refund-list-uppercase" }),
                },
            )
            .await
            .expect("refund should be created");

        let tenant = TenantContext {
            id: tenant_id,
            name: "Tenant".to_string(),
            slug: format!("tenant-{tenant_id}"),
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

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/refunds?status=%20PENDING%20")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn admin_refunds_transport_supports_order_id_filter() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
        let first_order = order_service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "usd".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-REFUND-ORDER-FILTER-1".to_string()),
                        title: "Admin Refund Order Filter 1".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("12.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-refund-order-filter" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-refund-order-filter" }),
                },
            )
            .await
            .expect("first order should be created");
        let second_order = order_service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "usd".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-REFUND-ORDER-FILTER-2".to_string()),
                        title: "Admin Refund Order Filter 2".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("14.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-refund-order-filter" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-refund-order-filter" }),
                },
            )
            .await
            .expect("second order should be created");

        let first_collection = PaymentService::new(db.clone())
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(first_order.id),
                    customer_id: first_order.customer_id,
                    currency_code: "USD".to_string(),
                    amount: first_order.total_amount,
                    metadata: json!({ "source": "admin-refund-order-filter" }),
                },
            )
            .await
            .expect("first collection should be created");
        let second_collection = PaymentService::new(db.clone())
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: None,
                    order_id: Some(second_order.id),
                    customer_id: second_order.customer_id,
                    currency_code: "USD".to_string(),
                    amount: second_order.total_amount,
                    metadata: json!({ "source": "admin-refund-order-filter" }),
                },
            )
            .await
            .expect("second collection should be created");

        PaymentService::new(db.clone())
            .create_refund(
                tenant_id,
                first_collection.id,
                CreateRefundInput {
                    amount: Decimal::from_str("3.00").expect("valid decimal"),
                    reason: Some("test".to_string()),
                    metadata: json!({ "source": "admin-refund-order-filter" }),
                },
            )
            .await
            .expect("first refund should be created");
        PaymentService::new(db.clone())
            .create_refund(
                tenant_id,
                second_collection.id,
                CreateRefundInput {
                    amount: Decimal::from_str("5.00").expect("valid decimal"),
                    reason: Some("test".to_string()),
                    metadata: json!({ "source": "admin-refund-order-filter" }),
                },
            )
            .await
            .expect("second refund should be created");

        let tenant = TenantContext {
            id: tenant_id,
            name: "Tenant".to_string(),
            slug: format!("tenant-{tenant_id}"),
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

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/admin/refunds?order_id={}", first_order.id))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be JSON");
        assert_eq!(payload["total"], json!(1));
        let items = payload["data"].as_array().expect("data should be array");
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0]["payment_collection_id"],
            json!(first_collection.id.to_string())
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
                            "translations": [{
                                "locale": "en",
                                "name": "Bulky Freight",
                                "description": "Large parcel handling"
                            }],
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
                            "translations": [{
                                "locale": "en",
                                "name": "Bulky Freight"
                            }],
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
                        sku: Some("ADMIN-FULFILLMENT-CREATE-1".to_string()),
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
                        sku: Some("ADMIN-FULFILLMENT-OVER-1".to_string()),
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
                        sku: Some("ADMIN-FULFILLMENT-1".to_string()),
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
                        sku: Some("ADMIN-FULFILLMENT-PARTIAL-1".to_string()),
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
                        sku: Some("ADMIN-FULFILLMENT-REOPEN-1".to_string()),
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

    #[tokio::test]
    async fn admin_return_decision_transport_creates_exchange_order_change() {
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

        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "usd".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-RETURN-DECISION-EXCHANGE".to_string()),
                        title: "Admin Return Decision Exchange".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("42.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-return-decision" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-return-decision" }),
                },
            )
            .await
            .expect("order should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/orders/{}/returns/decision", order.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "return_request": {
                                "reason": "wrong-size",
                                "note": "operator-reviewed",
                                "items": [
                                    {
                                        "line_item_id": order.line_items[0].id,
                                        "quantity": 1,
                                        "reason": "wrong-size",
                                        "metadata": { "source": "admin-return-decision" }
                                    }
                                ],
                                "metadata": { "source": "admin-return-decision" }
                            },
                            "decision": {
                                "action": "exchange",
                                "exchange": {
                                    "description": "Exchange for another size",
                                    "preview": { "swap": "new-size" },
                                    "metadata": { "operator": "returns-desk" }
                                },
                                "metadata": { "flow": "exchange" }
                            }
                        })
                        .to_string(),
                    ))
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
            StatusCode::CREATED,
            "unexpected decision body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be JSON");
        assert_eq!(payload["action"], json!("exchange"));
        assert_eq!(payload["order_return"]["order_id"], json!(order.id));
        assert_eq!(payload["order_change"]["change_type"], json!("exchange"));
        assert_eq!(
            payload["order_change"]["metadata"]["order_return_id"],
            payload["order_return"]["id"]
        );
        assert_eq!(payload["refund"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn admin_return_decision_transport_creates_claim_order_change() {
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

        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "usd".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-RETURN-DECISION-CLAIM".to_string()),
                        title: "Admin Return Decision Claim".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("37.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-return-claim-decision" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-return-claim-decision" }),
                },
            )
            .await
            .expect("order should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/orders/{}/returns/decision", order.id))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "return_request": {
                            "reason": "damaged",
                            "note": "claim reviewed by admin REST",
                            "items": [
                                {
                                    "line_item_id": order.line_items[0].id,
                                    "quantity": 1,
                                    "reason": "damaged",
                                    "metadata": { "source": "admin-return-claim-decision", "scope": "item" }
                                }
                            ],
                            "metadata": { "source": "admin-return-claim-decision", "scope": "return" }
                        },
                        "decision": {
                            "action": "claim",
                            "claim": {
                                "description": "Claim for damaged item",
                                "preview": { "claim_type": "damaged_item", "resolution": "review" },
                                "metadata": { "operator": "claims-desk" }
                            },
                            "metadata": { "flow": "claim" }
                        }
                    })
                    .to_string(),
                ))
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
            StatusCode::CREATED,
            "unexpected decision body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be JSON");
        assert_eq!(payload["action"], json!("claim"));
        assert_eq!(payload["metadata"]["flow"], json!("claim"));
        assert_eq!(payload["order_return"]["order_id"], json!(order.id));
        assert_eq!(payload["order_return"]["status"], json!("completed"));
        assert_eq!(payload["order_return"]["resolution_type"], json!("claim"));
        assert_eq!(payload["order_change"]["change_type"], json!("claim"));
        assert_eq!(
            payload["order_return"]["order_change_id"],
            payload["order_change"]["id"]
        );
        assert_eq!(
            payload["order_change"]["metadata"]["order_return_id"],
            payload["order_return"]["id"]
        );
        assert_eq!(
            payload["order_change"]["preview"]["order_return_id"],
            payload["order_return"]["id"]
        );
        assert_eq!(
            payload["order_change"]["preview"]["claim_type"],
            json!("damaged_item")
        );
        assert_eq!(payload["refund"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn admin_return_decision_transport_requires_payments_update_for_refund_action() {
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

        let order = OrderService::new(db.clone(), mock_transactional_event_bus())
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(Uuid::new_v4()),
                    currency_code: "usd".to_string(),
                    shipping_total: Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: Some(Uuid::new_v4()),
                        variant_id: Some(Uuid::new_v4()),
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("ADMIN-RETURN-DECISION-REFUND".to_string()),
                        title: "Admin Return Decision Refund".to_string(),
                        quantity: 1,
                        unit_price: Decimal::from_str("12.00").expect("valid decimal"),
                        metadata: json!({ "source": "admin-return-decision-permission" }),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({ "source": "admin-return-decision-permission" }),
                },
            )
            .await
            .expect("order should be created");

        let app = admin_transport_router(test_app_context(db), tenant, auth);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/admin/orders/{}/returns/decision", order.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "return_request": {
                                "reason": "damaged",
                                "metadata": { "source": "admin-return-decision-permission" }
                            },
                            "decision": {
                                "action": "refund",
                                "metadata": { "flow": "refund" }
                            }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
