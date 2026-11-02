//! Owner admission/recovery regressions. No peer Order/Inventory dependency is installed here;
//! explicit Noop is test intent, never a fallback for an unavailable selected capability.
use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_fulfillment::error::FulfillmentResult;
use rustok_fulfillment::providers::*;
use rustok_fulfillment::services::{
    BeginProviderOperation, FulfillmentProviderOperationJournal, PROVIDER_OPERATION_COMMITTED,
};
use rustok_fulfillment::*;
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::Expr};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use uuid::Uuid;
mod support;

struct CountingProvider(Arc<AtomicUsize>);
#[async_trait]
impl FulfillmentProvider for CountingProvider {
    fn descriptor(&self) -> FulfillmentProviderDescriptor {
        ManualFulfillmentProvider.descriptor()
    }
    async fn quote_rates(
        &self,
        input: FulfillmentRateQuoteRequest,
    ) -> FulfillmentResult<Vec<FulfillmentRateQuote>> {
        ManualFulfillmentProvider.quote_rates(input).await
    }
    async fn create_label(
        &self,
        input: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        self.0.fetch_add(1, Ordering::SeqCst);
        ManualFulfillmentProvider.create_label(input).await
    }
    async fn ship(
        &self,
        input: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        self.0.fetch_add(1, Ordering::SeqCst);
        ManualFulfillmentProvider.ship(input).await
    }
    async fn cancel(
        &self,
        input: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        self.0.fetch_add(1, Ordering::SeqCst);
        ManualFulfillmentProvider.cancel(input).await
    }
    async fn handle_tracking_webhook(
        &self,
        input: FulfillmentProviderWebhookRequest,
    ) -> FulfillmentResult<FulfillmentProviderWebhookResult> {
        ManualFulfillmentProvider
            .handle_tracking_webhook(input)
            .await
    }
}

fn context(tenant: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::user(Uuid::new_v4().to_string()),
        "fr",
        "current-operator-correlation",
    )
    .with_claim("fulfillments:update")
    .with_role("warehouse-operator")
    .with_channel("backoffice")
    .with_causation_id("operator-request")
    .with_traceparent("00-0123456789abcdef0123456789abcdef-0123456789abcdef-01")
    .with_idempotency_key(key)
    .with_deadline(Duration::from_secs(30))
}

async fn fixture() -> (DatabaseConnection, Uuid, FulfillmentResponse) {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    let tenant = Uuid::new_v4();
    let fulfillment = FulfillmentService::new(db.clone())
        .create_fulfillment(
            tenant,
            CreateFulfillmentInput {
                order_id: Uuid::new_v4(),
                shipping_option_id: None,
                customer_id: None,
                carrier: None,
                tracking_number: None,
                items: Some(vec![CreateFulfillmentItemInput {
                    order_line_item_id: Uuid::new_v4(),
                    quantity: 3,
                    metadata: json!({}),
                }]),
                metadata: json!({}),
            },
        )
        .await
        .expect("owner fixture");
    (db, tenant, fulfillment)
}

fn commands(
    db: &DatabaseConnection,
    stock: Arc<dyn ShipmentStockPort>,
    calls: Arc<AtomicUsize>,
) -> Arc<dyn FulfillmentAdminCommandPort> {
    let mut registry = FulfillmentProviderRegistry::new();
    registry.register_builtin(Arc::new(CountingProvider(calls)));
    FulfillmentAdminCommandRuntime::in_process_with_shipment_stock(db.clone(), registry, stock)
        .command_port()
}
fn ship_request(f: &FulfillmentResponse) -> ShipAdminFulfillmentRequest {
    ShipAdminFulfillmentRequest {
        fulfillment_id: f.id,
        input: ShipFulfillmentInput {
            carrier: "manual".into(),
            tracking_number: "ORIGINAL-TRACKING".into(),
            items: Some(vec![FulfillmentItemQuantityInput {
                fulfillment_item_id: f.items[0].id,
                quantity: 3,
            }]),
            metadata: json!({}),
        },
    }
}
fn cancel_request(f: &FulfillmentResponse) -> CancelAdminFulfillmentRequest {
    CancelAdminFulfillmentRequest {
        fulfillment_id: f.id,
        input: CancelFulfillmentInput {
            reason: Some("operator cancellation".into()),
            metadata: json!({}),
        },
    }
}
fn reopen_request(f: &FulfillmentResponse) -> ReopenAdminFulfillmentRequest {
    ReopenAdminFulfillmentRequest {
        fulfillment_id: f.id,
        input: ReopenFulfillmentInput {
            items: None,
            metadata: json!({}),
        },
    }
}

