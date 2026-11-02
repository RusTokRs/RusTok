//! Persistence regressions included by the registered inventory_service_test target.
//! They are source only in this continuation; the maintainer runs the test target.

use super::*;
use sea_orm::{ActiveModelTrait, Set};

struct ChannelStock {
    db: DatabaseConnection,
    service: InventoryService,
    catalog: CatalogService,
    tenant_id: Uuid,
    variant_id: Uuid,
    web_location_id: Uuid,
    mobile_location_id: Uuid,
}

async fn channel_stock() -> ChannelStock {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let (_, variant_id) = create_test_product(&catalog, tenant_id).await;
    service
        .set_inventory(tenant_id, Uuid::new_v4(), variant_id, 5)
        .await
        .unwrap();
    let item = entities::inventory_item::Entity::find()
        .filter(entities::inventory_item::Column::VariantId.eq(variant_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let web_level = entities::inventory_level::Entity::find()
        .filter(entities::inventory_level::Column::InventoryItemId.eq(item.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let location = entities::stock_location::Entity::find_by_id(web_level.location_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut web: entities::stock_location::ActiveModel = location.into();
    web.metadata =
        Set(serde_json::json!({"channel_visibility": {"allowed_channel_slugs": ["web"]}}));
    web.update(&db).await.unwrap();
    let mobile_location_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    entities::stock_location::ActiveModel {
        id: Set(mobile_location_id),
        tenant_id: Set(tenant_id),
        code: Set(Some("mobile".to_string())),
        metadata: Set(
            serde_json::json!({"channel_visibility": {"allowed_channel_slugs": ["mobile"]}}),
        ),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        deleted_at: Set(None),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    entities::inventory_level::ActiveModel {
        id: Set(Uuid::new_v4()),
        inventory_item_id: Set(item.id),
        location_id: Set(mobile_location_id),
        stocked_quantity: Set(100),
        reserved_quantity: Set(0),
        incoming_quantity: Set(0),
        low_stock_threshold: Set(None),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .unwrap();
    ChannelStock {
        db,
        service,
        catalog,
        tenant_id,
        variant_id,
        web_location_id: web_level.location_id,
        mobile_location_id,
    }
}

fn request(variant_id: Uuid, quantity: i32) -> InventoryIdentityReservationRequest {
    let reservation_id = Uuid::new_v4();
    InventoryIdentityReservationRequest {
        reservation_id,
        external_id: format!("channel-checkout:{reservation_id}"),
        variant_id,
        quantity,
        line_item_id: Some(Uuid::new_v4()),
        metadata: serde_json::json!({"allocated_channel_slug": "forged", "snapshot_hash": "immutable-test-snapshot"}),
    }
}

async fn reserved(stock: &ChannelStock, location_id: Uuid) -> i32 {
    entities::inventory_level::Entity::find()
        .filter(entities::inventory_level::Column::LocationId.eq(location_id))
        .one(&stock.db)
        .await
        .unwrap()
        .unwrap()
        .reserved_quantity
}

#[tokio::test]
async fn new_identity_reservation_uses_visible_stock_not_the_larger_hidden_location() {
    let stock = channel_stock().await;
    let port = in_process_inventory_reservation_identity_port(stock.db.clone());
    let command = request(stock.variant_id, 3);
    let result = port
        .reserve_inventory_by_identity(
            inventory_port_context(stock.tenant_id, "visible-reserve", true).with_channel("web"),
            command.clone(),
        )
        .await
        .unwrap();
    assert_eq!(result.variant_id, stock.variant_id);
    assert_eq!(result.location_id, stock.web_location_id);
    assert_eq!(result.reservation_id, command.reservation_id);
    assert_eq!(reserved(&stock, stock.web_location_id).await, 3);
    assert_eq!(reserved(&stock, stock.mobile_location_id).await, 0);
    let row = entities::reservation_item::Entity::find_by_id(command.reservation_id)
        .one(&stock.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.metadata["allocated_channel_slug"], "web");
    assert_eq!(row.metadata["snapshot_hash"], "immutable-test-snapshot");
}

#[tokio::test]
async fn global_scope_cannot_allocate_from_channel_only_locations() {
    let stock = channel_stock().await;
    let port = in_process_inventory_reservation_identity_port(stock.db.clone());
    let error = port
        .reserve_inventory_by_identity(
            inventory_port_context(stock.tenant_id, "global-reserve", true),
            request(stock.variant_id, 1),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, rustok_api::PortErrorKind::Conflict);
    assert_eq!(reserved(&stock, stock.web_location_id).await, 0);
    assert_eq!(reserved(&stock, stock.mobile_location_id).await, 0);
    assert_eq!(
        entities::reservation_item::Entity::find()
            .count(&stock.db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn matching_replay_and_release_keep_original_location_after_visibility_retirement() {
    let stock = channel_stock().await;
    let port = in_process_inventory_reservation_identity_port(stock.db.clone());
    let command = request(stock.variant_id, 3);
    let context =
        inventory_port_context(stock.tenant_id, "immutable-reserve", true).with_channel("web");
    let first = port
        .reserve_inventory_by_identity(context.clone(), command.clone())
        .await
        .unwrap();
    let location = entities::stock_location::Entity::find_by_id(stock.web_location_id)
        .one(&stock.db)
        .await
        .unwrap()
        .unwrap();
    let mut retired: entities::stock_location::ActiveModel = location.into();
    retired.deleted_at = Set(Some(chrono::Utc::now().into()));
    retired.update(&stock.db).await.unwrap();
    let replay = port
        .reserve_inventory_by_identity(context.clone(), command.clone())
        .await
        .unwrap();
    assert_eq!(replay.reservation_id, first.reservation_id);
    assert_eq!(replay.location_id, first.location_id);
    assert_eq!(replay.reserved_quantity, first.reserved_quantity);
    assert_eq!(reserved(&stock, stock.web_location_id).await, 3);
    assert_eq!(reserved(&stock, stock.mobile_location_id).await, 0);
    assert_eq!(
        entities::reservation_item::Entity::find()
            .count(&stock.db)
            .await
            .unwrap(),
        1
    );
    let release = InventoryIdentityReservationReleaseRequest {
        reservation_id: command.reservation_id,
        external_id: command.external_id,
    };
    assert_eq!(
        port.release_inventory_by_identity(context.clone(), release.clone())
            .await
            .unwrap()
            .released_quantity,
        3
    );
    assert_eq!(
        port.release_inventory_by_identity(context, release)
            .await
            .unwrap()
            .released_quantity,
        0
    );
    assert_eq!(reserved(&stock, stock.web_location_id).await, 0);
}

#[tokio::test]
async fn availability_uses_trusted_context_and_rejects_explicit_channel_contradiction() {
    let stock = channel_stock().await;
    let context =
        inventory_port_context(stock.tenant_id, "scoped-availability", false).with_channel("web");
    let result = InventoryReservationPort::check_availability(
        &stock.service,
        context.clone(),
        InventoryAvailabilityRequest {
            variant_id: stock.variant_id,
            requested_quantity: 6,
            channel_slug: None,
        },
    )
    .await
    .unwrap();
    assert!(
        !result.available,
        "hidden mobile stock cannot satisfy a web request"
    );
    let error = InventoryReservationPort::check_availability(
        &stock.service,
        context,
        InventoryAvailabilityRequest {
            variant_id: stock.variant_id,
            requested_quantity: 1,
            channel_slug: Some("mobile".to_string()),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "inventory.channel_mismatch");
}

#[tokio::test]
async fn backorder_does_not_authorize_a_hidden_location() {
    let stock = channel_stock().await;
    stock
        .catalog
        .update_variant(
            stock.tenant_id,
            Uuid::new_v4(),
            stock.variant_id,
            rustok_product::dto::UpdateVariantInput {
                inventory_policy: Some("continue".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let level = entities::inventory_level::Entity::find()
        .filter(entities::inventory_level::Column::LocationId.eq(stock.web_location_id))
        .one(&stock.db)
        .await
        .unwrap()
        .unwrap();
    let mut empty: entities::inventory_level::ActiveModel = level.into();
    empty.stocked_quantity = Set(0);
    empty.update(&stock.db).await.unwrap();
    let port = in_process_inventory_reservation_identity_port(stock.db.clone());
    let result = port
        .reserve_inventory_by_identity(
            inventory_port_context(stock.tenant_id, "backorder-reserve", true).with_channel("web"),
            request(stock.variant_id, 7),
        )
        .await
        .unwrap();
    assert_eq!(result.location_id, stock.web_location_id);
    assert_eq!(reserved(&stock, stock.web_location_id).await, 7);
    assert_eq!(reserved(&stock, stock.mobile_location_id).await, 0);
}
