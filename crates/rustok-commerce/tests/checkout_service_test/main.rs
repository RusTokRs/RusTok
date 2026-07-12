use rust_decimal::Decimal;
use rustok_cart::CartService;
use rustok_cart::dto::{
    AddCartLineItemInput, CartShippingSelectionInput, CreateCartInput, SetCartAdjustmentInput,
    UpdateCartContextInput,
};
use rustok_commerce::dto::CompleteCheckoutInput;
use rustok_commerce::services::{CheckoutError, CheckoutService};
use rustok_fulfillment::FulfillmentService;
use rustok_fulfillment::dto::{CreateShippingOptionInput, ShippingOptionTranslationInput};
use rustok_inventory::InventoryService;
use rustok_payment::PaymentService;
use rustok_product::CatalogService;
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_region::dto::{CreateRegionInput, RegionCountryTaxPolicyInput, RegionTranslationInput};
use rustok_region::services::RegionService;
use rustok_test_utils::{db::setup_test_db, mock_transactional_event_bus};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use std::str::FromStr;
use uuid::Uuid;

#[path = "../support.rs"]
mod support;

pub(crate) async fn setup() -> (
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
        CheckoutService::new(
            db.clone(),
            event_bus.clone(),
            std::sync::Arc::new(rustok_region::RegionService::new(db.clone())),
            std::sync::Arc::new(rustok_cart::CartService::new(db.clone())),
            std::sync::Arc::new(rustok_inventory::InventoryService::new(
                db.clone(),
                event_bus.clone(),
            )),
            std::sync::Arc::new(rustok_product::CatalogService::new(db.clone(), event_bus)),
        ),
        FulfillmentService::new(db),
    )
}

pub(crate) fn create_product_input() -> CreateProductInput {
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
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: None,
                amount: Decimal::from_str("25.00").expect("valid decimal"),
                compare_at_amount: None,
            }],
            inventory_quantity: 5,
            inventory_policy: "deny".to_string(),
            weight: None,
            weight_unit: None,
        }],
        seller_id: None,
        vendor: Some("Checkout Vendor".to_string()),
        product_type: Some("physical".to_string()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: vec![],
        publish: false,
        metadata: serde_json::json!({}),
    }
}

pub(crate) async fn seed_channel_binding(
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

pub(crate) async fn set_stock_location_channel_visibility(
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

pub(crate) async fn seed_tenant_context(db: &DatabaseConnection, tenant_id: Uuid) {
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

pub mod adjustments;
pub mod basic;
pub mod delivery;
pub mod recovery;
pub mod tax;
pub mod validation;
