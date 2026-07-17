use rust_decimal::Decimal;
use rustok_migrations::Migrator;
use rustok_payment::{
    BeginProviderOperation, CreatePaymentCollectionInput, PaymentProviderOperationJournal,
    PaymentService, PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
};
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

#[tokio::test]
async fn uncertain_executing_provider_operation_requires_reconciliation_without_reclaim() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let collection = PaymentService::new(db.clone())
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: None,
                customer_id: None,
                currency_code: "USD".to_string(),
                amount: Decimal::new(2_500, 2),
                metadata: serde_json::json!({"source": "uncertain-provider-outcome-test"}),
            },
        )
        .await
        .expect("payment collection fixture must be created");

    let journal = PaymentProviderOperationJournal::new(db);
    let operation = journal
        .begin(BeginProviderOperation {
            tenant_id,
            payment_collection_id: collection.id,
            refund_id: None,
            operation: "capture".to_string(),
            provider_id: "stripe".to_string(),
            idempotency_key: format!("capture:{}", collection.id),
            request_payload: serde_json::json!({
                "collection_id": collection.id,
                "amount": "25.00",
                "currency_code": "USD"
            }),
        })
        .await
        .expect("provider operation must be journaled");

    let claimed = journal
        .claim_execution(operation.id)
        .await
        .expect("pending operation must be claimable")
        .expect("claim must win");
    assert_eq!(claimed.status, "executing");

    let uncertain = journal
        .mark_reconciliation_required(
            operation.id,
            "provider response was lost after request dispatch",
        )
        .await
        .expect("executing operation must become reconciliation_required");
    assert_eq!(uncertain.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED);
    assert!(uncertain.provider_completed_at.is_none());
    assert!(uncertain.error_message.is_some());

    let second_claim = journal
        .claim_execution(operation.id)
        .await
        .expect("reconciliation state lookup must succeed");
    assert!(
        second_claim.is_none(),
        "an uncertain external outcome must never be automatically re-executed"
    );

    let committed = journal
        .mark_committed(operation.id)
        .await
        .expect("operator reconciliation may commit the durable outcome");
    assert_eq!(committed.status, PROVIDER_OPERATION_COMMITTED);
    assert!(committed.provider_completed_at.is_some());
    assert!(committed.committed_at.is_some());
}

async fn seed_tenant(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)\n         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            "Provider Outcome Test Tenant".into(),
            format!("provider-outcome-{tenant_id}").into(),
            sea_orm::Value::String(None),
            serde_json::json!({}).to_string().into(),
            "en".into(),
            true.into(),
        ],
    ))
    .await
    .expect("tenant fixture must be inserted");
}
