// Comprehensive unit tests for InventoryService
// These tests verify inventory management, stock tracking,
// low stock alerts, and availability checks.

use rust_decimal::Decimal;
use rustok_commerce::dto::{
    AdjustInventoryInput, CreateProductInput, CreateVariantInput, PriceInput,
    ProductTranslationInput,
};
use rustok_commerce::entities;
use rustok_commerce::services::{CatalogService, InventoryService};
use rustok_commerce::CommerceError;
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> (DatabaseConnection, InventoryService, CatalogService) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let event_bus = mock_transactional_event_bus();
    let inventory_service = InventoryService::new(db.clone(), event_bus.clone());
    let catalog_service = CatalogService::new(db.clone(), event_bus);
    (db, inventory_service, catalog_service)
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
                amount: Decimal::from_str("99.99").unwrap(),
                compare_at_amount: None,
            }],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: Some(Decimal::from_str("1.5").unwrap()),
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
// Adjust Inventory Tests
// =============================================================================

#[tokio::test]
async fn test_adjust_inventory_increase() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let input = AdjustInventoryInput {
        variant_id,
        adjustment: 10,
        reason: Some("Restocking".to_string()),
    };

    let result = service.adjust_inventory(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let new_quantity = result.unwrap();
    assert_eq!(new_quantity, 10);
}

#[tokio::test]
async fn test_adjust_inventory_decrease() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 20)
        .await
        .unwrap();

    let input = AdjustInventoryInput {
        variant_id,
        adjustment: -5,
        reason: Some("Sold".to_string()),
    };

    let result = service.adjust_inventory(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let new_quantity = result.unwrap();
    assert_eq!(new_quantity, 15);
}

#[tokio::test]
async fn test_adjust_inventory_multiple_adjustments() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 10)
        .await
        .unwrap();

    let input1 = AdjustInventoryInput {
        variant_id,
        adjustment: 5,
        reason: Some("Restock".to_string()),
    };
    let qty1 = service
        .adjust_inventory(tenant_id, actor_id, input1)
        .await
        .unwrap();
    assert_eq!(qty1, 15);

    let input2 = AdjustInventoryInput {
        variant_id,
        adjustment: -3,
        reason: Some("Sold".to_string()),
    };
    let qty2 = service
        .adjust_inventory(tenant_id, actor_id, input2)
        .await
        .unwrap();
    assert_eq!(qty2, 12);

    let input3 = AdjustInventoryInput {
        variant_id,
        adjustment: 8,
        reason: Some("Restock".to_string()),
    };
    let qty3 = service
        .adjust_inventory(tenant_id, actor_id, input3)
        .await
        .unwrap();
    assert_eq!(qty3, 20);
}

#[tokio::test]
async fn test_adjust_inventory_nonexistent_variant() {
    let (_db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let fake_variant_id = Uuid::new_v4();

    let input = AdjustInventoryInput {
        variant_id: fake_variant_id,
        adjustment: 10,
        reason: None,
    };

    let result = service.adjust_inventory(tenant_id, actor_id, input).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::VariantNotFound(_) => {}
        _ => panic!("Expected VariantNotFound error"),
    }
}

// =============================================================================
// Set Inventory Tests
// =============================================================================

#[tokio::test]
async fn test_set_inventory_success() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_inventory(tenant_id, actor_id, variant_id, 50)
        .await;

    assert!(result.is_ok());
    let quantity = result.unwrap();
    assert_eq!(quantity, 50);
}

#[tokio::test]
async fn test_set_inventory_zero() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_inventory(tenant_id, actor_id, variant_id, 0)
        .await;

    assert!(result.is_ok());
    let quantity = result.unwrap();
    assert_eq!(quantity, 0);
}

#[tokio::test]
async fn test_set_inventory_overwrite() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 100)
        .await
        .unwrap();
    let result = service
        .set_inventory(tenant_id, actor_id, variant_id, 25)
        .await;

    assert!(result.is_ok());
    let quantity = result.unwrap();
    assert_eq!(quantity, 25);
}

#[tokio::test]
async fn test_set_inventory_large_quantity() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_inventory(tenant_id, actor_id, variant_id, 10000)
        .await;

    assert!(result.is_ok());
    let quantity = result.unwrap();
    assert_eq!(quantity, 10000);
}

#[tokio::test]
async fn test_set_inventory_nonexistent_variant() {
    let (_db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let fake_variant_id = Uuid::new_v4();

    let result = service
        .set_inventory(tenant_id, actor_id, fake_variant_id, 50)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::VariantNotFound(_) => {}
        _ => panic!("Expected VariantNotFound error"),
    }
}

