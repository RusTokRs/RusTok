use super::*;

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
    let shipping_option_id = Uuid::new_v4();
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
                        shipping_option_id: Some(shipping_option_id),
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
    assert!(payload["order"]["tax_lines"][0]["line_item_id"].is_string());
    assert_eq!(
        payload["order"]["tax_lines"][1]["shipping_option_id"],
        json!(shipping_option_id)
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
                    currency_code: "eur".to_string(),
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
    assert_eq!(data[0]["total_amount"], json!("20"));
    assert_eq!(data[0]["tax_total"], json!("0"));
    assert_eq!(data[0]["tax_included"], json!(false));
    assert_eq!(data[0]["tax_lines"], json!([]));
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
