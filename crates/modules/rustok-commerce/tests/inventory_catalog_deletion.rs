//! Source regressions included by the registered catalog_service_test target. Not executed here.

use super::{
    bind_test_category_attribute, create_test_product_input, seed_test_structural_category,
    seed_test_variant_axis, setup,
};
use chrono::Utc;
use rustok_inventory::entities::{
    inventory_item, inventory_level, reservation_item, stock_location,
};
use rustok_inventory::return_restock::ledger;
use rustok_inventory::{BootstrapService, CatalogDeletionBlocker, CatalogDeletionError};
use rustok_pricing::entities::price;
use rustok_product::dto::{VariantAxisInput, VariantAxisValueInput};
use rustok_product::entities::{product, product_variant};
use rustok_product::{CatalogService, CommerceError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    IntoActiveModel, QueryFilter, QueryOrder, Set, Statement, TransactionTrait,
};
use uuid::Uuid;

#[derive(Debug, PartialEq)]
struct Snapshot {
    products: Vec<product::Model>,
    variants: Vec<product_variant::Model>,
    items: Vec<inventory_item::Model>,
    levels: Vec<inventory_level::Model>,
    reservations: Vec<reservation_item::Model>,
    returns: Vec<ledger::Model>,
    prices: Vec<price::Model>,
    events: Vec<rustok_outbox::entity::Model>,
}

async fn snapshot(db: &DatabaseConnection) -> Snapshot {
    Snapshot {
        products: product::Entity::find()
            .order_by_asc(product::Column::Id)
            .all(db)
            .await
            .unwrap(),
        variants: product_variant::Entity::find()
            .order_by_asc(product_variant::Column::Id)
            .all(db)
            .await
            .unwrap(),
        items: inventory_item::Entity::find()
            .order_by_asc(inventory_item::Column::Id)
            .all(db)
            .await
            .unwrap(),
        levels: inventory_level::Entity::find()
            .order_by_asc(inventory_level::Column::Id)
            .all(db)
            .await
            .unwrap(),
        reservations: reservation_item::Entity::find()
            .order_by_asc(reservation_item::Column::Id)
            .all(db)
            .await
            .unwrap(),
        returns: ledger::Entity::find()
            .order_by_asc(ledger::Column::ReturnItemId)
            .all(db)
            .await
            .unwrap(),
        prices: price::Entity::find()
            .order_by_asc(price::Column::Id)
            .all(db)
            .await
            .unwrap(),
        events: rustok_outbox::SysEvents::find()
            .order_by_asc(rustok_outbox::entity::Column::Id)
            .all(db)
            .await
            .unwrap(),
    }
}

struct Fixture {
    tenant: Uuid,
    actor: Uuid,
    product: Uuid,
    variant: Uuid,
    item: Uuid,
    location: Uuid,
    level: Uuid,
}

async fn create_fixture(db: &DatabaseConnection, service: &CatalogService) -> Fixture {
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let created = service
        .create_product(tenant, actor, create_test_product_input())
        .await
        .unwrap();
    fixture_for_variant(db, tenant, actor, created.id, created.variants[0].id).await
}

async fn fixture_for_variant(
    db: &DatabaseConnection,
    tenant: Uuid,
    actor: Uuid,
    product: Uuid,
    variant: Uuid,
) -> Fixture {
    let item = inventory_item::Entity::find()
        .filter(inventory_item::Column::VariantId.eq(variant))
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let level = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(item.id))
        .one(db)
        .await
        .unwrap()
        .unwrap();
    Fixture {
        tenant,
        actor,
        product,
        variant,
        item: item.id,
        location: level.location_id,
        level: level.id,
    }
}

async fn pair(db: &DatabaseConnection, service: &CatalogService) -> (Fixture, Fixture) {
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let category = seed_test_structural_category(db, tenant, "deletion-pair").await;
    let (attribute, options) =
        seed_test_variant_axis(db, tenant, "deletion-size", &["small", "large"]).await;
    bind_test_category_attribute(db, tenant, category, attribute).await;
    let mut input = create_test_product_input();
    input.primary_category_id = Some(category);
    input.variant_axes = vec![VariantAxisInput {
        attribute_id: attribute,
        position: 0,
        allowed_option_ids: options.clone(),
    }];
    input.variants[0].axis_values = vec![VariantAxisValueInput {
        attribute_id: attribute,
        option_id: options[0],
    }];
    let mut second = input.variants[0].clone();
    second.sku = Some(format!("DELETE-{}", Uuid::new_v4()));
    second.axis_values = vec![VariantAxisValueInput {
        attribute_id: attribute,
        option_id: options[1],
    }];
    input.variants.push(second);
    let created = service.create_product(tenant, actor, input).await.unwrap();
    (
        fixture_for_variant(db, tenant, actor, created.id, created.variants[0].id).await,
        fixture_for_variant(db, tenant, actor, created.id, created.variants[1].id).await,
    )
}