// =============================================================================
// Low Stock Threshold Tests
// =============================================================================

#[tokio::test]
async fn test_low_stock_threshold_default() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 10)
        .await
        .unwrap();

    let input = AdjustInventoryInput {
        variant_id,
        adjustment: -7,
        reason: Some("Sale".to_string()),
    };

    let result = service.adjust_inventory(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let quantity = result.unwrap();
    assert_eq!(quantity, 3);
}

#[tokio::test]
async fn test_custom_threshold() {
    let (db, _service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let event_bus = mock_transactional_event_bus();
    let service_with_custom_threshold =
        InventoryService::new(db.clone(), event_bus).with_threshold(10);

    service_with_custom_threshold
        .set_inventory(tenant_id, actor_id, variant_id, 12)
        .await
        .unwrap();

    let input = AdjustInventoryInput {
        variant_id,
        adjustment: -5,
        reason: Some("Sale".to_string()),
    };

    let result = service_with_custom_threshold
        .adjust_inventory(tenant_id, actor_id, input)
        .await;

    assert!(result.is_ok());
    let quantity = result.unwrap();
    assert_eq!(quantity, 7);
}

// =============================================================================
// Check Availability Tests
// =============================================================================

#[tokio::test]
async fn test_check_availability_sufficient_stock() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 20)
        .await
        .unwrap();

    let result = service.check_availability(tenant_id, variant_id, 10).await;

    assert!(result.is_ok());
    let available = result.unwrap();
    assert!(available);
}

#[tokio::test]
async fn test_check_availability_insufficient_stock() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 5)
        .await
        .unwrap();

    let result = service.check_availability(tenant_id, variant_id, 10).await;

    assert!(result.is_ok());
    let available = result.unwrap();
    assert!(!available);
}

#[tokio::test]
async fn test_check_availability_exact_stock() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 10)
        .await
        .unwrap();

    let result = service.check_availability(tenant_id, variant_id, 10).await;

    assert!(result.is_ok());
    let available = result.unwrap();
    assert!(available);
}

#[tokio::test]
async fn test_check_availability_zero_stock() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 0)
        .await
        .unwrap();

    let result = service.check_availability(tenant_id, variant_id, 1).await;

    assert!(result.is_ok());
    let available = result.unwrap();
    assert!(!available);
}

#[tokio::test]
async fn test_check_availability_nonexistent_variant() {
    let (_db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let fake_variant_id = Uuid::new_v4();

    let result = service
        .check_availability(tenant_id, fake_variant_id, 5)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::VariantNotFound(_) => {}
        _ => panic!("Expected VariantNotFound error"),
    }
}

// =============================================================================
// Reserve Inventory Tests
// =============================================================================

#[tokio::test]
async fn test_reserve_sufficient_stock() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 20)
        .await
        .unwrap();

    let result = service.reserve(tenant_id, variant_id, 10).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reserve_insufficient_stock() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 5)
        .await
        .unwrap();

    let result = service.reserve(tenant_id, variant_id, 10).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::InsufficientInventory {
            requested,
            available,
        } => {
            assert_eq!(requested, 10);
            assert_eq!(available, 5);
        }
        _ => panic!("Expected InsufficientInventory error"),
    }
}

#[tokio::test]
async fn test_reserve_exact_stock() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 10)
        .await
        .unwrap();

    let result = service.reserve(tenant_id, variant_id, 10).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reserve_zero_quantity() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 10)
        .await
        .unwrap();

    let result = service.reserve(tenant_id, variant_id, 0).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reserve_nonexistent_variant() {
    let (_db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let fake_variant_id = Uuid::new_v4();

    let result = service.reserve(tenant_id, fake_variant_id, 5).await;

    assert!(result.is_err());
}

// =============================================================================
// Integration & Edge Case Tests
// =============================================================================

#[tokio::test]
async fn test_inventory_workflow() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 100)
        .await
        .unwrap();

    let available = service
        .check_availability(tenant_id, variant_id, 10)
        .await
        .unwrap();
    assert!(available);

    service.reserve(tenant_id, variant_id, 10).await.unwrap();

    let input = AdjustInventoryInput {
        variant_id,
        adjustment: -10,
        reason: Some("Order fulfilled".to_string()),
    };
    let qty = service
        .adjust_inventory(tenant_id, actor_id, input)
        .await
        .unwrap();
    assert_eq!(qty, 80);

    let available2 = service
        .check_availability(tenant_id, variant_id, 95)
        .await
        .unwrap();
    assert!(!available2);
    assert!(service
        .check_availability(tenant_id, variant_id, 80)
        .await
        .unwrap());
}

