use super::*;

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
    assert!(
        created["metadata"]["delivery_group"]
            .get("seller_scope")
            .is_none()
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
