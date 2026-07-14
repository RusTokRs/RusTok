use rustok_migrations::Migrator;
use rustok_payment::{
    CompleteProviderEvent, FailProviderEvent, PaymentProviderEventJournal, ReceiveProviderEvent,
    PROVIDER_EVENT_FAILED, PROVIDER_EVENT_PROCESSED, PROVIDER_EVENT_PROCESSING,
};
use rustok_test_utils::db::setup_test_db_with_migrations;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn provider_event_inbox_deduplicates_and_replays_with_leases() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let journal = PaymentProviderEventJournal::new(db);
    let tenant_id = Uuid::new_v4();
    let request = ReceiveProviderEvent {
        tenant_id,
        provider_id: "manual".to_string(),
        delivery_id: "delivery-1".to_string(),
        idempotency_key: "event-1".to_string(),
        raw_payload: br#"{"type":"payment.authorized"}"#.to_vec(),
        signature_verified: true,
    };

    let received = journal
        .receive(request.clone())
        .await
        .expect("verified provider event must enter the inbox");
    let replay = journal
        .receive(request.clone())
        .await
        .expect("same delivery must be adopted on replay");
    assert_eq!(received.id, replay.id);
    assert_eq!(received.payload_hash, replay.payload_hash);

    let collision = journal
        .receive(ReceiveProviderEvent {
            raw_payload: br#"{"type":"payment.captured"}"#.to_vec(),
            ..request
        })
        .await
        .expect_err("same provider keys with another payload must be rejected");
    assert!(collision.to_string().contains("another delivery"));

    let first_owner = format!("provider-event-worker:{}", Uuid::new_v4());
    let processing = journal
        .claim_processing(tenant_id, received.id, first_owner.as_str(), 30)
        .await
        .expect("first claim must not fail")
        .expect("received event must be claimable");
    assert_eq!(processing.status, PROVIDER_EVENT_PROCESSING);
    assert_eq!(processing.attempt_count, 1);

    let failed = journal
        .mark_failed(FailProviderEvent {
            tenant_id,
            event_id: received.id,
            lease_owner: first_owner,
            error_code: "payment.webhook_owner_not_found".to_string(),
            error_message: "payment owner record is not available yet".to_string(),
            retryable: true,
            max_attempts: 3,
        })
        .await
        .expect("retryable owner failure must return event to failed state");
    assert_eq!(failed.status, PROVIDER_EVENT_FAILED);
    assert!(failed.processed_at.is_none());

    let second_owner = format!("provider-event-worker:{}", Uuid::new_v4());
    let retry = journal
        .claim_processing(tenant_id, received.id, second_owner.as_str(), 30)
        .await
        .expect("retry claim must not fail")
        .expect("failed event must be claimable");
    assert_eq!(retry.status, PROVIDER_EVENT_PROCESSING);
    assert_eq!(retry.attempt_count, 2);

    let processed = journal
        .mark_processed(CompleteProviderEvent {
            tenant_id,
            event_id: received.id,
            lease_owner: second_owner,
            event_type: "payment.authorized".to_string(),
            external_reference: Some("provider-payment-1".to_string()),
            event_metadata: json!({
                "collection_id": Uuid::new_v4(),
                "amount": "10.00",
                "currency_code": "USD",
            }),
        })
        .await
        .expect("successful owner apply must commit the inbox event");
    assert_eq!(processed.status, PROVIDER_EVENT_PROCESSED);
    assert!(processed.processed_at.is_some());
    assert!(processed.lease_owner.is_none());

    assert!(journal
        .claim_processing(tenant_id, received.id, "late-worker", 30)
        .await
        .expect("processed claim query must not fail")
        .is_none());
}
