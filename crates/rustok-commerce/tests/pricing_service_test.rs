// Comprehensive unit tests for PricingService
// These tests verify price management, currency support,
// discounts, and price validation logic.

use rust_decimal_macros::dec;
use rustok_commerce::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_commerce::entities;
use rustok_commerce::services::{CatalogService, PriceAdjustmentKind, PricingService};
use rustok_commerce::CommerceError;
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

mod support;

async fn setup() -> (DatabaseConnection, PricingService, CatalogService) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let event_bus = mock_transactional_event_bus();
    let pricing_service = PricingService::new(db.clone(), event_bus.clone());
    let catalog_service = CatalogService::new(db.clone(), event_bus);
    (db, pricing_service, catalog_service)
}

async fn create_test_product(catalog: &CatalogService, tenant_id: Uuid) -> (Uuid, Uuid) {
    let input = CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Test Product".to_string(),
            description: Some("A test product".to_string()),
            handle: Some(unique_slug("test-product")),
            meta_title: None,
            meta_description: None,
        }],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(format!(
                "SKU-{}",
                Uuid::new_v4().to_string().split('-').next().unwrap()
            )),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: None,
                amount: dec!(99.99),
                compare_at_amount: None,
            }],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: Some(dec!(1.5)),
            weight_unit: Some("kg".to_string()),
        }],
        seller_id: None,
        vendor: Some("Test Vendor".to_string()),
        product_type: Some("Physical".to_string()),
        shipping_profile_slug: None,
        tags: vec![],
        publish: false,
        metadata: serde_json::json!({}),
    };

    let product = catalog
        .create_product(tenant_id, Uuid::new_v4(), input)
        .await
        .unwrap();
    let variant_id = product.variants[0].id;
    (product.id, variant_id)
}

async fn create_test_product_with_seller(
    catalog: &CatalogService,
    tenant_id: Uuid,
    seller_id: &str,
) -> Uuid {
    let input = CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Seller Product".to_string(),
            description: Some("Seller scoped product".to_string()),
            handle: Some(unique_slug("seller-product")),
            meta_title: None,
            meta_description: None,
        }],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(format!(
                "SELLER-{}",
                Uuid::new_v4().to_string().split('-').next().unwrap()
            )),
            barcode: None,
            shipping_profile_slug: Some("default".to_string()),
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: None,
                amount: dec!(55.00),
                compare_at_amount: None,
            }],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: None,
            weight_unit: None,
        }],
        seller_id: Some(seller_id.to_string()),
        vendor: Some("Seller Display".to_string()),
        product_type: Some("Physical".to_string()),
        shipping_profile_slug: Some("default".to_string()),
        tags: vec!["featured".to_string()],
        publish: false,
        metadata: serde_json::json!({}),
    };

    catalog
        .create_product(tenant_id, Uuid::new_v4(), input)
        .await
        .unwrap()
        .id
}

async fn create_price_list(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    status: &str,
    starts_at: Option<chrono::DateTime<chrono::Utc>>,
    ends_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Uuid {
    create_price_list_with_channel(db, tenant_id, status, starts_at, ends_at, None, None).await
}

async fn create_price_list_with_channel(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    status: &str,
    starts_at: Option<chrono::DateTime<chrono::Utc>>,
    ends_at: Option<chrono::DateTime<chrono::Utc>>,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
) -> Uuid {
    let price_list_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    entities::price_list::ActiveModel {
        id: Set(price_list_id),
        tenant_id: Set(tenant_id),
        name: Set(format!("List-{price_list_id}")),
        description: Set(Some("Test list".to_string())),
        r#type: Set("sale".to_string()),
        status: Set(status.to_string()),
        channel_id: Set(channel_id),
        channel_slug: Set(channel_slug.map(|value| value.to_ascii_lowercase())),
        rule_kind: Set(None),
        adjustment_percent: Set(None),
        starts_at: Set(starts_at.map(Into::into)),
        ends_at: Set(ends_at.map(Into::into)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .unwrap();

    price_list_id
}

// =============================================================================
// Set Price Tests
// =============================================================================

#[tokio::test]
async fn test_set_price_success() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_set_price_with_compare_at() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(79.99),
            Some(dec!(99.99)),
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_set_price_tier_persists_quantity_window_and_resolves() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    service
        .set_price_tier(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(85.00),
            Some(dec!(100.00)),
            Some(10),
            None,
        )
        .await
        .unwrap();

    let prices = service.get_variant_prices(variant_id).await.unwrap();
    let tier = prices
        .into_iter()
        .find(|price| price.min_quantity == Some(10) && price.max_quantity.is_none())
        .expect("tier row should exist");
    assert_eq!(tier.amount, dec!(85.00));
    assert_eq!(tier.compare_at_amount, Some(dec!(100.00)));

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(12),
            },
        )
        .await
        .unwrap()
        .expect("tiered price should resolve");

    assert_eq!(resolved.amount, dec!(85.00));
    assert_eq!(resolved.min_quantity, Some(10));
    assert_eq!(resolved.max_quantity, None);
}

