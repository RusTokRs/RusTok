use rust_decimal::Decimal;
use rustok_commerce::dto::{
    AddCartLineItemInput, CompleteCheckoutInput, CreateCartInput, CreateProductInput,
    CreateShippingOptionInput, CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_commerce::services::{
    CartService, CatalogService, CheckoutError, CheckoutService, FulfillmentService, OrderService,
    PaymentService,
};
use rustok_region::dto::CreateRegionInput;
use rustok_region::services::RegionService;
use rustok_test_utils::{db::setup_test_db, mock_transactional_event_bus};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> (
    DatabaseConnection,
    CartService,
    CheckoutService,
    FulfillmentService,
) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let event_bus = mock_transactional_event_bus();
    (
        db.clone(),
        CartService::new(db.clone()),
        CheckoutService::new(db.clone(), event_bus),
        FulfillmentService::new(db),
    )
}

fn create_product_input() -> CreateProductInput {
    CreateProductInput {
        translations: vec![
            ProductTranslationInput {
                locale: "en".to_string(),
                title: "Checkout Inventory Product".to_string(),
                description: Some("English description".to_string()),
                handle: Some(format!("checkout-inventory-en-{}", Uuid::new_v4())),
                meta_title: None,
                meta_description: None,
            },
            ProductTranslationInput {
                locale: "de".to_string(),
                title: "Checkout Inventar Produkt".to_string(),
                description: Some("German description".to_string()),
                handle: Some(format!("checkout-inventory-de-{}", Uuid::new_v4())),
                meta_title: None,
                meta_description: None,
            },
        ],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some("CHK-INVENTORY-SKU-1".to_string()),
            barcode: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                amount: Decimal::from_str("25.00").expect("valid decimal"),
                compare_at_amount: None,
            }],
            inventory_quantity: 5,
            inventory_policy: "deny".to_string(),
            weight: None,
            weight_unit: None,
        }],
        vendor: Some("Checkout Vendor".to_string()),
        product_type: Some("physical".to_string()),
        shipping_profile_slug: None,
        tags: vec![],
        publish: false,
        metadata: serde_json::json!({}),
    }
}

async fn seed_channel_binding(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    channel_id: Uuid,
    channel_slug: &str,
) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO channels (id, tenant_id, slug, name, is_active, is_default, status, settings, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            channel_id.into(),
            tenant_id.into(),
            channel_slug.into(),
            format!("Channel {channel_slug}").into(),
            true.into(),
            false.into(),
            "active".into(),
            serde_json::json!({}).to_string().into(),
        ],
    ))
    .await
    .expect("channel should be inserted");

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO channel_module_bindings (id, channel_id, module_slug, is_enabled, settings, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            Uuid::new_v4().into(),
            channel_id.into(),
            "commerce".into(),
            true.into(),
            serde_json::json!({}).to_string().into(),
        ],
    ))
    .await
    .expect("channel binding should be inserted");
}

async fn set_stock_location_channel_visibility(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    allowed_channel_slugs: &[&str],
) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE stock_locations SET metadata = ? WHERE tenant_id = ?",
        vec![
            serde_json::json!({
                "channel_visibility": {
                    "allowed_channel_slugs": allowed_channel_slugs
                }
            })
            .to_string()
            .into(),
            tenant_id.into(),
        ],
    ))
    .await
    .expect("stock location visibility should be updated");
}

#[tokio::test]
async fn complete_checkout_builds_order_payment_and_fulfillment_flow() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "usd".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-test" }),
            },
        )
        .await
        .unwrap();

    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "checkout-test" }),
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
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                sku: Some("CHK-1".to_string()),
                title: "Checkout Product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");
    assert_eq!(completed.payment_collection.status, "captured");
    assert!(completed.fulfillment.is_some());
    assert_eq!(completed.context.locale, "de");
    assert_eq!(completed.context.currency_code.as_deref(), Some("USD"));
    assert_eq!(completed.cart.region_id, Some(region.id));
    assert_eq!(completed.cart.country_code.as_deref(), Some("DE"));
    assert_eq!(completed.cart.locale_code.as_deref(), Some("de"));
    assert_eq!(
        completed.cart.selected_shipping_option_id,
        Some(shipping_option.id)
    );
    assert_eq!(
        completed.context.region.as_ref().map(|region| region.id),
        Some(region.id)
    );
    assert_eq!(
        completed
            .fulfillment
            .as_ref()
            .and_then(|value| value.shipping_option_id),
        Some(shipping_option.id)
    );
}

