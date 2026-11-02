//! Native F/I transaction and retained-provenance regressions. Included by the registered
//! order_inventory_reservation_test target; these definitions are not execution evidence.
use super::*;
use rustok_fulfillment::dto::FulfillmentResponse;
use rustok_inventory::return_restock::{ReturnRestockLine, restock_returned_items};
use sea_orm::{ActiveModelTrait, IntoActiveModel, sea_query::Expr};

#[path = "shipment_stock_provenance/stock_events.rs"]
mod stock_events;

struct Fixture {
    db: DatabaseConnection,
    service: FulfillmentService,
    tenant: Uuid,
    variant: Uuid,
    fulfillment: FulfillmentResponse,
}

impl Fixture {
    async fn new() -> Self {
        let (db, catalog, inventory, orders) = setup().await;
        let service = rustok_commerce::fulfillment_service(
            db.clone(),
            rustok_order::in_process_order_read_port(db.clone(), mock_transactional_event_bus()),
        );
        let (tenant, variant, fulfillment) =
            paid_order_with_pending_fulfillment(&db, &catalog, &inventory, &orders, &service).await;
        Self {
            db,
            service,
            tenant,
            variant,
            fulfillment,
        }
    }
    fn ship_input(&self, quantity: i32) -> ShipFulfillmentInput {
        let mut input = ship_all_input(self.fulfillment.items[0].id);
        input.items.as_mut().expect("explicit items")[0].quantity = quantity;
        input
    }
    async fn ship(
        &self,
        quantity: i32,
    ) -> rustok_fulfillment::error::FulfillmentResult<FulfillmentResponse> {
        self.service
            .ship_fulfillment(
                support::shipment_context(self.tenant),
                self.fulfillment.id,
                self.ship_input(quantity),
            )
            .await
    }
    async fn cancel(&self) -> rustok_fulfillment::error::FulfillmentResult<FulfillmentResponse> {
        self.service
            .cancel_fulfillment(
                support::shipment_context(self.tenant),
                self.fulfillment.id,
                CancelFulfillmentInput {
                    reason: Some("inspection".into()),
                    metadata: serde_json::json!({}),
                },
            )
            .await
    }
    async fn reopen(&self) -> rustok_fulfillment::error::FulfillmentResult<FulfillmentResponse> {
        self.service
            .reopen_fulfillment(
                support::shipment_context(self.tenant),
                self.fulfillment.id,
                ReopenFulfillmentInput {
                    items: None,
                    metadata: serde_json::json!({}),
                },
            )
            .await
    }
    async fn current(&self) -> FulfillmentResponse {
        self.service
            .get_fulfillment(self.tenant, self.fulfillment.id)
            .await
            .expect("owner facts")
    }
    async fn allocation(&self) -> reservation_item::Model {
        reservation_item::Entity::find()
            .filter(
                reservation_item::Column::LineItemId
                    .eq(self.fulfillment.items[0].order_line_item_id),
            )
            .one(&self.db)
            .await
            .expect("allocation query")
            .expect("retained allocation")
    }
    async fn stock(&self) -> (i32, i32) {
        inventory_quantities(&self.db, self.variant).await
    }
    async fn set_stock(&self, quantity: i32) {
        let allocation = self.allocation().await;
        inventory_level::Entity::update_many()
            .col_expr(
                inventory_level::Column::StockedQuantity,
                Expr::value(quantity),
            )
            .filter(inventory_level::Column::InventoryItemId.eq(allocation.inventory_item_id))
            .filter(inventory_level::Column::LocationId.eq(allocation.location_id))
            .exec(&self.db)
            .await
            .expect("stock fixture");
    }
    async fn proof(&self) -> Option<ProofState> {
        self.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "SELECT * FROM inventory_shipment_stock_proofs WHERE tenant_id = ? AND fulfillment_item_id = ?",
            vec![self.tenant.into(), self.fulfillment.items[0].id.into()])).await.expect("proof query").map(|row| ProofState {
                shipped: row.try_get("", "shipped_quantity").expect("shipped"), restored: row.try_get("", "restored_quantity").expect("restored"),
                reservation: row.try_get("", "reservation_id").expect("reservation"), location: row.try_get("", "location_id").expect("location"),
                created: row.try_get("", "created_at").expect("created"), updated: row.try_get("", "updated_at").expect("updated"),
            })
    }
    async fn remove_proof_to_simulate_unverified_history(&self) {
        self.db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "DELETE FROM inventory_shipment_stock_proofs WHERE tenant_id = ? AND fulfillment_item_id = ?",
            vec![self.tenant.into(), self.fulfillment.items[0].id.into()])).await.expect("unverified historical fixture, not a production repair");
    }
    fn returned_line(&self) -> ReturnRestockLine {
        ReturnRestockLine {
            return_id: Uuid::new_v4(),
            return_item_id: Uuid::new_v4(),
            order_line_item_id: self.fulfillment.items[0].order_line_item_id,
            variant_id: Some(self.variant),
            quantity: 1,
        }
    }
}

