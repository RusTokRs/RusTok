use super::*;

#[tokio::test]
async fn complete_checkout_rejects_shipping_option_hidden_for_cart_channel() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_channel_binding(&db, tenant_id, channel_id, "web-store").await;

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Hidden Shipping".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({
                    "channel_visibility": {
                        "allowed_channel_slugs": ["mobile-app"]
                    }
                }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-hidden-shipping" }),
            },
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                shipping_profile_slug: None,
                sku: Some("CHK-HIDDEN-1".to_string()),
                title: "Checkout Hidden Shipping Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let error = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-hidden-shipping" }),
            },
        )
        .await
        .expect_err("hidden shipping option must fail checkout");

    match error {
        CheckoutError::Validation(message) => {
            assert!(
                message.contains("not available for the cart channel"),
                "unexpected validation message: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn complete_checkout_rejects_line_item_hidden_for_cart_channel() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_channel_binding(&db, tenant_id, channel_id, "web-store").await;

    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let mut product_input = create_product_input();
    product_input.metadata = serde_json::json!({
        "channel_visibility": {
            "allowed_channel_slugs": ["mobile-app"]
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

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Visible Shipping".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-hidden-product" }),
            },
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();
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
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let error = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-hidden-product" }),
            },
        )
        .await
        .expect_err("channel-hidden product must fail checkout");

    match error {
        CheckoutError::Validation(message) => {
            assert!(
                message.contains("is not available for the cart channel"),
                "unexpected validation message: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn complete_checkout_rejects_line_item_without_channel_visible_inventory() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    seed_channel_binding(&db, tenant_id, channel_id, "web-store").await;

    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
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
    set_stock_location_channel_visibility(&db, tenant_id, &["mobile-app"]).await;

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Visible Shipping".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-hidden-inventory" }),
            },
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();
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
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let error = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-hidden-inventory" }),
            },
        )
        .await
        .expect_err("channel-hidden inventory must fail checkout");

    match error {
        CheckoutError::Validation(message) => {
            assert!(
                message.contains("does not have enough available inventory for the cart channel"),
                "unexpected validation message: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn complete_checkout_rejects_shipping_option_incompatible_with_cart_shipping_profiles() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let mut product_input = create_product_input();
    product_input.metadata = serde_json::json!({
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

    let incompatible_shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Default Only".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                metadata: serde_json::json!({
                    "shipping_profiles": {
                        "allowed_slugs": ["default"]
                    }
                }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(incompatible_shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-shipping-profile" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
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
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let error = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-shipping-profile" }),
            },
        )
        .await
        .expect_err("incompatible shipping profile must fail checkout");

    match error {
        CheckoutError::Validation(message) => {
            assert!(
                message.contains("not compatible with delivery group bulky"),
                "unexpected validation message: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn complete_checkout_rejects_channel_hidden_inventory_on_deny_policy() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let channel_id = Uuid::new_v4();
    let channel_slug = format!("web-{}", Uuid::new_v4());
    seed_channel_binding(&db, tenant_id, channel_id, &channel_slug).await;

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "US Channel Inventory Test".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::ZERO,
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "channel-inventory-deny-test" }),
            },
        )
        .await
        .unwrap();

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("5.00").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({
                    "source": "channel-inventory-deny-test",
                    "channel_visibility": { "allowed_channel_slugs": [channel_slug.as_str()] }
                }),
            },
        )
        .await
        .unwrap();

    // Create a published product with deny policy
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let mut product_input = create_product_input();
    product_input.publish = true;
    // Mark the product as visible for the cart channel
    product_input.metadata = serde_json::json!({
        "channel_visibility": { "allowed_channel_slugs": [channel_slug.as_str()] }
    });
    let product = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .unwrap();
    let variant = product.variants.first().expect("variant must exist").clone();

    // Give the variant enough inventory from the commerce InventoryService side
    let inventory = InventoryService::new(db.clone(), mock_transactional_event_bus());
    inventory
        .set_inventory(tenant_id, actor_id, variant.id, 10)
        .await
        .unwrap();

    // Now restrict the stock_location to a *different* channel slug so that the
    // cart's channel cannot see the inventory.
    set_stock_location_channel_visibility(&db, tenant_id, &["mobile-only"]).await;

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("deny@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "channel-inventory-deny-test" }),
            },
        )
        .await
        .unwrap();

    // Patch cart to carry the channel context
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE carts SET channel_id = ?, channel_slug = ? WHERE id = ?",
        vec![
            channel_id.into(),
            channel_slug.clone().into(),
            cart.id.into(),
        ],
    ))
    .await
    .unwrap();

    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(product.id),
                variant_id: Some(variant.id),
                shipping_profile_slug: variant.shipping_profile_slug.clone(),
                sku: variant.sku.clone(),
                title: "Channel Hidden Inventory Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let error = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "channel-inventory-deny-test" }),
            },
        )
        .await
        .expect_err("checkout must reject cart with channel-hidden deny-policy inventory");

    match error {
        CheckoutError::Validation(message) => {
            assert!(
                message.contains("does not have enough available inventory for the cart channel"),
                "unexpected validation message: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn complete_checkout_allows_backorder_variant_when_channel_inventory_hidden() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let channel_id = Uuid::new_v4();
    let channel_slug = format!("app-{}", Uuid::new_v4());
    seed_channel_binding(&db, tenant_id, channel_id, &channel_slug).await;

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "US Backorder Channel Test".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::ZERO,
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "channel-backorder-test" }),
            },
        )
        .await
        .unwrap();

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("5.00").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({
                    "source": "channel-backorder-test",
                    "channel_visibility": { "allowed_channel_slugs": [channel_slug.as_str()] }
                }),
            },
        )
        .await
        .unwrap();

    // Create product with `continue` (backorder) inventory policy
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let product_input = CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Backorder Channel Product".to_string(),
            description: None,
            handle: Some(format!("backorder-channel-{}", Uuid::new_v4())),
            meta_title: None,
            meta_description: None,
        }],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(format!("BKORD-CHAN-{}", Uuid::new_v4())),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: None,
                amount: Decimal::from_str("20.00").unwrap(),
                compare_at_amount: None,
            }],
            inventory_quantity: 0,
            inventory_policy: "continue".to_string(), // backorder policy
            weight: None,
            weight_unit: None,
        }],
        seller_id: None,
        vendor: None,
        product_type: None,
        shipping_profile_slug: None,
        tags: vec![],
        publish: true,
        metadata: serde_json::json!({
            "channel_visibility": { "allowed_channel_slugs": [channel_slug.as_str()] }
        }),
    };
    let product = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .unwrap();
    let variant = product.variants.first().expect("variant must exist").clone();

    // Restrict stock location to a different channel — backorder should still pass
    set_stock_location_channel_visibility(&db, tenant_id, &["other-channel-only"]).await;

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("backorder@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "channel-backorder-test" }),
            },
        )
        .await
        .unwrap();

    // Patch cart to carry the channel context
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE carts SET channel_id = ?, channel_slug = ? WHERE id = ?",
        vec![
            channel_id.into(),
            channel_slug.clone().into(),
            cart.id.into(),
        ],
    ))
    .await
    .unwrap();

    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(product.id),
                variant_id: Some(variant.id),
                shipping_profile_slug: variant.shipping_profile_slug.clone(),
                sku: variant.sku.clone(),
                title: "Backorder Channel Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("20.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    // Checkout must succeed: backorder policy bypasses inventory availability check
    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "channel-backorder-test" }),
            },
        )
        .await
        .expect("backorder variant checkout must succeed even with channel-hidden stock location");

    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");
    assert_eq!(completed.order.line_items.len(), 1);
}

