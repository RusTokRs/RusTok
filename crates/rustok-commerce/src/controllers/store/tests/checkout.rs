use super::*;

#[tokio::test]
async fn store_payment_collection_transport_reuses_active_collection_and_preserves_cart_context_metadata()
 {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let mut product_input = storefront_product_input();
    product_input.variants[0].inventory_quantity = 5;
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let created = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");
    let app = commerce_transport_router(test_app_context(db.clone()), tenant);

    let create_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/carts")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "email": "buyer@example.com",
                        "currency_code": "eur",
                        "locale": "de"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create cart request should succeed");
    let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");
    let shipping_option = FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({ "source": "transport-checkout-test-shipping-option" }),
            },
        )
        .await
        .expect("shipping option should be created");
    let update_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "selected_shipping_option_id": shipping_option.id
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("update cart request should succeed");
    assert_eq!(update_cart_response.status(), StatusCode::OK);

    let add_line_item_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/line-items"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "variant_id": variant.id,
                        "quantity": 1,
                        "metadata": { "source": "transport-payment-test-line-item" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("add line item request should succeed");
    assert_eq!(add_line_item_response.status(), StatusCode::OK);

    let create_collection_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/payment-collections")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .header("x-medusa-locale", "de")
                .body(Body::from(
                    json!({
                        "cart_id": cart_id,
                        "metadata": { "source": "transport-payment-test" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create payment collection request should succeed");
    let create_collection_status = create_collection_response.status();
    let create_collection_body = to_bytes(create_collection_response.into_body(), usize::MAX)
        .await
        .expect("payment collection body should read");
    assert_eq!(
        create_collection_status,
        StatusCode::CREATED,
        "unexpected payment collection body: {}",
        String::from_utf8_lossy(&create_collection_body)
    );

    let first_collection: serde_json::Value = serde_json::from_slice(&create_collection_body)
        .expect("payment collection response should be JSON");
    assert_eq!(first_collection["status"], json!("pending"));
    assert_eq!(first_collection["currency_code"], json!("EUR"));
    assert_eq!(
        first_collection["metadata"]["source"],
        json!("transport-payment-test")
    );
    assert_eq!(
        first_collection["metadata"]["cart_context"]["locale"],
        json!("de")
    );
    assert_eq!(
        first_collection["metadata"]["cart_context"]["currency_code"],
        json!("EUR")
    );
    assert_eq!(
        first_collection["metadata"]["cart_context"]["email"],
        json!("buyer@example.com")
    );

    let reuse_collection_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/payment-collections")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .header("x-medusa-locale", "de")
                .body(Body::from(
                    json!({
                        "cart_id": cart_id,
                        "metadata": { "source": "transport-payment-test-retry" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("retry payment collection request should succeed");
    let reuse_collection_status = reuse_collection_response.status();
    let reuse_collection_body = to_bytes(reuse_collection_response.into_body(), usize::MAX)
        .await
        .expect("reused payment collection body should read");
    assert_eq!(
        reuse_collection_status,
        StatusCode::OK,
        "unexpected reused payment collection body: {}",
        String::from_utf8_lossy(&reuse_collection_body)
    );

    let reused_collection: serde_json::Value = serde_json::from_slice(&reuse_collection_body)
        .expect("reused payment collection response should be JSON");
    assert_eq!(reused_collection["id"], first_collection["id"]);
    assert_eq!(reused_collection["metadata"], first_collection["metadata"]);
}

#[tokio::test]
async fn store_checkout_transport_end_to_end_preserves_updated_cart_context() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Europe".to_string(),
                }],
                currency_code: "eur".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string()],
                metadata: json!({ "source": "store-checkout-flow-region" }),
            },
        )
        .await
        .expect("region should be created");
    let shipping_option = FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({ "source": "store-checkout-flow-shipping-option" }),
            },
        )
        .await
        .expect("shipping option should be created");
    let mut product_input = storefront_product_input();
    product_input.variants[0].inventory_quantity = 5;
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let created = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");
    let app = commerce_transport_router(test_app_context(db), tenant);

    let create_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/carts")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "email": "buyer@example.com",
                        "currency_code": "eur",
                        "locale": "en"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create cart request should succeed");
    assert_eq!(create_cart_response.status(), StatusCode::CREATED);
    let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");

    let update_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "email": "checkout@example.com",
                        "region_id": region.id,
                        "country_code": "de",
                        "locale": "de",
                        "selected_shipping_option_id": shipping_option.id
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("update cart request should succeed");
    let update_cart_status = update_cart_response.status();
    let update_cart_body = to_bytes(update_cart_response.into_body(), usize::MAX)
        .await
        .expect("update cart body should read");
    assert_eq!(
        update_cart_status,
        StatusCode::OK,
        "unexpected update cart body: {}",
        String::from_utf8_lossy(&update_cart_body)
    );
    let updated_cart: serde_json::Value =
        serde_json::from_slice(&update_cart_body).expect("update cart response should be JSON");
    assert_eq!(updated_cart["cart"]["email"], json!("checkout@example.com"));
    assert_eq!(updated_cart["cart"]["country_code"], json!("DE"));
    assert_eq!(updated_cart["cart"]["locale_code"], json!("de"));
    assert_eq!(updated_cart["cart"]["region_id"], json!(region.id));
    assert_eq!(
        updated_cart["cart"]["selected_shipping_option_id"],
        json!(null)
    );
    assert_eq!(updated_cart["context"]["locale"], json!("de"));
    assert_eq!(updated_cart["context"]["region"]["id"], json!(region.id));

    let add_line_item_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/line-items"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "variant_id": variant.id,
                        "quantity": 1,
                        "metadata": { "source": "store-checkout-flow-line-item" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("add line item request should succeed");
    assert_eq!(add_line_item_response.status(), StatusCode::OK);

    let shipping_options_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/store/shipping-options?cart_id={cart_id}&currency_code=usd"
                ))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("shipping options request should succeed");
    let shipping_options_status = shipping_options_response.status();
    let shipping_options_body = to_bytes(shipping_options_response.into_body(), usize::MAX)
        .await
        .expect("shipping options body should read");
    assert_eq!(
        shipping_options_status,
        StatusCode::OK,
        "unexpected shipping options body: {}",
        String::from_utf8_lossy(&shipping_options_body)
    );
    let shipping_options: serde_json::Value = serde_json::from_slice(&shipping_options_body)
        .expect("shipping options response should be JSON");
    let options = shipping_options
        .as_array()
        .expect("shipping options should be an array");
    assert_eq!(options.len(), 1);
    assert_eq!(options[0]["id"], json!(shipping_option.id));

    let payment_collection_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/payment-collections")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .header("x-medusa-locale", "de")
                .body(Body::from(
                    json!({
                        "cart_id": cart_id,
                        "metadata": { "source": "store-checkout-flow-payment" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create payment collection request should succeed");
    let payment_collection_status = payment_collection_response.status();
    let payment_collection_body = to_bytes(payment_collection_response.into_body(), usize::MAX)
        .await
        .expect("payment collection body should read");
    assert_eq!(
        payment_collection_status,
        StatusCode::CREATED,
        "unexpected payment collection body: {}",
        String::from_utf8_lossy(&payment_collection_body)
    );
    let payment_collection: serde_json::Value = serde_json::from_slice(&payment_collection_body)
        .expect("payment collection response should be JSON");
    assert_eq!(
        payment_collection["metadata"]["cart_context"]["region_id"],
        json!(region.id)
    );
    assert_eq!(
        payment_collection["metadata"]["cart_context"]["country_code"],
        json!("DE")
    );
    assert_eq!(
        payment_collection["metadata"]["cart_context"]["locale"],
        json!("de")
    );
    assert_eq!(
        payment_collection["metadata"]["cart_context"]["selected_shipping_option_id"],
        json!(shipping_option.id)
    );
    assert_eq!(
        payment_collection["metadata"]["cart_context"]["email"],
        json!("checkout@example.com")
    );

    let complete_checkout_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/complete"))
                .header("content-type", "application/json")
                .header("idempotency-key", "store-checkout-flow-complete")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "create_fulfillment": false,
                        "metadata": { "source": "store-checkout-flow-complete" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("complete checkout request should succeed");
    let complete_checkout_status = complete_checkout_response.status();
    let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
        .await
        .expect("complete checkout body should read");
    assert_eq!(
        complete_checkout_status,
        StatusCode::OK,
        "unexpected complete checkout body: {}",
        String::from_utf8_lossy(&complete_checkout_body)
    );
    let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
        .expect("complete checkout response should be JSON");
    assert_eq!(completed["cart"]["status"], json!("completed"));
    assert_eq!(completed["cart"]["country_code"], json!("DE"));
    assert_eq!(completed["cart"]["locale_code"], json!("de"));
    assert_eq!(completed["cart"]["region_id"], json!(region.id));
    assert_eq!(
        completed["cart"]["selected_shipping_option_id"],
        json!(shipping_option.id)
    );
    assert_eq!(completed["context"]["locale"], json!("de"));
    assert_eq!(completed["context"]["region"]["id"], json!(region.id));
    assert_eq!(completed["order"]["status"], json!("paid"));
    assert_eq!(completed["order"]["tax_included"], json!(true));
    assert_eq!(
        completed["cart"]["tax_total"],
        completed["order"]["tax_total"]
    );
    assert_eq!(
        completed["cart"]["tax_lines"][0]["provider_id"],
        json!("region_default")
    );
    assert_eq!(
        completed["order"]["tax_lines"][0]["provider_id"],
        json!("region_default")
    );
    assert_eq!(
        completed["payment_collection"]["id"],
        payment_collection["id"]
    );
    assert_eq!(completed["payment_collection"]["status"], json!("captured"));
    assert_eq!(
        completed["payment_collection"]["amount"],
        completed["order"]["total_amount"]
    );
    assert!(completed["fulfillment"].is_null());
}

#[tokio::test]
async fn store_checkout_transport_completes_guest_cart_with_existing_payment_and_no_fulfillment() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let auth = AuthContext {
        user_id: actor_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    let mut product_input = storefront_product_input();
    product_input.variants[0].inventory_quantity = 5;
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let created = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");
    let app = commerce_transport_router(test_app_context(db.clone()), tenant.clone());
    let authed_app =
        commerce_transport_router_with_auth(test_app_context(db.clone()), tenant, Some(auth));

    let create_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/carts")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "email": "guest@example.com",
                        "currency_code": "eur",
                        "locale": "de"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create cart request should succeed");
    assert_eq!(create_cart_response.status(), StatusCode::CREATED);
    let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");
    let shipping_option = FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({ "source": "transport-checkout-test-shipping-option" }),
            },
        )
        .await
        .expect("shipping option should be created");
    let update_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "selected_shipping_option_id": shipping_option.id
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("update cart request should succeed");
    assert_eq!(update_cart_response.status(), StatusCode::OK);

    let add_line_item_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/line-items"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "variant_id": variant.id,
                        "quantity": 1,
                        "metadata": { "source": "transport-checkout-test-line-item" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("add line item request should succeed");
    assert_eq!(add_line_item_response.status(), StatusCode::OK);

    let payment_collection_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/payment-collections")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .header("x-medusa-locale", "de")
                .body(Body::from(
                    json!({
                        "cart_id": cart_id,
                        "metadata": { "source": "transport-checkout-test-payment" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create payment collection request should succeed");
    assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

    let complete_checkout_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/complete"))
                .header("content-type", "application/json")
                .header("idempotency-key", "transport-checkout-test-complete")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "create_fulfillment": false,
                        "metadata": { "source": "transport-checkout-test-complete" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("complete checkout request should succeed");
    let complete_checkout_status = complete_checkout_response.status();
    let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
        .await
        .expect("complete checkout body should read");
    assert_eq!(
        complete_checkout_status,
        StatusCode::OK,
        "unexpected complete checkout body: {}",
        String::from_utf8_lossy(&complete_checkout_body)
    );

    let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
        .expect("complete checkout response should be JSON");
    assert_eq!(completed["cart"]["status"], json!("completed"));
    assert_eq!(completed["order"]["status"], json!("paid"));
    assert_eq!(completed["payment_collection"]["status"], json!("captured"));
    assert!(completed["fulfillment"].is_null());
    assert_eq!(completed["context"]["locale"], json!("de"));

    let get_order_response = authed_app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/store/orders/{}",
                    completed["order"]["id"]
                        .as_str()
                        .expect("order id should exist")
                ))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("get order request should complete");
    assert_eq!(get_order_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn store_checkout_transport_rejects_customer_owned_cart_without_auth() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let owner_auth = AuthContext {
        user_id: owner_user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    create_customer_for_user(&db, tenant_id, owner_user_id, "owner@example.com").await;

    let owner_app = commerce_transport_router_with_auth(
        test_app_context(db.clone()),
        tenant.clone(),
        Some(owner_auth),
    );
    let guest_app = commerce_transport_router(test_app_context(db), tenant);

    let create_cart_response = owner_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/carts")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "email": "owner@example.com",
                        "currency_code": "eur",
                        "locale": "de"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create cart request should succeed");
    assert_eq!(create_cart_response.status(), StatusCode::CREATED);
    let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");

    let complete_checkout_response = guest_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/complete"))
                .header("content-type", "application/json")
                .header("idempotency-key", "transport-checkout-owner-guard")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "create_fulfillment": false,
                        "metadata": { "source": "transport-checkout-owner-guard" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("complete checkout request should complete");
    let status = complete_checkout_response.status();
    let body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
        .await
        .expect("complete checkout body should read");
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected complete checkout body: {}",
        String::from_utf8_lossy(&body)
    );
}

#[tokio::test]
async fn store_payment_collection_transport_returns_not_found_for_unknown_cart() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let app = commerce_transport_router(test_app_context(db), tenant);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/payment-collections")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "cart_id": Uuid::new_v4(),
                        "metadata": { "source": "unknown-cart-payment-guard" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("payment collection request should complete");

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("payment collection body should read");
    assert_eq!(status, StatusCode::NOT_FOUND);
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("payment collection error should be JSON");
    assert_eq!(payload["code"], json!("cart.cart_not_found"));
}

#[tokio::test]
async fn store_checkout_transport_rejects_payment_collection_for_completed_cart() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let app = commerce_transport_router(test_app_context(db.clone()), tenant);

    let create_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/carts")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "email": "guest@example.com",
                        "currency_code": "eur",
                        "locale": "de"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create cart request should succeed");
    assert_eq!(create_cart_response.status(), StatusCode::CREATED);
    let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");

    let actor_id = Uuid::new_v4();
    let mut product_input = storefront_product_input();
    product_input.variants[0].inventory_quantity = 5;
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let created = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");

    let add_line_item_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/line-items"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "variant_id": variant.id,
                        "quantity": 1,
                        "metadata": { "source": "completed-cart-payment-guard" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("add line item request should succeed");
    assert_eq!(add_line_item_response.status(), StatusCode::OK);

    let cart_service = CartService::new(db.clone());
    let cart_uuid = Uuid::parse_str(cart_id).expect("cart id should be valid uuid");
    let completed = cart_service
        .complete_cart(tenant_id, cart_uuid)
        .await
        .expect("cart should transition to completed");
    assert_eq!(completed.status, "completed");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/payment-collections")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "cart_id": cart_id,
                        "metadata": { "source": "completed-cart-payment-guard" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("payment collection request should complete");

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("payment collection body should read");
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("payment collection error should be JSON");
    assert_eq!(payload["code"], json!("commerce_store_invalid"));
    assert_eq!(
        payload["message"],
        json!("Cannot create payment collection for completed cart")
    );
}

#[tokio::test]
async fn store_checkout_transport_carries_cart_channel_snapshot_into_order() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let mut product_input = storefront_product_input();
    product_input.variants[0].inventory_quantity = 5;
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let created = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");
    let mut channel = sample_channel_context("marketplace-eu");
    channel.tenant_id = tenant_id;
    let channel_id = channel.id;
    seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
    let app = commerce_transport_router_with_context(
        test_app_context(db.clone()),
        tenant,
        None,
        Some(channel),
    );

    let create_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/carts")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "email": "guest@example.com",
                        "currency_code": "eur",
                        "locale": "de"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create cart request should succeed");
    assert_eq!(create_cart_response.status(), StatusCode::CREATED);
    let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");
    assert_eq!(created_cart["cart"]["channel_id"], json!(channel_id));
    assert_eq!(
        created_cart["cart"]["channel_slug"],
        json!("marketplace-eu")
    );
    let shipping_option = FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Channel Shipping".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({ "source": "channel-checkout-shipping-option" }),
            },
        )
        .await
        .expect("shipping option should be created");
    let update_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "selected_shipping_option_id": shipping_option.id
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("update cart request should succeed");
    assert_eq!(update_cart_response.status(), StatusCode::OK);

    let add_line_item_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/line-items"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "variant_id": variant.id,
                        "quantity": 1,
                        "metadata": { "source": "channel-checkout-line-item" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("add line item request should succeed");
    assert_eq!(add_line_item_response.status(), StatusCode::OK);

    let payment_collection_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/payment-collections")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "cart_id": cart_id,
                        "metadata": { "source": "channel-checkout-payment" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create payment collection request should succeed");
    assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

    let complete_checkout_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/complete"))
                .header("content-type", "application/json")
                .header("idempotency-key", "channel-checkout-complete")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "create_fulfillment": false,
                        "metadata": { "source": "channel-checkout-complete" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("complete checkout request should succeed");
    assert_eq!(complete_checkout_response.status(), StatusCode::OK);

    let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
        .await
        .expect("complete checkout body should read");
    let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
        .expect("complete checkout response should be JSON");
    assert_eq!(completed["order"]["channel_id"], json!(channel_id));
    assert_eq!(completed["order"]["channel_slug"], json!("marketplace-eu"));
}

#[tokio::test]
async fn store_order_transport_returns_customer_owned_order_after_checkout() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let auth = AuthContext {
        user_id: actor_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    let customer_id =
        create_customer_for_user(&db, tenant_id, actor_id, "customer@example.com").await;
    let mut product_input = storefront_product_input();
    product_input.variants[0].inventory_quantity = 5;
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let created = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");
    let app = commerce_transport_router_with_auth(test_app_context(db.clone()), tenant, Some(auth));

    let create_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/carts")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "email": "customer@example.com",
                        "currency_code": "eur",
                        "locale": "de"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create cart request should succeed");
    assert_eq!(create_cart_response.status(), StatusCode::CREATED);
    let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");
    assert_eq!(created_cart["cart"]["customer_id"], json!(customer_id));
    let shipping_option = FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Order Shipping".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({ "source": "transport-order-test-shipping-option" }),
            },
        )
        .await
        .expect("shipping option should be created");
    let update_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "selected_shipping_option_id": shipping_option.id
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("update cart request should succeed");
    assert_eq!(update_cart_response.status(), StatusCode::OK);

    let add_line_item_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/line-items"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "variant_id": variant.id,
                        "quantity": 1,
                        "metadata": { "source": "transport-order-test-line-item" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("add line item request should succeed");
    assert_eq!(add_line_item_response.status(), StatusCode::OK);

    let payment_collection_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/payment-collections")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .header("x-medusa-locale", "de")
                .body(Body::from(
                    json!({
                        "cart_id": cart_id,
                        "metadata": { "source": "transport-order-test-payment" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create payment collection request should succeed");
    assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

    let complete_checkout_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/complete"))
                .header("content-type", "application/json")
                .header("idempotency-key", "transport-order-test-complete")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "create_fulfillment": false,
                        "metadata": { "source": "transport-order-test-complete" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("complete checkout request should succeed");
    assert_eq!(complete_checkout_response.status(), StatusCode::OK);
    let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
        .await
        .expect("complete checkout body should read");
    let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
        .expect("complete checkout response should be JSON");
    let order_id = completed["order"]["id"]
        .as_str()
        .expect("order id should be returned");

    let get_order_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/store/orders/{order_id}"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("get order request should succeed");
    let get_order_status = get_order_response.status();
    let get_order_body = to_bytes(get_order_response.into_body(), usize::MAX)
        .await
        .expect("get order body should read");
    assert_eq!(
        get_order_status,
        StatusCode::OK,
        "unexpected get order body: {}",
        String::from_utf8_lossy(&get_order_body)
    );

    let order: serde_json::Value =
        serde_json::from_slice(&get_order_body).expect("order response should be JSON");
    assert_eq!(order["id"], completed["order"]["id"]);
    assert_eq!(order["customer_id"], json!(customer_id));
    assert_eq!(order["status"], json!("paid"));
    assert_eq!(order["currency_code"], json!("EUR"));
    assert_eq!(
        order["subtotal_amount"],
        completed["order"]["subtotal_amount"]
    );
    assert_eq!(order["total_amount"], completed["order"]["total_amount"]);
    assert_eq!(order["tax_included"], completed["order"]["tax_included"]);
    assert_eq!(order["tax_total"], completed["order"]["tax_total"]);
    assert_eq!(order["tax_lines"], completed["order"]["tax_lines"]);
    assert_eq!(order["tax_total"], json!("0"));
    assert_eq!(order["tax_lines"], json!([]));
}

#[tokio::test]
async fn store_order_transport_rejects_order_for_another_customer() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let other_user_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let owner_auth = AuthContext {
        user_id: owner_user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    let other_auth = AuthContext {
        user_id: other_user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::ORDERS_READ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    create_customer_for_user(&db, tenant_id, owner_user_id, "owner@example.com").await;
    create_customer_for_user(&db, tenant_id, other_user_id, "other@example.com").await;
    let actor_id = owner_user_id;
    let mut product_input = storefront_product_input();
    product_input.variants[0].inventory_quantity = 5;
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let created = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");
    let owner_app = commerce_transport_router_with_auth(
        test_app_context(db.clone()),
        tenant.clone(),
        Some(owner_auth),
    );
    let other_app =
        commerce_transport_router_with_auth(test_app_context(db.clone()), tenant, Some(other_auth));

    let create_cart_response = owner_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/carts")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "email": "owner@example.com",
                        "currency_code": "eur",
                        "locale": "de"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create cart request should succeed");
    assert_eq!(create_cart_response.status(), StatusCode::CREATED);
    let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");
    let shipping_option = FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Ownership Shipping".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({ "source": "transport-order-ownership-shipping-option" }),
            },
        )
        .await
        .expect("shipping option should be created");
    let update_cart_response = owner_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "selected_shipping_option_id": shipping_option.id
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("update cart request should succeed");
    assert_eq!(update_cart_response.status(), StatusCode::OK);

    let add_line_item_response = owner_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/line-items"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "variant_id": variant.id,
                        "quantity": 1,
                        "metadata": { "source": "transport-order-ownership-line-item" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("add line item request should succeed");
    assert_eq!(add_line_item_response.status(), StatusCode::OK);

    let payment_collection_response = owner_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/payment-collections")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .header("x-medusa-locale", "de")
                .body(Body::from(
                    json!({
                        "cart_id": cart_id,
                        "metadata": { "source": "transport-order-ownership-payment" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create payment collection request should succeed");
    assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

    let complete_checkout_response = owner_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/complete"))
                .header("content-type", "application/json")
                .header("idempotency-key", "transport-order-ownership-complete")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "create_fulfillment": false,
                        "metadata": { "source": "transport-order-ownership-complete" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("complete checkout request should succeed");
    assert_eq!(complete_checkout_response.status(), StatusCode::OK);
    let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
        .await
        .expect("complete checkout body should read");
    let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
        .expect("complete checkout response should be JSON");
    let order_id = completed["order"]["id"]
        .as_str()
        .expect("order id should be returned");

    let get_order_response = other_app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/store/orders/{order_id}"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("get order request should complete");
    let get_order_status = get_order_response.status();
    let get_order_body = to_bytes(get_order_response.into_body(), usize::MAX)
        .await
        .expect("get order body should read");
    assert_eq!(
        get_order_status,
        StatusCode::UNAUTHORIZED,
        "unexpected get order body: {}",
        String::from_utf8_lossy(&get_order_body)
    );
}

#[tokio::test]
async fn store_customer_me_transport_returns_customer_for_authenticated_user() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let auth = AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::ORDERS_READ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    let customer_id =
        create_customer_for_user(&db, tenant_id, user_id, "customer-me@example.com").await;
    let app = commerce_transport_router_with_auth(test_app_context(db), tenant, Some(auth));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/store/customers/me")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("get me request should succeed");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("get me body should read");
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected get me body: {}",
        String::from_utf8_lossy(&body)
    );

    let customer: serde_json::Value =
        serde_json::from_slice(&body).expect("get me response should be JSON");
    assert_eq!(customer["id"], json!(customer_id));
    assert_eq!(customer["user_id"], json!(user_id));
    assert_eq!(customer["email"], json!("customer-me@example.com"));
    assert_eq!(customer["locale"], json!("de"));
}