#[derive(Debug, PartialEq)]
struct ProofState {
    shipped: i32,
    restored: i32,
    reservation: Option<Uuid>,
    location: Option<Uuid>,
    created: chrono::DateTime<chrono::FixedOffset>,
    updated: chrono::DateTime<chrono::FixedOffset>,
}

#[tokio::test]
async fn shipment_partial_delivery_cancel_reopen_preserves_the_original_allocation_and_delivered_units()
 {
    let f = Fixture::new().await;
    let original = f.allocation().await;
    assert_eq!(f.stock().await, (5, 3));
    f.ship(2).await.expect("ship two");
    assert_eq!(f.stock().await, (3, 1));
    assert_eq!(f.proof().await.expect("proof").restored, 0);
    f.service
        .deliver_fulfillment(
            f.tenant,
            f.fulfillment.id,
            DeliverFulfillmentInput {
                delivered_note: None,
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: f.fulfillment.items[0].id,
                    quantity: 1,
                }]),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("deliver one of two shipped units");
    assert_eq!(
        f.stock().await,
        (3, 1),
        "delivery never moves physical stock"
    );
    let cancelled = f.cancel().await.expect("restore only the undelivered unit");
    assert_eq!(cancelled.items[0].delivered_quantity, 1);
    assert_eq!(f.stock().await, (4, 1));
    let restored = f.proof().await.expect("proof");
    assert_eq!((restored.shipped, restored.restored), (2, 1));
    assert_eq!(
        (restored.reservation, restored.location),
        (Some(original.id), Some(original.location_id))
    );
    let reopened = f
        .reopen()
        .await
        .expect("reconsume exactly the restored unit");
    assert_eq!(reopened.status, "shipped");
    assert_eq!(
        (
            reopened.items[0].shipped_quantity,
            reopened.items[0].delivered_quantity
        ),
        (2, 1)
    );
    assert_eq!(f.stock().await, (3, 1));
    let remaining = f.allocation().await;
    assert_eq!(
        (remaining.id, remaining.location_id, remaining.quantity),
        (original.id, original.location_id, 1)
    );
    assert!(remaining.deleted_at.is_none());
    let proof = f.proof().await.expect("proof");
    assert_eq!((proof.shipped, proof.restored), (2, 0));
    assert_eq!(proof.created, restored.created);
    f.cancel().await.expect("a new cancellation intent");
    f.reopen().await.expect("a new reopening intent");
    assert_eq!(
        f.stock().await,
        (3, 1),
        "repeated legitimate cycles neither mint nor lose units"
    );
}

#[tokio::test]
async fn shipment_missing_native_history_never_infers_consumption_from_a_zero_reservation() {
    let f = Fixture::new().await;
    f.ship(3).await.expect("native ship");
    let allocation = f.allocation().await;
    assert_eq!(allocation.quantity, 0);
    assert!(allocation.deleted_at.is_some());
    f.remove_proof_to_simulate_unverified_history().await;
    assert!(f.cancel().await.is_err());
    assert_eq!(f.stock().await, (2, 0));
    assert_eq!(f.current().await.status, "shipped");
    assert_eq!(f.allocation().await, allocation);
    assert!(f.proof().await.is_none(), "no historical backfill");
}

#[tokio::test]
async fn shipment_cancelled_status_is_not_a_reconsumption_receipt() {
    let f = Fixture::new().await;
    f.ship(3).await.expect("ship");
    f.cancel().await.expect("cancel");
    f.remove_proof_to_simulate_unverified_history().await;
    assert!(f.reopen().await.is_err());
    assert_eq!(f.stock().await, (5, 0));
    assert_eq!(f.current().await.status, "cancelled");
    assert!(f.proof().await.is_none());
}