#[tokio::test]
async fn complete_checkout_accepts_variant_when_stock_location_visible_for_cart_channel() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let channel_id = Uuid::new_v4();
    let channel_slug = format!("shop-{}", Uuid::new_v4());
    seed_channel_binding(&db, tenant_id, channel_id, &channel_slug).await;

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "US Visible Channel Inventory Test".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::ZERO,
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "channel-visible-inventory-test" }),
            },
        )
        .await
        .unwrap();

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("5.00").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({
                    "source": "channel-visible-inventory-test",
                    "channel_visibility": { "allowed_channel_slugs": [channel_slug.as_str()] }
                }),
            },
        )
        .await
        .unwrap();

    // Create a published product with deny policy
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let mut product_input = create_product_input();
    product_input.publish = true;
    product_input.metadata = serde_json::json!({
        "channel_visibility": { "allowed_channel_slugs": [channel_slug.as_str()] }
    });
    let product = catalog
        .create_product(tenant_id, actor_id, product_input)
        .await
        .unwrap();
    let variant = product.variants.first().expect("variant must exist").clone();

    // Give sufficient inventory
    let inventory = InventoryService::new(db.clone(), mock_transactional_event_bus());
    inventory
        .set_inventory(tenant_id, actor_id, variant.id, 20)
        .await
        .unwrap();

    // Make the stock_location visible for the cart's channel slug
    set_stock_location_channel_visibility(&db, tenant_id, &[channel_slug.as_str()]).await;

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("visible@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "channel-visible-inventory-test" }),
            },
        )
        .await
        .unwrap();

    // Patch cart to carry the channel context
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE carts SET channel_id = ?, channel_slug = ? WHERE id = ?",
        vec![
            channel_id.into(),
            channel_slug.clone().into(),
            cart.id.into(),
        ],
    ))
    .await
    .unwrap();

    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(product.id),
                variant_id: Some(variant.id),
                shipping_profile_slug: variant.shipping_profile_slug.clone(),
                sku: variant.sku.clone(),
                title: "Channel Visible Inventory Product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    // Checkout must succeed: stock location visible for cart channel, sufficient stock
    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "channel-visible-inventory-test" }),
            },
        )
        .await
        .expect("checkout must succeed when stock location is visible for the cart channel");

    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");
    assert_eq!(completed.order.line_items.len(), 1);
    assert_eq!(completed.order.line_items[0].quantity, 2);
    assert_eq!(
        completed.payment_collection.amount,
        // 2 * 25.00 + 5.00 shipping
        Decimal::from_str("55.00").unwrap()
    );
}
