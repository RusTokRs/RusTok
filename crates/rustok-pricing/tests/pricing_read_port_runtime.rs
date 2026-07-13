use rust_decimal::Decimal;
use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_commerce_foundation::entities::product::ProductStatus;
use rustok_commerce_foundation::entities::{
    price, price_list, price_list_translation, product, product_variant,
};
use rustok_core::events::MemoryTransport;
use rustok_pricing::{
    PriceListProjectionRequest, PricingReadPort, PricingService, ResolveProductPriceRequest,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    Schema,
};
use std::sync::Arc;
use uuid::Uuid;

async fn create_entity_table(
    db: &DatabaseConnection,
    builder: &DbBackend,
    mut statement: sea_orm::sea_query::TableCreateStatement,
) {
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("failed to create pricing port runtime test table");
}

async fn setup_pricing_port_service() -> (DatabaseConnection, PricingService, Uuid, Uuid) {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("failed to open in-memory pricing port runtime database");
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(product::Entity),
    )
    .await;
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(product_variant::Entity),
    )
    .await;
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(price::Entity),
    )
    .await;
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(price_list::Entity),
    )
    .await;
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(price_list_translation::Entity),
    )
    .await;

    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    let variant_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    product::ActiveModel {
        id: Set(product_id),
        tenant_id: Set(tenant_id),
        status: Set(ProductStatus::Draft),
        seller_id: Set(None),
        vendor: Set(None),
        product_type: Set(None),
        shipping_profile_slug: Set(None),
        primary_category_id: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        published_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("failed to seed pricing product");
    product_variant::ActiveModel {
        id: Set(variant_id),
        product_id: Set(product_id),
        tenant_id: Set(tenant_id),
        sku: Set(Some("pricing-port-runtime".to_string())),
        barcode: Set(None),
        shipping_profile_slug: Set(None),
        ean: Set(None),
        upc: Set(None),
        inventory_policy: Set("deny".to_string()),
        inventory_management: Set("managed".to_string()),
        inventory_quantity: Set(0),
        weight: Set(None),
        weight_unit: Set(None),
        option1: Set(None),
        option2: Set(None),
        option3: Set(None),
        position: Set(0),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("failed to seed pricing variant");
    price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(None),
        amount: Set(Decimal::new(8999, 2)),
        compare_at_amount: Set(Some(Decimal::new(9999, 2))),
        legacy_amount: Set(Some(8999)),
        legacy_compare_at_amount: Set(Some(9999)),
        min_quantity: Set(None),
        max_quantity: Set(None),
    }
    .insert(&db)
    .await
    .expect("failed to seed pricing row");

    (
        db.clone(),
        PricingService::new(
            db,
            rustok_outbox::TransactionalEventBus::new(Arc::new(MemoryTransport::new())),
        ),
        tenant_id,
        variant_id,
    )
}

fn read_context(tenant_id: Uuid, correlation_id: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-cart.storefront"),
        "en",
        correlation_id,
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

#[tokio::test]
async fn pricing_read_port_executes_variant_first_resolution_against_persistence() {
    let (_db, service, tenant_id, variant_id) = setup_pricing_port_service().await;

    let snapshot = service
        .resolve_product_price(
            read_context(tenant_id, "pricing-port-runtime-variant-first"),
            ResolveProductPriceRequest {
                product_id: None,
                variant_id,
                region_id: None,
                channel_id: None,
                channel_slug: None,
                price_list_id: None,
                quantity: Some(2),
                currency_code: "usd".to_string(),
            },
        )
        .await
        .expect("variant-first pricing port resolution must succeed");

    assert_eq!(snapshot.product_id, None);
    assert_eq!(snapshot.variant_id, variant_id);
    assert_eq!(snapshot.currency_code, "USD");
    assert_eq!(snapshot.amount, Decimal::new(8999, 2));
    assert_eq!(snapshot.compare_at_amount, Some(Decimal::new(9999, 2)));
    assert!(snapshot.on_sale);
}

#[tokio::test]
async fn pricing_read_port_rejects_missing_read_deadline_before_provider_access() {
    let (_db, service, tenant_id, variant_id) = setup_pricing_port_service().await;

    let error = service
        .resolve_product_price(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::service("rustok-cart.storefront"),
                "en",
                "pricing-port-runtime-missing-deadline",
            ),
            ResolveProductPriceRequest {
                product_id: None,
                variant_id,
                region_id: None,
                channel_id: None,
                channel_slug: None,
                price_list_id: None,
                quantity: Some(1),
                currency_code: "USD".to_string(),
            },
        )
        .await
        .expect_err("read port must require a deadline");

    assert_eq!(error.kind, PortErrorKind::Timeout);
    assert_eq!(error.code, "port.deadline_required");
}

#[tokio::test]
async fn pricing_read_port_executes_price_list_projection_against_persistence() {
    let (db, service, tenant_id, _) = setup_pricing_port_service().await;
    let price_list_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    price_list::ActiveModel {
        id: Set(price_list_id),
        tenant_id: Set(tenant_id),
        r#type: Set("sale".to_string()),
        status: Set("active".to_string()),
        channel_id: Set(None),
        channel_slug: Set(None),
        rule_kind: Set(None),
        adjustment_percent: Set(None),
        starts_at: Set(None),
        ends_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("failed to seed active price list");
    price_list_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        price_list_id: Set(price_list_id),
        locale: Set("en".to_string()),
        name: Set("Storefront sale".to_string()),
        description: Set(None),
    }
    .insert(&db)
    .await
    .expect("failed to seed price list translation");

    let projection = service
        .read_price_list_projection(
            read_context(tenant_id, "pricing-port-runtime-price-list"),
            PriceListProjectionRequest {
                price_list_id,
                locale: Some("en".to_string()),
            },
        )
        .await
        .expect("price-list projection must resolve through the pricing port");

    assert_eq!(projection.price_list_id, price_list_id);
    assert_eq!(projection.title, "Storefront sale");
}