#[tokio::test]
async fn complete_checkout_rejects_empty_cart() {
    let (db, cart_service, checkout, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("empty@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale_code: None,
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({}),
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
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: false,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap_err();

    match error {
        CheckoutError::EmptyCart(cart_id) => assert_eq!(cart_id, cart.id),
        other => panic!("expected empty cart error, got {other:?}"),
    }
}

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
                name: "Hidden Shipping".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
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
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
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
                name: "Visible Shipping".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
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
                name: "Visible Shipping".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
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
                name: "Default Only".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
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
                message.contains("not compatible with the cart shipping profiles"),
                "unexpected validation message: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn repeated_complete_checkout_recovers_existing_result() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "usd".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-retry-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "checkout-retry-test" }),
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
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-retry-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                sku: Some("CHK-RETRY-1".to_string()),
                title: "Checkout Retry Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let first = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-retry-test" }),
            },
        )
        .await
        .unwrap();

    let second = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-retry-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(first.cart.id, second.cart.id);
    assert_eq!(first.order.id, second.order.id);
    assert_eq!(first.payment_collection.id, second.payment_collection.id);
    assert_eq!(
        first.fulfillment.as_ref().map(|value| value.id),
        second.fulfillment.as_ref().map(|value| value.id)
    );
}

#[tokio::test]
async fn complete_checkout_reuses_existing_cart_payment_collection() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "eur".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-existing-collection-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "checkout-existing-collection-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "checkout-existing-collection-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                sku: Some("CHK-EXISTING-1".to_string()),
                title: "Checkout Product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();
    let existing_collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            rustok_commerce::dto::CreatePaymentCollectionInput {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: cart.customer_id,
                currency_code: cart.currency_code.clone(),
                amount: cart.total_amount,
                metadata: serde_json::json!({ "source": "checkout-existing-collection-test" }),
            },
        )
        .await
        .unwrap();

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: false,
                metadata: serde_json::json!({ "flow": "checkout-existing-collection-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.payment_collection.id, existing_collection.id);
    assert_eq!(
        completed.payment_collection.order_id,
        Some(completed.order.id)
    );
    assert_eq!(completed.payment_collection.status, "captured");
    assert_eq!(completed.order.status, "paid");
    assert_eq!(completed.cart.status, "completed");
}

#[tokio::test]
async fn complete_checkout_prefers_persisted_cart_context_over_conflicting_overrides() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region_de = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Germany".to_string(),
                currency_code: "usd".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-context-priority-test" }),
            },
        )
        .await
        .unwrap();
    let region_fr = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "France".to_string(),
                currency_code: "usd".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["fr".to_string()],
                metadata: serde_json::json!({ "source": "checkout-context-priority-test" }),
            },
        )
        .await
        .unwrap();

    let shipping_option_de = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "German Standard".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "checkout-context-priority-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option_fr = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "French Standard".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("12.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "checkout-context-priority-test" }),
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
                region_id: Some(region_de.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option_de.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-context-priority-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                sku: Some("CHK-CONTEXT-1".to_string()),
                title: "Checkout Context Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: Some(shipping_option_fr.id),
                region_id: Some(region_fr.id),
                country_code: Some("fr".to_string()),
                locale: Some("fr".to_string()),
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-context-priority-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.cart.region_id, Some(region_de.id));
    assert_eq!(completed.cart.country_code.as_deref(), Some("DE"));
    assert_eq!(completed.cart.locale_code.as_deref(), Some("de"));
    assert_eq!(
        completed.cart.selected_shipping_option_id,
        Some(shipping_option_de.id)
    );
    assert_eq!(
        completed.context.region.as_ref().map(|region| region.id),
        Some(region_de.id)
    );
    assert_eq!(completed.context.locale, "de");
    assert_eq!(
        completed
            .fulfillment
            .as_ref()
            .and_then(|value| value.shipping_option_id),
        Some(shipping_option_de.id)
    );
}

#[tokio::test]
async fn complete_checkout_recovers_stuck_checking_out_cart_when_paid_artifacts_exist() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "usd".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-recovery-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "checkout-recovery-test" }),
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
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-recovery-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                sku: Some("CHK-RECOVER-1".to_string()),
                title: "Checkout Recovery Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let first = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-recovery-test" }),
            },
        )
        .await
        .unwrap();

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE carts SET status = ?, completed_at = NULL WHERE id = ? AND tenant_id = ?",
        vec!["checking_out".into(), cart.id.into(), tenant_id.into()],
    ))
    .await
    .unwrap();

    let recovered = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-recovery-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(recovered.cart.status, "completed");
    assert!(recovered.cart.completed_at.is_some());
    assert_eq!(first.cart.id, recovered.cart.id);
    assert_eq!(first.order.id, recovered.order.id);
    assert_eq!(first.payment_collection.id, recovered.payment_collection.id);
    assert_eq!(
        first.fulfillment.as_ref().map(|value| value.id),
        recovered.fulfillment.as_ref().map(|value| value.id)
    );
}