async fn counters(
    db: &DatabaseConnection,
    fixture: &Fixture,
    stocked: i32,
    reserved: i32,
    incoming: i32,
) {
    let mut row = inventory_level::Entity::find_by_id(fixture.level)
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    row.stocked_quantity = Set(stocked);
    row.reserved_quantity = Set(reserved);
    row.incoming_quantity = Set(incoming);
    row.update(db).await.unwrap();
}

async fn assert_product_refused(
    db: &DatabaseConnection,
    service: &CatalogService,
    fixture: &Fixture,
    reason: CatalogDeletionBlocker,
) {
    let before = snapshot(db).await;
    let error = service
        .delete_product(fixture.tenant, fixture.actor, fixture.product)
        .await
        .unwrap_err();
    assert!(matches!(error, CommerceError::InventoryDeletionBlocked(actual) if actual == reason));
    assert_eq!(
        snapshot(db).await,
        before,
        "all local effects and existing event payloads must survive refusal"
    );
}

async fn reservation(
    db: &DatabaseConnection,
    fixture: &Fixture,
    quantity: i32,
    deleted: bool,
    metadata: serde_json::Value,
) {
    let now = Utc::now();
    reservation_item::ActiveModel {
        id: Set(Uuid::new_v4()),
        inventory_item_id: Set(fixture.item),
        location_id: Set(fixture.location),
        quantity: Set(quantity),
        line_item_id: Set(Some(Uuid::new_v4())),
        description: Set(Some("retained operator allocation".into())),
        external_id: Set(Some(format!("retained-{}", Uuid::new_v4()))),
        metadata: Set(metadata),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        deleted_at: Set(if deleted { Some(now.into()) } else { None }),
    }
    .insert(db)
    .await
    .unwrap();
}

async fn shipment_proof(
    db: &DatabaseConnection,
    tenant: Uuid,
    variant: Option<Uuid>,
    item: Option<Uuid>,
    location: Option<Uuid>,
) {
    // Test-only owner fixture: native proof has no cross-owner relation requiring an
    // invented Fulfillment lifecycle. All identities/counters satisfy its real CHECKs.
    let reservation = item.map(|_| Uuid::new_v4());
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO inventory_shipment_stock_proofs
         (fulfillment_item_id, tenant_id, order_id, fulfillment_id, order_line_item_id,
          variant_id, fulfillment_quantity, inventory_item_id, reservation_id, location_id,
          shipped_quantity, restored_quantity, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?, 1, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            Uuid::new_v4().into(),
            tenant.into(),
            Uuid::new_v4().into(),
            Uuid::new_v4().into(),
            Uuid::new_v4().into(),
            variant.into(),
            item.into(),
            reservation.into(),
            location.into(),
        ],
    ))
    .await
    .unwrap();
}

async fn proof_snapshot(db: &DatabaseConnection) -> Vec<String> {
    // Fixture-only scalar encoding with explicit NULLs, identities and timestamps;
    // no production reader or duplicate public proof entity is introduced.
    db.query_all_raw(Statement::from_string(DbBackend::Sqlite,
        "SELECT quote(fulfillment_item_id) || ':' || quote(tenant_id) || ':' || quote(order_id) || ':' ||
         quote(fulfillment_id) || ':' || quote(order_line_item_id) || ':' || quote(variant_id) || ':' ||
         quote(fulfillment_quantity) || ':' || quote(inventory_item_id) || ':' || quote(reservation_id) || ':' ||
         quote(location_id) || ':' || quote(shipped_quantity) || ':' || quote(restored_quantity) || ':' ||
         quote(created_at) || ':' || quote(updated_at) AS value
         FROM inventory_shipment_stock_proofs ORDER BY fulfillment_item_id"
    )).await.unwrap().into_iter().map(|row| row.try_get("", "value").unwrap()).collect()
}