#[tokio::test]
async fn shipment_missing_tracked_allocation_never_downgrades_to_sku_less() {
    let f = Fixture::new().await;
    reservation_item::Entity::delete_by_id(f.allocation().await.id)
        .exec(&f.db)
        .await
        .expect("missing allocation fixture");
    assert!(f.ship(1).await.is_err());
    assert_eq!(f.stock().await, (5, 3));
    assert_eq!(f.current().await.items[0].shipped_quantity, 0);
    assert!(f.proof().await.is_none());
}

#[tokio::test]
async fn shipment_ambiguous_retained_allocations_never_choose_the_oldest() {
    let f = Fixture::new().await;
    let mut duplicate = f.allocation().await.into_active_model();
    duplicate.id = Set(Uuid::new_v4());
    duplicate.quantity = Set(0);
    duplicate.deleted_at = Set(Some(chrono::Utc::now().into()));
    duplicate
        .insert(&f.db)
        .await
        .expect("ambiguous historical fixture");
    assert!(f.ship(1).await.is_err());
    assert_eq!(f.stock().await, (5, 3));
    assert_eq!(f.current().await.status, "pending");
    assert!(f.proof().await.is_none());
}

#[tokio::test]
async fn shipment_archived_original_location_does_not_relocate_to_another_active_level() {
    use rustok_inventory::entities::stock_location;
    let f = Fixture::new().await;
    let allocation = f.allocation().await;
    let mut alternative = stock_location::Entity::find_by_id(allocation.location_id)
        .one(&f.db)
        .await
        .expect("location query")
        .expect("saved location")
        .into_active_model();
    let alternative_id = Uuid::new_v4();
    alternative.id = Set(alternative_id);
    alternative.code = Set(Some("ALTERNATIVE".into()));
    alternative.insert(&f.db).await.expect("active alternative");
    let mut alternative_level = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(allocation.inventory_item_id))
        .filter(inventory_level::Column::LocationId.eq(allocation.location_id))
        .one(&f.db)
        .await
        .expect("level query")
        .expect("original level")
        .into_active_model();
    alternative_level.id = Set(Uuid::new_v4());
    alternative_level.location_id = Set(alternative_id);
    alternative_level.stocked_quantity = Set(99);
    alternative_level.reserved_quantity = Set(0);
    alternative_level
        .insert(&f.db)
        .await
        .expect("alternative stock");
    stock_location::Entity::update_many()
        .col_expr(
            stock_location::Column::DeletedAt,
            Expr::value(Some(chrono::Utc::now().fixed_offset())),
        )
        .filter(stock_location::Column::Id.eq(allocation.location_id))
        .exec(&f.db)
        .await
        .expect("archive saved location");
    let error = f.ship(1).await.expect_err("archived location must refuse");
    assert!(
        matches!(error, rustok_fulfillment::error::FulfillmentError::OwnerPort(cause) if cause.code == "inventory.shipment_stock_location_archived" && !cause.retryable)
    );
    assert_eq!(f.allocation().await, allocation);
    assert!(f.proof().await.is_none());
    let levels = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(allocation.inventory_item_id))
        .all(&f.db)
        .await
        .expect("levels");
    assert_eq!(
        levels.iter().map(|row| row.stocked_quantity).sum::<i32>(),
        104
    );
    assert_eq!(f.current().await.status, "pending");
}

#[tokio::test]
async fn shipment_reopening_refuses_insufficient_restored_units_without_releasing_other_reservations()
 {
    let f = Fixture::new().await;
    f.ship(3).await.expect("ship");
    f.cancel().await.expect("cancel");
    let allocation = f.allocation().await;
    let mut other = allocation.clone().into_active_model();
    let other_id = Uuid::new_v4();
    other.id = Set(other_id);
    other.line_item_id = Set(Some(Uuid::new_v4()));
    other.quantity = Set(3);
    other.deleted_at = Set(None);
    other.metadata = Set(serde_json::json!({}));
    other
        .insert(&f.db)
        .await
        .expect("another order's protected reservation");
    inventory_level::Entity::update_many()
        .col_expr(inventory_level::Column::ReservedQuantity, Expr::value(3))
        .filter(inventory_level::Column::InventoryItemId.eq(allocation.inventory_item_id))
        .filter(inventory_level::Column::LocationId.eq(allocation.location_id))
        .exec(&f.db)
        .await
        .expect("protected reserved level");
    let proof = f.proof().await;
    assert!(f.reopen().await.is_err());
    assert_eq!(f.stock().await, (5, 3));
    assert_eq!(
        reservation_item::Entity::find_by_id(other_id)
            .one(&f.db)
            .await
            .expect("other reservation")
            .expect("retained")
            .quantity,
        3
    );
    assert_eq!(f.proof().await, proof);
    assert_eq!(f.allocation().await, allocation);
    assert_eq!(f.current().await.status, "cancelled");
}

