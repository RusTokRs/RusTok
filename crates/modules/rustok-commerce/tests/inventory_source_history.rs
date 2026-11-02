//! Definitions only: real owner/caller regressions, not executed by the agent.
use super::{create_test_product_input, setup};
use rustok_api::{Permission, PortActor, PortContext};
use rustok_events::{ContractEventEnvelope, ContractEventPayload, InventorySourceEvent};
use rustok_inventory::{
    InventoryService,
    entities::inventory_level,
    source_history::{
        InventoryBaselinePageRequest as Page, InventorySourceError as E, InventorySourceHistory,
        InventorySourceRecord,
    },
};
use rustok_product::{
    CatalogService,
    dto::{UpdateVariantInput, VariantAxisInput, VariantAxisValueInput},
};
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    Statement,
};
use std::time::Duration;
use uuid::Uuid;

struct Fixture {
    db: DatabaseConnection,
    catalog: CatalogService,
    context: PortContext,
    tenant: Uuid,
    actor: Uuid,
    product: Uuid,
    variant: Uuid,
    baseline: Uuid,
}

fn context(tenant: Uuid, actor: Uuid) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::user(actor.to_string()),
        "en",
        "source-history-regression",
    )
    .with_claim(Permission::INVENTORY_MANAGE.to_string())
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key("diagnostic-key-is-not-outcome")
}

async fn fixture(stock: i32) -> Fixture {
    let (db, catalog) = setup().await;
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let mut input = create_test_product_input();
    input.variants[0].inventory_quantity = stock;
    let product = catalog.create_product(tenant, actor, input).await.unwrap();
    Fixture {
        db,
        catalog,
        context: context(tenant, actor),
        tenant,
        actor,
        product: product.id,
        variant: product.variants[0].id,
        baseline: Uuid::new_v4(),
    }
}

