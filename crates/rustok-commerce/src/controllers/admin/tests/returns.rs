use super::*;

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