#[tokio::test]
async fn test_set_price_tier_rejects_invalid_quantity_window() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price_tier(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(85.00),
            None,
            Some(10),
            Some(5),
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::InvalidPrice(message) => {
            assert!(message.contains("Maximum quantity"));
        }
        other => panic!("Expected InvalidPrice error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_set_price_list_tier_resolves_active_price_list_override() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    service
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(70.00),
            Some(dec!(100.00)),
            Some(5),
            None,
        )
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(6),
            },
        )
        .await
        .unwrap()
        .expect("price list tier should resolve");

    assert_eq!(resolved.amount, dec!(70.00));
    assert_eq!(resolved.price_list_id, Some(price_list_id));
    assert_eq!(resolved.min_quantity, Some(5));
}

#[tokio::test]
async fn test_set_price_list_tier_rejects_inactive_price_list() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "draft", None, None).await;

    let result = service
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(70.00),
            None,
            None,
            None,
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("active price list"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_set_price_multiple_currencies() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();
    service
        .set_price(tenant_id, actor_id, variant_id, "EUR", dec!(89.99), None)
        .await
        .unwrap();
    service
        .set_price(tenant_id, actor_id, variant_id, "GBP", dec!(79.99), None)
        .await
        .unwrap();

    let usd_price = service.get_price(variant_id, "USD").await.unwrap();
    let eur_price = service.get_price(variant_id, "EUR").await.unwrap();
    let gbp_price = service.get_price(variant_id, "GBP").await.unwrap();

    assert_eq!(usd_price, Some(dec!(99.99)));
    assert_eq!(eur_price, Some(dec!(89.99)));
    assert_eq!(gbp_price, Some(dec!(79.99)));
}

#[tokio::test]
async fn test_set_price_negative_amount() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(-10.00), None)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::InvalidPrice(msg) => {
            assert!(msg.contains("negative"));
        }
        _ => panic!("Expected InvalidPrice error"),
    }
}

#[tokio::test]
async fn test_set_price_invalid_compare_at() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(99.99),
            Some(dec!(79.99)),
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::InvalidPrice(msg) => {
            assert!(msg.contains("greater"));
        }
        _ => panic!("Expected InvalidPrice error"),
    }
}

#[tokio::test]
async fn test_set_price_zero_amount() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(0.00), None)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_set_price_update_existing() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(79.99), None)
        .await
        .unwrap();

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(79.99)));
}