#[tokio::test]
async fn checkout_failure_releases_cart_back_to_active() {
    let (db, cart_service, checkout, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "usd".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-lock-release-test" }),
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
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-lock-release-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                sku: Some("CHK-LOCK-1".to_string()),
                title: "Checkout Lock Product".to_string(),
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
                shipping_option_id: Some(Uuid::new_v4()),
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-lock-release-test" }),
            },
        )
        .await
        .expect_err("invalid shipping option must fail checkout");

    match error {
        CheckoutError::StageFailure { stage, .. } => {
            assert_eq!(stage, "create_fulfillment");
        }
        other => panic!("expected stage failure, got {other:?}"),
    }

    let cart_after = cart_service.get_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(cart_after.status, "active");
    assert!(cart_after.completed_at.is_none());
}

#[tokio::test]
async fn checkout_failure_cancels_payment_and_order_artifacts() {
    let (db, cart_service, checkout, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "usd".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-compensation-test" }),
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
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-compensation-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                sku: Some("CHK-COMP-1".to_string()),
                title: "Checkout Compensation Product".to_string(),
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
                shipping_option_id: Some(Uuid::new_v4()),
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-compensation-test" }),
            },
        )
        .await
        .expect_err("invalid shipping option must trigger compensation");

    match error {
        CheckoutError::StageFailure { stage, .. } => assert_eq!(stage, "create_fulfillment"),
        other => panic!("expected stage failure, got {other:?}"),
    }

    let payment_service = PaymentService::new(db.clone());
    let order_service = OrderService::new(db, mock_transactional_event_bus());

    let payment_collection = payment_service
        .find_latest_collection_by_cart(tenant_id, cart.id)
        .await
        .unwrap()
        .expect("failed checkout should leave a payment collection to compensate");
    assert_eq!(payment_collection.status, "cancelled");
    assert!(payment_collection.cancelled_at.is_some());
    assert_eq!(payment_collection.payments.len(), 1);
    assert_eq!(payment_collection.payments[0].status, "cancelled");
    assert!(payment_collection.payments[0].cancelled_at.is_some());

    let order = order_service
        .get_order(
            tenant_id,
            payment_collection
                .order_id
                .expect("payment collection must stay linked to created order"),
        )
        .await
        .unwrap();
    assert_eq!(order.status, "cancelled");
    assert!(order.cancelled_at.is_some());
}

#[tokio::test]
async fn retry_after_compensated_failure_creates_fresh_checkout_artifacts() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "usd".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-retry-after-failure-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "checkout-retry-after-failure-test" }),
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
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-retry-after-failure-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                sku: Some("CHK-RETRY-AFTER-FAIL-1".to_string()),
                title: "Checkout Retry After Failure Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let first_error = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: Some(Uuid::new_v4()),
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-retry-after-failure-test" }),
            },
        )
        .await
        .expect_err("first checkout must fail on invalid shipping option");

    match first_error {
        CheckoutError::StageFailure { stage, .. } => assert_eq!(stage, "create_fulfillment"),
        other => panic!("expected stage failure, got {other:?}"),
    }

    let payment_service = PaymentService::new(db.clone());
    let failed_collection = payment_service
        .find_latest_collection_by_cart(tenant_id, cart.id)
        .await
        .unwrap()
        .expect("failed checkout should create cancellable collection");
    let failed_order_id = failed_collection
        .order_id
        .expect("failed collection should keep order linkage");

    let retried = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: Some(shipping_option.id),
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-retry-after-failure-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(retried.cart.status, "completed");
    assert_eq!(retried.order.status, "paid");
    assert_eq!(retried.payment_collection.status, "captured");
    assert_ne!(retried.payment_collection.id, failed_collection.id);
    assert_ne!(retried.order.id, failed_order_id);
}

#[tokio::test]
async fn checkout_without_fulfillment_flag_skips_fulfillment_creation() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "usd".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-without-fulfillment-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "checkout-without-fulfillment-test" }),
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
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-without-fulfillment-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                sku: Some("CHK-NO-FULFILL-1".to_string()),
                title: "Checkout Without Fulfillment Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: false,
                metadata: serde_json::json!({ "flow": "checkout-without-fulfillment-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");
    assert_eq!(completed.payment_collection.status, "captured");
    assert!(completed.fulfillment.is_none());
}

async fn seed_tenant_context(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            "Checkout Tenant".into(),
            format!("checkout-tenant-{tenant_id}").into(),
            sea_orm::Value::String(None),
            serde_json::json!({}).to_string().into(),
            "en".into(),
            true.into(),
        ],
    ))
    .await
    .unwrap();
    for (locale, name, native_name, is_default) in [
        ("en", "English", "English", true),
        ("de", "German", "Deutsch", false),
    ] {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                locale.into(),
                name.into(),
                native_name.into(),
                is_default.into(),
                true.into(),
                sea_orm::Value::String(None),
            ],
        ))
        .await
        .unwrap();
    }
}
