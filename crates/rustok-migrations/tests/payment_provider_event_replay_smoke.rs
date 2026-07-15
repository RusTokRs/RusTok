use async_trait::async_trait;
use rustok_migrations::Migrator;
use rustok_payment::providers::{PaymentProviderRegistry, PaymentProviderWebhookResult};
use rustok_payment::{
    CheckpointProviderEvent, FailProviderEvent, PaymentProviderEventApplier,
    PaymentProviderEventApplyError, PaymentProviderEventContext,
    PaymentProviderEventIngressService, PaymentProviderEventJournal, ReceiveProviderEvent,
    PROVIDER_EVENT_DEAD_LETTER, PROVIDER_EVENT_PROCESSED,
};
use rustok_test_utils::db::setup_test_db_with_migrations;
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;

struct AcceptingApplier {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl PaymentProviderEventApplier for AcceptingApplier {
    async fn apply(
        &self,
        _context: PaymentProviderEventContext,
        _event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn operator_replay_uses_verified_normalized_checkpoint_without_raw_payload() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let journal = PaymentProviderEventJournal::new(db.clone());
    let received = journal
        .receive(ReceiveProviderEvent {
            tenant_id,
            provider_id: "gateway".to_string(),
            delivery_id: "delivery-dead-letter-1".to_string(),
            idempotency_key: "event-dead-letter-1".to_string(),
            raw_payload: br#"{"type":"payment.authorized"}"#.to_vec(),
            signature_verified: true,
        })
        .await
        .expect("verified provider delivery must enter the inbox");

    let first_owner = format!("provider-event-worker:{}", Uuid::new_v4());
    journal
        .claim_processing(tenant_id, received.id, first_owner.as_str(), 30)
        .await
        .expect("initial claim must not fail")
        .expect("received event must be claimable");
    journal
        .checkpoint_normalized(CheckpointProviderEvent {
            tenant_id,
            event_id: received.id,
            lease_owner: first_owner.clone(),
            event_type: "payment.authorized".to_string(),
            external_reference: Some("provider-payment-dead-letter-1".to_string()),
            event_metadata: json!({
                "collection_id": Uuid::new_v4(),
                "amount": "10.00",
                "currency_code": "USD",
            }),
        })
        .await
        .expect("verified normalized provider facts must be checkpointed");
    let dead_letter = journal
        .mark_failed(FailProviderEvent {
            tenant_id,
            event_id: received.id,
            lease_owner: first_owner,
            error_code: "payment.webhook_operator_review".to_string(),
            error_message: "owner state requires operator correction".to_string(),
            retryable: false,
            max_attempts: 10,
        })
        .await
        .expect("permanent failure must enter dead-letter");
    assert_eq!(dead_letter.status, PROVIDER_EVENT_DEAD_LETTER);
    assert!(dead_letter.processed_at.is_some());
    assert!(journal
        .list_retryable(tenant_id, 10)
        .await
        .expect("retryable query must succeed")
        .is_empty());
    assert_eq!(
        journal
            .list_dead_letters(tenant_id, 10)
            .await
            .expect("dead-letter query must succeed")
            .len(),
        1
    );

    let calls = Arc::new(AtomicUsize::new(0));
    let ingress = PaymentProviderEventIngressService::new(
        db,
        PaymentProviderRegistry::with_manual_provider(),
        Arc::new(AcceptingApplier {
            calls: calls.clone(),
        }),
    );
    let replayed = ingress
        .replay_dead_letter(
            tenant_id,
            received.id,
            format!("provider-event-operator:{}", Uuid::new_v4()),
        )
        .await
        .expect("operator replay must apply the normalized checkpoint");

    assert!(replayed.replayed);
    assert_eq!(replayed.inbox_event.status, PROVIDER_EVENT_PROCESSED);
    assert_eq!(replayed.provider_event.provider_id, "gateway");
    assert_eq!(replayed.provider_event.event_type, "payment.authorized");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(journal
        .list_dead_letters(tenant_id, 10)
        .await
        .expect("dead-letter query must remain readable")
        .is_empty());
}
