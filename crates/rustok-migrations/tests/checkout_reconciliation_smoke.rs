use rust_decimal::Decimal;
use rustok_cart::dto::CreateCartInput;
use rustok_cart::{in_process_cart_checkout_port, CartService};
use rustok_commerce::{
    BeginCheckoutOperation, CheckoutCompensationSweepService, CheckoutOperationJournal,
};
use rustok_migrations::Migrator;
use rustok_payment::dto::CreatePaymentCollectionInput;
use rustok_payment::{BeginProviderOperation, PaymentProviderOperationJournal, PaymentService};
use rustok_test_utils::{db::setup_test_db_with_migrations, mock_transactional_event_bus};
use serde_json::json;
use uuid::Uuid;

const RECONCILIATION_REQUIRED: &str = "reconciliation_required";

#[tokio::test]
async fn manual_checkout_reconciliation_is_terminal_and_blocks_provider_execution() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let event_bus = mock_transactional_event_bus();
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

    let operation_journal = CheckoutOperationJournal::new(db.clone());
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

    let bound_operation = operation_journal
        .get(tenant_id, operation.id)
        .await
        .expect("checkout operation must remain readable");
    assert_eq!(bound_operation.payment_collection_id, Some(collection.id));

    let provider_journal = PaymentProviderOperationJournal::new(db.clone());
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
        .expect("manual compensation must be classified by the database guard");

    assert_eq!(reconciled.status, RECONCILIATION_REQUIRED);
    assert!(reconciled.completed_at.is_some());
    assert!(reconciled.lease_owner.is_none());
    assert!(reconciled.lease_expires_at.is_none());

    let claim_error = provider_journal
        .claim_execution(provider_operation.id)
        .await
        .expect_err("provider execution must be blocked during checkout reconciliation");
    assert!(
        claim_error
            .to_string()
            .contains("payment provider operation blocked by checkout compensation"),
        "unexpected provider execution error: {claim_error}"
    );

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
