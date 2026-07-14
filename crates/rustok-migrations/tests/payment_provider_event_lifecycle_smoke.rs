use rust_decimal::Decimal;
use rustok_migrations::Migrator;
use rustok_payment::dto::CreatePaymentCollectionInput;
use rustok_payment::providers::PaymentProviderWebhookResult;
use rustok_payment::{
    PaymentLifecycleEventApplier, PaymentProviderEventApplier, PaymentProviderEventContext,
    PaymentService,
};
use rustok_test_utils::db::setup_test_db_with_migrations;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn normalized_payment_webhooks_apply_authorize_and_capture_once() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let service = PaymentService::new(db.clone());
    let collection = service
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: None,
                customer_id: None,
                currency_code: "USD".to_string(),
                amount: Decimal::new(2500, 2),
                metadata: json!({"source": "provider-event-lifecycle-smoke"}),
            },
        )
        .await
        .expect("payment collection fixture must be created");
    let applier = PaymentLifecycleEventApplier::new(db);
    let context = PaymentProviderEventContext {
        event_id: Uuid::new_v4(),
        tenant_id,
        provider_id: "manual".to_string(),
        delivery_id: "delivery-authorized-1".to_string(),
        idempotency_key: "event-authorized-1".to_string(),
    };
    let authorized = PaymentProviderWebhookResult {
        event_type: "payment.authorized".to_string(),
        external_reference: Some("provider-payment-1".to_string()),
        metadata: json!({
            "collection_id": collection.id,
            "amount": "25.00",
            "currency_code": "USD",
            "metadata": {"provider_event": "authorized"},
        }),
    };

    applier
        .apply(context.clone(), authorized.clone())
        .await
        .expect("authorized event must apply");
    applier
        .apply(context, authorized)
        .await
        .expect("authorized event replay must be accepted");
    let authorized_collection = service
        .get_collection(tenant_id, collection.id)
        .await
        .expect("authorized collection must remain readable");
    assert_eq!(authorized_collection.status, "authorized");
    assert_eq!(authorized_collection.authorized_amount, Decimal::new(2500, 2));

    let capture_context = PaymentProviderEventContext {
        event_id: Uuid::new_v4(),
        tenant_id,
        provider_id: "manual".to_string(),
        delivery_id: "delivery-captured-1".to_string(),
        idempotency_key: "event-captured-1".to_string(),
    };
    let captured = PaymentProviderWebhookResult {
        event_type: "payment.captured".to_string(),
        external_reference: Some("provider-payment-1".to_string()),
        metadata: json!({
            "collection_id": collection.id,
            "amount": "25.00",
            "currency_code": "USD",
            "metadata": {"provider_event": "captured"},
        }),
    };
    applier
        .apply(capture_context.clone(), captured.clone())
        .await
        .expect("captured event must apply");
    applier
        .apply(capture_context, captured)
        .await
        .expect("captured event replay must be accepted");
    let captured_collection = service
        .get_collection(tenant_id, collection.id)
        .await
        .expect("captured collection must remain readable");
    assert_eq!(captured_collection.status, "captured");
    assert_eq!(captured_collection.captured_amount, Decimal::new(2500, 2));
}

#[tokio::test]
async fn normalized_payment_webhook_rejects_currency_mismatch_before_mutation() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let service = PaymentService::new(db.clone());
    let collection = service
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: None,
                customer_id: None,
                currency_code: "USD".to_string(),
                amount: Decimal::new(1000, 2),
                metadata: json!({}),
            },
        )
        .await
        .expect("payment collection fixture must be created");
    let applier = PaymentLifecycleEventApplier::new(db);
    let error = applier
        .apply(
            PaymentProviderEventContext {
                event_id: Uuid::new_v4(),
                tenant_id,
                provider_id: "manual".to_string(),
                delivery_id: "delivery-currency-mismatch".to_string(),
                idempotency_key: "event-currency-mismatch".to_string(),
            },
            PaymentProviderWebhookResult {
                event_type: "payment.authorized".to_string(),
                external_reference: Some("provider-payment-2".to_string()),
                metadata: json!({
                    "collection_id": collection.id,
                    "amount": "10.00",
                    "currency_code": "EUR",
                }),
            },
        )
        .await
        .expect_err("currency mismatch must fail");
    assert!(!error.retryable);
    assert_eq!(error.code, "payment.webhook_currency_mismatch");

    let unchanged = service
        .get_collection(tenant_id, collection.id)
        .await
        .expect("payment collection must remain readable");
    assert_eq!(unchanged.status, "pending");
}