#[tokio::test]
async fn test_set_price_nonexistent_variant() {
    let (_db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let fake_variant_id = Uuid::new_v4();

    let result = service
        .set_price(
            tenant_id,
            actor_id,
            fake_variant_id,
            "USD",
            dec!(99.99),
            None,
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::VariantNotFound(_) => {}
        _ => panic!("Expected VariantNotFound error"),
    }
}

// =============================================================================
// Set Prices (Bulk) Tests
// =============================================================================

#[tokio::test]
async fn test_set_prices_bulk() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let prices = vec![
        PriceInput {
            currency_code: "USD".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: dec!(99.99),
            compare_at_amount: None,
        },
        PriceInput {
            currency_code: "EUR".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: dec!(89.99),
            compare_at_amount: None,
        },
        PriceInput {
            currency_code: "GBP".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: dec!(79.99),
            compare_at_amount: None,
        },
    ];

    let result = service
        .set_prices(tenant_id, actor_id, variant_id, prices)
        .await;

    assert!(result.is_ok());

    let usd_price = service.get_price(variant_id, "USD").await.unwrap();
    let eur_price = service.get_price(variant_id, "EUR").await.unwrap();
    let gbp_price = service.get_price(variant_id, "GBP").await.unwrap();

    assert_eq!(usd_price, Some(dec!(99.99)));
    assert_eq!(eur_price, Some(dec!(89.99)));
    assert_eq!(gbp_price, Some(dec!(79.99)));
}

#[tokio::test]
async fn test_set_prices_empty_list() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_prices(tenant_id, actor_id, variant_id, vec![])
        .await;

    assert!(result.is_ok());
}

// =============================================================================
// Get Price Tests
// =============================================================================

#[tokio::test]
async fn test_get_price_existing() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    let result = service.get_price(variant_id, "USD").await;

    assert!(result.is_ok());
    let price = result.unwrap();
    assert_eq!(price, Some(dec!(99.99)));
}

#[tokio::test]
async fn test_get_price_nonexistent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service.get_price(variant_id, "EUR").await;

    assert!(result.is_ok());
    let price = result.unwrap();
    assert_eq!(price, None);
}

#[tokio::test]
async fn test_get_price_after_update() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(79.99), None)
        .await
        .unwrap();

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(79.99)));
}

// =============================================================================
// Get Variant Prices Tests
// =============================================================================

#[tokio::test]
async fn test_get_variant_prices_multiple() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();
    service
        .set_price(tenant_id, actor_id, variant_id, "EUR", dec!(89.99), None)
        .await
        .unwrap();
    service
        .set_price(tenant_id, actor_id, variant_id, "GBP", dec!(79.99), None)
        .await
        .unwrap();

    let result = service.get_variant_prices(variant_id).await;

    assert!(result.is_ok());
    let prices = result.unwrap();
    assert_eq!(prices.len(), 3);

    let currency_codes: Vec<String> = prices.iter().map(|p| p.currency_code.clone()).collect();
    assert!(currency_codes.contains(&"USD".to_string()));
    assert!(currency_codes.contains(&"EUR".to_string()));
    assert!(currency_codes.contains(&"GBP".to_string()));
}

#[tokio::test]
async fn test_get_variant_prices_empty() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let input = CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "No Price Product".to_string(),
            description: Some("Variant without prices".to_string()),
            handle: Some(unique_slug("no-price-product")),
            meta_title: None,
            meta_description: None,
        }],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(format!(
                "SKU-{}",
                Uuid::new_v4().to_string().split('-').next().unwrap()
            )),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: Some(dec!(1.5)),
            weight_unit: Some("kg".to_string()),
        }],
        seller_id: None,
        vendor: Some("Test Vendor".to_string()),
        product_type: Some("Physical".to_string()),
        shipping_profile_slug: None,
        tags: vec![],
        publish: false,
        metadata: serde_json::json!({}),
    };

    let product = catalog
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();
    let variant_id = product.variants[0].id;

    let result = service.get_variant_prices(variant_id).await;

    assert!(result.is_ok());
    let prices = result.unwrap();
    assert_eq!(prices.len(), 0);
}

// =============================================================================
// Apply Discount Tests
// =============================================================================

#[tokio::test]
async fn test_apply_discount_10_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(10))
        .await;

    assert!(result.is_ok());
    let new_amount = result.unwrap();
    assert_eq!(new_amount, dec!(90.00));

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(90.00)));
}

