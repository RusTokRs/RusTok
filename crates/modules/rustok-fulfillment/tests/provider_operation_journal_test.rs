use chrono::Utc;
use rustok_fulfillment::entities::fulfillment;
use rustok_fulfillment::{
    BeginProviderOperation, DeliverFulfillmentInput, FulfillmentProviderOperationJournal,
    FulfillmentProviderOperationRecovery, FulfillmentService, ReopenFulfillmentInput,
    PROVIDER_OPERATION_COMMITTED,
    PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
    PROVIDER_OPERATION_SUCCEEDED,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

mod support;

async fn ensure_provider_journal_guards(db: &sea_orm::DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_fulfillment::migrations::migrations()
        .into_iter()
        .skip(6)
        .take(3)
    {
        migration
            .up(&manager)
            .await
            .expect("provider journal migration should run");
    }
}


async fn insert_test_fulfillment(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    fulfillment_id: Uuid,
) {
    let now = Utc::now().fixed_offset();
    fulfillment::ActiveModel {
        id: Set(fulfillment_id),
        tenant_id: Set(tenant_id),
        order_id: Set(Uuid::new_v4()),
        shipping_option_id: Set(None),
        customer_id: Set(None),
        checkout_operation_id: Set(None),
        checkout_fulfillment_index: Set(None),
        checkout_plan_hash: Set(None),
        status: Set("pending".to_string()),
        carrier: Set(None),
        tracking_number: Set(None),
        delivered_note: Set(None),
        cancellation_reason: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        shipped_at: Set(None),
        delivered_at: Set(None),
        cancelled_at: Set(None),
    }
    .insert(db)
    .await
    .expect("test fulfillment should be inserted");
}

#[tokio::test]
async fn provider_execution_has_one_claimant_and_ambiguous_errors_require_reconciliation() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    ensure_provider_journal_guards(&db).await;
    let tenant_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();
    insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;
    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let wrong_tenant = Uuid::new_v4();
    let foreign_begin = journal
        .begin(BeginProviderOperation {
            tenant_id: wrong_tenant,
            fulfillment_id,
            operation: "ship".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "foreign-tenant-begin".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": wrong_tenant,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "foreign-tenant-begin",
                "metadata": {}
            }),
        })
        .await;
    assert!(matches!(
        foreign_begin,
        Err(rustok_fulfillment::error::FulfillmentError::FulfillmentNotFound(id))
            if id == fulfillment_id
    ));

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

    assert!(
        journal
            .get(wrong_tenant, operation.id)
            .await
            .is_err(),
        "foreign tenant must not read provider operation"
    );
    assert!(
        journal
            .claim_execution(wrong_tenant, operation.id)
            .await
            .expect("foreign claim should be evaluated")
            .is_none(),
        "foreign tenant must not claim provider operation"
    );
    assert!(
        journal
            .mark_provider_error(
                wrong_tenant,
                operation.id,
                "foreign tenant must not mutate this operation"
            )
            .await
            .is_err(),
        "foreign tenant must not mutate provider operation"
    );

    let first_journal = journal.clone();
    let second_journal = journal.clone();
    let (first, second) = tokio::join!(
        first_journal.claim_execution(tenant_id, operation.id),
        second_journal.claim_execution(tenant_id, operation.id)
    );
    let first = first.expect("first claim");
    let second = second.expect("second claim");
    assert_ne!(
        first.is_some(),
        second.is_some(),
        "exactly one caller must claim"
    );

    let ambiguous = journal
        .mark_provider_error(tenant_id, operation.id, "carrier request timed out")
        .await
        .expect("ambiguous outcome should be quarantined");
    assert_eq!(ambiguous.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED);
    assert!(ambiguous.provider_completed_at.is_some());
    assert!(ambiguous.provider_result.is_none());

    let recovery = FulfillmentProviderOperationRecovery::new(db.clone());
    let recovered_as_wrong_tenant = recovery
        .resolve_unknown_as_failed(wrong_tenant, operation.id, "wrong tenant")
        .await;
    assert!(recovered_as_wrong_tenant.is_err());
    let retryable = recovery
        .resolve_unknown_as_failed(tenant_id, operation.id, "carrier confirmed no shipment")
        .await
        .expect("confirmed failure should become retryable");
    assert_eq!(retryable.status, PROVIDER_OPERATION_ERROR);
    assert!(retryable.provider_completed_at.is_none());

    assert!(
        journal
            .claim_execution(tenant_id, operation.id)
            .await
            .expect("retry claim")
            .is_some()
    );
    let succeeded = journal
        .mark_provider_succeeded(
            tenant_id,
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
        .mark_committed(tenant_id, operation.id)
        .await
        .expect("journal commit");
    assert_eq!(committed.status, PROVIDER_OPERATION_COMMITTED);
}

