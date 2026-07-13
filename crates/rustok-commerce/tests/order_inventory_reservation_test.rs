use rust_decimal::Decimal;
use rustok_commerce::migrations;
use rustok_inventory::entities::{inventory_item, inventory_level, reservation_item};
use rustok_inventory::InventoryService;
use rustok_order::dto::{CreateOrderInput, CreateOrderLineItemInput};
use rustok_order::OrderService;
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_product::CatalogService;
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

mod support;

async fn setup() -> (
    DatabaseConnection,
    CatalogService,
    InventoryService,
    OrderService,
) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;

    let manager = SchemaManager::new(&db);
    let migration = migrations::migrations()
        .pop()
        .expect("order inventory reservation migration should be registered last");
    migration
        .up(&manager)
        .await
        .expect("order inventory reservation migration should install on SQLite");

    let event_bus = mock_transactional_event_bus();
    (
        db.clone(),
        CatalogService::new(db.clone(), event_bus.clone()),
        InventoryService::new(db.clone(), event_bus.clone()),
        OrderService::new(db, event_bus),
    )
}

async fn create_product_and_variant(
    catalog: &CatalogService,
    tenant_id: Uuid,
) -> (Uuid, Uuid) {
    let product = catalog
        .create_product(
            tenant_id,
            Uuid::new_v4(),
            CreateProductInput {
                translations: vec![ProductTranslationInput {
                    locale: "en".to_string(),
                    title: "Reserved product".to_string(),
                    description: None,
                    handle: Some(unique_slug("reserved-product")),
                    meta_title: None,
                    meta_description: None,
                }],
                options: Vec::new(),
                variants: vec![CreateVariantInput {
                    sku: Some(format!("RESERVE-{}", Uuid::new_v4())),
                    barcode: None,
                    shipping_profile_slug: None,
                    option1: Some("Default".to_string()),
                    option2: None,
                    option3: None,
                    prices: vec![PriceInput {
                        currency_code: "USD".to_string(),
                        channel_id: None,
                        channel_slug: None,
                        amount: Decimal::new(1000, 2),
                        compare_at_amount: None,
                    }],
                    inventory_quantity: 0,
                    inventory_policy: "deny".to_string(),
                    weight: None,
                    weight_unit: None,
                }],
                seller_id: None,
                vendor: None,
                product_type: Some("physical".to_string()),
                shipping_profile_slug: None,
                primary_category_id: None,
                tags: Vec::new(),
                publish: false,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("product should be created");

    (product.id, product.variants[0].id)
}

async fn create_order(
    service: &OrderService,
    tenant_id: Uuid,
    actor_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    quantity: i32,
) -> rustok_order::dto::OrderResponse {
    service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: None,
                currency_code: "USD".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(product_id),
                    variant_id: Some(variant_id),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("RESERVED-SKU".to_string()),
                    title: "Reserved product".to_string(),
                    quantity,
                    unit_price: Decimal::new(1000, 2),
                    metadata: serde_json::json!({}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({"source":"inventory-reservation-test"}),
            },
        )
        .await
        .expect("pending order should be created")
}

async fn reserved_quantity(db: &DatabaseConnection, variant_id: Uuid) -> i32 {
    let item = inventory_item::Entity::find()
        .filter(inventory_item::Column::VariantId.eq(variant_id))
        .one(db)
        .await
        .expect("inventory item query should succeed")
        .expect("inventory item should exist");

    inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(item.id))
        .one(db)
        .await
        .expect("inventory level query should succeed")
        .expect("inventory level should exist")
        .reserved_quantity
}

#[tokio::test]
async fn order_confirmation_reserves_and_cancellation_releases_inventory() {
    let (db, catalog, inventory, orders) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (product_id, variant_id) = create_product_and_variant(&catalog, tenant_id).await;

    inventory
        .set_inventory(tenant_id, actor_id, variant_id, 5)
        .await
        .expect("inventory should be stocked");

    let first = create_order(
        &orders,
        tenant_id,
        actor_id,
        product_id,
        variant_id,
        4,
    )
    .await;
    orders
        .confirm_order(tenant_id, actor_id, first.id)
        .await
        .expect("first order should reserve inventory");

    assert_eq!(reserved_quantity(&db, variant_id).await, 4);
    let first_reservation = reservation_item::Entity::find()
        .filter(reservation_item::Column::LineItemId.eq(first.line_items[0].id))
        .one(&db)
        .await
        .expect("reservation query should succeed")
        .expect("confirmed order should own a reservation");
    assert_eq!(first_reservation.quantity, 4);
    assert!(first_reservation.deleted_at.is_none());

    let second = create_order(
        &orders,
        tenant_id,
        actor_id,
        product_id,
        variant_id,
        2,
    )
    .await;
    let insufficient = orders.confirm_order(tenant_id, actor_id, second.id).await;
    assert!(insufficient.is_err(), "overselling confirmation must fail");
    assert_eq!(reserved_quantity(&db, variant_id).await, 4);

    orders
        .cancel_order(
            tenant_id,
            actor_id,
            first.id,
            Some("release stock".to_string()),
        )
        .await
        .expect("cancellation should release the reservation");
    assert_eq!(reserved_quantity(&db, variant_id).await, 0);

    let released = reservation_item::Entity::find_by_id(first.line_items[0].id)
        .one(&db)
        .await
        .expect("released reservation query should succeed")
        .expect("released reservation should remain as an audit row");
    assert_eq!(released.quantity, 0);
    assert!(released.deleted_at.is_some());

    orders
        .confirm_order(tenant_id, actor_id, second.id)
        .await
        .expect("inventory released by cancellation should be reservable");
    assert_eq!(reserved_quantity(&db, variant_id).await, 2);

    let immutable_update = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE order_line_items SET quantity = 1 WHERE order_id = ?",
            vec![second.id.into()],
        ))
        .await;
    assert!(
        immutable_update.is_err(),
        "confirmed order line items must be immutable so reservation totals cannot drift"
    );
}