#[tokio::test]
async fn test_apply_discount_25_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(80.00), None)
        .await
        .unwrap();

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(25))
        .await;

    assert!(result.is_ok());
    let new_amount = result.unwrap();
    assert_eq!(new_amount, dec!(60.00));
}

#[tokio::test]
async fn test_apply_discount_50_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(50))
        .await;

    assert!(result.is_ok());
    let new_amount = result.unwrap();
    assert_eq!(new_amount, dec!(50.00));
}

#[tokio::test]
async fn test_apply_discount_with_compare_at() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
        )
        .await
        .unwrap();

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(20))
        .await;

    assert!(result.is_ok());
    let new_amount = result.unwrap();
    assert_eq!(new_amount, dec!(80.00));
}

#[tokio::test]
async fn test_apply_discount_rounding() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(15))
        .await;

    assert!(result.is_ok());
    let new_amount = result.unwrap();
    assert_eq!(new_amount, dec!(84.99));
}

#[tokio::test]
async fn test_apply_discount_no_existing_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "EUR", dec!(10))
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::InvalidPrice(msg) => {
            assert!(msg.contains("No canonical price found"));
        }
        _ => panic!("Expected InvalidPrice error"),
    }
}

// =============================================================================
// Precision & Rounding Tests
// =============================================================================

#[tokio::test]
async fn test_price_precision() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(19.99), None)
        .await
        .unwrap();

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(19.99)));
}

#[tokio::test]
async fn test_price_with_many_decimal_places() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(19.999999),
            None,
        )
        .await
        .unwrap();

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert!(price.is_some());
}

// =============================================================================
// Currency Tests
// =============================================================================

#[tokio::test]
async fn test_multiple_currencies_independence() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price(tenant_id, actor_id, variant_id, "EUR", dec!(90.00), None)
        .await
        .unwrap();

    service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(10))
        .await
        .unwrap();

    let usd_price = service.get_price(variant_id, "USD").await.unwrap();
    let eur_price = service.get_price(variant_id, "EUR").await.unwrap();

    assert_eq!(usd_price, Some(dec!(90.00)));
    assert_eq!(eur_price, Some(dec!(90.00)));
}

#[tokio::test]
async fn test_currency_code_case_sensitive() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let usd_upper = service.get_price(variant_id, "USD").await.unwrap();
    let usd_lower = service.get_price(variant_id, "usd").await.unwrap();

    assert_eq!(usd_upper, Some(dec!(100.00)));
    assert_eq!(usd_lower, None);
}

// =============================================================================
// Integration & Edge Case Tests
// =============================================================================

#[tokio::test]
async fn test_price_workflow() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let prices = service.get_variant_prices(variant_id).await.unwrap();
    assert_eq!(prices.len(), 1);

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
        )
        .await
        .unwrap();

    service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(25))
        .await
        .unwrap();

    let final_price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(final_price, Some(dec!(75.00)));
}

#[tokio::test]
async fn test_very_large_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(999999999.99),
            None,
        )
        .await;

    assert!(result.is_ok());

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(999999999.99)));
}

#[tokio::test]
async fn test_very_small_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(0.01), None)
        .await;

    assert!(result.is_ok());

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(0.01)));
}

#[tokio::test]
async fn test_discount_chain() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(10))
        .await
        .unwrap();

    service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(10))
        .await
        .unwrap();

    let final_price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(final_price, Some(dec!(90.00)));
}

#[tokio::test]
async fn test_preview_percentage_discount_returns_typed_adjustment() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
        )
        .await
        .unwrap();

    let preview = service
        .preview_percentage_discount(variant_id, "USD", dec!(25))
        .await
        .unwrap();

    assert_eq!(preview.kind, PriceAdjustmentKind::PercentageDiscount);
    assert_eq!(preview.currency_code, "USD");
    assert_eq!(preview.current_amount, dec!(80.00));
    assert_eq!(preview.base_amount, dec!(100.00));
    assert_eq!(preview.adjustment_percent, dec!(25));
    assert_eq!(preview.adjusted_amount, dec!(75.00));
    assert_eq!(preview.compare_at_amount, Some(dec!(100.00)));
    assert_eq!(preview.price_list_id, None);
}