async fn second_empty_level(db: &DatabaseConnection, fixture: &Fixture) -> Uuid {
    let mut location = stock_location::Entity::find_by_id(fixture.location)
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model()
        .reset_all();
    let id = Uuid::new_v4();
    location.id = Set(id);
    location.code = Set(Some(format!("empty-{id}")));
    location.insert(db).await.unwrap();
    let mut level = inventory_level::Entity::find_by_id(fixture.level)
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model()
        .reset_all();
    level.id = Set(Uuid::new_v4());
    level.location_id = Set(id);
    level.insert(db).await.unwrap();
    id
}

#[tokio::test]
async fn nonempty_individual_counters_and_net_zero_stock_refuse() {
    for (stocked, reserved, incoming) in [(1, 0, 0), (0, 1, 0), (0, 0, 1), (5, 5, 0)] {
        let (db, service) = setup().await;
        let fixture = create_fixture(&db, &service).await;
        counters(&db, &fixture, stocked, reserved, incoming).await;
        assert_product_refused(
            &db,
            &service,
            &fixture,
            CatalogDeletionBlocker::StockPresent,
        )
        .await;
    }
}

#[tokio::test]
async fn reservation_history_survives_zero_soft_deleted_and_terminal_rows() {
    for (quantity, deleted, metadata) in [
        (
            1,
            false,
            serde_json::json!({"source": "inventory_identity_port"}),
        ),
        (
            0,
            false,
            serde_json::json!({"released_at": false, "source": "inventory_identity_port"}),
        ),
        (
            0,
            true,
            serde_json::json!({"disposed_at": 0, "retained": "original history"}),
        ),
        (
            0,
            true,
            serde_json::json!({"source": "unrecognized historical source"}),
        ),
    ] {
        let (db, service) = setup().await;
        let fixture = create_fixture(&db, &service).await;
        reservation(&db, &fixture, quantity, deleted, metadata).await;
        assert_product_refused(
            &db,
            &service,
            &fixture,
            CatalogDeletionBlocker::HistoryPresent,
        )
        .await;
    }
}

#[tokio::test]
async fn shipment_history_protects_both_references_and_contradictory_tenant_labels() {
    for mode in 0..3 {
        let (db, service) = setup().await;
        let fixture = create_fixture(&db, &service).await;
        let tenant = if mode == 2 {
            Uuid::new_v4()
        } else {
            fixture.tenant
        };
        let variant = if mode == 1 {
            Uuid::new_v4()
        } else {
            fixture.variant
        };
        let item = if mode == 0 {
            Uuid::new_v4()
        } else {
            fixture.item
        };
        shipment_proof(
            &db,
            tenant,
            Some(variant),
            Some(item),
            Some(fixture.location),
        )
        .await;
        let saved = proof_snapshot(&db).await;
        assert_product_refused(
            &db,
            &service,
            &fixture,
            CatalogDeletionBlocker::HistoryPresent,
        )
        .await;
        assert_eq!(proof_snapshot(&db).await, saved);
    }
}

#[tokio::test]
async fn returned_stock_receipt_is_not_purge_permission_after_stock_reaches_zero() {
    for foreign_label in [false, true] {
        let (db, service) = setup().await;
        let fixture = create_fixture(&db, &service).await;
        ledger::ActiveModel {
            return_item_id: Set(Uuid::new_v4()),
            tenant_id: Set(if foreign_label {
                Uuid::new_v4()
            } else {
                fixture.tenant
            }),
            return_id: Set(Uuid::new_v4()),
            order_line_item_id: Set(Uuid::new_v4()),
            inventory_item_id: Set(fixture.item),
            location_id: Set(fixture.location),
            quantity: Set(1),
            created_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .unwrap();
        assert_product_refused(
            &db,
            &service,
            &fixture,
            CatalogDeletionBlocker::HistoryPresent,
        )
        .await;
    }
}

#[tokio::test]
async fn native_tenant_proof_precedes_any_other_owner_cleanup() {
    let (db, service) = setup().await;
    let fixture = create_fixture(&db, &service).await;
    let before = snapshot(&db).await;
    let txn = db.begin().await.unwrap();
    let error = BootstrapService::delete_records_for_variants_in_tx(
        &txn,
        Uuid::new_v4(),
        fixture.actor,
        &[fixture.variant],
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        CatalogDeletionError::Rejected(CatalogDeletionBlocker::IdentityUnavailable)
    ));
    txn.rollback().await.unwrap();
    assert_eq!(snapshot(&db).await, before);
}