fn history(f: &Fixture) -> InventorySourceHistory {
    InventorySourceHistory::new(f.db.clone())
}
fn inventory(f: &Fixture) -> InventoryService {
    InventoryService::new(f.db.clone(), mock_transactional_event_bus())
}
fn page(f: &Fixture) -> Page {
    Page {
        baseline_id: f.baseline,
        page_id: Uuid::new_v4(),
        after_variant_id: None,
        limit: 128,
    }
}
async fn activate(f: &Fixture) {
    history(f)
        .start_baseline(&f.context, f.baseline)
        .await
        .unwrap();
    assert!(
        history(f)
            .advance_baseline(&f.context, page(f))
            .await
            .unwrap()
            .active
    );
}
async fn row(f: &Fixture) -> InventorySourceRecord {
    history(f)
        .load(&f.context, f.baseline, &[f.variant])
        .await
        .unwrap()
        .remove(0)
}
async fn sql(f: &Fixture, text: &str) {
    f.db.execute_raw(Statement::from_string(DbBackend::Sqlite, text))
        .await
        .unwrap();
}
async fn count(f: &Fixture, table: &str) -> i64 {
    f.db.query_one_raw(Statement::from_string(
        DbBackend::Sqlite,
        format!("SELECT COUNT(*) AS n FROM {table}"),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "n")
    .unwrap()
}

#[tokio::test]
async fn source_history_never_automatically_baselines_existing_inventory() {
    let f = fixture(5).await;
    assert!(
        history(&f)
            .baseline_status(&f.context)
            .await
            .unwrap()
            .is_none()
    );
    inventory(&f)
        .set_inventory(f.tenant, f.actor, f.variant, 6)
        .await
        .unwrap();
    assert_eq!(count(&f, "inventory_source_baselines").await, 0);
    assert_eq!(count(&f, "inventory_source_records").await, 0);
    assert_eq!(count(&f, "inventory_source_changes").await, 0);
    assert!(matches!(
        history(&f).load(&f.context, f.baseline, &[f.variant]).await,
        Err(E::BaselineUnavailable)
    ));
}

#[tokio::test]
async fn source_history_paged_baseline_fences_changes_and_replays_original_page_receipts() {
    let f = fixture(5).await;
    let mut input = create_test_product_input();
    input.variants[0].inventory_quantity = 4;
    f.catalog
        .create_product(f.tenant, f.actor, input)
        .await
        .unwrap();
    let started = history(&f)
        .start_baseline(&f.context, f.baseline)
        .await
        .unwrap();
    assert!(!started.active);
    let first = Page {
        limit: 1,
        ..page(&f)
    };
    let first_result = history(&f)
        .advance_baseline(&f.context, first.clone())
        .await
        .unwrap();
    assert!(!first_result.active);
    assert!(
        inventory(&f)
            .set_inventory(f.tenant, f.actor, f.variant, 9)
            .await
            .is_err()
    );
    assert_eq!(
        inventory(&f)
            .set_inventory(f.tenant, f.actor, f.variant, 5)
            .await
            .unwrap(),
        5
    );
    assert!(matches!(
        history(&f).load(&f.context, f.baseline, &[f.variant]).await,
        Err(E::BaselineUnavailable)
    ));
    let second = Page {
        after_variant_id: first_result.after_variant_id,
        ..page(&f)
    };
    let completed = history(&f)
        .advance_baseline(&f.context, second)
        .await
        .unwrap();
    assert!(completed.active);
    assert_eq!(completed.record_count, 2);
    let n = count(&f, "inventory_source_changes").await;
    assert_eq!(
        history(&f)
            .advance_baseline(&f.context, first.clone())
            .await
            .unwrap(),
        first_result
    );
    assert_eq!(count(&f, "inventory_source_changes").await, n);
    let changed_request = Page {
        after_variant_id: completed.after_variant_id,
        ..first
    };
    assert!(matches!(
        history(&f)
            .advance_baseline(&f.context, changed_request)
            .await,
        Err(E::Conflict)
    ));
    assert_eq!(row(&f).await.available, 5);
}

#[tokio::test]
async fn source_history_empty_baseline_is_explicit_and_unknown_load_is_never_tombstone() {
    let (db, _) = setup().await;
    let h = InventorySourceHistory::new(db);
    let c = context(Uuid::new_v4(), Uuid::new_v4());
    let id = Uuid::new_v4();
    assert!(h.baseline_status(&c).await.unwrap().is_none());
    h.start_baseline(&c, id).await.unwrap();
    let p = h
        .advance_baseline(
            &c,
            Page {
                baseline_id: id,
                page_id: Uuid::new_v4(),
                after_variant_id: None,
                limit: 1,
            },
        )
        .await
        .unwrap();
    assert!(p.active);
    assert_eq!(p.record_count, 0);
    assert!(h.scan(&c, id, None, 1).await.unwrap().records.is_empty());
    assert!(matches!(
        h.load(&c, id, &[Uuid::new_v4()]).await,
        Err(E::StateUnverified)
    ));
}

#[tokio::test]
async fn source_history_native_and_product_changes_advance_but_stock_noops_do_not() {
    let f = fixture(5).await;
    activate(&f).await;
    let original = row(&f).await;
    assert_eq!(original.source_version, 1);
    inventory(&f)
        .set_inventory(f.tenant, f.actor, f.variant, 5)
        .await
        .unwrap();
    assert_eq!(row(&f).await, original);
    inventory(&f)
        .adjust_variant_quantity(f.tenant, f.actor, f.variant, 2, None)
        .await
        .unwrap();
    assert_eq!(row(&f).await.source_version, 2);
    f.catalog
        .update_variant(
            f.tenant,
            f.actor,
            f.variant,
            UpdateVariantInput {
                inventory_quantity: Some(9),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let current = row(&f).await;
    assert_eq!(current.source_version, 3);
    assert_eq!(current.available, 9);
    let changes = history(&f)
        .changes(&f.context, f.baseline, f.variant, 0, 128)
        .await
        .unwrap();
    assert_eq!(
        changes.iter().map(|c| c.source_version).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

#[tokio::test]
async fn source_history_canonical_outbox_failure_rolls_back_stock_revision_and_journal() {
    let f = fixture(5).await;
    activate(&f).await;
    let original = row(&f).await;
    sql(&f, "DROP TABLE sys_events").await;
    assert!(
        inventory(&f)
            .set_inventory(f.tenant, f.actor, f.variant, 9)
            .await
            .is_err()
    );
    assert_eq!(row(&f).await, original);
    assert_eq!(count(&f, "inventory_source_changes").await, 1);
}

#[tokio::test]
async fn source_history_ignored_revision_or_journal_effect_refuses_entire_domain_write() {
    for trigger in [
        "CREATE TRIGGER ignore_source_update BEFORE UPDATE ON inventory_source_records BEGIN SELECT RAISE(IGNORE); END",
        "CREATE TRIGGER ignore_source_change BEFORE INSERT ON inventory_source_changes BEGIN SELECT RAISE(IGNORE); END",
    ] {
        let f = fixture(5).await;
        activate(&f).await;
        let original = row(&f).await;
        sql(&f, trigger).await;
        assert!(
            inventory(&f)
                .set_inventory(f.tenant, f.actor, f.variant, 9)
                .await
                .is_err()
        );
        assert_eq!(row(&f).await, original);
        let level = inventory_level::Entity::find()
            .filter(inventory_level::Column::InventoryItemId.eq(original.inventory_item_id))
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(level.stocked_quantity, 5);
        assert_eq!(count(&f, "inventory_source_changes").await, 1);
    }
}

#[tokio::test]
async fn source_history_guarded_product_delete_retains_higher_version_original_tombstone() {
    let f = fixture(0).await;
    activate(&f).await;
    let original = row(&f).await;
    f.catalog
        .delete_product(f.tenant, f.actor, f.product)
        .await
        .unwrap();
    let tombstone = row(&f).await;
    assert!(tombstone.deleted);
    assert_eq!(tombstone.source_version, 2);
    assert_eq!(tombstone.inventory_item_id, original.inventory_item_id);
    assert_eq!(tombstone.product_id, original.product_id);
    let changes = history(&f)
        .changes(&f.context, f.baseline, f.variant, 0, 128)
        .await
        .unwrap();
    assert_eq!(changes.last().unwrap().kind, "deleted");
    assert!(
        history(&f)
            .load(&f.context, f.baseline, &[Uuid::new_v4()])
            .await
            .is_err()
    );
    // An empty replacement binding is not the original source identity. The pre-delete proof
    // refuses before physical deletes; the deliberately corrupted fixture remains untouched.
    let f = fixture(0).await;
    activate(&f).await;
    let original = row(&f).await;
    let events = count(&f, "sys_events").await;
    let replacement = Uuid::new_v4();
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE inventory_levels SET id = ? WHERE inventory_item_id = ?",
        vec![replacement.into(), original.inventory_item_id.into()],
    ))
    .await
    .unwrap();
    assert!(
        f.catalog
            .delete_product(f.tenant, f.actor, f.product)
            .await
            .is_err()
    );
    assert_eq!(row(&f).await, original);
    assert_eq!(count(&f, "sys_events").await, events);
    assert!(
        inventory_level::Entity::find_by_id(replacement)
            .one(&f.db)
            .await
            .unwrap()
            .is_some()
    );
    assert!(f.catalog.get_product(f.tenant, f.product).await.is_ok());
}

#[tokio::test]
async fn source_history_fresh_product_creation_after_activation_starts_new_owner_identity_only() {
    let f = fixture(5).await;
    activate(&f).await;
    let mut input = create_test_product_input();
    input.variants[0].inventory_quantity = 7;
    let p = f
        .catalog
        .create_product(f.tenant, f.actor, input)
        .await
        .unwrap();
    let rows = history(&f)
        .load(&f.context, f.baseline, &[p.variants[0].id, f.variant])
        .await
        .unwrap();
    assert_eq!(rows[0].source_version, 1);
    assert_eq!(rows[0].available, 7);
    assert_eq!(rows[1].source_version, 1);
    let changes = history(&f)
        .changes(&f.context, f.baseline, p.variants[0].id, 0, 128)
        .await
        .unwrap();
    assert_eq!(changes[0].kind, "created");
}

#[tokio::test]
async fn source_history_missing_active_revision_is_not_silently_recreated() {
    let f = fixture(5).await;
    activate(&f).await;
    sql(&f, "DELETE FROM inventory_source_records").await;
    assert!(
        inventory(&f)
            .set_inventory(f.tenant, f.actor, f.variant, 9)
            .await
            .is_err()
    );
    assert_eq!(count(&f, "inventory_source_records").await, 0);
    assert_eq!(count(&f, "inventory_source_changes").await, 1);
    assert_eq!(
        inventory(&f)
            .set_inventory(f.tenant, f.actor, f.variant, 5)
            .await
            .unwrap(),
        5
    );
    assert!(
        history(&f)
            .load(&f.context, f.baseline, &[f.variant])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn source_history_ignored_page_checkpoint_or_receipt_keeps_building_without_partial_copy() {
    for trigger in [
        "CREATE TRIGGER ignore_page_checkpoint BEFORE UPDATE ON inventory_source_baselines WHEN NEW.last_page_id IS NOT NULL BEGIN SELECT RAISE(IGNORE); END",
        "CREATE TRIGGER ignore_page_receipt BEFORE INSERT ON inventory_source_baseline_pages BEGIN SELECT RAISE(IGNORE); END",
    ] {
        let f = fixture(5).await;
        history(&f)
            .start_baseline(&f.context, f.baseline)
            .await
            .unwrap();
        let before = history(&f).baseline_status(&f.context).await.unwrap();
        sql(&f, trigger).await;
        assert!(
            history(&f)
                .advance_baseline(&f.context, page(&f))
                .await
                .is_err()
        );
        assert_eq!(
            history(&f).baseline_status(&f.context).await.unwrap(),
            before
        );
        assert_eq!(count(&f, "inventory_source_records").await, 0);
        assert_eq!(count(&f, "inventory_source_changes").await, 0);
        assert_eq!(count(&f, "inventory_source_baseline_pages").await, 0);
    }
}

#[tokio::test]
async fn source_history_anonymous_reservation_and_release_advance_once_per_actual_effect() {
    let f = fixture(10).await;
    activate(&f).await;
    inventory(&f)
        .reserve(&f.context, f.variant, 3)
        .await
        .unwrap();
    let reserved = row(&f).await;
    assert_eq!(reserved.source_version, 2);
    assert_eq!(reserved.reserved, 3);
    inventory(&f)
        .reserve(&f.context, f.variant, 0)
        .await
        .unwrap();
    assert_eq!(row(&f).await, reserved);
    inventory(&f)
        .release_reservation_quantity(&f.context, f.variant, 1)
        .await
        .unwrap();
    let released = row(&f).await;
    assert_eq!(released.source_version, 3);
    assert_eq!(released.reserved, 2);
    inventory(&f)
        .release_reservation_quantity(&f.context, f.variant, 0)
        .await
        .unwrap();
    assert_eq!(row(&f).await, released);
}

// Canonical delivery continuation definitions; not a selected Distribution/Index consumer.
async fn source_facts(f: &Fixture) -> Vec<ContractEventEnvelope> {
    rustok_outbox::SysEvents::find()
        .filter(rustok_outbox::entity::Column::EventType.is_in([
            "inventory.source.changed",
            "inventory.source.baseline_activated",
        ]))
        .all(&f.db)
        .await
        .unwrap()
        .into_iter()
        .map(|saved| {
            let envelope: ContractEventEnvelope = serde_json::from_value(saved.payload).unwrap();
            envelope.validate_registered_schema().unwrap();
            assert_eq!(saved.id, envelope.id());
            assert_eq!(saved.event_type, envelope.event_type());
            assert_eq!(
                saved.schema_version,
                i16::try_from(envelope.schema_version()).unwrap()
            );
            envelope
        })
        .filter(|envelope| envelope.tenant_id() == f.tenant)
        .collect()
}

async fn assert_source_fact(f: &Fixture, record: &InventorySourceRecord, actor: Option<Uuid>) {
    let facts = source_facts(f).await;
    let fact = facts
        .iter()
        .find(|fact| fact.id() == record.change_id)
        .unwrap();
    assert_eq!(fact.actor_id(), actor);
    assert_eq!(fact.correlation_id(), record.change_id);
    assert_eq!(fact.causation_id(), None);
    assert_eq!(
        fact.payload().unwrap(),
        &ContractEventPayload::InventorySource(InventorySourceEvent::Changed {
            baseline_id: record.baseline_id,
            variant_id: record.variant_id,
            source_version: record.source_version,
        })
    );
    let head = history(f)
        .changes(
            &f.context,
            record.baseline_id,
            record.variant_id,
            record.source_version - 1,
            1,
        )
        .await
        .unwrap();
    assert_eq!(head.len(), 1);
    assert_eq!(head[0].change_id, fact.id());
    assert_eq!(head[0].source_version, record.source_version);
}

#[tokio::test]
async fn source_delivery_baseline_pages_publish_only_one_atomic_final_activation_and_replay_none() {
    let f = fixture(5).await;
    f.catalog
        .create_product(f.tenant, f.actor, create_test_product_input())
        .await
        .unwrap();
    history(&f)
        .start_baseline(&f.context, f.baseline)
        .await
        .unwrap();
    assert!(source_facts(&f).await.is_empty());
    let first = Page {
        limit: 1,
        ..page(&f)
    };
    let progress = history(&f)
        .advance_baseline(&f.context, first.clone())
        .await
        .unwrap();
    assert!(!progress.active);
    assert!(source_facts(&f).await.is_empty());
    let last = Page {
        after_variant_id: progress.after_variant_id,
        ..page(&f)
    };
    assert!(
        history(&f)
            .advance_baseline(&f.context, last.clone())
            .await
            .unwrap()
            .active
    );
    let facts = source_facts(&f).await;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].id(), f.baseline);
    assert_eq!(facts[0].actor_id(), Some(f.actor));
    assert_eq!(
        facts[0].payload().unwrap(),
        &ContractEventPayload::InventorySource(InventorySourceEvent::BaselineActivated {
            baseline_id: f.baseline
        })
    );
    history(&f)
        .advance_baseline(&f.context, first)
        .await
        .unwrap();
    history(&f)
        .advance_baseline(&f.context, last)
        .await
        .unwrap();
    assert_eq!(source_facts(&f).await, facts);
    assert_eq!(count(&f, "inventory_source_changes").await, 2);
}

#[tokio::test]
async fn source_delivery_empty_target_tenant_still_has_one_explicit_activation_without_fake_versions()
 {
    let mut f = fixture(0).await;
    // The Product fixture belongs to a different tenant; the target has no SKU to invent.
    f.tenant = Uuid::new_v4();
    f.context = context(f.tenant, f.actor);
    history(&f)
        .start_baseline(&f.context, f.baseline)
        .await
        .unwrap();
    let result = history(&f)
        .advance_baseline(&f.context, page(&f))
        .await
        .unwrap();
    assert!(result.active);
    assert_eq!(result.record_count, 0);
    assert_eq!(count(&f, "inventory_source_changes").await, 0);
    let facts = source_facts(&f).await;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].id(), f.baseline);
    assert!(matches!(
        facts[0].payload().unwrap(),
        ContractEventPayload::InventorySource(InventorySourceEvent::BaselineActivated { .. })
    ));
}

#[tokio::test]
async fn source_delivery_quantity_uses_current_actor_and_original_journal_uuid_but_noop_emits_none()
{
    let f = fixture(5).await;
    activate(&f).await;
    let actor = Uuid::new_v4();
    inventory(&f)
        .set_inventory(f.tenant, actor, f.variant, 6)
        .await
        .unwrap();
    let current = row(&f).await;
    assert_eq!(current.source_version, 2);
    assert_source_fact(&f, &current, Some(actor)).await;
    let before = source_facts(&f).await;
    inventory(&f)
        .set_inventory(f.tenant, Uuid::new_v4(), f.variant, 6)
        .await
        .unwrap();
    assert_eq!(row(&f).await, current);
    assert_eq!(source_facts(&f).await, before);
}

#[tokio::test]
async fn source_delivery_suppressed_changed_insert_rolls_back_quantity_and_guarded_delete() {
    for deletion in [false, true] {
        let f = fixture(0).await;
        activate(&f).await;
        let original = row(&f).await;
        let events = count(&f, "sys_events").await;
        sql(&f, "CREATE TRIGGER ignore_inventory_source_fact BEFORE INSERT ON sys_events WHEN NEW.event_type = 'inventory.source.changed' BEGIN SELECT RAISE(IGNORE); END").await;
        if deletion {
            assert!(
                f.catalog
                    .delete_product(f.tenant, f.actor, f.product)
                    .await
                    .is_err()
            );
        } else {
            assert!(
                inventory(&f)
                    .set_inventory(f.tenant, f.actor, f.variant, 9)
                    .await
                    .is_err()
            );
        }
        assert_eq!(row(&f).await, original);
        assert_eq!(count(&f, "inventory_source_changes").await, 1);
        assert_eq!(count(&f, "sys_events").await, events);
        assert!(
            rustok_product::entities::product::Entity::find_by_id(f.product)
                .one(&f.db)
                .await
                .unwrap()
                .is_some()
        );
        let level = inventory_level::Entity::find()
            .filter(inventory_level::Column::InventoryItemId.eq(original.inventory_item_id))
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(level.stocked_quantity, 0);
    }
}

#[tokio::test]
async fn source_delivery_activation_publication_failure_keeps_original_building_checkpoint_and_page()
 {
    let f = fixture(5).await;
    history(&f)
        .start_baseline(&f.context, f.baseline)
        .await
        .unwrap();
    let request = page(&f);
    sql(&f, "CREATE TRIGGER ignore_inventory_activation BEFORE INSERT ON sys_events WHEN NEW.event_type = 'inventory.source.baseline_activated' BEGIN SELECT RAISE(IGNORE); END").await;
    assert!(matches!(
        history(&f)
            .advance_baseline(&f.context, request.clone())
            .await,
        Err(E::PublicationUnavailable)
    ));
    let status = history(&f)
        .baseline_status(&f.context)
        .await
        .unwrap()
        .unwrap();
    assert!(!status.active);
    assert_eq!(status.record_count, 0);
    for table in [
        "inventory_source_records",
        "inventory_source_changes",
        "inventory_source_baseline_pages",
    ] {
        assert_eq!(count(&f, table).await, 0);
    }
    assert!(source_facts(&f).await.is_empty());
    sql(&f, "DROP TRIGGER ignore_inventory_activation").await;
    // Native owner returned after an awaited successful rollback; continue the original page.
    assert!(
        history(&f)
            .advance_baseline(&f.context, request)
            .await
            .unwrap()
            .active
    );
    assert_eq!(source_facts(&f).await[0].id(), f.baseline);
}

#[tokio::test]
async fn source_delivery_fresh_product_and_guarded_product_delete_forward_distinct_current_actors()
{
    let f = fixture(5).await;
    activate(&f).await;
    let actor = Uuid::new_v4();
    let product = f
        .catalog
        .create_product(f.tenant, actor, create_test_product_input())
        .await
        .unwrap();
    let created = history(&f)
        .load(&f.context, f.baseline, &[product.variants[0].id])
        .await
        .unwrap()
        .remove(0);
    assert_eq!(created.source_version, 1);
    assert_source_fact(&f, &created, Some(actor)).await;
    let deleting_actor = Uuid::new_v4();
    f.catalog
        .delete_product(f.tenant, deleting_actor, product.id)
        .await
        .unwrap();
    let deleted = history(&f)
        .load(&f.context, f.baseline, &[created.variant_id])
        .await
        .unwrap()
        .remove(0);
    assert!(deleted.deleted);
    assert_eq!(deleted.source_version, 2);
    assert_eq!(deleted.inventory_item_id, created.inventory_item_id);
    assert_source_fact(&f, &deleted, Some(deleting_actor)).await;
}

#[tokio::test]
async fn source_delivery_fresh_variant_and_guarded_variant_delete_keep_unified_axis_identity() {
    let f = fixture(5).await;
    activate(&f).await;
    let category =
        super::seed_test_structural_category(&f.db, f.tenant, "source-delivery-axis").await;
    let (attribute, options) =
        super::seed_test_variant_axis(&f.db, f.tenant, "source-delivery-size", &["small", "large"])
            .await;
    super::bind_test_category_attribute(&f.db, f.tenant, category, attribute).await;
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
    second.sku = Some(format!("SOURCE-{}", Uuid::new_v4()));
    second.axis_values[0].option_id = options[1];
    let product = f
        .catalog
        .create_product(f.tenant, f.actor, input)
        .await
        .unwrap();
    let creating_actor = Uuid::new_v4();
    let variant = f
        .catalog
        .create_variant(f.tenant, creating_actor, product.id, second)
        .await
        .unwrap();
    let created = history(&f)
        .load(&f.context, f.baseline, &[variant.id])
        .await
        .unwrap()
        .remove(0);
    assert_source_fact(&f, &created, Some(creating_actor)).await;
    let deleting_actor = Uuid::new_v4();
    f.catalog
        .delete_variant(f.tenant, deleting_actor, variant.id)
        .await
        .unwrap();
    let deleted = history(&f)
        .load(&f.context, f.baseline, &[variant.id])
        .await
        .unwrap()
        .remove(0);
    assert!(deleted.deleted);
    assert_eq!(deleted.source_version, 2);
    assert_eq!(deleted.product_id, product.id);
    assert_source_fact(&f, &deleted, Some(deleting_actor)).await;
}

#[tokio::test]
async fn source_delivery_reservation_facts_share_owner_transaction_and_use_current_service_scope() {
    let f = fixture(5).await;
    activate(&f).await;
    let mut current = f.context.clone();
    current.actor = PortActor::service("source-delivery-current-service");
    inventory(&f).reserve(&current, f.variant, 2).await.unwrap();
    let reserved = row(&f).await;
    assert_eq!(reserved.source_version, 2);
    assert_source_fact(&f, &reserved, None).await;
    let before = source_facts(&f).await;
    inventory(&f).reserve(&current, f.variant, 0).await.unwrap();
    assert_eq!(source_facts(&f).await, before);
    inventory(&f)
        .release_reservation_quantity(&current, f.variant, 2)
        .await
        .unwrap();
    let released = row(&f).await;
    assert_eq!(released.source_version, 3);
    assert_source_fact(&f, &released, None).await;
    let reservations = rustok_outbox::SysEvents::find()
        .filter(rustok_outbox::entity::Column::EventType.eq("inventory.reservation.changed"))
        .all(&f.db)
        .await
        .unwrap();
    assert_eq!(reservations.len(), 2);
    for saved in reservations {
        let envelope: ContractEventEnvelope = serde_json::from_value(saved.payload).unwrap();
        assert_eq!(envelope.actor_id(), None);
        assert_eq!(
            serde_json::to_value(envelope).unwrap()["event"]["event"]["data"]
                .as_object()
                .unwrap()
                .len(),
            2
        );
    }
}

#[tokio::test]
async fn source_delivery_diagnostics_never_fabricate_missing_prior_activation_history() {
    let f = fixture(5).await;
    history(&f)
        .start_baseline(&f.context, f.baseline)
        .await
        .unwrap();
    let request = page(&f);
    history(&f)
        .advance_baseline(&f.context, request.clone())
        .await
        .unwrap();
    // Deliberately simulate an earlier binary that activated source state without publication.
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "DELETE FROM sys_events WHERE id = ?",
        vec![f.baseline.into()],
    ))
    .await
    .unwrap();
    assert!(
        history(&f)
            .baseline_status(&f.context)
            .await
            .unwrap()
            .unwrap()
            .active
    );
    assert!(
        history(&f)
            .start_baseline(&f.context, f.baseline)
            .await
            .unwrap()
            .active
    );
    assert!(
        history(&f)
            .advance_baseline(&f.context, request)
            .await
            .unwrap()
            .active
    );
    assert_eq!(row(&f).await.source_version, 1);
    assert!(source_facts(&f).await.is_empty());
}