#[tokio::test]
async fn test_apply_percentage_discount_targets_base_row_only() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_tier(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(85.00),
            Some(dec!(100.00)),
            Some(10),
            None,
        )
        .await
        .unwrap();

    let preview = service
        .apply_percentage_discount(tenant_id, actor_id, variant_id, "USD", dec!(10))
        .await
        .unwrap();

    assert_eq!(preview.adjusted_amount, dec!(90.00));

    let resolved_base = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();
    let resolved_tier = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(10),
            },
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(resolved_base.amount, dec!(90.00));
    assert_eq!(resolved_tier.amount, dec!(85.00));
}

#[tokio::test]
async fn test_preview_percentage_discount_supports_channel_scoped_base_row() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let channel_id = Uuid::new_v4();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            Some(channel_id),
            Some("web-store".to_string()),
            None,
            None,
        )
        .await
        .unwrap();

    let preview = service
        .preview_percentage_discount_with_channel(
            variant_id,
            "USD",
            dec!(10),
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(preview.current_amount, dec!(80.00));
    assert_eq!(preview.base_amount, dec!(100.00));
    assert_eq!(preview.adjusted_amount, dec!(90.00));
    assert_eq!(preview.price_list_id, None);
    assert_eq!(preview.channel_id, Some(channel_id));
    assert_eq!(preview.channel_slug.as_deref(), Some("web-store"));
}

#[tokio::test]
async fn test_apply_percentage_discount_targets_channel_scoped_base_row_only() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let channel_id = Uuid::new_v4();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            Some(channel_id),
            Some("web-store".to_string()),
            None,
            None,
        )
        .await
        .unwrap();

    let preview = service
        .apply_percentage_discount_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(10),
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(preview.adjusted_amount, dec!(90.00));
    assert_eq!(preview.channel_id, Some(channel_id));
    assert_eq!(preview.channel_slug.as_deref(), Some("web-store"));

    let resolved_global = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();
    let resolved_channel = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(resolved_global.amount, dec!(100.00));
    assert_eq!(resolved_channel.amount, dec!(90.00));
    assert_eq!(resolved_channel.channel_id, Some(channel_id));
    assert_eq!(resolved_channel.channel_slug.as_deref(), Some("web-store"));
}

#[tokio::test]
async fn test_preview_price_list_percentage_discount_returns_typed_adjustment() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            None,
            None,
        )
        .await
        .unwrap();

    let preview = service
        .preview_price_list_percentage_discount(
            tenant_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
        )
        .await
        .unwrap();

    assert_eq!(preview.kind, PriceAdjustmentKind::PercentageDiscount);
    assert_eq!(preview.current_amount, dec!(80.00));
    assert_eq!(preview.base_amount, dec!(100.00));
    assert_eq!(preview.adjusted_amount, dec!(90.00));
    assert_eq!(preview.price_list_id, Some(price_list_id));
}

#[tokio::test]
async fn test_apply_price_list_percentage_discount_targets_override_only() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            None,
            None,
        )
        .await
        .unwrap();

    let preview = service
        .apply_price_list_percentage_discount(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
        )
        .await
        .unwrap();

    assert_eq!(preview.adjusted_amount, dec!(90.00));
    assert_eq!(preview.price_list_id, Some(price_list_id));

    let resolved_base = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();
    let resolved_price_list = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(resolved_base.amount, dec!(100.00));
    assert_eq!(resolved_price_list.amount, dec!(90.00));
    assert_eq!(resolved_price_list.price_list_id, Some(price_list_id));
}

