use super::*;

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
    assert_eq!(payload["meta"]["total"], json!(0));
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
        permissions: vec![Permission::PAYMENTS_UPDATE],
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
    assert_eq!(payload["meta"]["total"], json!(1));
    let items = payload["data"].as_array().expect("data should be array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]["payment_collection_id"],
        json!(first_collection.id.to_string())
    );
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