#[tokio::test]
async fn missing_item_is_not_zero_stock_or_absence_proof() {
    let (db, service) = setup().await;
    let fixture = create_fixture(&db, &service).await;
    inventory_level::Entity::delete_by_id(fixture.level)
        .exec(&db)
        .await
        .unwrap();
    inventory_item::Entity::delete_by_id(fixture.item)
        .exec(&db)
        .await
        .unwrap();
    assert_product_refused(
        &db,
        &service,
        &fixture,
        CatalogDeletionBlocker::IdentityUnavailable,
    )
    .await;
}

#[tokio::test]
async fn foreign_original_location_refuses_instead_of_reselecting_default() {
    let (db, service) = setup().await;
    let fixture = create_fixture(&db, &service).await;
    let other = create_fixture(&db, &service).await;
    let mut row = inventory_level::Entity::find_by_id(fixture.level)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    row.location_id = Set(other.location);
    row.update(&db).await.unwrap();
    assert_product_refused(
        &db,
        &service,
        &fixture,
        CatalogDeletionBlocker::StateUnverified,
    )
    .await;
}

#[tokio::test]
async fn empty_archived_original_locations_are_not_reopened_or_deleted() {
    let (db, service) = setup().await;
    let fixture = create_fixture(&db, &service).await;
    let other_location = second_empty_level(&db, &fixture).await;
    let mut location = stock_location::Entity::find_by_id(fixture.location)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    location.deleted_at = Set(Some(Utc::now().into()));
    location.metadata = Set(serde_json::json!({"retained": "archived original"}));
    let saved = location.update(&db).await.unwrap();
    service
        .delete_product(fixture.tenant, fixture.actor, fixture.product)
        .await
        .unwrap();
    assert_eq!(
        stock_location::Entity::find_by_id(saved.id)
            .one(&db)
            .await
            .unwrap(),
        Some(saved)
    );
    assert!(
        stock_location::Entity::find_by_id(other_location)
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        inventory_item::Entity::find_by_id(fixture.item)
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        inventory_level::Entity::find()
            .filter(inventory_level::Column::InventoryItemId.eq(fixture.item))
            .all(&db)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn complete_product_child_graph_is_required_and_later_sku_refusal_preserves_it() {
    let (db, service) = setup().await;
    let (first, second) = pair(&db, &service).await;
    counters(&db, &second, 0, 0, 1).await;
    assert_product_refused(&db, &service, &first, CatalogDeletionBlocker::StockPresent).await;

    // Test-only corrupt graph: an older writer removed the catalog children. Even if
    // their empty Inventory rows survived, root deletion must not call an empty native
    // request and mistake that no-op for complete absence/cleanup proof.
    counters(&db, &second, 0, 0, 0).await;
    product_variant::Entity::delete_many()
        .filter(product_variant::Column::ProductId.eq(first.product))
        .exec(&db)
        .await
        .unwrap();
    let before = snapshot(&db).await;
    let error = service
        .delete_product(first.tenant, first.actor, first.product)
        .await
        .unwrap_err();
    assert!(matches!(error, CommerceError::NoVariants));
    assert_eq!(snapshot(&db).await, before);
}

#[tokio::test]
async fn standalone_variant_deletion_uses_the_same_history_guard() {
    let (db, service) = setup().await;
    let (first, second) = pair(&db, &service).await;
    reservation(&db, &first, 0, true, serde_json::json!({"released_at": 0})).await;
    let before = snapshot(&db).await;
    let error = service
        .delete_variant(first.tenant, first.actor, first.variant)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        CommerceError::InventoryDeletionBlocked(CatalogDeletionBlocker::HistoryPresent)
    ));
    assert_eq!(snapshot(&db).await, before);
    assert!(
        inventory_item::Entity::find_by_id(second.item)
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn empty_standalone_variant_delete_preserves_sibling_and_last_sku_rule() {
    let (db, service) = setup().await;
    let (first, second) = pair(&db, &service).await;
    let sibling = inventory_item::Entity::find_by_id(second.item)
        .one(&db)
        .await
        .unwrap();
    let sibling_level = inventory_level::Entity::find_by_id(second.level)
        .one(&db)
        .await
        .unwrap();
    service
        .delete_variant(first.tenant, first.actor, first.variant)
        .await
        .unwrap();
    assert_eq!(
        inventory_item::Entity::find_by_id(second.item)
            .one(&db)
            .await
            .unwrap(),
        sibling
    );
    assert_eq!(
        inventory_level::Entity::find_by_id(second.level)
            .one(&db)
            .await
            .unwrap(),
        sibling_level
    );
    assert!(
        product_variant::Entity::find_by_id(first.variant)
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );
    let before = snapshot(&db).await;
    let error = service
        .delete_variant(second.tenant, second.actor, second.variant)
        .await
        .unwrap_err();
    assert!(matches!(error, CommerceError::CannotDeleteOnlyVariant));
    assert_eq!(snapshot(&db).await, before);
}

#[tokio::test]
async fn exact_one_failure_rolls_back_prior_levels_prices_and_catalog_cleanup() {
    for trigger in [
        "CREATE TRIGGER refuse_delete BEFORE DELETE ON inventory_levels
         WHEN (SELECT COUNT(*) FROM inventory_levels WHERE inventory_item_id = OLD.inventory_item_id) = 1
         BEGIN SELECT RAISE(IGNORE); END",
        "CREATE TRIGGER refuse_delete BEFORE DELETE ON inventory_items BEGIN SELECT RAISE(IGNORE); END",
        "CREATE TRIGGER refuse_delete BEFORE DELETE ON product_variants BEGIN SELECT RAISE(IGNORE); END",
        "CREATE TRIGGER refuse_delete BEFORE DELETE ON products BEGIN SELECT RAISE(IGNORE); END",
    ] {
        let (db, service) = setup().await;
        let fixture = create_fixture(&db, &service).await;
        second_empty_level(&db, &fixture).await;
        db.execute_raw(Statement::from_string(DbBackend::Sqlite, trigger)).await.unwrap();
        assert_product_refused(&db, &service, &fixture, CatalogDeletionBlocker::ConcurrentChange).await;
    }
}

#[tokio::test]
async fn missing_history_storage_is_not_assumed_empty() {
    let (db, service) = setup().await;
    let fixture = create_fixture(&db, &service).await;
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "DROP TABLE inventory_shipment_stock_proofs",
    ))
    .await
    .unwrap();
    let before = snapshot(&db).await;
    let error = service
        .delete_product(fixture.tenant, fixture.actor, fixture.product)
        .await
        .unwrap_err();
    assert!(matches!(&error, CommerceError::Database(_)));
    let public =
        rustok_product::map_product_public_error(&error, "delete_product", "deletion_test");
    assert!(public.retryable);
    assert_eq!(public.code, "PRODUCT_TEMPORARILY_UNAVAILABLE");
    assert!(!public.message.contains("inventory_shipment_stock_proofs"));
    assert_eq!(snapshot(&db).await, before);
}

