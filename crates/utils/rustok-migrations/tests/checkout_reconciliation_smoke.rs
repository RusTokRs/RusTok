use rust_decimal::Decimal;
use rustok_cart::dto::CreateCartInput;
use rustok_cart::{CartService, in_process_cart_checkout_port};
use rustok_commerce::{
    BeginCheckoutOperation, CheckoutCompensationSweepService, CheckoutOperationCheckpoint,
    CheckoutOperationJournal, CheckoutOperationStage, checkout_execution_admission_port,
};
use rustok_api::{PortActor, PortContext};
use rustok_migrations::SqliteTestMigrator;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_payment::dto::CreatePaymentCollectionInput;
use rustok_payment::{
    BeginProviderOperation, CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,
    InProcessCheckoutPaymentCompensationPort, PaymentCollectionStatusKind,
    PaymentProviderOperationJournal, PaymentService,
};
use rustok_test_utils::db::setup_test_db_with_migrations;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

const RECONCILIATION_REQUIRED: &str = "reconciliation_required";

#[tokio::test]
async fn manual_checkout_reconciliation_is_terminal_and_blocks_provider_execution() {
    let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
    // The journal publishes `checkout.operation.parked` through the writer's
    // transaction, and in-transaction contract writes require the real outbox
    // transport, not a recording mock.
    let event_bus =
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("reconciliation@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "USD".to_string(),
                metadata: json!({"source": "checkout-reconciliation-smoke"}),
            },
        )
        .await
        .expect("cart fixture must be created");

    let operation_journal = CheckoutOperationJournal::new(db.clone(), event_bus.clone());
    let operation = operation_journal
        .begin(BeginCheckoutOperation {
            tenant_id,
            cart_id: cart.id,
            idempotency_key: format!("checkout-reconciliation-{}", Uuid::new_v4()),
            request_hash: "a".repeat(64),
            snapshot_hash: None,
        })
        .await
        .expect("checkout operation must begin");
    // A fresh operation opens the first admission generation for the cart.
    assert_eq!(operation.execution_admission, "open");
    assert_eq!(operation.admission_epoch, 1);

    let lease_owner = format!("checkout-reconciliation-test:{}", Uuid::new_v4());
    operation_journal
        .claim_execution(tenant_id, operation.id, lease_owner.as_str(), 30)
        .await
        .expect("checkout execution claim must not fail")
        .expect("checkout execution must be claimable");

    let collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: None,
                currency_code: "USD".to_string(),
                amount: Decimal::new(1000, 2),
                metadata: json!({
                    "checkout": {
                        "operation_id": operation.id,
                    }
                }),
            },
        )
        .await
        .expect("checkout payment collection must be created");

    // The payment stage binds the collection to the operation (the same
    // checkpoint the checkout pipeline writes), and that binding is what the
    // payment claim gate reads its admission through.
    operation_journal
        .checkpoint(CheckoutOperationCheckpoint {
            tenant_id,
            operation_id: operation.id,
            lease_owner: lease_owner.clone(),
            expected_stage: CheckoutOperationStage::Created,
            next_stage: CheckoutOperationStage::PaymentReady,
            snapshot_hash: None,
            order_id: None,
            payment_collection_id: Some(collection.id),
            lease_seconds: 30,
        })
        .await
        .expect("payment collection must be bound to the checkout operation");

    let bound_operation = operation_journal
        .get(tenant_id, operation.id)
        .await
        .expect("checkout operation must remain readable");
    assert_eq!(bound_operation.payment_collection_id, Some(collection.id));

    // The binding is write-once — the rule the removed payment-collection guard
    // enforced from the collection side. A checkpoint may accept a collection
    // while the operation carries none, or re-assert the one it carries, but a
    // different collection of the same tenant is refused, because provider
    // operations, marketplace financial rows and refunds are keyed by it.
    let other_cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("rebind-attempt@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "USD".to_string(),
                metadata: json!({"source": "checkout-rebind-guard-smoke"}),
            },
        )
        .await
        .expect("second cart fixture must be created");
    let other_collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: Some(other_cart.id),
                order_id: None,
                customer_id: None,
                currency_code: "USD".to_string(),
                amount: Decimal::new(1000, 2),
                metadata: json!({}),
            },
        )
        .await
        .expect("second collection fixture must be created");
    let rebind = operation_journal
        .checkpoint(CheckoutOperationCheckpoint {
            tenant_id,
            operation_id: operation.id,
            lease_owner: lease_owner.clone(),
            expected_stage: CheckoutOperationStage::PaymentReady,
            next_stage: CheckoutOperationStage::PaymentAuthorized,
            snapshot_hash: None,
            order_id: None,
            payment_collection_id: Some(other_collection.id),
            lease_seconds: 30,
        })
        .await
        .expect_err("a checkpoint must not re-point the binding at another collection");
    assert!(
        rebind.to_string().contains("already bound to payment collection"),
        "the refusal must name the write-once binding: {rebind}"
    );
    let after_rebind_attempt = operation_journal
        .get(tenant_id, operation.id)
        .await
        .expect("checkout operation must remain readable");
    assert_eq!(
        after_rebind_attempt.payment_collection_id,
        Some(collection.id),
        "the refused checkpoint must leave the original binding in place"
    );
    assert_eq!(after_rebind_attempt.stage, "payment_ready");

    // Production builds the journal with the checkout admission reader; without
    // it the gate refuses the claim as `checkout_admission_unavailable`, which
    // is the fail-closed path for a runtime that has no wire.
    let provider_journal = PaymentProviderOperationJournal::new(db.clone())
        .with_checkout_execution_admission_port(checkout_execution_admission_port(db.clone()));
    let provider_operation = provider_journal
        .begin(BeginProviderOperation {
            tenant_id,
            payment_collection_id: collection.id,
            refund_id: None,
            operation: "authorize".to_string(),
            provider_id: "manual".to_string(),
            idempotency_key: format!("authorize-{}", operation.id),
            request_payload: json!({
                "checkout_operation_id": operation.id,
                "amount": "10.00",
                "currency_code": "USD",
            }),
        })
        .await
        .expect("provider operation must be journaled before reconciliation");

    operation_journal
        .mark_compensation_required(
            tenant_id,
            operation.id,
            lease_owner.as_str(),
            "checkout.pipeline_failed",
            "checkout test failure",
        )
        .await
        .expect("checkout must enter compensation_required");
    let compensation_owner = format!("checkout-compensation-test:{}", Uuid::new_v4());
    operation_journal
        .claim_compensation(tenant_id, operation.id, compensation_owner.as_str(), 30)
        .await
        .expect("compensation claim must not fail")
        .expect("compensation must be claimable");
    let reconciled = operation_journal
        .mark_compensation_retryable(
            tenant_id,
            operation.id,
            compensation_owner,
            "checkout.compensation_manual_reconciliation",
            "provider outcome requires operator reconciliation",
        )
        .await
        .expect("manual compensation must be parked by the checkout journal");

    assert_eq!(reconciled.status, RECONCILIATION_REQUIRED);
    assert!(reconciled.completed_at.is_some());
    assert!(reconciled.lease_owner.is_none());
    assert!(reconciled.lease_expires_at.is_none());

    // The manual-reconciliation park moves the operation into the `settling`
    // admission level and bumps the generation: a provider operation created
    // under the open generation is refused instead of racing the compensation.
    // This is the checkout-owned half of the admission contract; the
    // payment-owned half is the provider claim gate.
    assert_eq!(reconciled.execution_admission, "settling");
    assert!(reconciled.admission_epoch > operation.admission_epoch);

    // A refusal is not an error: the claim gate returns `Ok(None)` and records a
    // bounded reason on the operation, which the caller turns into an owner error
    // through `execution_admission_refusal_error`.
    let refused = provider_journal
        .claim_execution(tenant_id, provider_operation.id)
        .await
        .expect("the claim gate must answer, not fail");
    assert!(
        refused.is_none(),
        "provider execution must be refused during checkout reconciliation"
    );
    let refused_operation = provider_journal
        .get(tenant_id, provider_operation.id)
        .await
        .expect("the refused provider operation must remain readable");
    assert_eq!(
        refused_operation.admission_refusal_code.as_deref(),
        Some("checkout_admission_settling"),
        "the refusal must be recorded with the settlement reason"
    );
    let refusal_error = rustok_payment::execution_admission_refusal_error(&refused_operation)
        .expect("a recorded refusal must produce a bounded owner error");
    assert_eq!(refusal_error.code.as_str(), "payment.checkout_admission_settling");

    let sweep = CheckoutCompensationSweepService::new(
        db.clone(),
        event_bus,
        rustok_inventory::in_process_inventory_reservation_identity_port(db.clone()),
        in_process_cart_checkout_port(db),
    )
    .run(tenant_id, actor_id, "reconciliation-smoke", Some(10))
    .await
    .expect("compensation sweep query must succeed");
    assert_eq!(sweep.scanned, 0);
    assert_eq!(sweep.compensated, 0);
    assert_eq!(sweep.retryable, 0);
}

