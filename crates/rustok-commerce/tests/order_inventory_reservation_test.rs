use rust_decimal::Decimal;
use rustok_commerce::migrations as commerce_migrations;
use rustok_fulfillment::dto::{
    CreateFulfillmentInput, CreateFulfillmentItemInput, DeliverFulfillmentInput,
    FulfillmentItemQuantityInput, ShipFulfillmentInput,
};
use rustok_fulfillment::{FulfillmentService, migrations as fulfillment_migrations};
use rustok_inventory::InventoryService;
use rustok_inventory::entities::{inventory_item, inventory_level, reservation_item};
use rustok_order::OrderService;
use rustok_order::dto::{CreateOrderInput, CreateOrderLineItemInput};
use rustok_product::CatalogService;
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    Statement,
};
use sea_orm_migration::SchemaManager;
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
    let mut registered = commerce_migrations::migrations();
    let delivery_guard = registered
        .pop()
        .expect("fulfillment delivery guard should be registered last");
    let fulfillment_shipping = registered
        .pop()
        .expect("fulfillment shipping consumption should precede delivery guard");
    let order_delivery = registered
        .pop()
        .expect("order delivery consumption should precede fulfillment shipping");
    let reservation = registered
        .pop()
        .expect("order confirmation reservation should precede consumption");
    reservation
        .up(&manager)
        .await
        .expect("order inventory reservation migration should install on SQLite");
    order_delivery
        .up(&manager)
        .await
        .expect("order inventory delivery consumption should install on SQLite");

    let fulfillment_progress = fulfillment_migrations::migrations()
        .pop()
        .expect("fulfillment progress serialization migration should be registered last");
    fulfillment_progress
        .up(&manager)
        .await
        .expect("fulfillment progress serialization should install on SQLite");
    fulfillment_shipping
        .up(&manager)
        .await
        .expect("fulfillment shipping consumption should install on SQLite");
    delivery_guard
        .up(&manager)
        .await
        .expect("fulfillment delivery guard should install on SQLite");

    let event_bus = mock_transactional_event_bus();
    (
        db.clone(),
        CatalogService::new(db.clone(), event_bus.clone()),
        InventoryService::new(db.clone(), event_bus.clone()),
        OrderService::new(db, event_bus),
    )
}

async fn create_product_and_variant(catalog: &CatalogService, tenant_id: Uuid) -> (Uuid, Uuid) {
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

async fn inventory_quantities(db: &DatabaseConnection, variant_id: Uuid) -> (i32, i32) {
    let item = inventory_item::Entity::find()
        .filter(inventory_item::Column::VariantId.eq(variant_id))
        .one(db)
        .await
        .expect("inventory item query should succeed")
        .expect("inventory item should exist");

    let level = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(item.id))
        .one(db)
        .await
        .expect("inventory level query should succeed")
        .expect("inventory level should exist");
    (level.stocked_quantity, level.reserved_quantity)
}

#[tokio::test]
async fn order_inventory_is_reserved_released_and_consumed_across_lifecycle() {
    let (db, catalog, inventory, orders) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (product_id, variant_id) = create_product_and_variant(&catalog, tenant_id).await;

    inventory
        .set_inventory(tenant_id, actor_id, variant_id, 5)
        .await
        .expect("inventory should be stocked");

    let first = create_order(&orders, tenant_id, actor_id, product_id, variant_id, 4).await;
    orders
        .confirm_order(tenant_id, actor_id, first.id)
        .await
        .expect("first order should reserve inventory");

    assert_eq!(inventory_quantities(&db, variant_id).await, (5, 4));
    let first_reservation = reservation_item::Entity::find()
        .filter(reservation_item::Column::LineItemId.eq(first.line_items[0].id))
        .one(&db)
        .await
        .expect("reservation query should succeed")
        .expect("confirmed order should own a reservation");
    assert_eq!(first_reservation.quantity, 4);
    assert!(first_reservation.deleted_at.is_none());

    let second = create_order(&orders, tenant_id, actor_id, product_id, variant_id, 2).await;
    let insufficient = orders.confirm_order(tenant_id, actor_id, second.id).await;
    assert!(insufficient.is_err(), "overselling confirmation must fail");
    assert_eq!(inventory_quantities(&db, variant_id).await, (5, 4));

    orders
        .cancel_order(
            tenant_id,
            actor_id,
            first.id,
            Some("release stock".to_string()),
        )
        .await
        .expect("cancellation should release the reservation");
    assert_eq!(inventory_quantities(&db, variant_id).await, (5, 0));

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
    assert_eq!(inventory_quantities(&db, variant_id).await, (5, 2));

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

    orders
        .mark_paid(
            tenant_id,
            actor_id,
            second.id,
            "payment-ref".to_string(),
            "manual".to_string(),
        )
        .await
        .expect("confirmed order should become paid");
    orders
        .ship_order(
            tenant_id,
            actor_id,
            second.id,
            "TRACK-DELIVERY".to_string(),
            "manual".to_string(),
        )
        .await
        .expect("paid order should ship");
    orders
        .deliver_order(tenant_id, actor_id, second.id, None)
        .await
        .expect("legacy order without fulfillments should deliver and consume inventory");

    assert_eq!(inventory_quantities(&db, variant_id).await, (3, 0));
    let consumed = reservation_item::Entity::find_by_id(second.line_items[0].id)
        .one(&db)
        .await
        .expect("consumed reservation query should succeed")
        .expect("consumed reservation should remain as an audit row");
    assert_eq!(consumed.quantity, 0);
    assert!(consumed.deleted_at.is_some());
}

