use super::*;

#[tokio::test]
async fn store_cart_transport_persists_channel_snapshot() {
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
    let mut channel = sample_channel_context("web-store");
    channel.tenant_id = tenant_id;
    let channel_id = channel.id;
    seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
    let app =
        commerce_transport_router_with_context(test_app_context(db), tenant, None, Some(channel));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/carts")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "email": "channel-cart@example.com",
                        "currency_code": "eur",
                        "locale": "de"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create cart request should succeed");
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&body).expect("create cart response should be JSON");
    assert_eq!(created_cart["cart"]["channel_id"], json!(channel_id));
    assert_eq!(created_cart["cart"]["channel_slug"], json!("web-store"));
}

#[tokio::test]
async fn store_cart_line_item_transport_rejects_channel_hidden_product() {
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
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let mut hidden_input = storefront_product_input();
    hidden_input.translations[0].handle = Some("channel-hidden-variant-en".to_string());
    hidden_input.translations[1].handle = Some("channel-hidden-variant-de".to_string());
    hidden_input.variants[0].sku = Some("STOREFRONT-CHANNEL-HIDDEN-SKU-1".to_string());
    hidden_input.metadata = json!({
        "channel_visibility": {
            "allowed_channel_slugs": ["mobile-app"]
        }
    });
    let hidden = catalog
        .create_product(tenant_id, actor_id, hidden_input)
        .await
        .expect("hidden product should be created");
    let hidden = catalog
        .publish_product(tenant_id, actor_id, hidden.id)
        .await
        .expect("hidden product should be published");
    let variant = hidden
        .variants
        .first()
        .expect("hidden product should have variant");

    let mut channel = sample_channel_context("web-store");
    channel.tenant_id = tenant_id;
    seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
    let app =
        commerce_transport_router_with_context(test_app_context(db), tenant, None, Some(channel));

    let create_response = app
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
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");

    let add_response = app
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
                        "metadata": {}
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("add line item request should complete");

    assert_eq!(add_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn store_cart_transport_uses_tristate_update_semantics_end_to_end() {
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

    let create_response = app
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
    let create_status = create_response.status();
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    assert_eq!(
        create_status,
        StatusCode::CREATED,
        "unexpected create cart body: {}",
        String::from_utf8_lossy(&create_body)
    );

    let created: serde_json::Value =
        serde_json::from_slice(&create_body).expect("create cart response should be JSON");
    let cart_id = created["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");
    assert_eq!(created["cart"]["email"], json!("buyer@example.com"));
    assert_eq!(created["cart"]["locale_code"], json!("de"));
    assert_eq!(created["context"]["locale"], json!("de"));

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .header("x-medusa-locale", "en")
                .body(Body::from(
                    json!({
                        "email": null,
                        "locale": null
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("update cart request should succeed");
    let update_status = update_response.status();
    let update_body = to_bytes(update_response.into_body(), usize::MAX)
        .await
        .expect("update cart body should read");
    assert_eq!(
        update_status,
        StatusCode::OK,
        "unexpected update cart body: {}",
        String::from_utf8_lossy(&update_body)
    );

    let updated: serde_json::Value =
        serde_json::from_slice(&update_body).expect("update cart response should be JSON");
    assert_eq!(updated["cart"]["id"], json!(cart_id));
    assert!(updated["cart"]["email"].is_null());
    assert_eq!(updated["cart"]["locale_code"], json!("en"));
    assert_eq!(updated["context"]["locale"], json!("en"));
}

#[tokio::test]
async fn store_cart_transport_rejects_currency_mismatch_for_region() {
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
                metadata: json!({ "source": "store-cart-region-mismatch" }),
            },
        )
        .await
        .expect("region should be created");
    let app = commerce_transport_router(test_app_context(db.clone()), tenant);

    let create_response = app
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
                        "region_id": region.id,
                        "currency_code": "usd",
                        "locale": "de"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create cart request should complete");
    let status = create_response.status();
    let body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let body_text = String::from_utf8_lossy(&body);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unexpected create cart body: {body_text}",
    );
    assert!(
        body_text.contains("USD"),
        "body should mention requested currency: {body_text}"
    );
    assert!(
        body_text.contains("EUR"),
        "body should mention region currency: {body_text}"
    );
    assert!(
        body_text.contains(&region.id.to_string()),
        "body should mention conflicting region: {body_text}"
    );
}

#[tokio::test]
async fn store_cart_line_item_transport_resolves_backend_title_and_price() {
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

    let create_response = app
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
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");

    let line_item_response = app
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
                        "quantity": 2,
                        "metadata": { "source": "transport-line-item-test" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("add line item request should succeed");
    let line_item_status = line_item_response.status();
    let line_item_body = to_bytes(line_item_response.into_body(), usize::MAX)
        .await
        .expect("line item body should read");
    assert_eq!(
        line_item_status,
        StatusCode::OK,
        "unexpected add line item body: {}",
        String::from_utf8_lossy(&line_item_body)
    );

    let updated_cart: serde_json::Value =
        serde_json::from_slice(&line_item_body).expect("updated cart should be JSON");
    assert_eq!(
        updated_cart["line_items"][0]["variant_id"],
        json!(variant.id)
    );
    assert_eq!(
        updated_cart["line_items"][0]["product_id"],
        json!(published.id)
    );
    assert_eq!(
        updated_cart["line_items"][0]["sku"],
        json!("STOREFRONT-SKU-1")
    );
    assert_eq!(
        updated_cart["line_items"][0]["title"],
        json!("Storefront Produkt / Default")
    );
    assert_eq!(updated_cart["line_items"][0]["unit_price"], json!("19.99"));
    assert_eq!(updated_cart["line_items"][0]["quantity"], json!(2));
    assert_eq!(
        updated_cart["line_items"][0]["metadata"],
        json!({
            "seller": { "id": null },
            "source": "transport-line-item-test"
        })
    );
}

#[tokio::test]
async fn store_cart_line_item_transport_returns_not_found_for_unknown_variant() {
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

    let create_response = app
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
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .expect("create cart body should read");
    let created_cart: serde_json::Value =
        serde_json::from_slice(&create_body).expect("create cart response should be JSON");
    let cart_id = created_cart["cart"]["id"]
        .as_str()
        .expect("cart id should be returned");

    let line_item_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{cart_id}/line-items"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "variant_id": Uuid::new_v4(),
                        "quantity": 1,
                        "metadata": {}
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("add line item request should complete");

    assert_eq!(line_item_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn store_cart_transport_returns_typed_adjustments_and_totals() {
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
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let created = catalog
        .create_product(tenant_id, actor_id, storefront_product_input())
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
    let cart_service = CartService::new(db.clone());
    let (metadata, guest_cart_token) = rustok_cart::prepare_guest_cart_metadata(
        None,
        json!({ "source": "store-cart-adjustment-cart" }),
    );
    let app = commerce_transport_router(test_app_context(db.clone()), tenant)
        .with_guest_cart_token(guest_cart_token.expect("guest cart token should be issued"));
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: None,
                currency_code: "eur".to_string(),
                metadata,
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
            },
        )
        .await
        .expect("cart should be created");
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(variant.id),
                shipping_profile_slug: None,
                sku: variant.sku.clone(),
                title: variant.title.clone(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: json!({ "source": "store-cart-adjustment-line-item" }),
            },
        )
        .await
        .expect("line item should be added");
    let cart_id = cart.id;
    let line_item_id = cart.line_items[0].id;

    cart_service
        .set_adjustments(
            tenant_id,
            cart_id,
            vec![SetCartAdjustmentInput {
                line_item_id: Some(line_item_id),
                source_type: "Promotion".to_string(),
                source_id: Some("promo-store".to_string()),
                amount: Decimal::from_str("4.99").expect("valid decimal"),
                metadata: json!({
                    "rule_code": "store-adjustment",
                    "display_label": "Store promotion"
                }),
            }],
        )
        .await
        .expect("cart adjustment should be stored");

    let get_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/store/carts/{cart_id}"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("get cart request should succeed");
    let get_cart_status = get_cart_response.status();
    let get_cart_body = to_bytes(get_cart_response.into_body(), usize::MAX)
        .await
        .expect("get cart body should read");
    assert_eq!(
        get_cart_status,
        StatusCode::OK,
        "unexpected get cart adjustment body: {}",
        String::from_utf8_lossy(&get_cart_body)
    );

    let cart: serde_json::Value =
        serde_json::from_slice(&get_cart_body).expect("cart response should be JSON");
    assert_eq!(cart["subtotal_amount"], json!("19.99"));
    assert_eq!(cart["adjustment_total"], json!("4.99"));
    assert_eq!(cart["total_amount"], json!("15"));
    assert_eq!(cart["adjustments"][0]["line_item_id"], json!(line_item_id));
    assert_eq!(cart["adjustments"][0]["source_type"], json!("promotion"));
    assert_eq!(cart["adjustments"][0]["source_id"], json!("promo-store"));
    assert_eq!(cart["adjustments"][0]["amount"], json!("4.99"));
    assert_eq!(cart["adjustments"][0]["currency_code"], json!("EUR"));
    assert_eq!(
        cart["adjustments"][0]["metadata"],
        json!({ "rule_code": "store-adjustment" })
    );
}

#[tokio::test]
async fn store_cart_transport_returns_shipping_total_and_shipping_scoped_promotion() {
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
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let created = catalog
        .create_product(tenant_id, actor_id, storefront_product_input())
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
                metadata: json!({ "source": "store-cart-shipping-promotion" }),
            },
        )
        .await
        .expect("shipping option should be created");
    let cart_service = CartService::new(db.clone());
    let (metadata, guest_cart_token) = rustok_cart::prepare_guest_cart_metadata(
        None,
        json!({ "source": "store-cart-shipping-promotion" }),
    );
    let app = commerce_transport_router(test_app_context(db.clone()), tenant)
        .with_guest_cart_token(guest_cart_token.expect("guest cart token should be issued"));
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: None,
                currency_code: "eur".to_string(),
                metadata,
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
            },
        )
        .await
        .expect("cart should be created");
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(variant.id),
                shipping_profile_slug: None,
                sku: variant.sku.clone(),
                title: variant.title.clone(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: json!({ "source": "store-cart-shipping-promotion-line-item" }),
            },
        )
        .await
        .expect("line item should be added");
    let cart_id = cart.id;

    cart_service
        .apply_fixed_shipping_promotion(
            tenant_id,
            cart_id,
            "promo-shipping-store",
            Decimal::from_str("4.99").expect("valid decimal"),
            json!({
                "campaign": "shipping-half-off",
                "display_label": "Shipping half off"
            }),
        )
        .await
        .expect("shipping promotion should be stored");

    let get_cart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/store/carts/{cart_id}"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("get cart request should succeed");
    let get_cart_status = get_cart_response.status();
    let get_cart_body = to_bytes(get_cart_response.into_body(), usize::MAX)
        .await
        .expect("get cart body should read");
    assert_eq!(
        get_cart_status,
        StatusCode::OK,
        "unexpected get cart shipping promotion body: {}",
        String::from_utf8_lossy(&get_cart_body)
    );

    let cart: serde_json::Value =
        serde_json::from_slice(&get_cart_body).expect("cart response should be JSON");
    assert_eq!(cart["subtotal_amount"], json!("19.99"));
    assert_eq!(cart["shipping_total"], json!("9.99"));
    assert_eq!(cart["adjustment_total"], json!("4.99"));
    assert_eq!(cart["total_amount"], json!("24.99"));
    assert_eq!(cart["adjustments"][0]["line_item_id"], json!(null));
    assert_eq!(cart["adjustments"][0]["source_type"], json!("promotion"));
    assert_eq!(
        cart["adjustments"][0]["source_id"],
        json!("promo-shipping-store")
    );
    assert_eq!(cart["adjustments"][0]["amount"], json!("4.99"));
    assert_eq!(cart["adjustments"][0]["currency_code"], json!("EUR"));
    assert_eq!(
        cart["adjustments"][0]["metadata"],
        json!({ "campaign": "shipping-half-off", "kind": "fixed_discount", "scope": "shipping", "fixed_amount": "4.99" })
    );
}

#[tokio::test]
async fn store_cart_transport_rejects_customer_owned_cart_for_another_customer() {
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
        permissions: vec![Permission::ORDERS_READ],
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
    let owner_customer_id =
        create_customer_for_user(&db, tenant_id, owner_user_id, "cart-owner@example.com").await;
    create_customer_for_user(&db, tenant_id, other_user_id, "cart-other@example.com").await;
    let owner_app = commerce_transport_router_with_auth(
        test_app_context(db.clone()),
        tenant.clone(),
        Some(owner_auth),
    );
    let other_app =
        commerce_transport_router_with_auth(test_app_context(db), tenant, Some(other_auth));

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
                        "email": "cart-owner@example.com",
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
    assert_eq!(
        created_cart["cart"]["customer_id"],
        json!(owner_customer_id)
    );

    let get_cart_response = other_app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/store/carts/{cart_id}"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("get cart request should complete");
    let get_cart_status = get_cart_response.status();
    let get_cart_body = to_bytes(get_cart_response.into_body(), usize::MAX)
        .await
        .expect("get cart body should read");
    assert_eq!(
        get_cart_status,
        StatusCode::UNAUTHORIZED,
        "unexpected get cart body: {}",
        String::from_utf8_lossy(&get_cart_body)
    );
}