#[tokio::test]
async fn test_apply_price_list_percentage_discount_targets_channel_scoped_override_only() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let channel_id = Uuid::new_v4();
    let price_list_id = create_price_list_with_channel(
        &db,
        tenant_id,
        "active",
        None,
        None,
        Some(channel_id),
        Some("web-store"),
    )
    .await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_list_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            Some(channel_id),
            Some("web-store".to_string()),
            None,
            None,
        )
        .await
        .unwrap();

    let preview = service
        .apply_price_list_percentage_discount_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(preview.adjusted_amount, dec!(90.00));
    assert_eq!(preview.price_list_id, Some(price_list_id));
    assert_eq!(preview.channel_id, Some(channel_id));
    assert_eq!(preview.channel_slug.as_deref(), Some("web-store"));

    let resolved_base = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();
    let resolved_channel_override = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(resolved_base.amount, dec!(100.00));
    assert_eq!(resolved_channel_override.amount, dec!(90.00));
    assert_eq!(resolved_channel_override.channel_id, Some(channel_id));
    assert_eq!(
        resolved_channel_override.channel_slug.as_deref(),
        Some("web-store")
    );
}

#[tokio::test]
async fn test_preview_price_list_percentage_discount_rejects_inactive_list() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "draft", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let error = service
        .preview_price_list_percentage_discount(
            tenant_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
        )
        .await
        .unwrap_err();

    match error {
        CommerceError::Validation(message) => {
            assert!(message.contains("active price list"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_preview_percentage_discount_rejects_invalid_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let error = service
        .preview_percentage_discount(variant_id, "USD", dec!(125))
        .await
        .unwrap_err();

    match error {
        CommerceError::InvalidPrice(message) => {
            assert!(message.contains("discount_percent"));
        }
        other => panic!("Expected InvalidPrice error, got {other:?}"),
    }
}

// =============================================================================
// Price Resolution Context Tests
// =============================================================================

#[tokio::test]
async fn test_resolve_variant_price_prefers_exact_region_over_global() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let region_id = Uuid::new_v4();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(Some(region_id)),
        amount: Set(dec!(79.99)),
        compare_at_amount: Set(Some(dec!(99.99))),
        legacy_amount: Set(Some(7999)),
        legacy_compare_at_amount: Set(Some(9999)),
        min_quantity: Set(None),
        max_quantity: Set(None),
    }
    .insert(&_db)
    .await
    .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "usd".to_string(),
                region_id: Some(region_id),
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("region price should resolve");

    assert_eq!(resolved.amount, dec!(79.99));
    assert_eq!(resolved.region_id, Some(region_id));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_prefers_more_specific_quantity_tier() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(None),
        amount: Set(dec!(90.00)),
        compare_at_amount: Set(None),
        legacy_amount: Set(Some(9000)),
        legacy_compare_at_amount: Set(None),
        min_quantity: Set(Some(5)),
        max_quantity: Set(Some(9)),
    }
    .insert(&_db)
    .await
    .unwrap();

    entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(None),
        amount: Set(dec!(85.00)),
        compare_at_amount: Set(None),
        legacy_amount: Set(Some(8500)),
        legacy_compare_at_amount: Set(None),
        min_quantity: Set(Some(10)),
        max_quantity: Set(None),
    }
    .insert(&_db)
    .await
    .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(12),
            },
        )
        .await
        .unwrap()
        .expect("tiered price should resolve");

    assert_eq!(resolved.amount, dec!(85.00));
    assert_eq!(resolved.min_quantity, Some(10));
    assert_eq!(resolved.max_quantity, None);
}

#[tokio::test]
async fn test_resolve_variant_price_falls_back_to_global_price_without_region_specific_match() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: Some(Uuid::new_v4()),
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: None,
            },
        )
        .await
        .unwrap()
        .expect("global price should resolve");

    assert_eq!(resolved.amount, dec!(100.00));
    assert_eq!(resolved.region_id, None);
}

