use rust_decimal::Decimal;
use rustok_commerce::migrations as commerce_migrations;
use rustok_fulfillment::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, CreateFulfillmentItemInput,
    DeliverFulfillmentInput, FulfillmentItemQuantityInput, ReopenFulfillmentInput,
    ShipFulfillmentInput,
};
use rustok_fulfillment::{FulfillmentService, migrations as fulfillment_migrations};
use rustok_inventory::InventoryService;
use rustok_inventory::entities::{inventory_item, inventory_level, reservation_item};
use rustok_order::OrderService;
use rustok_order::dto::{
    CreateOrderInput, CreateOrderLineItemInput, OrderLineFulfillmentRequirement,
};
use rustok_product::CatalogService;
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter, Set,
    Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[path = "shipment_stock_provenance.rs"]
mod shipment_stock_provenance;
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
    let find_migration = |name: &str| {
        commerce_migrations::migrations()
            .into_iter()
            .find(|m| m.name() == name)
            .unwrap_or_else(|| panic!("migration {name} should exist"))
    };
    let delivery_guard =
        find_migration("m20260713_000008_require_fulfillment_before_order_delivery");
    let order_delivery = find_migration("m20260713_000006_consume_inventory_on_order_delivery");
    let reservation = find_migration("m20260713_000005_reserve_inventory_on_order_confirmation");
    reservation
        .up(&manager)
        .await
        .expect("order inventory reservation migration should install on SQLite");
    order_delivery
        .up(&manager)
        .await
        .expect("order inventory delivery consumption should install on SQLite");

    commerce_inventory_migration("m20261021_000024_create_inventory_return_restocks")
        .up(&manager)
        .await
        .expect("inventory return restock ledger should install on SQLite");

    let fulfillment_progress = fulfillment_migrations::migrations()
        .into_iter()
        .find(|m| m.name() == "m20260713_000110_serialize_fulfillment_progress")
        .expect("fulfillment progress serialization migration should exist");
    fulfillment_progress
        .up(&manager)
        .await
        .expect("fulfillment progress serialization should install on SQLite");
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

fn commerce_inventory_migration(name: &str) -> Box<dyn sea_orm_migration::MigrationTrait> {
    rustok_inventory::migrations::migrations()
        .into_iter()
        .find(|m| m.name() == name)
        .unwrap_or_else(|| panic!("inventory migration {name} should exist"))
}

