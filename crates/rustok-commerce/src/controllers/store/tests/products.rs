use super::*;

#[tokio::test]
async fn store_products_transport_rejects_disabled_channel_module() {
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
    seed_channel_binding(&db, &channel, MODULE_SLUG, false).await;
    let app =
        commerce_transport_router_with_context(test_app_context(db), tenant, None, Some(channel));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/store/products")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("store products request should complete");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn store_products_transport_filters_channel_hidden_products() {
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

    let mut visible_input = storefront_product_input();
    visible_input.translations[0].title = "Visible Product".to_string();
    visible_input.translations[0].handle = Some("visible-storefront-product-en".to_string());
    visible_input.translations[1].title = "Sichtbares Produkt".to_string();
    visible_input.translations[1].handle = Some("sichtbares-storefront-product-de".to_string());
    visible_input.variants[0].sku = Some("STOREFRONT-VISIBLE-SKU-1".to_string());
    let visible = catalog
        .create_product(tenant_id, actor_id, visible_input)
        .await
        .expect("visible product should be created");
    catalog
        .publish_product(tenant_id, actor_id, visible.id)
        .await
        .expect("visible product should be published");

    let mut hidden_input = storefront_product_input();
    hidden_input.translations[0].title = "Hidden Product".to_string();
    hidden_input.translations[0].handle = Some("hidden-storefront-product-en".to_string());
    hidden_input.translations[1].title = "Verstecktes Produkt".to_string();
    hidden_input.translations[1].handle = Some("verstecktes-storefront-product-de".to_string());
    hidden_input.variants[0].sku = Some("STOREFRONT-HIDDEN-SKU-1".to_string());
    hidden_input.metadata = json!({
        "channel_visibility": {
            "allowed_channel_slugs": ["mobile-app"]
        }
    });
    let hidden = catalog
        .create_product(tenant_id, actor_id, hidden_input)
        .await
        .expect("hidden product should be created");
    catalog
        .publish_product(tenant_id, actor_id, hidden.id)
        .await
        .expect("hidden product should be published");

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
                .uri("/store/products?locale=de")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("store products request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("store products body should read");
    let json: serde_json::Value =
        serde_json::from_slice(&body).expect("store products response should be JSON");
    let items = json["data"]
        .as_array()
        .expect("product list should be an array");
    assert_eq!(json["meta"]["total"], json!(1));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], json!("Sichtbares Produkt"));
}

#[tokio::test]
async fn storefront_line_item_resolution_uses_backend_variant_title_and_price() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let mut product_input = storefront_product_input();
    product_input.variants[0].inventory_quantity = 5;

    let created = service
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = service
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");
    let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
    let pricing_context = pricing_context("EUR", 2);

    let resolved = resolve_store_line_item_input(
        &db,
        tenant_id,
        StoreLineItemResolution {
            pricing_read_port: &pricing_service,
            pricing_context: &pricing_context,
            locale: "de",
            default_locale: "en",
            public_channel_slug: None,
            input: StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 2,
                metadata: json!({ "source": "store-line-item-test" }),
            },
        },
    )
    .await
    .expect("store line item should resolve from backend catalog");

    assert_eq!(resolved.add_line_item.product_id, Some(published.id));
    assert_eq!(resolved.add_line_item.variant_id, Some(variant.id));
    assert_eq!(
        resolved.add_line_item.sku.as_deref(),
        Some("STOREFRONT-SKU-1")
    );
    assert_eq!(resolved.add_line_item.title, "Storefront Produkt / Default");
    assert_eq!(
        resolved.add_line_item.unit_price,
        Decimal::from_str("19.99").expect("valid decimal")
    );
    assert_eq!(resolved.add_line_item.quantity, 2);
    assert_eq!(
        resolved.add_line_item.metadata,
        json!({
            "seller": { "id": null },
            "source": "store-line-item-test"
        })
    );
}

#[tokio::test]
async fn storefront_line_item_resolution_rejects_missing_price_for_cart_currency() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let created = service
        .create_product(tenant_id, actor_id, storefront_product_input())
        .await
        .expect("product should be created");
    let published = service
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");
    let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
    let pricing_context = pricing_context("USD", 1);

    let error = resolve_store_line_item_input(
        &db,
        tenant_id,
        StoreLineItemResolution {
            pricing_read_port: &pricing_service,
            pricing_context: &pricing_context,
            locale: "de",
            default_locale: "en",
            public_channel_slug: None,
            input: StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 1,
                metadata: json!({}),
            },
        },
    )
    .await
    .expect_err("store line item must reject missing price in cart currency");

    assert_eq!(error.status, StatusCode::NOT_FOUND);
    assert_eq!(error.code, "pricing.price_not_found");
    assert_eq!(
        error.message,
        format!("price for variant {} was not found", variant.id)
    );
}

