// Integration test for Product creation РІвЂ вЂ™ Event РІвЂ вЂ™ Index update flow
// This test verifies the complete workflow from product creation to indexing

use rust_decimal::Decimal;
use rustok_outbox::{OutboxTransport, SysEvents, SysEventsMigration, TransactionalEventBus};
use rustok_product::CatalogService;
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput, UpdateProductInput,
};
use rustok_product::entities::product::ProductStatus;
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};
use sea_orm_migration::MigrationTrait;
use sea_orm_migration::prelude::SchemaManager;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

mod support;

async fn setup_service() -> (DatabaseConnection, CatalogService) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let schema_manager = SchemaManager::new(&db);
    SysEventsMigration
        .up(&schema_manager)
        .await
        .expect("outbox schema should be available for product event tests");
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db.clone(), CatalogService::new(db, event_bus))
}

async fn event_count(db: &DatabaseConnection, event_type: &str) -> u64 {
    SysEvents::find()
        .filter(rustok_outbox::entity::Column::EventType.eq(event_type))
        .count(db)
        .await
        .expect("event count should load")
}

fn create_product_input(handle: &str, title: &str, sku: &str) -> CreateProductInput {
    CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: title.to_string(),
            description: Some(format!("{} description", title)),
            handle: Some(handle.to_string()),
            meta_title: None,
            meta_description: None,
        }],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(sku.to_string()),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: None,
                amount: Decimal::from_str("99.99").unwrap(),
                compare_at_amount: Some(Decimal::from_str("149.99").unwrap()),
            }],
            inventory_quantity: 10,
            inventory_policy: "deny".to_string(),
            weight: Some(Decimal::from_str("1.5").unwrap()),
            weight_unit: Some("kg".to_string()),
        }],
        seller_id: None,
        vendor: Some("Test Vendor".to_string()),
        product_type: Some("Physical".to_string()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: vec![],
        publish: false,
        metadata: serde_json::json!({}),
    }
}

#[tokio::test]
async fn test_product_creation_triggers_event() {
    let (db, service) = setup_service().await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let input = create_product_input("test-product", "Test Product", "TEST-SKU-001");

    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    assert_eq!(event_count(&db, "product.created").await, 1);
    assert_eq!(product.translations[0].handle, "test-product");
}

#[tokio::test]
async fn test_product_update_triggers_event() {
    let (db, service) = setup_service().await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let product = service
        .create_product(
            tenant_id,
            actor_id,
            create_product_input("original-product", "Original Product", "ORIG-SKU-001"),
        )
        .await
        .unwrap();

    let update_input = UpdateProductInput {
        translations: Some(vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Updated Product".to_string(),
            description: Some("Updated description".to_string()),
            handle: None,
            meta_title: None,
            meta_description: None,
        }]),
        seller_id: None,
        vendor: Some("Updated Vendor".to_string()),
        product_type: Some("Digital".to_string()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: None,
        status: Some(ProductStatus::Active),
        metadata: None,
    };

    service
        .update_product(tenant_id, actor_id, product.id, update_input)
        .await
        .unwrap();

    assert_eq!(event_count(&db, "product.created").await, 1);
    assert_eq!(event_count(&db, "product.updated").await, 1);
}

#[tokio::test]
async fn test_product_publishing_triggers_event() {
    let (db, service) = setup_service().await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let product = service
        .create_product(
            tenant_id,
            actor_id,
            create_product_input("draft-product", "Draft Product", "DRAFT-SKU-001"),
        )
        .await
        .unwrap();

    service
        .publish_product(tenant_id, actor_id, product.id)
        .await
        .unwrap();

    assert_eq!(event_count(&db, "product.created").await, 1);
    assert_eq!(event_count(&db, "product.published").await, 1);
}

#[tokio::test]
async fn test_product_deletion_triggers_event() {
    let (db, service) = setup_service().await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let product = service
        .create_product(
            tenant_id,
            actor_id,
            create_product_input("to-be-deleted", "To be deleted", "DELETE-SKU-001"),
        )
        .await
        .unwrap();

    service
        .delete_product(tenant_id, actor_id, product.id)
        .await
        .unwrap();

    assert_eq!(event_count(&db, "product.created").await, 1);
    assert_eq!(event_count(&db, "product.deleted").await, 1);
}

#[tokio::test]
async fn test_variant_creation_triggers_event() {
    let (db, service) = setup_service().await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let mut input = create_product_input(
        "product-with-variants",
        "Product with Variants",
        "VARIANT-SKU-001",
    );
    input.variants.push(CreateVariantInput {
        sku: Some("VARIANT-SKU-002".to_string()),
        barcode: None,
        shipping_profile_slug: None,
        option1: Some("Large".to_string()),
        option2: None,
        option3: None,
        prices: vec![PriceInput {
            currency_code: "USD".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::from_str("119.99").unwrap(),
            compare_at_amount: Some(Decimal::from_str("169.99").unwrap()),
        }],
        inventory_quantity: 5,
        inventory_policy: "deny".to_string(),
        weight: Some(Decimal::from_str("2.0").unwrap()),
        weight_unit: Some("kg".to_string()),
    });

    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    assert_eq!(event_count(&db, "product.created").await, 1);
    assert_eq!(event_count(&db, "variant.created").await, 0);
    assert_eq!(product.variants.len(), 2);
}