async fn create_product_and_variant(catalog: &CatalogService, tenant_id: Uuid) -> (Uuid, Uuid) {
    let product = catalog
        .create_product(
            tenant_id,
            Uuid::new_v4(),
            CreateProductInput {
                attribute_values: Default::default(),
                translations: vec![ProductTranslationInput {
                    locale: "en".to_string(),
                    title: "Reserved product".to_string(),
                    description: None,
                    handle: Some(unique_slug("reserved-product")),
                    meta_title: None,
                    meta_description: None,
                }],
                variant_axes: vec![],
                variants: vec![CreateVariantInput {
                    sku: Some(format!("RESERVE-{}", Uuid::new_v4())),
                    barcode: None,
                    shipping_profile_slug: None,
                    axis_values: vec![],
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
                    fulfillment_requirement: OrderLineFulfillmentRequirement::Physical,
                    shipping_profile_slug: Some("default".to_string()),
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
        .execute_raw(Statement::from_sql_and_values(
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

    // Stock moves in the owner's transaction through the inventory-backed port; the former
    // `fulfillment_inventory_consume_on_ship` trigger is gone.
    let fulfillment_service = rustok_commerce::fulfillment_service(
        db.clone(),
        rustok_order::in_process_order_read_port(db.clone(), mock_transactional_event_bus()),
    );
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
            support::shipment_context(tenant_id),
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
            support::shipment_context(tenant_id),
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

#[tokio::test]
async fn checkout_order_inventory_lifecycle_guard_rejects_inventory_reserved_and_requires_order_created_with_adopted_ledger()
 {
    let (db, catalog, inventory, orders) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (product_id, variant_id) = create_product_and_variant(&catalog, tenant_id).await;

    inventory
        .set_inventory(tenant_id, actor_id, variant_id, 10)
        .await
        .expect("inventory should be stocked");

    // Install migration 000013 cutover checkout inventory lifecycle trigger
    let manager = SchemaManager::new(&db);
    let cutover = commerce_migrations::migrations()
        .into_iter()
        .find(|m| m.name() == "m20260713_000013_cutover_checkout_inventory_lifecycle")
        .expect("cutover migration should exist");
    cutover
        .up(&manager)
        .await
        .expect("cutover migration should apply on SQLite");

    let operation_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();

    // Create a pending order bound to checkout operation_id in metadata
    let order = orders
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
                    fulfillment_requirement: OrderLineFulfillmentRequirement::Physical,
                    shipping_profile_slug: Some("default".to_string()),
                    seller_id: None,
                    sku: Some("RESERVED-SKU".to_string()),
                    title: "Reserved product".to_string(),
                    quantity: 2,
                    unit_price: Decimal::new(1000, 2),
                    metadata: serde_json::json!({}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({
                    "checkout": {
                        "operation_id": operation_id.to_string(),
                    }
                }),
            },
        )
        .await
        .expect("pending order should be created");

    let now = chrono::Utc::now().fixed_offset();

    // Insert checkout operation at stage 'inventory_reserved' (Defect 1 scenario)
    use rustok_commerce::entities::{checkout_inventory_reservation, checkout_operation};

    let op = checkout_operation::ActiveModel {
        id: Set(operation_id),
        tenant_id: Set(tenant_id),
        cart_id: Set(cart_id),
        idempotency_key: Set("test-key".to_string()),
        request_hash: Set("req-hash".to_string()),
        snapshot_hash: Set(Some("snap-hash".to_string())),
        status: Set("executing".to_string()),
        execution_admission: Set("open".to_string()),
        admission_epoch: Set(1),
        stage: Set("inventory_reserved".to_string()),
        order_id: Set(Some(order.id)),
        payment_collection_id: Set(None),
        attempt_count: Set(1),
        lease_owner: Set(Some("test-worker".to_string())),
        lease_expires_at: Set(Some(now + chrono::Duration::seconds(60))),
        last_error_code: Set(None),
        last_error_message: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        completed_at: Set(None),
    };
    checkout_operation::Entity::insert(op)
        .exec(&db)
        .await
        .expect("operation inserted");

    // Attempting to confirm order directly from `inventory_reserved` must fail via DB trigger
    let confirm_err = orders.confirm_order(tenant_id, actor_id, order.id).await;
    assert!(
        confirm_err.is_err(),
        "confirming order on `inventory_reserved` MUST be rejected by DB trigger"
    );

    // Update operation to `order_created`, but without adopted inventory ledger
    checkout_operation::Entity::update_many()
        .col_expr(
            checkout_operation::Column::Stage,
            sea_orm::sea_query::Expr::value("order_created"),
        )
        .filter(checkout_operation::Column::Id.eq(operation_id))
        .exec(&db)
        .await
        .expect("operation stage updated");

    let confirm_no_ledger_err = orders.confirm_order(tenant_id, actor_id, order.id).await;
    assert!(
        confirm_no_ledger_err.is_err(),
        "confirming order without adopted inventory reservations MUST be rejected by DB trigger"
    );

    // Now insert the adopted reservation in both reservation_items (inventory) and checkout_inventory_reservations (commerce)
    let item = inventory_item::Entity::find()
        .filter(inventory_item::Column::VariantId.eq(variant_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    let level = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(item.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    let res_item_id = Uuid::new_v4();
    let reservation_record = reservation_item::ActiveModel {
        id: Set(res_item_id),
        inventory_item_id: Set(item.id),
        location_id: Set(level.location_id),
        quantity: Set(2),
        line_item_id: Set(Some(order.line_items[0].id)),
        description: Set(None),
        external_id: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
    };
    reservation_item::Entity::insert(reservation_record)
        .exec(&db)
        .await
        .unwrap();

    let cir = checkout_inventory_reservation::ActiveModel {
        reservation_id: Set(res_item_id),
        tenant_id: Set(tenant_id),
        checkout_operation_id: Set(operation_id),
        cart_line_item_id: Set(Uuid::new_v4()),
        order_line_item_id: Set(Some(order.line_items[0].id)),
        external_id: Set("ext-1".to_string()),
        variant_id: Set(variant_id),
        quantity: Set(2),
        location_id: Set(None),
        status: Set("reserved".to_string()),
        last_error_code: Set(None),
        last_error_message: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        released_at: Set(None),
        consumed_at: Set(None),
    };
    checkout_inventory_reservation::Entity::insert(cir)
        .exec(&db)
        .await
        .unwrap();

    // Now confirmation must succeed because stage is `order_created` and inventory is adopted!
    orders
        .confirm_order(tenant_id, actor_id, order.id)
        .await
        .expect(
            "order confirmation MUST succeed once stage is order_created and inventory is adopted",
        );
}

/// Orders, stocks and pays a 3-unit order, then creates (but does not ship) a fulfillment.
async fn paid_order_with_pending_fulfillment(
    db: &DatabaseConnection,
    catalog: &CatalogService,
    inventory: &InventoryService,
    orders: &OrderService,
    fulfillment_service: &FulfillmentService,
) -> (Uuid, Uuid, rustok_fulfillment::dto::FulfillmentResponse) {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (product_id, variant_id) = create_product_and_variant(catalog, tenant_id).await;
    inventory
        .set_inventory(tenant_id, actor_id, variant_id, 5)
        .await
        .expect("inventory should be stocked");
    let order = create_order(orders, tenant_id, actor_id, product_id, variant_id, 3).await;
    orders
        .confirm_order(tenant_id, actor_id, order.id)
        .await
        .expect("order should reserve inventory");
    orders
        .mark_paid(
            tenant_id,
            actor_id,
            order.id,
            "stock-port-payment".to_string(),
            "manual".to_string(),
        )
        .await
        .expect("confirmed order should become paid");
    assert_eq!(inventory_quantities(db, variant_id).await, (5, 3));
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
                metadata: serde_json::json!({"source":"stock-port-test"}),
            },
        )
        .await
        .expect("fulfillment should be created");
    (tenant_id, variant_id, fulfillment)
}

fn ship_all_input(fulfillment_item_id: Uuid) -> ShipFulfillmentInput {
    ShipFulfillmentInput {
        carrier: "manual".to_string(),
        tracking_number: "STOCK-PORT-SHIP".to_string(),
        items: Some(vec![FulfillmentItemQuantityInput {
            fulfillment_item_id,
            quantity: 3,
        }]),
        metadata: serde_json::json!({}),
    }
}

#[tokio::test]
async fn cancelling_a_shipped_fulfillment_returns_stock_and_reopening_takes_it_again() {
    let (db, catalog, inventory, orders) = setup().await;
    let service = rustok_commerce::fulfillment_service(
        db.clone(),
        rustok_order::in_process_order_read_port(db.clone(), mock_transactional_event_bus()),
    );
    let (tenant_id, variant_id, fulfillment) =
        paid_order_with_pending_fulfillment(&db, &catalog, &inventory, &orders, &service).await;
    let item_id = fulfillment.items[0].id;

    service
        .ship_fulfillment(
            support::shipment_context(tenant_id),
            fulfillment.id,
            ship_all_input(item_id),
        )
        .await
        .expect("shipping should consume the reservation");
    assert_eq!(inventory_quantities(&db, variant_id).await, (2, 0));

    service
        .cancel_fulfillment(
            support::shipment_context(tenant_id),
            fulfillment.id,
            CancelFulfillmentInput {
                reason: Some("lost in transit".to_string()),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("a shipped fulfillment can be cancelled");
    assert_eq!(
        inventory_quantities(&db, variant_id).await,
        (5, 0),
        "undelivered shipped units go back on the shelf"
    );

    let reopened = service
        .reopen_fulfillment(
            support::shipment_context(tenant_id),
            fulfillment.id,
            ReopenFulfillmentInput {
                items: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("a cancelled shipped fulfillment can be reopened");
    assert_eq!(reopened.status, "shipped");
    assert_eq!(
        inventory_quantities(&db, variant_id).await,
        (2, 0),
        "reopening takes the units off the shelf again"
    );
}

#[tokio::test]
async fn shipping_through_an_unwired_fulfillment_service_fails_closed() {
    let (db, catalog, inventory, orders) = setup().await;
    let wired = rustok_commerce::fulfillment_service(
        db.clone(),
        rustok_order::in_process_order_read_port(db.clone(), mock_transactional_event_bus()),
    );
    let (tenant_id, variant_id, fulfillment) =
        paid_order_with_pending_fulfillment(&db, &catalog, &inventory, &orders, &wired).await;

    let unwired = FulfillmentService::new(db.clone());
    let error = unwired
        .ship_fulfillment(
            support::shipment_context(tenant_id),
            fulfillment.id,
            ship_all_input(fulfillment.items[0].id),
        )
        .await
        .expect_err("a service without a stock port must refuse to ship");
    assert!(
        matches!(error, rustok_fulfillment::FulfillmentError::OwnerPort(cause)
        if cause.code == "fulfillment.shipment_stock_not_configured" && !cause.retryable)
    );
    assert_eq!(
        inventory_quantities(&db, variant_id).await,
        (5, 3),
        "the failed shipment must roll back completely"
    );
}

#[tokio::test]
async fn return_restock_adds_stock_once_and_skips_untracked_lines() {
    use rustok_inventory::return_restock::{ReturnRestockLine, restock_returned_items};

    let (db, catalog, inventory, orders) = setup().await;
    let service = rustok_commerce::fulfillment_service(
        db.clone(),
        rustok_order::in_process_order_read_port(db.clone(), mock_transactional_event_bus()),
    );
    let (tenant_id, variant_id, fulfillment) =
        paid_order_with_pending_fulfillment(&db, &catalog, &inventory, &orders, &service).await;
    service
        .ship_fulfillment(
            support::shipment_context(tenant_id),
            fulfillment.id,
            ship_all_input(fulfillment.items[0].id),
        )
        .await
        .expect("shipping should consume the reservation");
    assert_eq!(inventory_quantities(&db, variant_id).await, (2, 0));

    let line = ReturnRestockLine {
        return_item_id: Uuid::new_v4(),
        return_id: Uuid::new_v4(),
        order_line_item_id: fulfillment.items[0].order_line_item_id,
        variant_id: Some(variant_id),
        quantity: 2,
    };
    let untracked = ReturnRestockLine {
        return_item_id: Uuid::new_v4(),
        return_id: line.return_id,
        order_line_item_id: Uuid::new_v4(),
        variant_id: None,
        quantity: 1,
    };

    let first = restock_returned_items(
        &db,
        return_restock_context(tenant_id, line.return_id),
        &[line, untracked],
    )
    .await
    .expect("restock should succeed");
    assert_eq!(first.restocked, vec![(line.return_item_id, 2)]);
    assert_eq!(first.untracked, vec![untracked.return_item_id]);
    assert_eq!(
        inventory_quantities(&db, variant_id).await,
        (4, 0),
        "returned units are available again"
    );

    let replay = restock_returned_items(
        &db,
        return_restock_context(tenant_id, line.return_id),
        &[line],
    )
    .await
    .expect("replay should succeed");
    assert!(replay.restocked.is_empty());
    assert_eq!(replay.already_restocked, vec![line.return_item_id]);
    assert_eq!(
        inventory_quantities(&db, variant_id).await,
        (4, 0),
        "the ledger makes restock idempotent"
    );

    let invalid = ReturnRestockLine {
        quantity: 0,
        return_item_id: Uuid::new_v4(),
        ..line
    };
    assert!(
        restock_returned_items(
            &db,
            return_restock_context(tenant_id, line.return_id),
            &[invalid]
        )
        .await
        .is_err(),
        "non-positive restock quantities are rejected"
    );
}

fn return_restock_context(tenant_id: Uuid, return_id: Uuid) -> rustok_api::PortContext {
    rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::system(),
        "en",
        "inventory-return-regression",
    )
    .with_idempotency_key(format!("return:{return_id}:restock"))
    .with_deadline(std::time::Duration::from_secs(3))
}

struct ReturnStockFixture {
    db: DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    order_id: Uuid,
    line: rustok_inventory::return_restock::ReturnRestockLine,
}

async fn shipped_return_stock_fixture() -> ReturnStockFixture {
    let (db, catalog, inventory, orders) = setup().await;
    let service = rustok_commerce::fulfillment_service(
        db.clone(),
        rustok_order::in_process_order_read_port(db.clone(), mock_transactional_event_bus()),
    );
    let (tenant_id, variant_id, fulfillment) =
        paid_order_with_pending_fulfillment(&db, &catalog, &inventory, &orders, &service).await;
    service
        .ship_fulfillment(
            support::shipment_context(tenant_id),
            fulfillment.id,
            ship_all_input(fulfillment.items[0].id),
        )
        .await
        .expect("consume the saved shipment allocation");
    ReturnStockFixture {
        db,
        tenant_id,
        variant_id,
        order_id: fulfillment.order_id,
        line: rustok_inventory::return_restock::ReturnRestockLine {
            return_item_id: Uuid::new_v4(),
            return_id: Uuid::new_v4(),
            order_line_item_id: fulfillment.items[0].order_line_item_id,
            variant_id: Some(variant_id),
            quantity: 2,
        },
    }
}

async fn return_stock_receipts(
    f: &ReturnStockFixture,
) -> Vec<rustok_inventory::return_restock::ledger::Model> {
    use rustok_inventory::return_restock::ledger;
    ledger::Entity::find()
        .filter(ledger::Column::TenantId.eq(f.tenant_id))
        .filter(ledger::Column::ReturnId.eq(f.line.return_id))
        .all(&f.db)
        .await
        .expect("read tenant-scoped restock receipts")
}

#[tokio::test]
async fn return_restock_refuses_foreign_tenant_and_does_not_reclassify_missing_tracked_allocation()
{
    use rustok_inventory::return_restock::{ReturnRestockLine, restock_returned_items};
    let f = shipped_return_stock_fixture().await;
    let foreign = return_restock_context(Uuid::new_v4(), f.line.return_id);
    assert!(
        restock_returned_items(&f.db, foreign, &[f.line])
            .await
            .is_err()
    );
    let missing = ReturnRestockLine {
        order_line_item_id: Uuid::new_v4(),
        ..f.line
    };
    let error = restock_returned_items(
        &f.db,
        return_restock_context(f.tenant_id, f.line.return_id),
        &[missing],
    )
    .await
    .expect_err("a tracked missing allocation is not SKU-less");
    assert_eq!(error.code, "inventory.return_restock_allocation_unverified");
    assert_eq!(inventory_quantities(&f.db, f.variant_id).await, (2, 0));
    assert!(return_stock_receipts(&f).await.is_empty());
}

#[tokio::test]
async fn return_restock_refuses_ambiguous_consumed_allocations_instead_of_taking_the_oldest() {
    let f = shipped_return_stock_fixture().await;
    let saved = reservation_item::Entity::find()
        .filter(reservation_item::Column::LineItemId.eq(f.line.order_line_item_id))
        .one(&f.db)
        .await
        .expect("read allocation")
        .expect("saved allocation");
    let mut duplicate: reservation_item::ActiveModel = saved.into();
    duplicate.id = Set(Uuid::new_v4());
    duplicate.external_id = Set(None);
    reservation_item::Entity::insert(duplicate)
        .exec(&f.db)
        .await
        .expect("second consumed historical allocation");
    let error = rustok_inventory::return_restock::restock_returned_items(
        &f.db,
        return_restock_context(f.tenant_id, f.line.return_id),
        &[f.line],
    )
    .await
    .expect_err("allocation ambiguity refuses");
    assert_eq!(error.code, "inventory.return_restock_allocation_unverified");
    assert_eq!(inventory_quantities(&f.db, f.variant_id).await, (2, 0));
    assert!(return_stock_receipts(&f).await.is_empty());
}

#[tokio::test]
async fn return_restock_changed_replay_refuses_and_preserves_the_original_receipt() {
    use rustok_inventory::return_restock::{ReturnRestockLine, restock_returned_items};
    let f = shipped_return_stock_fixture().await;
    restock_returned_items(
        &f.db,
        return_restock_context(f.tenant_id, f.line.return_id),
        &[f.line],
    )
    .await
    .expect("initial restock");
    let before = return_stock_receipts(&f).await;
    for changed in [
        ReturnRestockLine {
            quantity: 1,
            ..f.line
        },
        ReturnRestockLine {
            return_id: Uuid::new_v4(),
            ..f.line
        },
        ReturnRestockLine {
            order_line_item_id: Uuid::new_v4(),
            ..f.line
        },
        ReturnRestockLine {
            variant_id: None,
            ..f.line
        },
    ] {
        assert!(
            restock_returned_items(
                &f.db,
                return_restock_context(f.tenant_id, changed.return_id),
                &[changed]
            )
            .await
            .is_err()
        );
    }
    assert_eq!(inventory_quantities(&f.db, f.variant_id).await, (4, 0));
    assert_eq!(return_stock_receipts(&f).await, before);
}

#[tokio::test]
async fn return_restock_exact_replay_uses_saved_receipt_without_reselecting_a_level_or_reservation()
{
    use rustok_inventory::entities::stock_location;
    use rustok_inventory::return_restock::restock_returned_items;
    let f = shipped_return_stock_fixture().await;
    restock_returned_items(
        &f.db,
        return_restock_context(f.tenant_id, f.line.return_id),
        &[f.line],
    )
    .await
    .expect("initial restock");
    let before = return_stock_receipts(&f).await;
    let saved = &before[0];
    // Fixture-only removal proves that exact replay does not require a replacement allocation.
    reservation_item::Entity::delete_many()
        .filter(reservation_item::Column::LineItemId.eq(f.line.order_line_item_id))
        .exec(&f.db)
        .await
        .expect("remove fixture reservation history");
    inventory_level::Entity::delete_many()
        .filter(inventory_level::Column::InventoryItemId.eq(saved.inventory_item_id))
        .filter(inventory_level::Column::LocationId.eq(saved.location_id))
        .exec(&f.db)
        .await
        .expect("remove fixture level");
    let location = stock_location::Entity::find_by_id(saved.location_id)
        .one(&f.db)
        .await
        .expect("read location")
        .expect("location");
    let mut archived: stock_location::ActiveModel = location.into();
    archived.deleted_at = Set(Some(chrono::Utc::now().into()));
    stock_location::Entity::update(archived)
        .exec(&f.db)
        .await
        .expect("archive saved fixture location");
    let replay = restock_returned_items(
        &f.db,
        return_restock_context(f.tenant_id, f.line.return_id),
        &[f.line],
    )
    .await
    .expect("exact saved receipt");
    assert_eq!(replay.already_restocked, vec![f.line.return_item_id]);
    assert!(replay.restocked.is_empty());
    assert_eq!(return_stock_receipts(&f).await, before);
}

#[tokio::test]
async fn return_restock_overflow_rolls_back_every_new_receipt_and_stock_addition() {
    use rustok_inventory::return_restock::{ReturnRestockLine, restock_returned_items};
    let f = shipped_return_stock_fixture().await;
    let first = ReturnRestockLine {
        return_item_id: Uuid::from_u128(1),
        quantity: i32::MAX,
        ..f.line
    };
    let second = ReturnRestockLine {
        return_item_id: Uuid::from_u128(2),
        quantity: 1,
        ..f.line
    };
    let error = restock_returned_items(
        &f.db,
        return_restock_context(f.tenant_id, f.line.return_id),
        &[first, second],
    )
    .await
    .expect_err("checked grouped addition overflows after receipt admissions");
    assert_eq!(error.code, "inventory.return_restock_quantity_overflow");
    assert_eq!(inventory_quantities(&f.db, f.variant_id).await, (2, 0));
    assert!(return_stock_receipts(&f).await.is_empty());
    let error = restock_returned_items(
        &f.db,
        return_restock_context(f.tenant_id, f.line.return_id),
        &[first],
    )
    .await
    .expect_err("checked on-hand addition also rolls back its receipt");
    assert_eq!(error.code, "inventory.return_restock_quantity_overflow");
    assert!(return_stock_receipts(&f).await.is_empty());
    assert_eq!(inventory_quantities(&f.db, f.variant_id).await, (2, 0));
}

#[tokio::test]
async fn return_restock_rejects_a_later_invalid_line_without_partial_batch_effects() {
    use rustok_inventory::return_restock::{ReturnRestockLine, restock_returned_items};
    let f = shipped_return_stock_fixture().await;
    let invalid = ReturnRestockLine {
        return_item_id: Uuid::new_v4(),
        order_line_item_id: Uuid::new_v4(),
        ..f.line
    };
    assert!(
        restock_returned_items(
            &f.db,
            return_restock_context(f.tenant_id, f.line.return_id),
            &[f.line, invalid]
        )
        .await
        .is_err()
    );
    assert_eq!(inventory_quantities(&f.db, f.variant_id).await, (2, 0));
    assert!(return_stock_receipts(&f).await.is_empty());
}

#[tokio::test]
async fn return_restock_global_primary_key_collision_never_adopts_a_foreign_tenant_receipt() {
    use rustok_inventory::return_restock::{ReturnRestockLine, restock_returned_items};
    let (db, catalog, inventory, orders) = setup().await;
    let service = rustok_commerce::fulfillment_service(
        db.clone(),
        rustok_order::in_process_order_read_port(db.clone(), mock_transactional_event_bus()),
    );
    let (tenant_a, variant_a, a) =
        paid_order_with_pending_fulfillment(&db, &catalog, &inventory, &orders, &service).await;
    let (tenant_b, variant_b, b) =
        paid_order_with_pending_fulfillment(&db, &catalog, &inventory, &orders, &service).await;
    for (tenant, fulfillment) in [(tenant_a, &a), (tenant_b, &b)] {
        service
            .ship_fulfillment(
                support::shipment_context(tenant),
                fulfillment.id,
                ship_all_input(fulfillment.items[0].id),
            )
            .await
            .expect("ship");
    }
    let line = ReturnRestockLine {
        return_item_id: Uuid::new_v4(),
        return_id: Uuid::new_v4(),
        order_line_item_id: a.items[0].order_line_item_id,
        variant_id: Some(variant_a),
        quantity: 1,
    };
    restock_returned_items(
        &db,
        return_restock_context(tenant_a, line.return_id),
        &[line],
    )
    .await
    .expect("tenant A restock");
    let conflicting = ReturnRestockLine {
        return_id: Uuid::new_v4(),
        order_line_item_id: b.items[0].order_line_item_id,
        variant_id: Some(variant_b),
        ..line
    };
    let error = restock_returned_items(
        &db,
        return_restock_context(tenant_b, conflicting.return_id),
        &[conflicting],
    )
    .await
    .expect_err("foreign key claim refuses");
    assert_eq!(error.code, "inventory.return_restock_identity_conflict");
    assert_eq!(inventory_quantities(&db, variant_a).await, (3, 0));
    assert_eq!(inventory_quantities(&db, variant_b).await, (2, 0));
}

#[tokio::test]
async fn order_owner_reads_only_requested_saved_inventory_bindings_and_never_a_foreign_or_partial_batch()
 {
    let f = shipped_return_stock_fixture().await;
    let port =
        rustok_order::in_process_order_read_port(f.db.clone(), mock_transactional_event_bus());
    let request = rustok_order::ReadOrderLineInventoryBindingsRequest {
        order_id: f.order_id,
        order_line_item_ids: vec![f.line.order_line_item_id],
    };
    let snapshot = port
        .read_order_line_inventory_bindings(
            return_restock_context(f.tenant_id, f.line.return_id),
            request.clone(),
        )
        .await
        .expect("saved binding");
    assert_eq!(snapshot.tenant_id, f.tenant_id);
    assert_eq!(snapshot.order_id, f.order_id);
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].variant_id, Some(f.variant_id));
    assert_eq!(snapshot.items[0].quantity, 3);
    assert!(
        port.read_order_line_inventory_bindings(
            return_restock_context(Uuid::new_v4(), f.line.return_id),
            request.clone()
        )
        .await
        .is_err()
    );
    let mut partial = request;
    partial.order_line_item_ids.push(Uuid::new_v4());
    assert!(
        port.read_order_line_inventory_bindings(
            return_restock_context(f.tenant_id, f.line.return_id),
            partial
        )
        .await
        .is_err()
    );
    assert_eq!(inventory_quantities(&f.db, f.variant_id).await, (2, 0));
}

#[tokio::test]
async fn concurrent_return_restock_keeps_one_receipt_and_one_stock_addition() {
    use rustok_inventory::return_restock::restock_returned_items;
    let f = shipped_return_stock_fixture().await;
    let lines = [f.line];
    let (left, right) = tokio::join!(
        restock_returned_items(
            &f.db,
            return_restock_context(f.tenant_id, f.line.return_id),
            &lines
        ),
        restock_returned_items(
            &f.db,
            return_restock_context(f.tenant_id, f.line.return_id),
            &lines
        ),
    );
    assert!(left.is_ok() || right.is_ok());
    for error in [left.err(), right.err()].into_iter().flatten() {
        assert!(
            error.retryable,
            "contention must not authorize a changed request: {error:?}"
        );
    }
    assert_eq!(return_stock_receipts(&f).await.len(), 1);
    assert_eq!(inventory_quantities(&f.db, f.variant_id).await, (4, 0));
    let replay = restock_returned_items(
        &f.db,
        return_restock_context(f.tenant_id, f.line.return_id),
        &lines,
    )
    .await
    .expect("same-identity replay");
    assert_eq!(replay.already_restocked, vec![f.line.return_item_id]);
}

#[tokio::test]
async fn archived_return_location_replays_exactly_but_refuses_new_or_mixed_stock() {
    use rustok_inventory::entities::stock_location;
    use rustok_inventory::return_restock::{ReturnRestockLine, restock_returned_items};

    let mut f = shipped_return_stock_fixture().await;
    f.line.quantity = 1; // Split the selected two units across replay and fresh inspection.
    // Install the actual storage rule, not an entity-only fixture approximation. This test
    // remains unrun; it defines the expected native refusal before a database-trigger error.
    commerce_inventory_migration(
        "m20260713_000021_bound_reservation_metadata_and_inactive_locations",
    )
    .up(&SchemaManager::new(&f.db))
    .await
    .expect("install the existing inactive-location storage guard");
    let reservation = reservation_item::Entity::find()
        .filter(reservation_item::Column::LineItemId.eq(f.line.order_line_item_id))
        .one(&f.db)
        .await
        .expect("read saved fixture allocation")
        .expect("saved allocation");
    let location = stock_location::Entity::find_by_id(reservation.location_id)
        .one(&f.db)
        .await
        .expect("read saved location")
        .expect("saved location");
    let mut hidden: stock_location::ActiveModel = location.into();
    hidden.metadata = Set(serde_json::json!({"allowed_channel_slugs": ["internal-only"]}));
    let hidden = stock_location::Entity::update(hidden)
        .exec(&f.db)
        .await
        .expect("hide the saved location from the caller's publication channel");
    restock_returned_items(
        &f.db,
        return_restock_context(f.tenant_id, f.line.return_id).with_channel("public-shop"),
        &[f.line],
    )
    .await
    .expect("saved active location does not require fresh channel visibility");
    let receipts_before = return_stock_receipts(&f).await;
    assert_eq!(receipts_before.len(), 1);
    let receipt = &receipts_before[0];
    assert_eq!(receipt.location_id, hidden.id);
    let level = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(receipt.inventory_item_id))
        .filter(inventory_level::Column::LocationId.eq(receipt.location_id))
        .one(&f.db)
        .await
        .expect("read saved level")
        .expect("saved level");
    // Fixture-only later depletion makes archival legal under the real storage guard.
    let mut depleted: inventory_level::ActiveModel = level.into();
    depleted.stocked_quantity = Set(0);
    let depleted = inventory_level::Entity::update(depleted)
        .exec(&f.db)
        .await
        .expect("deplete fixture stock before archival");
    let mut archived: stock_location::ActiveModel = hidden.into();
    archived.deleted_at = Set(Some(chrono::Utc::now().into()));
    let archived = stock_location::Entity::update(archived)
        .exec(&f.db)
        .await
        .expect("archive the empty saved location");

    let replay = restock_returned_items(
        &f.db,
        return_restock_context(f.tenant_id, f.line.return_id),
        &[f.line],
    )
    .await
    .expect("exact archived receipt still proves an earlier movement");
    assert_eq!(replay.already_restocked, vec![f.line.return_item_id]);
    let fresh = ReturnRestockLine {
        return_item_id: Uuid::new_v4(),
        ..f.line
    };
    // The mixed batch also proves that cached read-only replay ownership cannot bypass
    // the independent receiving-location lock and active-state preflight.
    for lines in [vec![fresh], vec![f.line, fresh]] {
        let error = restock_returned_items(
            &f.db,
            return_restock_context(f.tenant_id, f.line.return_id),
            &lines,
        )
        .await
        .expect_err("fresh archived-location movement requires reconciliation");
        assert_eq!(error.code, "inventory.return_restock_location_archived");
        assert_eq!(error.kind, rustok_api::PortErrorKind::InvariantViolation);
        assert!(!error.retryable);
        assert_eq!(return_stock_receipts(&f).await, receipts_before);
        assert_eq!(
            inventory_level::Entity::find_by_id(depleted.id)
                .one(&f.db)
                .await
                .expect("read unchanged level"),
            Some(depleted.clone()),
        );
        assert_eq!(
            stock_location::Entity::find_by_id(archived.id)
                .one(&f.db)
                .await
                .expect("read unchanged archived location"),
            Some(archived.clone()),
        );
    }
}
