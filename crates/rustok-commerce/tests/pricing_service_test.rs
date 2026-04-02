// Comprehensive unit tests for PricingService
// These tests verify price management, currency support,
// discounts, and price validation logic.

use rust_decimal_macros::dec;
use rustok_commerce::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_commerce::services::{CatalogService, PricingService};
use rustok_commerce::CommerceError;
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::DatabaseConnection;
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
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                amount: dec!(99.99),
                compare_at_amount: None,
            }],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: Some(dec!(1.5)),
            weight_unit: Some("kg".to_string()),
        }],
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
            amount: dec!(99.99),
            compare_at_amount: None,
        },
        PriceInput {
            currency_code: "EUR".to_string(),
            amount: dec!(89.99),
            compare_at_amount: None,
        },
        PriceInput {
            currency_code: "GBP".to_string(),
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
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: Some(dec!(1.5)),
            weight_unit: Some("kg".to_string()),
        }],
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
            assert!(msg.contains("No price found"));
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