#[tokio::test]
async fn manual_success_reconciliation_validates_provider_identity() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    ensure_provider_journal_guards(&db).await;
    let tenant_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();
    insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;
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
        .claim_execution(tenant_id, operation.id)
        .await
        .expect("claim")
        .expect("claimed");
    journal
        .mark_provider_error(tenant_id, operation.id, "connection closed after request")
        .await
        .expect("ambiguous result");

    let recovery = FulfillmentProviderOperationRecovery::new(db);
    let wrong_provider = serde_json::json!({
        "provider_id": "other-carrier",
        "external_reference": "label-1",
        "tracking_number": "TRACK-1",
        "metadata": {}
    });
    assert!(
        recovery
            .resolve_unknown_as_succeeded(
                tenant_id,
                operation.id,
                Some("label-1".to_string()),
                wrong_provider,
            )
            .await
            .is_err()
    );

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
    assert_eq!(reconciled.provider_reference.as_deref(), Some("label-1"));
}


#[tokio::test]
async fn provider_operation_insert_cannot_cross_fulfillment_tenant_boundary() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    ensure_provider_journal_guards(&db).await;

    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();
    insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;

    let now = Utc::now().fixed_offset();
    let result = rustok_fulfillment::entities::provider_operation::ActiveModel {
        id: sea_orm::Set(Uuid::new_v4()),
        tenant_id: sea_orm::Set(foreign_tenant_id),
        fulfillment_id: sea_orm::Set(fulfillment_id),
        operation: sea_orm::Set("ship".to_string()),
        provider_id: sea_orm::Set("carrier".to_string()),
        idempotency_key: sea_orm::Set("cross-tenant-insert".to_string()),
        status: sea_orm::Set("pending".to_string()),
        request_payload: sea_orm::Set(serde_json::json!({"source":"integrity-test"})),
        provider_reference: sea_orm::Set(None),
        provider_result: sea_orm::Set(None),
        error_message: sea_orm::Set(None),
        created_at: sea_orm::Set(now),
        updated_at: sea_orm::Set(now),
        provider_completed_at: sea_orm::Set(None),
        committed_at: sea_orm::Set(None),
    }
    .insert(&db)
    .await;

    assert!(
        result.is_err(),
        "provider operation storage must reject a fulfillment from another tenant"
    );
}