#[tokio::test]
async fn test_concurrent_inventory_adjustments() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 50)
        .await
        .unwrap();

    let input1 = AdjustInventoryInput {
        variant_id,
        adjustment: -5,
        reason: Some("Order 1".to_string()),
    };
    let input2 = AdjustInventoryInput {
        variant_id,
        adjustment: -3,
        reason: Some("Order 2".to_string()),
    };
    let input3 = AdjustInventoryInput {
        variant_id,
        adjustment: -7,
        reason: Some("Order 3".to_string()),
    };

    service
        .adjust_inventory(tenant_id, actor_id, input1)
        .await
        .unwrap();
    service
        .adjust_inventory(tenant_id, actor_id, input2)
        .await
        .unwrap();
    let final_qty = service
        .adjust_inventory(tenant_id, actor_id, input3)
        .await
        .unwrap();

    assert_eq!(final_qty, 35);
}

#[tokio::test]
async fn test_negative_adjustment_to_zero() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 10)
        .await
        .unwrap();

    let input = AdjustInventoryInput {
        variant_id,
        adjustment: -10,
        reason: Some("Sold out".to_string()),
    };

    let result = service.adjust_inventory(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let quantity = result.unwrap();
    assert_eq!(quantity, 0);
}

#[tokio::test]
async fn test_large_inventory_quantities() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 1000000)
        .await
        .unwrap();

    let input = AdjustInventoryInput {
        variant_id,
        adjustment: 500000,
        reason: Some("Massive restock".to_string()),
    };

    let result = service.adjust_inventory(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let quantity = result.unwrap();
    assert_eq!(quantity, 1500000);
}

#[tokio::test]
async fn test_inventory_boundary_at_threshold() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 6)
        .await
        .unwrap();

    let input = AdjustInventoryInput {
        variant_id,
        adjustment: -1,
        reason: Some("Sold".to_string()),
    };

    let result = service.adjust_inventory(tenant_id, actor_id, input).await;

    assert!(result.is_ok());
    let quantity = result.unwrap();
    assert_eq!(quantity, 5);
}

#[tokio::test]
async fn test_set_inventory_creates_normalized_inventory_records() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 25)
        .await
        .unwrap();

    let inventory_item = entities::inventory_item::Entity::find()
        .filter(entities::inventory_item::Column::VariantId.eq(variant_id))
        .one(&_db)
        .await
        .unwrap()
        .expect("inventory item should be created");
    let level = entities::inventory_level::Entity::find()
        .filter(entities::inventory_level::Column::InventoryItemId.eq(inventory_item.id))
        .one(&_db)
        .await
        .unwrap()
        .expect("inventory level should be created");
    let location_count = entities::stock_location::Entity::find()
        .filter(entities::stock_location::Column::TenantId.eq(tenant_id))
        .count(&_db)
        .await
        .unwrap();

    assert_eq!(level.stocked_quantity, 25);
    assert_eq!(level.reserved_quantity, 0);
    assert_eq!(location_count, 1);
}

#[tokio::test]
async fn test_reserve_creates_reservation_and_syncs_available_shadow() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_inventory(tenant_id, actor_id, variant_id, 20)
        .await
        .unwrap();
    service.reserve(tenant_id, variant_id, 7).await.unwrap();

    let inventory_item = entities::inventory_item::Entity::find()
        .filter(entities::inventory_item::Column::VariantId.eq(variant_id))
        .one(&_db)
        .await
        .unwrap()
        .expect("inventory item should exist");
    let level = entities::inventory_level::Entity::find()
        .filter(entities::inventory_level::Column::InventoryItemId.eq(inventory_item.id))
        .one(&_db)
        .await
        .unwrap()
        .expect("inventory level should exist");
    let reservation = entities::reservation_item::Entity::find()
        .filter(entities::reservation_item::Column::InventoryItemId.eq(inventory_item.id))
        .one(&_db)
        .await
        .unwrap()
        .expect("reservation item should be created");
    assert_eq!(level.stocked_quantity, 20);
    assert_eq!(level.reserved_quantity, 7);
    assert_eq!(reservation.quantity, 7);
    assert!(service
        .check_availability(tenant_id, variant_id, 13)
        .await
        .unwrap());
    assert!(!service
        .check_availability(tenant_id, variant_id, 14)
        .await
        .unwrap());
}