#[tokio::test]
async fn unrelated_or_explicit_sku_less_proof_does_not_block_empty_sku() {
    let (db, service) = setup().await;
    let fixture = create_fixture(&db, &service).await;
    shipment_proof(
        &db,
        Uuid::new_v4(),
        Some(Uuid::new_v4()),
        Some(Uuid::new_v4()),
        Some(Uuid::new_v4()),
    )
    .await;
    shipment_proof(&db, fixture.tenant, None, None, None).await;
    let saved = proof_snapshot(&db).await;
    service
        .delete_product(fixture.tenant, fixture.actor, fixture.product)
        .await
        .unwrap();
    assert_eq!(proof_snapshot(&db).await, saved);
}

#[tokio::test]
async fn stock_refusal_is_receipt_replayed_even_after_independent_stock_reconciliation() {
    use rustok_product::ProductCatalogCommandPort;
    let (db, service) = setup().await;
    let fixture = create_fixture(&db, &service).await;
    counters(&db, &fixture, 1, 0, 0).await;
    let context = super::catalog_receipt_context(fixture.tenant, fixture.actor, "blocked-delete");
    let first =
        ProductCatalogCommandPort::delete_product(&service, context.clone(), fixture.product)
            .await
            .unwrap_err();
    assert_eq!(first.code, "product.inventory_deletion_stock_present");
    assert!(rustok_product::product_catalog_command_error_is_terminal(
        &first
    ));
    counters(&db, &fixture, 0, 0, 0).await;
    let before = snapshot(&db).await;
    let replay = ProductCatalogCommandPort::delete_product(&service, context, fixture.product)
        .await
        .unwrap_err();
    assert_eq!(
        first, replay,
        "the original command must not silently become a fresh delete"
    );
    assert_eq!(snapshot(&db).await, before);
    assert!(
        product::Entity::find_by_id(fixture.product)
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );
}