#[tokio::test]
async fn fulfillment_shipping_consumes_reservation_and_gates_order_delivery() {
    let (db, catalog, inventory, orders) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (product_id, variant_id) = create_product_and_variant(&catalog, tenant_id).await;

    inventory
        .set_inventory(tenant_id, actor_id, variant_id, 5)
        .await
        .expect("inventory should be stocked");
    let order = create_order(&orders, tenant_id, actor_id, product_id, variant_id, 3).await;
    orders
        .confirm_order(tenant_id, actor_id, order.id)
        .await
        .expect("order should reserve inventory");
    orders
        .mark_paid(
            tenant_id,
            actor_id,
            order.id,
            "fulfillment-payment".to_string(),
            "manual".to_string(),
        )
        .await
        .expect("confirmed order should become paid");
    assert_eq!(inventory_quantities(&db, variant_id).await, (5, 3));

    let fulfillment_service = FulfillmentService::new(db.clone());
    let fulfillment = fulfillment_service
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id: order.id,
                shipping_option_id: None,
                customer_id: None,
                carrier: None,
                tracking_number: None,
                items: Some(vec![CreateFulfillmentItemInput {
                    order_line_item_id: order.line_items[0].id,
                    quantity: 3,
                    metadata: serde_json::json!({}),
                }]),
                metadata: serde_json::json!({"source":"shipping-consumption-test"}),
            },
        )
        .await
        .expect("fulfillment should be created");
    let fulfillment_item_id = fulfillment.items[0].id;

    let partial = fulfillment_service
        .ship_fulfillment(
            tenant_id,
            fulfillment.id,
            ShipFulfillmentInput {
                carrier: "manual".to_string(),
                tracking_number: "PARTIAL-SHIP".to_string(),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id,
                    quantity: 1,
                }]),
                metadata: serde_json::json!({"step":"partial"}),
            },
        )
        .await
        .expect("partial shipment should consume one reserved unit");
    assert_eq!(partial.status, "shipped");
    assert_eq!(partial.items[0].shipped_quantity, 1);
    assert_eq!(inventory_quantities(&db, variant_id).await, (4, 2));

    let remaining_reservation = reservation_item::Entity::find_by_id(order.line_items[0].id)
        .one(&db)
        .await
        .expect("reservation query should succeed")
        .expect("partial shipment should leave an active reservation");
    assert_eq!(remaining_reservation.quantity, 2);
    assert!(remaining_reservation.deleted_at.is_none());

    let fully_shipped = fulfillment_service
        .ship_fulfillment(
            tenant_id,
            fulfillment.id,
            ShipFulfillmentInput {
                carrier: "manual".to_string(),
                tracking_number: "FULL-SHIP".to_string(),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id,
                    quantity: 2,
                }]),
                metadata: serde_json::json!({"step":"complete"}),
            },
        )
        .await
        .expect("remaining shipment should consume the reservation");
    assert_eq!(fully_shipped.items[0].shipped_quantity, 3);
    assert_eq!(inventory_quantities(&db, variant_id).await, (2, 0));

    let consumed = reservation_item::Entity::find_by_id(order.line_items[0].id)
        .one(&db)
        .await
        .expect("consumed reservation query should succeed")
        .expect("consumed reservation should remain as an audit row");
    assert_eq!(consumed.quantity, 0);
    assert!(consumed.deleted_at.is_some());

    orders
        .ship_order(
            tenant_id,
            actor_id,
            order.id,
            "ORDER-SHIP".to_string(),
            "manual".to_string(),
        )
        .await
        .expect("paid order should be markable as shipped");

    let premature_delivery = orders
        .deliver_order(tenant_id, actor_id, order.id, None)
        .await;
    assert!(
        premature_delivery.is_err(),
        "order delivery must wait for its active fulfillment to be delivered"
    );
    assert_eq!(inventory_quantities(&db, variant_id).await, (2, 0));

    let delivered_fulfillment = fulfillment_service
        .deliver_fulfillment(
            tenant_id,
            fulfillment.id,
            DeliverFulfillmentInput {
                delivered_note: Some("received".to_string()),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id,
                    quantity: 3,
                }]),
                metadata: serde_json::json!({"step":"delivered"}),
            },
        )
        .await
        .expect("fully shipped fulfillment should be deliverable");
    assert_eq!(delivered_fulfillment.status, "delivered");
    assert_eq!(delivered_fulfillment.items[0].delivered_quantity, 3);

    orders
        .deliver_order(tenant_id, actor_id, order.id, None)
        .await
        .expect("order should deliver after fulfillment completion");
    assert_eq!(
        inventory_quantities(&db, variant_id).await,
        (2, 0),
        "order delivery must not double-consume inventory already shipped by fulfillment"
    );
}