/// The park-time fence is scoped to the **cart**, not to the collection the
/// journal has already bound.
///
/// This is the window the removed binding trigger used to cover: the collection
/// exists for the cart, but the payment stage has not run `checkpoint` yet, so
/// `checkout_operations.payment_collection_id` is still null. The park must still
/// reach that collection's non-terminal provider operations, otherwise a claim
/// admitted under the previous generation could start a provider call after the
/// checkout began unwinding.
#[tokio::test]
async fn park_time_fence_reaches_provider_operations_before_the_collection_is_bound() {
    let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let tenant_id = Uuid::new_v4();
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("unbound-fence@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "USD".to_string(),
                metadata: json!({"source": "checkout-unbound-fence-smoke"}),
            },
        )
        .await
        .expect("cart fixture must be created");

    let operation_journal = CheckoutOperationJournal::new(db.clone(), event_bus);
    let operation = operation_journal
        .begin(BeginCheckoutOperation {
            tenant_id,
            cart_id: cart.id,
            idempotency_key: format!("checkout-unbound-fence-{}", Uuid::new_v4()),
            request_hash: "b".repeat(64),
            snapshot_hash: None,
        })
        .await
        .expect("checkout operation must begin");
    assert_eq!(operation.execution_admission, "open");
    assert_eq!(operation.admission_epoch, 1);

    let collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: None,
                currency_code: "USD".to_string(),
                amount: Decimal::new(1000, 2),
                metadata: json!({
                    "checkout": {
                        "operation_id": operation.id,
                    }
                }),
            },
        )
        .await
        .expect("checkout payment collection must be created");

    // Creating the collection does not bind it: `checkpoint` is the only writer
    // of the binding column now that the database trigger is gone.
    let unbound = operation_journal
        .get(tenant_id, operation.id)
        .await
        .expect("checkout operation must remain readable");
    assert_eq!(
        unbound.payment_collection_id, None,
        "the collection must not be bound before the payment stage checkpoints it"
    );

    let provider_journal = PaymentProviderOperationJournal::new(db.clone())
        .with_checkout_execution_admission_port(checkout_execution_admission_port(db.clone()));
    let provider_operation = provider_journal
        .begin(BeginProviderOperation {
            tenant_id,
            payment_collection_id: collection.id,
            refund_id: None,
            operation: "authorize".to_string(),
            provider_id: "manual".to_string(),
            idempotency_key: format!("authorize-unbound-{}", operation.id),
            request_payload: json!({
                "checkout_operation_id": operation.id,
                "amount": "10.00",
                "currency_code": "USD",
            }),
        })
        .await
        .expect("provider operation must be journaled");
    assert_eq!(provider_operation.admission_epoch, operation.admission_epoch);

    let lease_owner = format!("checkout-unbound-fence-test:{}", Uuid::new_v4());
    operation_journal
        .claim_execution(tenant_id, operation.id, lease_owner.as_str(), 30)
        .await
        .expect("checkout execution claim must not fail")
        .expect("checkout execution must be claimable");

    let parked = operation_journal
        .mark_compensation_required(
            tenant_id,
            operation.id,
            lease_owner.as_str(),
            "checkout.pipeline_failed",
            "checkout test failure",
        )
        .await
        .expect("checkout must enter compensation_required");
    assert_eq!(parked.execution_admission, "settling");
    assert!(parked.admission_epoch > operation.admission_epoch);

    let stamped = provider_journal
        .get(tenant_id, provider_operation.id)
        .await
        .expect("the fenced provider operation must remain readable");
    assert_eq!(
        stamped.admission_epoch, parked.admission_epoch,
        "the park must stamp the generation through the cart, not through the binding"
    );
    let still_unbound = operation_journal
        .get(tenant_id, operation.id)
        .await
        .expect("checkout operation must remain readable");
    assert_eq!(
        still_unbound.payment_collection_id, None,
        "the fence must not write the binding it no longer reads"
    );

    let refused = provider_journal
        .claim_execution(tenant_id, provider_operation.id)
        .await
        .expect("the claim gate must answer, not fail");
    assert!(
        refused.is_none(),
        "a claim from the previous generation must be refused after the park"
    );
}

