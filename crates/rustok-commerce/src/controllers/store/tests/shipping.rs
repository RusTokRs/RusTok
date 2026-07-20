use super::*;

#[tokio::test]
async fn store_shipping_options_transport_filters_channel_hidden_options() {
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
    let fulfillment = FulfillmentService::new(db.clone());
    let visible_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Visible Shipping".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({}),
            },
        )
        .await
        .expect("visible shipping option should be created");
    fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Hidden Shipping".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("19.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({
                    "channel_visibility": {
                        "allowed_channel_slugs": ["mobile-app"]
                    }
                }),
            },
        )
        .await
        .expect("hidden shipping option should be created");

    let mut channel = sample_channel_context("web-store");
    channel.tenant_id = tenant_id;
    seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
    let app =
        commerce_transport_router_with_context(test_app_context(db), tenant, None, Some(channel));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/store/shipping-options?currency_code=eur&locale=de")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("shipping options request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("shipping options body should read");
    let json: serde_json::Value =
        serde_json::from_slice(&body).expect("shipping options response should be JSON");
    let options = json
        .as_array()
        .expect("shipping options should be an array");
    assert_eq!(options.len(), 1);
    assert_eq!(options[0]["id"], json!(visible_option.id));
}

#[tokio::test]
async fn store_shipping_options_transport_filters_incompatible_shipping_profiles() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
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
    let mut product_input = storefront_product_input();
    product_input.metadata = json!({
        "shipping_profile": {
            "slug": "bulky"
        }
    });
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
        .expect("published product should include variant");

    let fulfillment = FulfillmentService::new(db.clone());
    fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Default Shipping".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                metadata: json!({
                    "shipping_profiles": {
                        "allowed_slugs": ["default"]
                    }
                }),
            },
        )
        .await
        .expect("default shipping option should be created");
    let bulky_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Bulky Freight".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("29.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["bulky".to_string()]),
                metadata: json!({
                    "shipping_profiles": {
                        "allowed_slugs": ["bulky"]
                    }
                }),
            },
        )
        .await
        .expect("bulky shipping option should be created");

    let cart_service = CartService::new(db.clone());
    let (metadata, guest_cart_token) = rustok_cart::prepare_guest_cart_metadata(
        None,
        json!({ "source": "store-shipping-profile-filter" }),
    );
    let cart = cart_service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata,
            },
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .expect("cart should be created");
    cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(variant.id),
                shipping_profile_slug: Some("bulky".to_string()),
                sku: variant.sku.clone(),
                title: variant.title.clone(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: json!({ "slot": 1 }),
            },
        )
        .await
        .expect("line item should be added");

    let mut channel = sample_channel_context("web-store");
    channel.id = channel_id;
    channel.tenant_id = tenant_id;
    seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
    let app =
        commerce_transport_router_with_context(test_app_context(db), tenant, None, Some(channel))
            .with_guest_cart_token(guest_cart_token.expect("guest cart token should be issued"));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/store/shipping-options?cart_id={}&currency_code=eur&locale=de",
                    cart.id
                ))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("shipping options request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("shipping options body should read");
    let json: serde_json::Value =
        serde_json::from_slice(&body).expect("shipping options response should be JSON");
    let options = json
        .as_array()
        .expect("shipping options should be an array");
    assert_eq!(options.len(), 1);
    assert_eq!(options[0]["id"], json!(bulky_option.id));
    assert_eq!(
        options[0]["allowed_shipping_profile_slugs"],
        json!(["bulky"])
    );
}

#[tokio::test]
async fn store_update_cart_context_rejects_incompatible_shipping_profile_option() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
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
    let mut product_input = storefront_product_input();
    product_input.metadata = json!({
        "shipping_profile": {
            "slug": "bulky"
        }
    });
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
        .expect("published product should include variant");

    let incompatible_option = FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Default Shipping".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                metadata: json!({
                    "shipping_profiles": {
                        "allowed_slugs": ["default"]
                    }
                }),
            },
        )
        .await
        .expect("shipping option should be created");

    let cart_service = CartService::new(db.clone());
    let (metadata, guest_cart_token) = rustok_cart::prepare_guest_cart_metadata(
        None,
        json!({ "source": "store-shipping-profile-update" }),
    );
    let cart = cart_service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata,
            },
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .expect("cart should be created");
    cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(variant.id),
                shipping_profile_slug: Some("bulky".to_string()),
                sku: variant.sku.clone(),
                title: variant.title.clone(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: json!({ "slot": 1 }),
            },
        )
        .await
        .expect("line item should be added");

    let mut channel = sample_channel_context("web-store");
    channel.id = channel_id;
    channel.tenant_id = tenant_id;
    seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
    let app =
        commerce_transport_router_with_context(test_app_context(db), tenant, None, Some(channel))
            .with_guest_cart_token(guest_cart_token.expect("guest cart token should be issued"));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/store/carts/{}", cart.id))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "selected_shipping_option_id": incompatible_option.id
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("update cart request should complete");

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("update cart body should read");
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected update cart body: {}",
        String::from_utf8_lossy(&body)
    );
    let updated_cart: serde_json::Value =
        serde_json::from_slice(&body).expect("updated cart response should be JSON");
    assert_eq!(
        updated_cart["cart"]["selected_shipping_option_id"],
        json!(null)
    );
    assert_eq!(
        updated_cart["cart"]["delivery_groups"][0]["shipping_profile_slug"],
        json!("bulky")
    );
    assert_eq!(
        updated_cart["cart"]["delivery_groups"][0]["available_shipping_options"],
        json!([])
    );
}

#[tokio::test]
async fn store_shipping_options_transport_uses_cart_context_currency_over_query_drift() {
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
    let fulfillment = FulfillmentService::new(db.clone());
    let eur_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "EU Standard".to_string(),
                }],
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({ "source": "store-shipping-options-eur" }),
            },
        )
        .await
        .expect("EUR shipping option should be created");
    let usd_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "US Express".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("19.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({ "source": "store-shipping-options-usd" }),
            },
        )
        .await
        .expect("USD shipping option should be created");
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
    let status = shipping_options_response.status();
    let body = to_bytes(shipping_options_response.into_body(), usize::MAX)
        .await
        .expect("shipping options body should read");
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected shipping options body: {}",
        String::from_utf8_lossy(&body)
    );

    let shipping_options: serde_json::Value =
        serde_json::from_slice(&body).expect("shipping options response should be JSON");
    let options = shipping_options
        .as_array()
        .expect("shipping options should be an array");
    assert_eq!(options.len(), 1, "cart context should override query drift");
    assert_eq!(options[0]["id"], json!(eur_option.id));
    assert_eq!(options[0]["currency_code"], json!("EUR"));
    assert_ne!(options[0]["id"], json!(usd_option.id));
}

#[tokio::test]
async fn store_shipping_options_transport_rejects_customer_owned_cart_for_foreign_customer() {
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
        permissions: vec![],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    let other_auth = AuthContext {
        user_id: other_user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    create_customer_for_user(&db, tenant_id, owner_user_id, "owner@example.com").await;
    create_customer_for_user(&db, tenant_id, other_user_id, "other@example.com").await;

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

    let shipping_options_response = other_app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/store/shipping-options?cart_id={cart_id}"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("shipping options request should complete");
    let status = shipping_options_response.status();
    let body = to_bytes(shipping_options_response.into_body(), usize::MAX)
        .await
        .expect("shipping options body should read");
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unexpected shipping options body: {}",
        String::from_utf8_lossy(&body)
    );
}
