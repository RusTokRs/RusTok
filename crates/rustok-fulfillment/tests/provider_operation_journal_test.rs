use rustok_fulfillment::{
    BeginProviderOperation, FulfillmentProviderOperationJournal,
    FulfillmentProviderOperationRecovery, PROVIDER_OPERATION_COMMITTED,
    PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
    PROVIDER_OPERATION_SUCCEEDED,
};
use rustok_test_utils::db::setup_test_db;
use uuid::Uuid;

mod support;

#[tokio::test]
async fn provider_execution_has_one_claimant_and_ambiguous_errors_require_reconciliation() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();
    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let operation = journal
        .begin(BeginProviderOperation {
            tenant_id,
            fulfillment_id,
            operation: "ship".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "ship-once".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": tenant_id,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "ship-once",
                "metadata": {}
            }),
        })
        .await
        .expect("journal operation");

    let first_journal = journal.clone();
    let second_journal = journal.clone();
    let (first, second) = tokio::join!(
        first_journal.claim_execution(operation.id),
        second_journal.claim_execution(operation.id)
    );
    let first = first.expect("first claim");
    let second = second.expect("second claim");
    assert_ne!(first.is_some(), second.is_some(), "exactly one caller must claim");

    let ambiguous = journal
        .mark_provider_error(operation.id, "carrier request timed out")
        .await
        .expect("ambiguous outcome should be quarantined");
    assert_eq!(
        ambiguous.status,
        PROVIDER_OPERATION_RECONCILIATION_REQUIRED
    );
    assert!(ambiguous.provider_completed_at.is_some());
    assert!(ambiguous.provider_result.is_none());

    let recovery = FulfillmentProviderOperationRecovery::new(db.clone());
    assert!(recovery
        .resolve_unknown_as_failed(Uuid::new_v4(), operation.id, "wrong tenant")
        .await
        .is_err());

    let retryable = recovery
        .resolve_unknown_as_failed(tenant_id, operation.id, "carrier confirmed no shipment")
        .await
        .expect("confirmed failure should become retryable");
    assert_eq!(retryable.status, PROVIDER_OPERATION_ERROR);
    assert!(retryable.provider_completed_at.is_none());

    assert!(journal
        .claim_execution(operation.id)
        .await
        .expect("retry claim")
        .is_some());
    let succeeded = journal
        .mark_provider_succeeded(
            operation.id,
            Some("shipment-1".to_string()),
            serde_json::json!({
                "provider_id": "carrier",
                "external_reference": "shipment-1",
                "tracking_number": "TRACK-1",
                "metadata": {}
            }),
        )
        .await
        .expect("provider success");
    assert_eq!(succeeded.status, PROVIDER_OPERATION_SUCCEEDED);

    let committed = journal
        .mark_committed(operation.id)
        .await
        .expect("journal commit");
    assert_eq!(committed.status, PROVIDER_OPERATION_COMMITTED);
}

#[tokio::test]
async fn manual_success_reconciliation_validates_provider_identity() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();
    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let operation = journal
        .begin(BeginProviderOperation {
            tenant_id,
            fulfillment_id,
            operation: "create_label".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "label-once".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": tenant_id,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "label-once",
                "metadata": {}
            }),
        })
        .await
        .expect("journal operation");
    journal
        .claim_execution(operation.id)
        .await
        .expect("claim")
        .expect("claimed");
    journal
        .mark_provider_error(operation.id, "connection closed after request")
        .await
        .expect("ambiguous result");

    let recovery = FulfillmentProviderOperationRecovery::new(db);
    let wrong_provider = serde_json::json!({
        "provider_id": "other-carrier",
        "external_reference": "label-1",
        "tracking_number": "TRACK-1",
        "metadata": {}
    });
    assert!(recovery
        .resolve_unknown_as_succeeded(
            tenant_id,
            operation.id,
            Some("label-1".to_string()),
            wrong_provider,
        )
        .await
        .is_err());

    let reconciled = recovery
        .resolve_unknown_as_succeeded(
            tenant_id,
            operation.id,
            Some("label-1".to_string()),
            serde_json::json!({
                "provider_id": "carrier",
                "external_reference": "label-1",
                "tracking_number": "TRACK-1",
                "metadata": {}
            }),
        )
        .await
        .expect("valid result should be persisted");
    assert_eq!(reconciled.status, PROVIDER_OPERATION_SUCCEEDED);
    assert_eq!(
        reconciled.provider_reference.as_deref(),
        Some("label-1")
    );
}