#[tokio::test]
async fn shipment_restoration_overflow_keeps_stock_proof_and_fulfillment_unchanged() {
    let f = Fixture::new().await;
    f.ship(3).await.expect("ship");
    f.set_stock(i32::MAX).await;
    let proof = f.proof().await;
    let allocation = f.allocation().await;
    assert!(f.cancel().await.is_err());
    assert_eq!(f.stock().await, (i32::MAX, 0));
    assert_eq!(f.proof().await, proof);
    assert_eq!(f.allocation().await, allocation);
    assert_eq!(f.current().await.status, "shipped");
}

#[tokio::test]
async fn shipment_return_restock_fact_blocks_cancellation_without_double_credit() {
    let f = Fixture::new().await;
    f.ship(3).await.expect("ship");
    f.service
        .deliver_fulfillment(
            f.tenant,
            f.fulfillment.id,
            DeliverFulfillmentInput {
                delivered_note: None,
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: f.fulfillment.items[0].id,
                    quantity: 1,
                }]),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("deliver one");
    let line = f.returned_line();
    restock_returned_items(
        &f.db,
        return_restock_context(f.tenant, line.return_id),
        &[line],
    )
    .await
    .expect("return delivered unit");
    assert_eq!(f.stock().await, (3, 0));
    let proof = f.proof().await;
    assert!(
        f.cancel().await.is_err(),
        "conservative line-wide overlap refusal"
    );
    assert_eq!(f.stock().await, (3, 0));
    assert_eq!(f.proof().await, proof);
    assert_eq!(f.current().await.status, "shipped");
}

#[tokio::test]
async fn shipment_outstanding_restoration_blocks_fresh_return_restock_without_a_new_receipt() {
    let f = Fixture::new().await;
    f.ship(3).await.expect("ship");
    f.cancel().await.expect("cancel");
    let line = f.returned_line();
    assert!(
        restock_returned_items(
            &f.db,
            return_restock_context(f.tenant, line.return_id),
            &[line]
        )
        .await
        .is_err()
    );
    assert_eq!(f.stock().await, (5, 0));
    assert_eq!(f.proof().await.expect("proof").restored, 3);
    let row =
        f.db.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT 1 FROM inventory_return_restocks WHERE return_item_id = ?",
            vec![line.return_item_id.into()],
        ))
        .await
        .expect("receipt query");
    assert!(row.is_none());
}

#[tokio::test]
async fn shipment_preparation_probes_global_proof_collisions_without_changing_foreign_history() {
    let f = Fixture::new().await;
    f.ship(1).await.expect("seed a valid physical proof");
    let foreign_tenant = Uuid::new_v4();
    // Corrupt/foreign historical identity is a refusal fixture, never an adoption/backfill path.
    f.db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE inventory_shipment_stock_proofs SET tenant_id = ? WHERE fulfillment_item_id = ?",
        vec![foreign_tenant.into(), f.fulfillment.items[0].id.into()],
    ))
    .await
    .expect("foreign proof fixture");
    let row_before =
        f.db.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT updated_at FROM inventory_shipment_stock_proofs WHERE fulfillment_item_id = ?",
            vec![f.fulfillment.items[0].id.into()],
        ))
        .await
        .expect("foreign timestamp")
        .expect("retained row");
    let timestamp: chrono::DateTime<chrono::FixedOffset> =
        row_before.try_get("", "updated_at").expect("timestamp");
    // A new pending item reuses the global proof key but has zero native F progress; a
    // scoped SELECT alone sees no proof. Preparation must exercise the global key fence.
    rustok_fulfillment::entities::fulfillment_item::Entity::update_many()
        .col_expr(
            rustok_fulfillment::entities::fulfillment_item::Column::ShippedQuantity,
            Expr::value(0),
        )
        .filter(
            rustok_fulfillment::entities::fulfillment_item::Column::Id
                .eq(f.fulfillment.items[0].id),
        )
        .exec(&f.db)
        .await
        .expect("zero progress fixture");
    let before = f.stock().await;
    assert!(
        f.service
            .prepare_shipment_stock(
                support::shipment_context(f.tenant),
                f.fulfillment.id,
                rustok_fulfillment::ShipmentStockAction::Ship,
                f.ship_input(1).items.as_deref()
            )
            .await
            .is_err()
    );
    assert_eq!(f.stock().await, before);
    let row = f.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite, "SELECT tenant_id, updated_at FROM inventory_shipment_stock_proofs WHERE fulfillment_item_id = ?", vec![f.fulfillment.items[0].id.into()])).await.expect("retained proof query").expect("foreign row preserved");
    assert_eq!(
        row.try_get::<Uuid>("", "tenant_id").expect("tenant"),
        foreign_tenant
    );
    assert_eq!(
        row.try_get::<chrono::DateTime<chrono::FixedOffset>>("", "updated_at")
            .expect("timestamp"),
        timestamp
    );
}