struct RecordingStock {
    db: DatabaseConnection,
    contexts: Arc<Mutex<Vec<PortContext>>>,
    change_snapshot: bool,
}
#[async_trait]
impl ShipmentStockPort for RecordingStock {
    async fn prepare(
        &self,
        ctx: &PortContext,
        scope: &ShipmentStockScope,
    ) -> Result<Arc<dyn PreparedShipmentStock>, ShipmentStockError> {
        self.contexts.lock().expect("contexts").push(ctx.clone());
        // This callback deliberately uses a peer connection before the F write transaction.
        if entities::fulfillment::Entity::find_by_id(scope.fulfillment_id)
            .filter(entities::fulfillment::Column::TenantId.eq(scope.tenant_id))
            .one(&self.db)
            .await?
            .is_none()
        {
            return Err(ShipmentStockError::Rejected(
                "owner fixture disappeared".into(),
            ));
        }
        if self.change_snapshot {
            entities::fulfillment::Entity::update_many()
                .col_expr(
                    entities::fulfillment::Column::UpdatedAt,
                    Expr::value(scope.updated_at + chrono::Duration::seconds(1)),
                )
                .filter(entities::fulfillment::Column::Id.eq(scope.fulfillment_id))
                .exec(&self.db)
                .await?;
        }
        Ok(Arc::new(NoopShipmentStockPort))
    }
}

#[tokio::test]
async fn shipment_unconfigured_preparation_refuses_before_carrier_and_counters() {
    let (db, tenant, f) = fixture().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let error = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone())
        .ship_fulfillment(context(tenant, "unconfigured-ship"), ship_request(&f))
        .await
        .expect_err("missing stock");
    assert_eq!(error.code, "fulfillment.shipment_stock_not_configured");
    assert!(!error.retryable);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        FulfillmentService::new(db)
            .get_fulfillment(tenant, f.id)
            .await
            .expect("current")
            .items[0]
            .shipped_quantity,
        0
    );
}

#[tokio::test]
async fn shipment_changed_owner_snapshot_refuses_before_carrier_claim() {
    let (db, tenant, f) = fixture().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let stock = Arc::new(RecordingStock {
        db: db.clone(),
        contexts: Arc::new(Mutex::new(vec![])),
        change_snapshot: true,
    });
    let error = commands(&db, stock, calls.clone())
        .ship_fulfillment(context(tenant, "stale-ship"), ship_request(&f))
        .await
        .expect_err("stale snapshot");
    assert_eq!(error.code, "fulfillment.shipment_stock_snapshot_conflict");
    assert!(!error.retryable);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        FulfillmentService::new(db)
            .get_fulfillment(tenant, f.id)
            .await
            .expect("current")
            .items[0]
            .shipped_quantity,
        0
    );
}

#[tokio::test]
async fn shipment_committed_provider_replay_does_not_reacquire_stock_or_call_the_carrier() {
    let (db, tenant, f) = fixture().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let ctx = context(tenant, "committed-ship");
    commands(&db, Arc::new(NoopShipmentStockPort), calls.clone())
        .ship_fulfillment(ctx.clone(), ship_request(&f))
        .await
        .expect("first ship");
    let replay = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone())
        .ship_fulfillment(ctx, ship_request(&f))
        .await
        .expect("receipt-only replay without stock availability");
    assert_eq!(replay.items[0].shipped_quantity, 3);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn shipment_pending_only_cancel_needs_no_stock_but_never_widens_to_shipped() {
    let (db, tenant, f) = fixture().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let cmd = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone());
    cmd.cancel_pending_fulfillment(context(tenant, "pending-only"), cancel_request(&f))
        .await
        .expect("pending cancel has no stock movement");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let (db, tenant, f) = fixture().await;
    commands(&db, Arc::new(NoopShipmentStockPort), calls.clone())
        .ship_fulfillment(context(tenant, "ship-before-cancel"), ship_request(&f))
        .await
        .expect("ship");
    let before = calls.load(Ordering::SeqCst);
    let error = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone())
        .cancel_pending_fulfillment(context(tenant, "not-pending"), cancel_request(&f))
        .await
        .expect_err("pending precondition");
    assert_eq!(error.code, "fulfillment.pending_cancellation_conflict");
    assert_eq!(calls.load(Ordering::SeqCst), before);
    assert_eq!(
        FulfillmentService::new(db)
            .get_fulfillment(tenant, f.id)
            .await
            .expect("current")
            .status,
        "shipped"
    );
}

#[tokio::test]
async fn shipment_reopen_receipt_replay_after_another_cancel_does_not_reopen_again() {
    let (db, tenant, f) = fixture().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let cmd = commands(&db, Arc::new(NoopShipmentStockPort), calls.clone());
    cmd.ship_fulfillment(context(tenant, "cycle-ship"), ship_request(&f))
        .await
        .expect("ship");
    cmd.cancel_fulfillment(context(tenant, "first-cancel"), cancel_request(&f))
        .await
        .expect("cancel");
    let reopen_context = context(tenant, "first-reopen");
    let first = cmd
        .reopen_fulfillment(reopen_context.clone(), reopen_request(&f))
        .await
        .expect("reopen");
    cmd.cancel_fulfillment(context(tenant, "second-cancel"), cancel_request(&f))
        .await
        .expect("a distinct later cancellation");
    let replay = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone())
        .reopen_fulfillment(reopen_context.clone(), reopen_request(&f))
        .await
        .expect("exact owner receipt replay");
    assert_eq!(
        serde_json::to_value(first).expect("response"),
        serde_json::to_value(replay).expect("response")
    );
    assert_eq!(
        FulfillmentService::new(db.clone())
            .get_fulfillment(tenant, f.id)
            .await
            .expect("current")
            .status,
        "cancelled"
    );
    let mut changed = reopen_request(&f);
    changed.input.metadata = json!({"changed":true});
    let error = cmd
        .reopen_fulfillment(reopen_context, changed)
        .await
        .expect_err("immutable key binds payload");
    assert_eq!(error.kind, PortErrorKind::Conflict);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

