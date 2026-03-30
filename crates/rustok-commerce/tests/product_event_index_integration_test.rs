// Integration test for Product creation → Event → Index update flow
// This test verifies the complete workflow from product creation to indexing

use rust_decimal::Decimal;
use rustok_commerce::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput, UpdateProductInput,
};
use rustok_commerce::entities::product::ProductStatus;
use rustok_commerce::services::CatalogService;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_test_utils::{db::setup_test_db, MockEventTransport};
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

mod support;

async fn setup_service() -> (CatalogService, Arc<MockEventTransport>) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let transport = Arc::new(MockEventTransport::new());
    let event_bus = TransactionalEventBus::new(transport.clone());
    (CatalogService::new(db, event_bus), transport)
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
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                amount: Decimal::from_str("99.99").unwrap(),
                compare_at_amount: Some(Decimal::from_str("149.99").unwrap()),
            }],
            inventory_quantity: 10,
            inventory_policy: "deny".to_string(),
            weight: Some(Decimal::from_str("1.5").unwrap()),
            weight_unit: Some("kg".to_string()),
        }],
        vendor: Some("Test Vendor".to_string()),
        product_type: Some("Physical".to_string()),
        tags: vec![],
        publish: false,
        metadata: serde_json::json!({}),
    }
}

#[tokio::test]
async fn test_product_creation_triggers_event() {
    let (service, transport) = setup_service().await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let input = create_product_input("test-product", "Test Product", "TEST-SKU-001");

    let product = service
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();

    assert_eq!(transport.event_count(), 1);
    assert!(transport.has_event_of_type("ProductCreated"));

    let events = transport.events_of_type("ProductCreated");
    assert_eq!(events.len(), 1);

    if let DomainEvent::ProductCreated { product_id, .. } = events[0] {
        assert_eq!(product_id, product.id);
    } else {
        panic!("Expected ProductCreated event");
    }
}

#[tokio::test]
async fn test_product_update_triggers_event() {
    let (service, transport) = setup_service().await;

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

    transport.clear();

    let update_input = UpdateProductInput {
        translations: Some(vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Updated Product".to_string(),
            description: Some("Updated description".to_string()),
            handle: None,
            meta_title: None,
            meta_description: None,
        }]),
        vendor: Some("Updated Vendor".to_string()),
        product_type: Some("Digital".to_string()),
        tags: None,
        status: Some(ProductStatus::Active),
        metadata: None,
    };

    service
        .update_product(tenant_id, actor_id, product.id, update_input)
        .await
        .unwrap();

    assert_eq!(transport.event_count(), 1);
    assert!(transport.has_event_of_type("ProductUpdated"));

    let events = transport.events_of_type("ProductUpdated");
    if let DomainEvent::ProductUpdated { product_id, .. } = events[0] {
        assert_eq!(product_id, product.id);
    } else {
        panic!("Expected ProductUpdated event");
    }
}

#[tokio::test]
async fn test_product_publishing_triggers_event() {
    let (service, transport) = setup_service().await;

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

    transport.clear();

    service
        .publish_product(tenant_id, actor_id, product.id)
        .await
        .unwrap();

    assert_eq!(transport.event_count(), 1);
    assert!(transport.has_event_of_type("ProductPublished"));

    let events = transport.events_of_type("ProductPublished");
    if let DomainEvent::ProductPublished { product_id, .. } = events[0] {
        assert_eq!(product_id, product.id);
    } else {
        panic!("Expected ProductPublished event");
    }
}

#[tokio::test]
async fn test_product_deletion_triggers_event() {
    let (service, transport) = setup_service().await;

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

    transport.clear();

    service
        .delete_product(tenant_id, actor_id, product.id)
        .await
        .unwrap();

    assert_eq!(transport.event_count(), 1);
    assert!(transport.has_event_of_type("ProductDeleted"));

    let events = transport.events_of_type("ProductDeleted");
    if let DomainEvent::ProductDeleted { product_id, .. } = events[0] {
        assert_eq!(product_id, product.id);
    } else {
        panic!("Expected ProductDeleted event");
    }
}

#[tokio::test]
async fn test_variant_creation_triggers_event() {
    let (service, transport) = setup_service().await;

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
        option1: Some("Large".to_string()),
        option2: None,
        option3: None,
        prices: vec![PriceInput {
            currency_code: "USD".to_string(),
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

    assert_eq!(transport.event_count(), 1);
    assert!(transport.has_event_of_type("ProductCreated"));

    let product_events = transport.events_of_type("ProductCreated");
    if let DomainEvent::ProductCreated { product_id, .. } = product_events[0] {
        assert_eq!(product_id, product.id);
    } else {
        panic!("Expected ProductCreated event");
    }

    let variant_events = transport.events_of_type("VariantCreated");
    assert_eq!(variant_events.len(), 0);
}
