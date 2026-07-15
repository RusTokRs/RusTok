use rustok_migrations::Migrator;
use rustok_payment::{PaymentProviderEventJournal, ReceiveProviderEvent, VerifiedProviderEvent};
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn normalized_provider_event_facts_are_immutable_even_through_direct_sql() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let event = PaymentProviderEventJournal::new(db.clone())
        .receive_verified(
            ReceiveProviderEvent {
                tenant_id,
                provider_id: "gateway".to_string(),
                delivery_id: "delivery-immutable-1".to_string(),
                idempotency_key: "event-immutable-1".to_string(),
                raw_payload: br#"{"type":"payment.authorized"}"#.to_vec(),
                signature_verified: true,
            },
            VerifiedProviderEvent {
                event_type: "payment.authorized".to_string(),
                external_reference: Some("provider-payment-immutable-1".to_string()),
                event_metadata: json!({
                    "collection_id": Uuid::new_v4(),
                    "amount": "10.00",
                    "currency_code": "USD",
                }),
            },
        )
        .await
        .expect("verified provider event must be persisted");

    let error = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE payment_provider_events SET event_type = ? WHERE id = ? AND tenant_id = ?",
            ["payment.captured".into(), event.id.into(), tenant_id.into()],
        ))
        .await
        .expect_err("direct SQL must not rewrite normalized provider facts");

    assert!(
        error
            .to_string()
            .contains("payment provider event normalized facts are immutable"),
        "unexpected normalized immutability error: {error}"
    );
}