async fn saved_ship_result(
    db: &DatabaseConnection,
    tenant: Uuid,
    f: &FulfillmentResponse,
    key: &str,
    items: serde_json::Value,
) -> Uuid {
    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let request = FulfillmentProviderOperationRequest {
        tenant_id: tenant,
        fulfillment_id: f.id,
        idempotency_key: Some(key.into()),
        metadata: json!({"operator_note":"retained", "commerce_orchestration":{"operation":"ship", "carrier":"manual", "tracking_number":"SAVED-TRACKING", "items":items}}),
    };
    let row = journal
        .begin(BeginProviderOperation {
            tenant_id: tenant,
            fulfillment_id: f.id,
            operation: "ship".into(),
            provider_id: "manual".into(),
            idempotency_key: key.into(),
            request_payload: serde_json::to_value(&request).expect("request"),
        })
        .await
        .expect("retained request");
    assert!(
        journal
            .claim_execution(tenant, row.id)
            .await
            .expect("claim")
            .is_some()
    );
    // A local manual test double, not a network/provider call by the code under test.
    let result = ManualFulfillmentProvider
        .ship(request)
        .await
        .expect("known outcome");
    journal
        .mark_provider_succeeded(
            tenant,
            row.id,
            None,
            serde_json::to_value(result).expect("result"),
        )
        .await
        .expect("durable provider outcome");
    row.id
}

#[tokio::test]
async fn shipment_local_recovery_preserves_saved_legacy_key_and_real_operator_context_without_carrier()
 {
    let (db, tenant, f) = fixture().await;
    let key = format!(
        " legacy:fulfillment:{}:ship:0123456789abcdef0123456789abcdef ",
        f.id
    );
    let operation_id = saved_ship_result(
        &db,
        tenant,
        &f,
        &key,
        json!([{ "fulfillment_item_id":f.items[0].id, "quantity":1 }]),
    )
    .await;
    let calls = Arc::new(AtomicUsize::new(0));
    let contexts = Arc::new(Mutex::new(vec![]));
    let stock = Arc::new(RecordingStock {
        db: db.clone(),
        contexts: contexts.clone(),
        change_snapshot: false,
    });
    let cmd = commands(&db, stock, calls.clone());
    let mut actual = context(tenant, "not-an-intent");
    actual.idempotency_key = None;
    let result = cmd
        .retry_local_persistence(
            actual.clone(),
            RetryLocalFulfillmentPersistenceRequest { operation_id },
        )
        .await
        .expect("local recovery");
    assert_eq!(result.items[0].shipped_quantity, 1);
    assert_eq!(result.tracking_number.as_deref(), Some("SAVED-TRACKING"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let seen = contexts.lock().expect("contexts").clone();
    assert_eq!(seen.len(), 1);
    let mut expected = actual;
    expected.idempotency_key = Some(key.clone());
    expected.deadline_ms = seen[0].deadline_ms;
    assert!(
        seen[0]
            .deadline_ms
            .is_some_and(|value| value > 0 && value <= 30_000)
    );
    assert_eq!(
        seen[0], expected,
        "only the saved key and decreasing budget may change"
    );
    assert_eq!(
        FulfillmentProviderOperationJournal::new(db.clone())
            .get(tenant, operation_id)
            .await
            .expect("journal")
            .status,
        PROVIDER_OPERATION_COMMITTED
    );
    commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone())
        .retry_local_persistence(
            context(
                tenant,
                expected.idempotency_key.as_deref().expect("saved key"),
            ),
            RetryLocalFulfillmentPersistenceRequest { operation_id },
        )
        .await
        .expect("committed recovery needs no stock dependency");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let error = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone())
        .retry_local_persistence(
            context(tenant, key.trim()),
            RetryLocalFulfillmentPersistenceRequest { operation_id },
        )
        .await
        .expect_err("trimming a retained literal key is identity rotation, not replay");
    assert_eq!(
        error.code,
        "fulfillment.shipment_stock_recovery_identity_unverified"
    );
}

