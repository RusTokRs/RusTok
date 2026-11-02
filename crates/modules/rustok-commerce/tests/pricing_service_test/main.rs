// Comprehensive unit tests for PricingService
// These tests verify price management, currency support,
// discounts, and price validation logic.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rustok_commerce::CommerceError;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_pricing::entities;
use rustok_pricing::{PriceAdjustmentKind, PricingService};
use rustok_product::CatalogService;
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_test_utils::{
    MockEventTransport, db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use std::sync::Arc;
use uuid::Uuid;

#[path = "../support/mod.rs"]
mod support;

pub(crate) fn pricing_read_context(tenant_id: Uuid) -> rustok_api::PortContext {
    rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::service("pricing-owner-test"),
        "en",
        "pricing-owner-test",
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

pub(crate) async fn seed_pricing_channel(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
    slug: &str,
) {
    use rustok_channel::entities::channel;
    if let Some(existing) = channel::Entity::find_by_id(id).one(db).await.unwrap() {
        assert_eq!(existing.tenant_id, tenant_id);
        assert_eq!(existing.slug, slug);
        return;
    }
    let now = chrono::Utc::now();
    channel::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        slug: Set(slug.into()),
        name: Set(slug.into()),
        is_active: Set(true),
        is_default: Set(false),
        status: Set("active".into()),
        settings: Set(serde_json::json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .unwrap();
}

pub(crate) async fn seed_pricing_region(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
    currency: &str,
) {
    use rustok_region::entities::region;
    let now = chrono::Utc::now();
    region::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        currency_code: Set(currency.into()),
        tax_provider_id: Set(None),
        tax_rate: Set(Decimal::ZERO),
        tax_included: Set(false),
        countries: Set(serde_json::json!([])),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .unwrap();
}

pub(crate) async fn setup() -> (DatabaseConnection, PricingService, CatalogService) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let event_bus = mock_transactional_event_bus();
    let pricing_service = PricingService::new(db.clone(), event_bus.clone());
    let catalog_service = CatalogService::new(db.clone(), event_bus);
    (db, pricing_service, catalog_service)
}

pub(crate) async fn setup_with_transport() -> (
    DatabaseConnection,
    PricingService,
    CatalogService,
    Arc<MockEventTransport>,
) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let transport = Arc::new(MockEventTransport::new());
    let event_bus = TransactionalEventBus::new(transport.clone());
    let pricing_service = PricingService::new(db.clone(), event_bus.clone());
    let catalog_service = CatalogService::new(db.clone(), event_bus);
    (db, pricing_service, catalog_service, transport)
}

pub(crate) async fn create_test_product(catalog: &CatalogService, tenant_id: Uuid) -> (Uuid, Uuid) {
    let input = CreateProductInput {
        attribute_values: Default::default(),
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Test Product".to_string(),
            description: Some("A test product".to_string()),
            handle: Some(unique_slug("test-product")),
            meta_title: None,
            meta_description: None,
        }],
        variant_axes: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(format!(
                "SKU-{}",
                Uuid::new_v4().to_string().split('-').next().unwrap()
            )),
            barcode: None,
            shipping_profile_slug: None,
            axis_values: vec![],
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
        primary_category_id: None,
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

pub(crate) async fn create_test_product_with_seller(
    catalog: &CatalogService,
    tenant_id: Uuid,
    seller_id: &str,
) -> Uuid {
    let input = CreateProductInput {
        attribute_values: Default::default(),
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "Seller Product".to_string(),
            description: Some("Seller scoped product".to_string()),
            handle: Some(unique_slug("seller-product")),
            meta_title: None,
            meta_description: None,
        }],
        variant_axes: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(format!(
                "SELLER-{}",
                Uuid::new_v4().to_string().split('-').next().unwrap()
            )),
            barcode: None,
            shipping_profile_slug: Some("default".to_string()),
            axis_values: vec![],
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
        primary_category_id: None,
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

pub(crate) async fn create_price_list(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    status: &str,
    starts_at: Option<chrono::DateTime<chrono::Utc>>,
    ends_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Uuid {
    create_price_list_with_channel(db, tenant_id, status, starts_at, ends_at, None, None).await
}

pub(crate) async fn create_price_list_with_channel(
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

    entities::price_list_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        price_list_id: Set(price_list_id),
        locale: Set("en".to_string()),
        name: Set(format!("List-{price_list_id}")),
        description: Set(Some("Test list".to_string())),
    }
    .insert(db)
    .await
    .unwrap();

    price_list_id
}

pub mod discount;
pub mod listing;
pub mod price_list;
pub mod price_set;
pub mod resolve;