#[tokio::test]
async fn test_resolve_variant_price_prefers_channel_scoped_base_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let channel_id = Uuid::new_v4();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_prices(
            tenant_id,
            actor_id,
            variant_id,
            vec![PriceInput {
                currency_code: "USD".to_string(),
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                amount: dec!(79.00),
                compare_at_amount: Some(dec!(100.00)),
            }],
        )
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("channel scoped price should resolve");

    assert_eq!(resolved.amount, dec!(79.00));
    assert_eq!(resolved.channel_id, Some(channel_id));
    assert_eq!(resolved.channel_slug.as_deref(), Some("web-store"));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_does_not_leak_channel_scoped_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let channel_id = Uuid::new_v4();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_prices(
            tenant_id,
            actor_id,
            variant_id,
            vec![PriceInput {
                currency_code: "USD".to_string(),
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                amount: dec!(79.00),
                compare_at_amount: Some(dec!(100.00)),
            }],
        )
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: Some(Uuid::new_v4()),
                channel_slug: Some("mobile-app".to_string()),
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("global price should still resolve");

    assert_eq!(resolved.amount, dec!(100.00));
    assert_eq!(resolved.channel_id, None);
    assert_eq!(resolved.channel_slug, None);
}

#[tokio::test]
async fn test_list_admin_product_pricing_uses_locale_fallback_and_preserves_seller_id() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let product_id = create_test_product_with_seller(&catalog, tenant_id, "seller-alpha").await;

    let list = service
        .list_admin_product_pricing_with_locale_fallback(
            tenant_id,
            "ru",
            Some("en"),
            Some("Seller Product"),
            None,
            1,
            24,
        )
        .await
        .unwrap();

    let item = list
        .items
        .into_iter()
        .find(|item| item.id == product_id)
        .expect("admin pricing item should be present");

    assert_eq!(item.seller_id.as_deref(), Some("seller-alpha"));
    assert_eq!(item.title, "Seller Product");
    assert_eq!(item.handle, item.handle.to_lowercase());
    assert_eq!(item.shipping_profile_slug.as_deref(), Some("default"));
}

#[tokio::test]
async fn test_resolve_variant_price_prefers_active_price_list_over_base_price() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(Some(price_list_id)),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(None),
        amount: Set(dec!(80.00)),
        compare_at_amount: Set(Some(dec!(100.00))),
        legacy_amount: Set(Some(8000)),
        legacy_compare_at_amount: Set(Some(10000)),
        min_quantity: Set(None),
        max_quantity: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("price list price should resolve");

    assert_eq!(resolved.amount, dec!(80.00));
    assert_eq!(resolved.price_list_id, Some(price_list_id));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_applies_active_price_list_rule_without_override() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, Some(dec!(15)))
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("price list rule should resolve from base row");

    assert_eq!(resolved.amount, dec!(85.00));
    assert_eq!(resolved.compare_at_amount, Some(dec!(100.00)));
    assert_eq!(resolved.discount_percent, Some(dec!(15)));
    assert_eq!(resolved.price_list_id, Some(price_list_id));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_rule_uses_channel_quantity_tier_and_rounds() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list_with_channel(
        &db,
        tenant_id,
        "active",
        None,
        None,
        Some(channel_id),
        Some("web-store"),
    )
    .await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(19.99),
            None,
            Some(channel_id),
            Some("web-store".to_string()),
            Some(10),
            None,
        )
        .await
        .unwrap();
    service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, Some(dec!(12.5)))
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                quantity: Some(12),
            },
        )
        .await
        .unwrap()
        .expect("price list rule should resolve from the channel quantity tier");

    assert_eq!(resolved.amount, dec!(17.49));
    assert_eq!(resolved.compare_at_amount, Some(dec!(19.99)));
    assert_eq!(resolved.discount_percent, Some(dec!(12.5)));
    assert_eq!(resolved.price_list_id, Some(price_list_id));
    assert_eq!(resolved.channel_id, Some(channel_id));
    assert_eq!(resolved.channel_slug.as_deref(), Some("web-store"));
    assert_eq!(resolved.min_quantity, Some(10));
    assert_eq!(resolved.max_quantity, None);
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_prefers_explicit_override_over_price_list_rule() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, Some(dec!(15)))
        .await
        .unwrap();
    service
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(70.00),
            Some(dec!(100.00)),
            None,
            None,
        )
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("explicit override should win");

    assert_eq!(resolved.amount, dec!(70.00));
    assert_eq!(resolved.compare_at_amount, Some(dec!(100.00)));
    assert_eq!(resolved.price_list_id, Some(price_list_id));
}

