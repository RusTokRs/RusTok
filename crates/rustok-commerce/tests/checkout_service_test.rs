use rust_decimal::Decimal;
use rustok_commerce::dto::{
    AddCartLineItemInput, CompleteCheckoutInput, CreateCartInput, CreateShippingOptionInput,
};
use rustok_commerce::services::{CartService, CheckoutError, CheckoutService, FulfillmentService};
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