#[tokio::test]
async fn source_delivery_baseline_service_scope_does_not_impersonate_its_creator_uuid() {
    let mut f = fixture(0).await;
    f.context.actor = PortActor::service(f.actor.to_string());
    activate(&f).await;
    let facts = source_facts(&f).await;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].id(), f.baseline);
    assert_eq!(facts[0].actor_id(), None);
}

#[tokio::test]
async fn source_delivery_fresh_bootstrap_cannot_commit_without_its_source_fact() {
    let f = fixture(0).await;
    activate(&f).await;
    let tables = [
        "products",
        "product_variants",
        "inventory_items",
        "inventory_levels",
        "inventory_source_records",
        "inventory_source_changes",
        "sys_events",
    ];
    let mut before = vec![];
    for table in tables {
        before.push(count(&f, table).await);
    }
    sql(&f, "CREATE TRIGGER ignore_inventory_created_source BEFORE INSERT ON sys_events WHEN NEW.event_type = 'inventory.source.changed' BEGIN SELECT RAISE(IGNORE); END").await;
    assert!(
        f.catalog
            .create_product(f.tenant, Uuid::new_v4(), create_test_product_input())
            .await
            .is_err()
    );
    let mut after = vec![];
    for table in tables {
        after.push(count(&f, table).await);
    }
    assert_eq!(after, before);
}