/// The window the removed payment-collection guard used to cover: a checkout
/// parks after its collection exists but before `checkpoint` writes the binding
/// into the journal, so the compensation request carries no collection id. The
/// payment owner resolves the attempt's collection from its own metadata link
/// and cancels it, instead of reporting "nothing recorded" and leaving an open
/// collection behind.
#[tokio::test]
async fn compensation_resolves_the_collection_before_the_binding_is_written() {
    let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let tenant_id = Uuid::new_v4();
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("unbound-compensation@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "USD".to_string(),
                metadata: json!({"source": "checkout-unbound-compensation-smoke"}),
            },
        )
        .await
        .expect("cart fixture must be created");

    let operation_journal = CheckoutOperationJournal::new(db.clone(), event_bus);
    let operation = operation_journal
        .begin(BeginCheckoutOperation {
            tenant_id,
            cart_id: cart.id,
            idempotency_key: format!("checkout-unbound-compensation-{}", Uuid::new_v4()),
            request_hash: "c".repeat(64),
            snapshot_hash: None,
        })
        .await
        .expect("checkout operation must begin");

    let collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: None,
                currency_code: "USD".to_string(),
                amount: Decimal::new(1000, 2),
                metadata: json!({
                    "checkout": {
                        "operation_id": operation.id,
                    }
                }),
            },
        )
        .await
        .expect("checkout payment collection must be created");

    // The window: the collection exists, the journal has no binding for it.
    let unbound = operation_journal
        .get(tenant_id, operation.id)
        .await
        .expect("checkout operation must remain readable");
    assert_eq!(unbound.payment_collection_id, None);

    let compensation = InProcessCheckoutPaymentCompensationPort::new(db.clone());
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("checkout-unbound-compensation-smoke"),
        "en",
        format!("checkout:{}:compensation:payment", operation.id),
    )
    .with_causation_id(operation.id.to_string())
    .with_idempotency_key(format!("checkout:{}:compensation:payment", operation.id))
    .with_deadline(std::time::Duration::from_secs(10));

    let snapshot = compensation
        .compensate_checkout_payment(
            context,
            CheckoutPaymentCompensationRequest {
                checkout_operation_id: operation.id,
                cart_id: cart.id,
                collection_id: None,
                reason: Some("checkout_compensation".to_string()),
                metadata: json!({
                    "checkout": {
                        "operation_id": operation.id,
                        "compensation": true,
                    }
                }),
            },
        )
        .await
        .expect("payment compensation must answer")
        .expect("the unbound collection must be resolved through the cart metadata link");

    assert_eq!(snapshot.collection_id, collection.id);
    assert_eq!(snapshot.status_kind(), PaymentCollectionStatusKind::Cancelled);

    let persisted = PaymentService::new(db.clone())
        .get_collection(tenant_id, collection.id)
        .await
        .expect("the compensated collection must remain readable");
    assert_eq!(persisted.status_kind(), PaymentCollectionStatusKind::Cancelled);

    // A different checkout must not adopt this collection: the resolution is
    // keyed by the collection's metadata link, not by the cart alone.
    let other_operation_id = Uuid::new_v4();
    let other_context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("checkout-unbound-compensation-smoke"),
        "en",
        format!("checkout:{other_operation_id}:compensation:payment"),
    )
    .with_causation_id(other_operation_id.to_string())
    .with_idempotency_key(format!("checkout:{other_operation_id}:compensation:payment"))
    .with_deadline(std::time::Duration::from_secs(10));
    let unmatched = compensation
        .compensate_checkout_payment(
            other_context,
            CheckoutPaymentCompensationRequest {
                checkout_operation_id: other_operation_id,
                cart_id: cart.id,
                collection_id: None,
                reason: Some("checkout_compensation".to_string()),
                metadata: json!({"checkout": {"operation_id": other_operation_id}}),
            },
        )
        .await
        .expect("payment compensation must answer")
        .is_none();
    assert!(
        unmatched,
        "another checkout must not adopt the collection of a different attempt"
    );
}
