use rust_decimal::Decimal;
use rustok_cart::dto::CreateCartInput;
use rustok_cart::{CartService, in_process_cart_checkout_port};
use rustok_commerce::{
    BeginCheckoutOperation, CheckoutCompensationSweepService, CheckoutOperationCheckpoint,
    CheckoutOperationJournal, CheckoutOperationStage, checkout_execution_admission_port,
};
use rustok_migrations::SqliteTestMigrator;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_payment::dto::CreatePaymentCollectionInput;
use rustok_payment::{BeginProviderOperation, PaymentProviderOperationJournal, PaymentService};
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