#[tokio::test]
async fn shipment_late_fulfillment_failure_rolls_back_stock_reservation_and_new_proof() {
    let f = Fixture::new().await;
    f.db.execute_raw(Statement::from_string(DbBackend::Sqlite, "CREATE TRIGGER fail_shipment_progress BEFORE UPDATE OF status ON fulfillments WHEN NEW.status = 'shipped' BEGIN SELECT RAISE(ABORT, 'shipment fixture refuses late progress'); END".to_string())).await.expect("late failure fixture");
    let allocation = f.allocation().await;
    assert!(f.ship(3).await.is_err());
    assert_eq!(f.stock().await, (5, 3));
    assert_eq!(f.allocation().await, allocation);
    assert!(f.proof().await.is_none());
    let current = f.current().await;
    assert_eq!(current.status, "pending");
    assert_eq!(current.items[0].shipped_quantity, 0);
}

#[tokio::test]
async fn shipment_successful_preparation_rolls_back_its_key_probe_without_stock_or_receipts() {
    let f = Fixture::new().await;
    let allocation = f.allocation().await;
    let plan = f
        .service
        .prepare_shipment_stock(
            support::shipment_context(f.tenant),
            f.fulfillment.id,
            rustok_fulfillment::ShipmentStockAction::Ship,
            f.ship_input(1).items.as_deref(),
        )
        .await
        .expect("prepared scope");
    assert!(
        plan.remaining_context()
            .expect("remaining budget")
            .deadline_ms
            .is_some_and(|value| value > 0 && value <= 30_000)
    );
    assert!(
        f.proof().await.is_none(),
        "preparation is not durable movement history"
    );
    assert_eq!(f.stock().await, (5, 3));
    assert_eq!(f.allocation().await, allocation);
    assert_eq!(f.current().await.items[0].shipped_quantity, 0);
}

#[tokio::test]
async fn shipment_existing_return_receipt_replay_does_not_depend_on_shipment_proof_storage() {
    use rustok_inventory::return_restock::ledger;
    let f = Fixture::new().await;
    f.ship(3).await.expect("ship");
    let line = f.returned_line();
    restock_returned_items(
        &f.db,
        return_restock_context(f.tenant, line.return_id),
        &[line],
    )
    .await
    .expect("restock");
    let receipt = ledger::Entity::find_by_id(line.return_item_id)
        .one(&f.db)
        .await
        .expect("receipt query")
        .expect("receipt");
    // Simulate an unavailable new dependency for an already committed historical receipt.
    // Production migration down refuses discarding nonempty shipment provenance.
    f.db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "DROP TABLE inventory_shipment_stock_proofs".to_string(),
    ))
    .await
    .expect("unavailable shipment storage fixture");
    let replay = restock_returned_items(
        &f.db,
        return_restock_context(f.tenant, line.return_id),
        &[line],
    )
    .await
    .expect("receipt-only return replay");
    assert_eq!(replay.already_restocked, vec![line.return_item_id]);
    assert!(replay.restocked.is_empty());
    assert_eq!(
        ledger::Entity::find_by_id(line.return_item_id)
            .one(&f.db)
            .await
            .expect("receipt query"),
        Some(receipt)
    );
    assert_eq!(f.stock().await, (3, 0));
}