#[tokio::test]
async fn storefront_line_item_resolution_falls_back_to_first_product_translation_when_locale_missing()
 {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let mut product_input = storefront_product_input();
    product_input.variants[0].inventory_quantity = 5;

    let created = service
        .create_product(tenant_id, actor_id, product_input)
        .await
        .expect("product should be created");
    let published = service
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");
    let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
    let pricing_context = pricing_context("EUR", 1);

    let resolved = resolve_store_line_item_input(
        &db,
        tenant_id,
        StoreLineItemResolution {
            pricing_read_port: &pricing_service,
            pricing_context: &pricing_context,
            locale: "fr",
            default_locale: "en",
            public_channel_slug: None,
            input: StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 1,
                metadata: json!({}),
            },
        },
    )
    .await
    .expect("store line item should fall back to an existing product translation");

    assert_eq!(resolved.add_line_item.product_id, Some(published.id));
    assert_eq!(resolved.add_line_item.variant_id, Some(variant.id));
    assert_eq!(
        resolved.add_line_item.sku.as_deref(),
        Some("STOREFRONT-SKU-1")
    );
    assert_eq!(resolved.add_line_item.title, "Storefront Product / Default");
    assert_eq!(
        resolved.add_line_item.unit_price,
        Decimal::from_str("19.99").expect("valid decimal")
    );
}

#[tokio::test]
async fn storefront_line_item_resolution_returns_not_found_for_unknown_variant() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
    let pricing_context = pricing_context("EUR", 1);

    let error = resolve_store_line_item_input(
        &db,
        tenant_id,
        StoreLineItemResolution {
            pricing_read_port: &pricing_service,
            pricing_context: &pricing_context,
            locale: "de",
            default_locale: "en",
            public_channel_slug: None,
            input: StoreAddCartLineItemInput {
                variant_id: Uuid::new_v4(),
                quantity: 1,
                metadata: json!({}),
            },
        },
    )
    .await
    .expect_err("unknown variant must not resolve");

    assert_eq!(error.status, StatusCode::NOT_FOUND);
    assert_eq!(error.code, "commerce_store_not_found");
    assert_eq!(error.message, "Commerce resource not found");
}

#[tokio::test]
async fn storefront_line_item_resolution_rejects_quantity_above_channel_visible_inventory() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = storefront_product_input();
    input.variants[0].inventory_quantity = 5;
    let created = service
        .create_product(tenant_id, actor_id, input)
        .await
        .expect("product should be created");
    let published = service
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    let variant = published
        .variants
        .first()
        .expect("published product must include variant");
    let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
    let pricing_context = pricing_context("EUR", 1);
    set_stock_location_channel_visibility(&db, tenant_id, &["mobile-app"]).await;

    let error = resolve_store_line_item_input(
        &db,
        tenant_id,
        StoreLineItemResolution {
            pricing_read_port: &pricing_service,
            pricing_context: &pricing_context,
            locale: "de",
            default_locale: "en",
            public_channel_slug: Some("web-store"),
            input: StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 1,
                metadata: json!({}),
            },
        },
    )
    .await
    .expect_err("hidden inventory should reject storefront line item resolution");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(error.code, "commerce_store_invalid");
    assert_eq!(
        error.message,
        format!(
            "Variant {} does not have enough available inventory for the current channel",
            variant.id
        )
    );
}

#[tokio::test]
async fn store_product_transport_uses_channel_visible_inventory() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
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
    let mut channel = sample_channel_context("web-store");
    channel.tenant_id = tenant_id;

    let mut input = storefront_product_input();
    input.variants[0].inventory_quantity = 7;
    let created = service
        .create_product(tenant_id, actor_id, input)
        .await
        .expect("product should be created");
    let published = service
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .expect("product should be published");
    set_stock_location_channel_visibility(&db, tenant_id, &["mobile-app"]).await;
    seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
    let request_context = RequestContext {
        tenant_id,
        user_id: None,
        channel_id: Some(channel.id),
        channel_slug: Some(channel.slug.clone()),
        channel_resolution_source: Some(ChannelResolutionSource::Host),
        locale: "de".to_string(),
    };

    let runtime = test_app_context(db);
    let product = super::super::products::show_product(
        State(runtime),
        tenant,
        request_context,
        Path(published.id),
    )
    .await
    .expect("store product handler should succeed")
    .0;

    assert_eq!(product.variants.len(), 1);
    assert_eq!(product.variants[0].inventory_quantity, 0);
    assert!(!product.variants[0].in_stock);
}