#[tokio::test]
async fn shipment_recovery_keeps_null_all_selection_distinct_from_empty_items() {
    let (db, tenant, f) = fixture().await;
    let operation_id =
        saved_ship_result(&db, tenant, &f, "saved-null-items", serde_json::Value::Null).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let result = commands(&db, Arc::new(NoopShipmentStockPort), calls.clone())
        .retry_local_persistence(
            context(tenant, "saved-null-items"),
            RetryLocalFulfillmentPersistenceRequest { operation_id },
        )
        .await
        .expect("retained null means all remaining");
    assert_eq!(result.items[0].shipped_quantity, 3);
    let (db, tenant, f) = fixture().await;
    let operation_id = saved_ship_result(&db, tenant, &f, "saved-empty-items", json!([])).await;
    assert!(
        commands(&db, Arc::new(NoopShipmentStockPort), calls.clone())
            .retry_local_persistence(
                context(tenant, "saved-empty-items"),
                RetryLocalFulfillmentPersistenceRequest { operation_id }
            )
            .await
            .is_err()
    );
    assert_eq!(
        FulfillmentService::new(db)
            .get_fulfillment(tenant, f.id)
            .await
            .expect("current")
            .items[0]
            .shipped_quantity,
        0
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn shipment_recovery_refuses_changed_keys_and_foreign_operation_scope_without_carrier() {
    let (db, tenant, f) = fixture().await;
    let operation_id =
        saved_ship_result(&db, tenant, &f, "immutable-key", serde_json::Value::Null).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let cmd = commands(&db, Arc::new(NoopShipmentStockPort), calls.clone());
    let error = cmd
        .retry_local_persistence(
            context(tenant, "rotated-key"),
            RetryLocalFulfillmentPersistenceRequest { operation_id },
        )
        .await
        .expect_err("changed key");
    assert_eq!(
        error.code,
        "fulfillment.shipment_stock_recovery_identity_unverified"
    );
    assert!(!error.retryable);
    assert!(
        cmd.retry_local_persistence(
            context(Uuid::new_v4(), "immutable-key"),
            RetryLocalFulfillmentPersistenceRequest { operation_id }
        )
        .await
        .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        FulfillmentService::new(db)
            .get_fulfillment(tenant, f.id)
            .await
            .expect("current")
            .items[0]
            .shipped_quantity,
        0
    );
}

#[tokio::test]
async fn shipment_delivered_partial_reopening_is_movement_free_without_stock_capability() {
    let (db, tenant, f) = fixture().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let initial = commands(&db, Arc::new(NoopShipmentStockPort), calls.clone());
    initial
        .ship_fulfillment(context(tenant, "deliver-ship"), ship_request(&f))
        .await
        .expect("ship");
    let unavailable = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone());
    unavailable
        .deliver_fulfillment(
            context(tenant, "deliver-all"),
            DeliverAdminFulfillmentRequest {
                fulfillment_id: f.id,
                input: DeliverFulfillmentInput {
                    delivered_note: None,
                    items: None,
                    metadata: json!({}),
                },
            },
        )
        .await
        .expect("delivery has no stock dependency");
    let mut request = reopen_request(&f);
    request.input.items = Some(vec![FulfillmentItemQuantityInput {
        fulfillment_item_id: f.items[0].id,
        quantity: 1,
    }]);
    let reopened = unavailable
        .reopen_fulfillment(context(tenant, "reopen-delivered-one"), request)
        .await
        .expect("undo delivery only");
    assert_eq!(
        (
            reopened.items[0].shipped_quantity,
            reopened.items[0].delivered_quantity
        ),
        (3, 2)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn shipment_retained_success_or_legacy_error_cannot_be_bypassed_with_a_new_carrier_key() {
    use rustok_fulfillment::entities::provider_operation;
    for legacy_error in [false, true] {
        let (db, tenant, f) = fixture().await;
        if legacy_error {
            // Seed retained pre-cutover state directly, not by an illegal live downgrade from
            // provider_succeeded. New guards never rewrite historical rows or invent history.
            let operation_id = Uuid::new_v4();
            let now = chrono::Utc::now().fixed_offset();
            provider_operation::Entity::insert(provider_operation::ActiveModel {
                id: sea_orm::Set(operation_id), tenant_id: sea_orm::Set(tenant), fulfillment_id: sea_orm::Set(f.id),
                operation: sea_orm::Set("ship".into()), provider_id: sea_orm::Set("manual".into()), idempotency_key: sea_orm::Set("original-unresolved-key".into()),
                status: sea_orm::Set("provider_error".into()), request_payload: sea_orm::Set(json!({"tenant_id":tenant,"fulfillment_id":f.id,"idempotency_key":"original-unresolved-key","metadata":{}})),
                provider_reference: sea_orm::Set(None), provider_result: sea_orm::Set(None), error_message: sea_orm::Set(Some("retained ambiguous outcome".into())),
                created_at: sea_orm::Set(now), updated_at: sea_orm::Set(now), provider_completed_at: sea_orm::Set(None), committed_at: sea_orm::Set(None),
            }).exec(&db).await.expect("retained legacy fixture");
        } else {
            saved_ship_result(
                &db,
                tenant,
                &f,
                "original-unresolved-key",
                serde_json::Value::Null,
            )
            .await;
        }
        let calls = Arc::new(AtomicUsize::new(0));
        assert!(
            commands(&db, Arc::new(NoopShipmentStockPort), calls.clone())
                .ship_fulfillment(context(tenant, "new-key-cannot-unfence"), ship_request(&f))
                .await
                .is_err()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            FulfillmentService::new(db)
                .get_fulfillment(tenant, f.id)
                .await
                .expect("current")
                .items[0]
                .shipped_quantity,
            0
        );
    }
}

#[tokio::test]
async fn shipment_header_only_scope_needs_an_explicit_opt_out_not_an_unconfigured_port() {
    for opt_out in [false, true] {
        let (db, tenant, _) = fixture().await;
        let f = FulfillmentService::new(db.clone())
            .create_fulfillment(
                tenant,
                CreateFulfillmentInput {
                    order_id: Uuid::new_v4(),
                    shipping_option_id: None,
                    customer_id: None,
                    carrier: None,
                    tracking_number: None,
                    items: None,
                    metadata: json!({}),
                },
            )
            .await
            .expect("historical header-only fixture");
        let calls = Arc::new(AtomicUsize::new(0));
        let stock: Arc<dyn ShipmentStockPort> = if opt_out {
            Arc::new(NoopShipmentStockPort)
        } else {
            Arc::new(UnconfiguredShipmentStockPort)
        };
        let mut request = ship_request_for_header(f.id);
        let cmd = commands(&db, stock, calls.clone());
        let result = cmd
            .ship_fulfillment(context(tenant, "header-only-ship"), request.clone())
            .await;
        if opt_out {
            assert_eq!(
                result.expect("explicit deployment/test opt-out").status,
                "shipped"
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            request.input.metadata = json!({"new_intent":true});
            assert!(
                cmd.ship_fulfillment(context(tenant, "header-only-new-intent"), request)
                    .await
                    .is_err(),
                "no second fresh header-only shipment"
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        } else {
            assert_eq!(
                result.expect_err("unknown scope is not SKU-less").code,
                "fulfillment.shipment_stock_not_configured"
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
    }
}
fn ship_request_for_header(fulfillment_id: Uuid) -> ShipAdminFulfillmentRequest {
    ShipAdminFulfillmentRequest {
        fulfillment_id,
        input: ShipFulfillmentInput {
            carrier: "manual".into(),
            tracking_number: "HEADER-TRACKING".into(),
            items: None,
            metadata: json!({}),
        },
    }
}

#[tokio::test]
async fn shipment_owner_item_bound_refuses_creation_before_persisting_an_unshippable_parent() {
    let (db, tenant, original) = fixture().await;
    let service = FulfillmentService::new(db.clone());
    let result = service
        .create_fulfillment(
            tenant,
            CreateFulfillmentInput {
                order_id: Uuid::new_v4(),
                shipping_option_id: None,
                customer_id: None,
                carrier: None,
                tracking_number: None,
                items: Some(
                    (0..=MAX_SHIPMENT_STOCK_ITEMS)
                        .map(|_| CreateFulfillmentItemInput {
                            order_line_item_id: Uuid::new_v4(),
                            quantity: 1,
                            metadata: json!({}),
                        })
                        .collect(),
                ),
                metadata: json!({}),
            },
        )
        .await;
    assert!(result.is_err());
    let rows = entities::fulfillment::Entity::find()
        .filter(entities::fulfillment::Column::TenantId.eq(tenant))
        .all(&db)
        .await
        .expect("owner parents");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, original.id);
}

#[tokio::test]
async fn shipment_local_recovery_requires_a_current_deadline_before_any_journal_read() {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("no owner tables");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut ctx = context(Uuid::new_v4(), "unused");
    ctx.idempotency_key = None;
    ctx.deadline_ms = None;
    let error = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone())
        .retry_local_persistence(
            ctx,
            RetryLocalFulfillmentPersistenceRequest {
                operation_id: Uuid::new_v4(),
            },
        )
        .await
        .expect_err("deadline before storage");
    assert_eq!(error.code, "port.deadline_required");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn shipment_owner_reads_refuse_oversized_retained_items_without_truncating_or_deleting_history()
 {
    let (db, tenant, f) = fixture().await;
    let retained = entities::fulfillment_item::Entity::find_by_id(f.items[0].id)
        .one(&db)
        .await
        .expect("retained item")
        .expect("item");
    let rows = (0..MAX_SHIPMENT_STOCK_ITEMS)
        .map(|_| {
            let mut row: entities::fulfillment_item::ActiveModel = retained.clone().into();
            row.id = sea_orm::Set(Uuid::new_v4());
            row.order_line_item_id = sea_orm::Set(Uuid::new_v4());
            row
        })
        .collect::<Vec<_>>();
    entities::fulfillment_item::Entity::insert_many(rows)
        .exec_without_returning(&db)
        .await
        .expect("explicit retained invalid history fixture");
    let service = FulfillmentService::new(db.clone());
    for error in [
        service
            .get_fulfillment(tenant, f.id)
            .await
            .expect_err("no truncated single projection"),
        service
            .list_fulfillments(
                tenant,
                ListFulfillmentsInput {
                    order_id: Some(f.order_id),
                    ..Default::default()
                },
            )
            .await
            .expect_err("no truncated page projection"),
    ] {
        assert!(
            matches!(error, FulfillmentError::OwnerPort(cause) if cause.code == "fulfillment.shipment_stock_item_limit")
        );
    }
    let retained_rows = entities::fulfillment_item::Entity::find()
        .filter(entities::fulfillment_item::Column::FulfillmentId.eq(f.id))
        .all(&db)
        .await
        .expect("retained history unchanged");
    assert_eq!(retained_rows.len(), MAX_SHIPMENT_STOCK_ITEMS + 1);
}

#[tokio::test]
async fn shipment_explicit_operator_no_effect_resolution_preserves_keys_and_retained_outcome_evidence()
 {
    use rustok_fulfillment::entities::provider_operation;
    for legacy in [false, true] {
        let (db, tenant, f) = fixture().await;
        let key = "retained-original-provider-identity";
        let request =
            json!({"tenant_id":tenant,"fulfillment_id":f.id,"idempotency_key":key,"metadata":{}});
        let journal = FulfillmentProviderOperationJournal::new(db.clone());
        let row = if legacy {
            let now = chrono::Utc::now().fixed_offset();
            let id = Uuid::new_v4();
            provider_operation::Entity::insert(provider_operation::ActiveModel {
                id: sea_orm::Set(id),
                tenant_id: sea_orm::Set(tenant),
                fulfillment_id: sea_orm::Set(f.id),
                operation: sea_orm::Set("ship".into()),
                provider_id: sea_orm::Set("manual".into()),
                idempotency_key: sea_orm::Set(key.into()),
                status: sea_orm::Set("provider_error".into()),
                request_payload: sea_orm::Set(request.clone()),
                provider_reference: sea_orm::Set(None),
                provider_result: sea_orm::Set(None),
                error_message: sea_orm::Set(Some("retained ambiguous failure".into())),
                created_at: sea_orm::Set(now),
                updated_at: sea_orm::Set(now),
                provider_completed_at: sea_orm::Set(None),
                committed_at: sea_orm::Set(None),
            })
            .exec(&db)
            .await
            .expect("retained legacy row");
            journal.get(tenant, id).await.expect("retained")
        } else {
            let row = journal
                .begin(BeginProviderOperation {
                    tenant_id: tenant,
                    fulfillment_id: f.id,
                    operation: "ship".into(),
                    provider_id: "manual".into(),
                    idempotency_key: key.into(),
                    request_payload: request.clone(),
                })
                .await
                .expect("begin");
            journal
                .claim_execution(tenant, row.id)
                .await
                .expect("claim")
                .expect("claimant");
            journal
                .mark_provider_error(tenant, row.id, "unknown external outcome")
                .await
                .expect("quarantine")
        };
        assert!(
            journal
                .claim_execution(tenant, row.id)
                .await
                .expect("no automatic retry")
                .is_none()
        );
        let recovery = FulfillmentProviderOperationRecovery::new(db.clone());
        assert!(
            recovery
                .list_reconciliation_required(tenant, 100)
                .await
                .expect("operator worklist")
                .iter()
                .any(|work| work.id == row.id)
        );
        assert!(
            recovery
                .resolve_unknown_as_failed(Uuid::new_v4(), row.id, "foreign operator")
                .await
                .is_err()
        );
        let requeued = recovery
            .resolve_unknown_as_failed(tenant, row.id, "operator confirmed no external effect")
            .await
            .expect("explicit authority, not an error-status inference");
        assert_eq!(requeued.status, PROVIDER_OPERATION_PENDING);
        assert_eq!(requeued.id, row.id);
        assert_eq!(requeued.idempotency_key, key);
        assert_eq!(requeued.request_payload, request);
        assert!(
            requeued.provider_reference.is_none()
                && requeued.provider_result.is_none()
                && requeued.provider_completed_at.is_none()
                && requeued.committed_at.is_none()
        );
        journal
            .claim_execution(tenant, row.id)
            .await
            .expect("same key after explicit confirmation")
            .expect("claimant");
        journal
            .mark_execution_reconciliation_required(
                tenant,
                row.id,
                Some("RETAINED-REF".into()),
                None,
                "retained reference without result",
            )
            .await
            .expect("retained outcome evidence");
        assert!(
            recovery
                .resolve_unknown_as_failed(tenant, row.id, "must not erase saved reference")
                .await
                .is_err()
        );
        let result = |reference: &str| json!({"provider_id":"manual","external_reference":reference,"tracking_number":null,"metadata":{}});
        assert!(
            recovery
                .resolve_unknown_as_succeeded(tenant, row.id, Some("OTHER".into()), result("OTHER"))
                .await
                .is_err()
        );
        let current = journal
            .get(tenant, row.id)
            .await
            .expect("preserved reference");
        assert_eq!(current.provider_reference.as_deref(), Some("RETAINED-REF"));
        assert_eq!(current.idempotency_key, key);
        assert!(current.provider_result.is_none());
        let resolved = recovery
            .resolve_unknown_as_succeeded(
                tenant,
                row.id,
                Some("RETAINED-REF".into()),
                result("RETAINED-REF"),
            )
            .await
            .expect("correlated explicit success");
        assert_eq!(resolved.status, PROVIDER_OPERATION_SUCCEEDED);
        assert_eq!(resolved.idempotency_key, key);
        assert_eq!(resolved.request_payload, request);
    }
}

#[tokio::test]
async fn shipment_cancel_replay_binds_the_original_request_even_when_already_cancelled() {
    let (db, tenant, f) = fixture().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let cmd = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone());
    let original = context(tenant, "cancel-original-key");
    cmd.cancel_fulfillment(original.clone(), cancel_request(&f))
        .await
        .expect("pending cancellation needs no stock peer");
    let replay = cmd
        .cancel_fulfillment(original.clone(), cancel_request(&f))
        .await
        .expect("journal replay");
    assert_eq!(replay.status, "cancelled");
    let mut changed = cancel_request(&f);
    changed.input.reason = Some("different request must not be acknowledged".into());
    cmd.cancel_fulfillment(original, changed)
        .await
        .expect_err("terminal status is not a payload receipt");
    cmd.cancel_fulfillment(
        context(tenant, "different-cancel-intent"),
        cancel_request(&f),
    )
    .await
    .expect_err("fresh cancellation of Cancelled refuses before carrier");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn shipment_committed_key_pins_provider_before_mutable_shipping_option_selection() {
    use sea_orm::{ActiveModelTrait, Set};
    let (db, tenant, f) = fixture().await;
    let option_id = Uuid::new_v4();
    let now = chrono::Utc::now().fixed_offset();
    entities::shipping_option::ActiveModel {
        id: Set(option_id),
        tenant_id: Set(tenant),
        currency_code: Set("EUR".into()),
        amount: Set(rust_decimal::Decimal::ZERO),
        provider_id: Set("manual".into()),
        active: Set(true),
        metadata: Set(json!({})),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .expect("saved shipping option");
    entities::fulfillment::Entity::update_many()
        .col_expr(
            entities::fulfillment::Column::ShippingOptionId,
            Expr::value(Some(option_id)),
        )
        .filter(entities::fulfillment::Column::TenantId.eq(tenant))
        .filter(entities::fulfillment::Column::Id.eq(f.id))
        .exec(&db)
        .await
        .expect("link option");
    let calls = Arc::new(AtomicUsize::new(0));
    let ctx = context(tenant, "provider-pinned-original");
    commands(&db, Arc::new(NoopShipmentStockPort), calls.clone())
        .ship_fulfillment(ctx.clone(), ship_request(&f))
        .await
        .expect("initial shipment");
    entities::shipping_option::Entity::update_many()
        .col_expr(
            entities::shipping_option::Column::ProviderId,
            Expr::value("unavailable-after-option-edit"),
        )
        .filter(entities::shipping_option::Column::TenantId.eq(tenant))
        .filter(entities::shipping_option::Column::Id.eq(option_id))
        .exec(&db)
        .await
        .expect("mutable option changed");
    let cmd = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone());
    let replay = cmd
        .ship_fulfillment(ctx.clone(), ship_request(&f))
        .await
        .expect("original journal provider wins without stock preparation");
    assert_eq!(replay.status, "shipped");
    let mut changed = ship_request(&f);
    changed.input.tracking_number = "DIFFERENT-PAYLOAD".into();
    cmd.ship_fulfillment(ctx, changed)
        .await
        .expect_err("provider pinning cannot weaken request equality");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn shipment_reship_is_movement_free_but_shipped_status_is_not_key_payload_replay() {
    let (db, tenant, f) = fixture().await;
    let calls = Arc::new(AtomicUsize::new(0));
    commands(&db, Arc::new(NoopShipmentStockPort), calls.clone())
        .ship_fulfillment(context(tenant, "before-reship"), ship_request(&f))
        .await
        .expect("ship");
    let cmd = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone());
    cmd.deliver_fulfillment(
        context(tenant, "before-reship-deliver"),
        DeliverAdminFulfillmentRequest {
            fulfillment_id: f.id,
            input: DeliverFulfillmentInput {
                delivered_note: None,
                items: None,
                metadata: json!({}),
            },
        },
    )
    .await
    .expect("deliver");
    let request = ReshipAdminFulfillmentRequest {
        fulfillment_id: f.id,
        input: ReshipFulfillmentInput {
            carrier: "manual".into(),
            tracking_number: "RESHIP-ORIGINAL".into(),
            items: None,
            metadata: json!({}),
        },
    };
    let ctx = context(tenant, "reship-original-key");
    let applied = cmd
        .reship_fulfillment(ctx.clone(), request.clone())
        .await
        .expect("Delivered reship needs no stock peer");
    assert_eq!(applied.status, "shipped");
    assert_eq!(applied.items[0].shipped_quantity, 3);
    assert_eq!(applied.items[0].delivered_quantity, 0);
    cmd.reship_fulfillment(ctx.clone(), request.clone())
        .await
        .expect("exact journal replay");
    let mut changed = request.clone();
    changed.input.tracking_number = "RESHIP-DIFFERENT".into();
    cmd.reship_fulfillment(ctx, changed)
        .await
        .expect_err("metadata is not a payload receipt");
    cmd.reship_fulfillment(context(tenant, "reship-different-key"), request)
        .await
        .expect_err("fresh intent requires Delivered before carrier");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn shipment_reopening_pending_cancellation_keeps_pending_without_inventing_shipped_at() {
    let (db, tenant, f) = fixture().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let cmd = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone());
    cmd.cancel_pending_fulfillment(
        context(tenant, "pending-cancel-before-reopen"),
        cancel_request(&f),
    )
    .await
    .expect("cancel Pending");
    let reopened = cmd
        .reopen_fulfillment(context(tenant, "pending-reopen"), reopen_request(&f))
        .await
        .expect("movement-free reopen");
    assert_eq!(reopened.status, "pending");
    assert!(reopened.shipped_at.is_none());
    assert_eq!(reopened.items[0].shipped_quantity, 0);
    assert_eq!(reopened.items[0].delivered_quantity, 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn shipment_pending_only_cancel_replay_after_later_shipment_is_read_only() {
    let (db, tenant, f) = fixture().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let cmd = commands(&db, Arc::new(NoopShipmentStockPort), calls.clone());
    let original = context(tenant, "pending-only-original");
    cmd.cancel_pending_fulfillment(original.clone(), cancel_request(&f))
        .await
        .expect("cancel Pending");
    cmd.reopen_fulfillment(context(tenant, "pending-only-reopen"), reopen_request(&f))
        .await
        .expect("reopen Pending");
    cmd.ship_fulfillment(context(tenant, "pending-only-later-ship"), ship_request(&f))
        .await
        .expect("distinct later shipment");
    let replay = commands(&db, Arc::new(UnconfiguredShipmentStockPort), calls.clone())
        .cancel_pending_fulfillment(original, cancel_request(&f))
        .await
        .expect("Committed replay precedes the fresh pending-only precondition");
    assert_eq!(replay.status, "shipped");
    assert_eq!(replay.items[0].shipped_quantity, 3);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn shipment_unavailable_registry_keeps_original_intent_pending_without_unknown_outcome() {
    use sea_orm::{ActiveModelTrait, Set};
    let (db, tenant, f) = fixture().await;
    let provider = "not-installed-provider";
    let key = "known-uninvoked-original";
    let option_id = Uuid::new_v4();
    let now = chrono::Utc::now().fixed_offset();
    entities::shipping_option::ActiveModel {
        id: Set(option_id),
        tenant_id: Set(tenant),
        currency_code: Set("EUR".into()),
        amount: Set(rust_decimal::Decimal::ZERO),
        provider_id: Set(provider.into()),
        active: Set(true),
        metadata: Set(json!({})),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .expect("saved shipping option");
    entities::fulfillment::Entity::update_many()
        .col_expr(
            entities::fulfillment::Column::ShippingOptionId,
            Expr::value(Some(option_id)),
        )
        .filter(entities::fulfillment::Column::TenantId.eq(tenant))
        .filter(entities::fulfillment::Column::Id.eq(f.id))
        .exec(&db)
        .await
        .expect("link option");
    let calls = Arc::new(AtomicUsize::new(0));
    commands(&db, Arc::new(NoopShipmentStockPort), calls.clone())
        .ship_fulfillment(context(tenant, key), ship_request(&f))
        .await
        .expect_err("registry cannot invoke this provider");
    let row = FulfillmentProviderOperationJournal::new(db.clone())
        .find_by_key(tenant, provider, key)
        .await
        .expect("journal lookup")
        .expect("retained original intent");
    assert_eq!(row.status, rustok_fulfillment::PROVIDER_OPERATION_PENDING);
    assert!(row.provider_reference.is_none());
    assert!(row.provider_result.is_none());
    assert!(row.provider_completed_at.is_none());
    assert!(row.committed_at.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        FulfillmentService::new(db)
            .get_fulfillment(tenant, f.id)
            .await
            .expect("current")
            .items[0]
            .shipped_quantity,
        0
    );
}

#[tokio::test]
async fn shipment_shared_create_label_success_resumes_local_commit_instead_of_false_ack() {
    let (db, tenant, f) = fixture().await;
    let key = "known-create-label-needs-local-commit";
    let request = CreateAdminFulfillmentRequest {
        provider_id: "manual".into(),
        input: CreateFulfillmentInput {
            order_id: f.order_id,
            shipping_option_id: None,
            customer_id: None,
            carrier: None,
            tracking_number: None,
            items: Some(vec![CreateFulfillmentItemInput {
                order_line_item_id: f.items[0].order_line_item_id,
                quantity: 3,
                metadata: json!({}),
            }]),
            metadata: json!({}),
        },
    };
    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let operation = journal
        .begin(BeginProviderOperation {
            tenant_id: tenant,
            fulfillment_id: f.id,
            provider_id: "manual".into(),
            operation: "create_label".into(),
            idempotency_key: key.into(),
            request_payload: serde_json::to_value(&request).expect("owner create request"),
        })
        .await
        .expect("original create intent");
    journal
        .claim_execution(tenant, operation.id)
        .await
        .expect("claim")
        .expect("one claimant");
    let succeeded = journal.mark_provider_succeeded(tenant, operation.id, Some("LABEL-REF".into()), json!({
        "provider_id":"manual", "external_reference":"LABEL-REF", "tracking_number":"LABEL-TRACK", "metadata":{"label":"retained-success"},
    })).await.expect("retained actual provider success; local state still absent");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = FulfillmentProviderRegistry::new();
    registry.register_builtin(Arc::new(CountingProvider(calls.clone())));
    let applied = FulfillmentAdminCreateCommandRuntime::in_process(db.clone(), registry)
        .command_port()
        .create_fulfillment(
            context(tenant, key).with_claim("fulfillments:create"),
            request,
        )
        .await
        .expect("reuse actual result and complete local persistence");
    assert_eq!(applied.id, f.id);
    assert!(
        applied
            .metadata
            .as_object()
            .is_some_and(|metadata| !metadata.is_empty())
    );
    let committed = journal.get(tenant, operation.id).await.expect("committed");
    assert_eq!(committed.status, PROVIDER_OPERATION_COMMITTED);
    assert_eq!(committed.provider_result, succeeded.provider_result);
    assert_eq!(
        committed.provider_completed_at,
        succeeded.provider_completed_at
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
