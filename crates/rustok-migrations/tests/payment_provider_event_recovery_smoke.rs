use async_trait::async_trait;
use rustok_migrations::Migrator;
use rustok_payment::providers::PaymentProviderWebhookResult;
use rustok_payment::{
    PaymentProviderEventApplier, PaymentProviderEventApplyError, PaymentProviderEventContext,
    PaymentProviderEventJournal, PaymentProviderEventRecoveryService, ReceiveProviderEvent,
    VerifiedProviderEvent, PROVIDER_EVENT_DEAD_LETTER, PROVIDER_EVENT_PROCESSED,
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
async fn recovery_worker_applies_verified_normalized_event_without_provider_reparse() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let journal = PaymentProviderEventJournal::new(db.clone());
    let event = journal
        .receive_verified(
            ReceiveProviderEvent {
                tenant_id,
                provider_id: "gateway".to_string(),
                delivery_id: "delivery-recovery-1".to_string(),
                idempotency_key: "event-recovery-1".to_string(),
                raw_payload: br#"{"type":"payment.authorized"}"#.to_vec(),
                signature_verified: true,
            },
            VerifiedProviderEvent {
                event_type: "payment.authorized".to_string(),
                external_reference: Some("provider-payment-recovery-1".to_string()),
                event_metadata: json!({
                    "collection_id": Uuid::new_v4(),
                    "amount": "10.00",
                    "currency_code": "USD",
                }),
            },
        )
        .await
        .expect("verified provider event must enter inbox");
    let calls = Arc::new(AtomicUsize::new(0));
    let recovery = PaymentProviderEventRecoveryService::new(
        db,
        Arc::new(AcceptingApplier {
            calls: calls.clone(),
        }),
    );

    let report = recovery
        .run(tenant_id, "payment-provider-event-worker", Some(10))
        .await
        .expect("provider event recovery sweep must succeed");
    assert_eq!(report.scanned, 1);
    assert_eq!(report.processed, 1);
    assert_eq!(report.retryable, 0);
    assert_eq!(report.dead_letter, 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let processed = journal
        .get(tenant_id, event.id)
        .await
        .expect("processed provider event must remain readable");
    assert_eq!(processed.status, PROVIDER_EVENT_PROCESSED);
}

#[tokio::test]
async fn recovery_worker_dead_letters_legacy_event_without_normalized_checkpoint() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let journal = PaymentProviderEventJournal::new(db.clone());
    let event = journal
        .receive(ReceiveProviderEvent {
            tenant_id,
            provider_id: "gateway".to_string(),
            delivery_id: "delivery-recovery-missing-checkpoint".to_string(),
            idempotency_key: "event-recovery-missing-checkpoint".to_string(),
            raw_payload: br#"{"type":"payment.authorized"}"#.to_vec(),
            signature_verified: true,
        })
        .await
        .expect("legacy provider event must enter inbox");
    let calls = Arc::new(AtomicUsize::new(0));
    let recovery = PaymentProviderEventRecoveryService::new(
        db,
        Arc::new(AcceptingApplier {
            calls: calls.clone(),
        }),
    );

    let report = recovery
        .run(tenant_id, "payment-provider-event-worker", Some(10))
        .await
        .expect("missing-checkpoint sweep must complete safely");
    assert_eq!(report.scanned, 1);
    assert_eq!(report.dead_letter, 1);
    assert_eq!(report.processed, 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        report.failures[0].error_code.as_deref(),
        Some("payment.webhook_normalized_checkpoint_missing")
    );

    let dead_letter = journal
        .get(tenant_id, event.id)
        .await
        .expect("dead-letter provider event must remain readable");
    assert_eq!(dead_letter.status, PROVIDER_EVENT_DEAD_LETTER);
}