#[tokio::test]
async fn non_provider_delivery_cannot_commit_provider_operation_from_metadata_patch() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    ensure_provider_journal_guards(&db).await;

    let tenant_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();
    insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;

    let fulfillment_model = fulfillment::Entity::find_by_id(fulfillment_id)
        .one(&db)
        .await
        .expect("load fulfillment")
        .expect("fulfillment exists");
    let mut fulfillment_active: fulfillment::ActiveModel = fulfillment_model.into();
    fulfillment_active.status = Set("shipped".to_string());
    fulfillment_active.updated_at = Set(Utc::now().fixed_offset());
    fulfillment_active
        .update(&db)
        .await
        .expect("mark fulfillment shipped");

    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let operation = journal
        .begin(BeginProviderOperation {
            tenant_id,
            fulfillment_id,
            operation: "ship".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "delivery-must-not-commit".to_string(),
            request_payload: serde_json::json!({
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "delivery-must-not-commit",
                "metadata": {}
            }),
        })
        .await
        .expect("journal operation");
    journal
        .claim_execution(tenant_id, operation.id)
        .await
        .expect("claim")
        .expect("claimed");
    journal
        .mark_provider_succeeded(
            tenant_id,
            operation.id,
            Some("shipment-delivery-guard".to_string()),
            serde_json::json!({
                "provider_id": "carrier",
                "external_reference": "shipment-delivery-guard",
                "tracking_number": "TRACK-GUARD",
                "metadata": {}
            }),
        )
        .await
        .expect("provider success");

    let service = FulfillmentService::new(db.clone());
    service
        .deliver_fulfillment(
            tenant_id,
            fulfillment_id,
            DeliverFulfillmentInput {
                delivered_note: Some("delivered".to_string()),
                items: None,
                metadata: serde_json::json!({
                    "provider_operation": {
                        "id": operation.id,
                        "operation": "ship"
                    },
                    "operator_note": "ordinary delivery metadata"
                }),
            },
        )
        .await
        .expect("delivery should succeed without consuming reserved provider metadata");

    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("provider operation remains readable");
    assert_eq!(
        current.status,
        PROVIDER_OPERATION_SUCCEEDED,
        "delivery metadata must not commit a provider operation"
    );

    service
        .reopen_fulfillment(
            tenant_id,
            fulfillment_id,
            ReopenFulfillmentInput {
                items: None,
                metadata: serde_json::json!({
                    "provider_operation": {
                        "id": operation.id,
                        "operation": "ship"
                    },
                    "reopen_note": "ordinary reopen metadata"
                }),
            },
        )
        .await
        .expect("reopen should succeed without consuming reserved provider metadata");

    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("provider operation remains readable after reopen");
    assert_eq!(
        current.status,
        PROVIDER_OPERATION_SUCCEEDED,
        "reopen metadata must not commit a provider operation"
    );

    let fulfillment = fulfillment::Entity::find_by_id(fulfillment_id)
        .one(&db)
        .await
        .expect("load updated fulfillment")
        .expect("updated fulfillment exists");
    assert!(
        fulfillment.metadata.get("provider_operation").is_none(),
        "ordinary delivery metadata must not introduce a provider-operation receipt"
    );
    assert_eq!(
        fulfillment
            .metadata
            .get("operator_note")
            .and_then(serde_json::Value::as_str),
        Some("ordinary delivery metadata")
    );
    assert_eq!(
        fulfillment
            .metadata
            .get("reopen_note")
            .and_then(serde_json::Value::as_str),
        Some("ordinary reopen metadata")
    );
}

#[tokio::test]
async fn fulfillment_metadata_commits_provider_operation_in_the_same_database_write() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    ensure_provider_journal_guards(&db).await;
    let tenant_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();

    fulfillment::ActiveModel {
        id: Set(fulfillment_id),
        tenant_id: Set(tenant_id),
        order_id: Set(Uuid::new_v4()),
        shipping_option_id: Set(None),
        customer_id: Set(None),
        checkout_operation_id: Set(None),
        checkout_fulfillment_index: Set(None),
        checkout_plan_hash: Set(None),
        status: Set("pending".to_string()),
        carrier: Set(None),
        tracking_number: Set(None),
        delivered_note: Set(None),
        cancellation_reason: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        shipped_at: Set(None),
        delivered_at: Set(None),
        cancelled_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("fulfillment row");

    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let operation = journal
        .begin(BeginProviderOperation {
            tenant_id,
            fulfillment_id,
            operation: "ship".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "trigger-commit".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": tenant_id,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "trigger-commit",
                "metadata": {}
            }),
        })
        .await
        .expect("journal operation");
    journal
        .claim_execution(tenant_id, operation.id)
        .await
        .expect("claim")
        .expect("claimed");
    journal
        .mark_provider_succeeded(
            tenant_id,
            operation.id,
            Some("shipment-trigger".to_string()),
            serde_json::json!({
                "provider_id": "carrier",
                "external_reference": "shipment-trigger",
                "tracking_number": "TRACK-TRIGGER",
                "metadata": {}
            }),
        )
        .await
        .expect("provider success");

    let model = fulfillment::Entity::find_by_id(fulfillment_id)
        .one(&db)
        .await
        .expect("load fulfillment")
        .expect("fulfillment exists");
    let mut active: fulfillment::ActiveModel = model.into();
    active.metadata = Set(serde_json::json!({
        "provider_operation": {
            "id": operation.id,
            "operation": "ship"
        }
    }));
    active.update(&db).await.expect("owner metadata update");

    let committed = journal.get(tenant_id, operation.id).await.expect("committed journal");
    assert_eq!(committed.status, PROVIDER_OPERATION_COMMITTED);
    assert!(committed.committed_at.is_some());
}