#[tokio::test]
async fn test_resolve_variant_price_rejects_inactive_price_list_context() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "draft", None, None).await;

    let result = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("active price list"));
        }
        other => panic!("Expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_resolve_variant_price_rejects_price_list_outside_requested_channel() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let web_channel_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list_with_channel(
        &db,
        tenant_id,
        "active",
        None,
        None,
        Some(web_channel_id),
        Some("web-store"),
    )
    .await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let result = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: Some(Uuid::new_v4()),
                channel_slug: Some("mobile-app".to_string()),
                quantity: Some(1),
            },
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("price_list_id is not available for the requested channel"));
        }
        other => panic!("Expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_resolve_variant_price_reports_discount_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(84.99),
            Some(dec!(99.99)),
        )
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .expect("price resolution should succeed")
        .expect("sale price should resolve");

    assert_eq!(resolved.discount_percent, Some(dec!(15.00)));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_omits_discount_percent_for_non_sale_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_commerce::services::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .expect("price resolution should succeed")
        .expect("base price should resolve");

    assert_eq!(resolved.discount_percent, None);
    assert!(!resolved.on_sale);
}

#[tokio::test]
async fn test_list_active_price_lists_only_returns_currently_active_lists() {
    let (db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let active_id = create_price_list(&db, tenant_id, "active", None, None).await;
    let future_id = create_price_list(
        &db,
        tenant_id,
        "active",
        Some(now + chrono::Duration::days(1)),
        None,
    )
    .await;
    let expired_id = create_price_list(
        &db,
        tenant_id,
        "active",
        None,
        Some(now - chrono::Duration::days(1)),
    )
    .await;
    let draft_id = create_price_list(&db, tenant_id, "draft", None, None).await;

    let lists = service.list_active_price_lists(tenant_id).await.unwrap();

    assert!(lists.iter().any(|list| list.id == active_id));
    assert!(!lists.iter().any(|list| list.id == future_id));
    assert!(!lists.iter().any(|list| list.id == expired_id));
    assert!(!lists.iter().any(|list| list.id == draft_id));
}

#[tokio::test]
async fn test_list_active_price_lists_exposes_rule_metadata() {
    let (db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, Some(dec!(12.5)))
        .await
        .unwrap();

    let lists = service.list_active_price_lists(tenant_id).await.unwrap();
    let option = lists
        .into_iter()
        .find(|list| list.id == price_list_id)
        .expect("active price list should be present");

    assert_eq!(option.rule_kind.as_deref(), Some("percentage_discount"));
    assert_eq!(option.adjustment_percent, Some(dec!(12.5)));
}

#[tokio::test]
async fn test_list_active_price_lists_filters_by_channel_scope() {
    let (db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let global_id = create_price_list(&db, tenant_id, "active", None, None).await;
    let scoped_id = create_price_list_with_channel(
        &db,
        tenant_id,
        "active",
        None,
        None,
        Some(channel_id),
        Some("web-store"),
    )
    .await;

    let web_lists = service
        .list_active_price_lists_for_channel(tenant_id, Some(channel_id), Some("web-store"))
        .await
        .unwrap();
    let mobile_lists = service
        .list_active_price_lists_for_channel(tenant_id, Some(Uuid::new_v4()), Some("mobile-app"))
        .await
        .unwrap();

    assert!(web_lists.iter().any(|list| list.id == global_id));
    assert!(web_lists.iter().any(|list| list.id == scoped_id));
    assert!(mobile_lists.iter().any(|list| list.id == global_id));
    assert!(!mobile_lists.iter().any(|list| list.id == scoped_id));
}