#[tokio::test]
async fn shipment_provider_success_local_failure_and_owner_recovery_commit_stock_and_checkpoint_together()
 {
    use rustok_fulfillment::entities::provider_operation;
    use rustok_fulfillment::providers::FulfillmentProviderRegistry;
    use rustok_fulfillment::{
        RetryLocalFulfillmentPersistenceRequest, ShipAdminFulfillmentRequest,
    };
    let f = Fixture::new().await;
    f.db.execute_raw(Statement::from_string(DbBackend::Sqlite, "CREATE TRIGGER fail_shipment_progress BEFORE UPDATE OF status ON fulfillments WHEN NEW.status = 'shipped' BEGIN SELECT RAISE(ABORT, 'shipment recovery fixture'); END".to_string())).await.expect("late failure fixture");
    let selected_order =
        || rustok_order::in_process_order_read_port(f.db.clone(), mock_transactional_event_bus());
    let commands = rustok_commerce::fulfillment_admin_command_runtime(
        f.db.clone(),
        FulfillmentProviderRegistry::with_manual_provider(),
        selected_order(),
    )
    .command_port();
    let key = format!(
        "fulfillment:{}:ship:0123456789abcdef0123456789abcdef",
        f.fulfillment.id
    );
    let context = support::shipment_context(f.tenant).with_idempotency_key(&key);
    assert!(
        commands
            .ship_fulfillment(
                context.clone(),
                ShipAdminFulfillmentRequest {
                    fulfillment_id: f.fulfillment.id,
                    input: f.ship_input(3)
                }
            )
            .await
            .is_err()
    );
    assert_eq!(f.stock().await, (5, 3));
    assert!(f.proof().await.is_none());
    let operation = provider_operation::Entity::find()
        .filter(provider_operation::Column::TenantId.eq(f.tenant))
        .filter(provider_operation::Column::IdempotencyKey.eq(&key))
        .one(&f.db)
        .await
        .expect("operation query")
        .expect("retained provider result");
    assert_eq!(operation.status, "reconciliation_required");
    assert!(operation.provider_result.is_some());
    f.db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "DROP TRIGGER fail_shipment_progress".to_string(),
    ))
    .await
    .expect("operator corrects local storage");
    // No carrier is registered: recovery must need only the retained outcome and local owners.
    let recovery = rustok_commerce::fulfillment_admin_command_runtime(
        f.db.clone(),
        FulfillmentProviderRegistry::new(),
        selected_order(),
    )
    .command_port();
    let mut current_operator = support::shipment_context(f.tenant);
    current_operator.idempotency_key = None;
    recovery
        .retry_local_persistence(
            current_operator,
            RetryLocalFulfillmentPersistenceRequest {
                operation_id: operation.id,
            },
        )
        .await
        .expect("owner-local recovery without carrier dependency");
    assert_eq!(f.stock().await, (2, 0));
    let proof = f.proof().await.expect("native proof");
    assert_eq!((proof.shipped, proof.restored), (3, 0));
    assert_eq!(f.current().await.items[0].shipped_quantity, 3);
    assert_eq!(
        provider_operation::Entity::find_by_id(operation.id)
            .one(&f.db)
            .await
            .expect("operation query")
            .expect("retained operation")
            .status,
        "committed"
    );
    recovery
        .retry_local_persistence(
            context,
            RetryLocalFulfillmentPersistenceRequest {
                operation_id: operation.id,
            },
        )
        .await
        .expect("committed recovery replay");
    assert_eq!(f.stock().await, (2, 0));
    assert_eq!(f.proof().await, Some(proof));
}

#[tokio::test]
async fn shipment_native_factory_never_interprets_missing_fulfillment_items_as_sku_less() {
    let f = Fixture::new().await;
    let header = f
        .service
        .create_fulfillment(
            f.tenant,
            CreateFulfillmentInput {
                order_id: f.fulfillment.order_id,
                shipping_option_id: None,
                customer_id: None,
                carrier: None,
                tracking_number: None,
                items: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("header-only historical fixture");
    assert!(
        f.service
            .ship_fulfillment(
                support::shipment_context(f.tenant),
                header.id,
                ShipFulfillmentInput {
                    carrier: "manual".into(),
                    tracking_number: "UNKNOWN-SCOPE".into(),
                    items: None,
                    metadata: serde_json::json!({}),
                }
            )
            .await
            .is_err()
    );
    assert_eq!(f.stock().await, (5, 3));
    assert!(f.proof().await.is_none());
    assert_eq!(
        f.service
            .get_fulfillment(f.tenant, header.id)
            .await
            .expect("header")
            .status,
        "pending"
    );
}
