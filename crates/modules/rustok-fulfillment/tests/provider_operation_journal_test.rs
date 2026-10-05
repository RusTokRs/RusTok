use chrono::Utc;
use rustok_fulfillment::entities::fulfillment;
use rustok_fulfillment::{
    BeginProviderOperation, DeliverFulfillmentInput, FulfillmentProviderOperationJournal,
    FulfillmentProviderOperationRecovery, FulfillmentService, PROVIDER_OPERATION_COMMITTED,
    PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
    PROVIDER_OPERATION_SUCCEEDED, ReopenFulfillmentInput,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DbBackend, EntityTrait, Set, Statement};
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

async fn ensure_test_orders_schema(db: &sea_orm::DatabaseConnection) {
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE orders (id BLOB NOT NULL PRIMARY KEY, tenant_id BLOB NOT NULL, status VARCHAR(32) NOT NULL)"
            .to_string(),
    ))
    .await
    .expect("orders test table should be created");
}

async fn insert_test_order(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    order_id: Uuid,
    status: &str,
) {
    let id_hex = |id: Uuid| -> String {
        id.as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    };
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        format!(
            "INSERT INTO orders (id, tenant_id, status) VALUES (X'{order}', X'{tenant}', '{status}')",
            order = id_hex(order_id),
            tenant = id_hex(tenant_id),
            status = status
        ),
    ))
    .await
    .expect("test order should be inserted");
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
        journal.get(wrong_tenant, operation.id).await.is_err(),
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
async fn provider_result_writer_rejects_mismatched_journal_identity() {
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
            operation: "ship".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "result-identity-guard".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": tenant_id,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "result-identity-guard",
                "metadata": {}
            }),
        })
        .await
        .expect("journal operation");

    journal
        .claim_execution(tenant_id, operation.id)
        .await
        .expect("claim")
        .expect("operation should be claimable");

    let wrong_provider = serde_json::json!({
        "provider_id": "other-carrier",
        "external_reference": "shipment-1",
        "tracking_number": "TRACK-1",
        "metadata": {}
    });
    let wrong_provider_error = journal
        .mark_provider_succeeded(
            tenant_id,
            operation.id,
            Some("shipment-1".to_string()),
            wrong_provider,
        )
        .await
        .expect_err("journal writer must reject a result for another provider");
    assert!(matches!(
        wrong_provider_error,
        rustok_fulfillment::error::FulfillmentError::ProviderResultInvalid(_)
    ));

    let wrong_reference = serde_json::json!({
        "provider_id": "carrier",
        "external_reference": "shipment-2",
        "tracking_number": "TRACK-1",
        "metadata": {}
    });
    let wrong_reference_error = journal
        .mark_provider_succeeded(
            tenant_id,
            operation.id,
            Some("shipment-1".to_string()),
            wrong_reference,
        )
        .await
        .expect_err("journal writer must reject mismatched provider reference");
    assert!(matches!(
        wrong_reference_error,
        rustok_fulfillment::error::FulfillmentError::ProviderResultInvalid(_)
    ));

    let scalar_metadata_error = journal
        .mark_provider_succeeded(
            tenant_id,
            operation.id,
            Some("shipment-1".to_string()),
            serde_json::json!({
                "provider_id": "carrier",
                "external_reference": "shipment-1",
                "tracking_number": "TRACK-1",
                "metadata": "not-an-object"
            }),
        )
        .await
        .expect_err("journal writer must reject non-object provider metadata");
    assert!(matches!(
        scalar_metadata_error,
        rustok_fulfillment::error::FulfillmentError::ProviderResultInvalid(_)
    ));

    let oversized_reference_error = journal
        .mark_provider_succeeded(
            tenant_id,
            operation.id,
            Some("r".repeat(192)),
            serde_json::json!({
                "provider_id": "carrier",
                "external_reference": null,
                "tracking_number": "TRACK-1",
                "metadata": {}
            }),
        )
        .await
        .expect_err("journal writer must reject oversized provider references");
    assert!(matches!(
        oversized_reference_error,
        rustok_fulfillment::error::FulfillmentError::ProviderResultInvalid(_)
    ));

    let oversized_reconciliation_reference_error = journal
        .mark_execution_reconciliation_required(
            tenant_id,
            operation.id,
            Some("r".repeat(192)),
            None,
            "provider outcome is unknown",
        )
        .await
        .expect_err("reconciliation writer must reject oversized provider references without a result");
    assert!(matches!(
        oversized_reconciliation_reference_error,
        rustok_fulfillment::error::FulfillmentError::Validation(_)
    ));

    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("operation should remain readable after rejected reconciliation reference");
    assert_eq!(
        current.status,
        rustok_fulfillment::PROVIDER_OPERATION_EXECUTING,
        "invalid provider reference must not mutate the executing operation"
    );
    assert!(
        current.provider_reference.is_none(),
        "invalid provider reference must not be persisted"
    );

    let unresolved_result = journal
        .mark_execution_reconciliation_required(
            tenant_id,
            operation.id,
            Some("shipment-1".to_string()),
            Some(serde_json::json!({
                "provider_id": "other-carrier",
                "external_reference": "shipment-1",
                "tracking_number": "TRACK-1",
                "metadata": {}
            })),
            "provider result identity mismatch",
        )
        .await
        .expect_err("reconciliation writer must reject a mismatched provider result");
    assert!(matches!(
        unresolved_result,
        rustok_fulfillment::error::FulfillmentError::ProviderResultInvalid(_)
    ));

    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("operation should remain readable");
    assert_eq!(
        current.status,
        rustok_fulfillment::PROVIDER_OPERATION_EXECUTING,
        "identity rejection must not destroy the unresolved execution state"
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
        .expect("matching provider result should persist");
    assert_eq!(
        succeeded.provider_reference.as_deref(),
        Some("shipment-1")
    );
}

#[tokio::test]
async fn provider_reconciliation_rollback_blocks_unresolved_external_outcomes() {
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
            operation: "ship".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "rollback-must-block".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": tenant_id,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "rollback-must-block",
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
    let unresolved = journal
        .mark_execution_reconciliation_required(
            tenant_id,
            operation.id,
            None,
            None,
            "external outcome is unknown",
        )
        .await
        .expect("reconciliation state");

    assert_eq!(
        unresolved.status,
        PROVIDER_OPERATION_RECONCILIATION_REQUIRED
    );
    assert!(unresolved.provider_result.is_none());

    let migration = rustok_fulfillment::migrations::migrations()
        .into_iter()
        .nth(8)
        .expect("reconciliation migration should exist");
    let rollback = migration.down(&SchemaManager::new(&db)).await;

    assert!(
        rollback.is_err(),
        "rollback must be blocked while an external outcome is unresolved"
    );

    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("provider operation remains readable after blocked rollback");
    assert_eq!(
        current.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
        "blocked rollback must not make an unresolved operation retryable"
    );
    assert!(current.provider_result.is_none());
}

#[tokio::test]
async fn checkout_label_payment_rollback_blocks_retryable_unpaid_operation() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();

    ensure_test_orders_schema(&db).await;
    insert_test_order(&db, tenant_id, order_id, "pending").await;

    insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;

    let fulfillment_model = fulfillment::Entity::find_by_id(fulfillment_id)
        .one(&db)
        .await
        .expect("load fulfillment")
        .expect("fulfillment exists");
    let mut fulfillment_active: fulfillment::ActiveModel = fulfillment_model.into();
    fulfillment_active.order_id = Set(order_id);
    fulfillment_active
        .update(&db)
        .await
        .expect("bind fulfillment to unpaid test order");

    let migrations = rustok_fulfillment::migrations::migrations();
    let manager = SchemaManager::new(&db);
    migrations
        .get(6)
        .expect("provider journal migration should exist")
        .up(&manager)
        .await
        .expect("provider journal schema should install");
    migrations
        .get(7)
        .expect("provider receipt migration should exist")
        .up(&manager)
        .await
        .expect("provider receipt schema should install");
    migrations
        .get(8)
        .expect("reconciliation migration should exist")
        .up(&manager)
        .await
        .expect("reconciliation schema should install");
    migrations
        .get(9)
        .expect("checkout label payment migration should exist")
        .up(&manager)
        .await
        .expect("checkout label payment guard should install");

    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let operation = journal
        .begin(BeginProviderOperation {
            tenant_id,
            fulfillment_id,
            operation: "create_label".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "rollback-payment-guard".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": tenant_id,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "rollback-payment-guard",
                "metadata": {}
            }),
        })
        .await
        .expect("provider operation");

    let rollback = migrations
        .get(9)
        .expect("checkout label payment migration should exist")
        .down(&manager)
        .await;
    assert!(
        rollback.is_err(),
        "rollback must be blocked while a retryable create-label operation targets an unpaid order"
    );

    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("provider operation remains readable after blocked rollback");
    assert_eq!(
        current.status,
        rustok_fulfillment::PROVIDER_OPERATION_PENDING
    );

    assert!(
        journal
            .claim_execution(tenant_id, operation.id)
            .await
            .is_err(),
        "payment guard must remain active after blocked rollback"
    );
}

#[tokio::test]
async fn cancelled_order_quarantines_existing_executing_checkout_label_on_migration_upgrade() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();

    ensure_test_orders_schema(&db).await;
    insert_test_order(&db, tenant_id, order_id, "paid").await;
    insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;

    let fulfillment_model = fulfillment::Entity::find_by_id(fulfillment_id)
        .one(&db)
        .await
        .expect("load fulfillment")
        .expect("fulfillment exists");
    let mut fulfillment_active: fulfillment::ActiveModel = fulfillment_model.into();
    fulfillment_active.order_id = Set(order_id);
    fulfillment_active
        .update(&db)
        .await
        .expect("bind fulfillment to paid test order");

    let migrations = rustok_fulfillment::migrations::migrations();
    let manager = SchemaManager::new(&db);
    for index in 6..=9 {
        migrations
            .get(index)
            .expect("required provider migration should exist")
            .up(&manager)
            .await
            .expect("required provider migration should install");
    }

    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let operation = journal
        .begin(BeginProviderOperation {
            tenant_id,
            fulfillment_id,
            operation: "create_label".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "cancel-upgrade-quarantine".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": tenant_id,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "cancel-upgrade-quarantine",
                "metadata": {}
            }),
        })
        .await
        .expect("provider operation");

    journal
        .claim_execution(tenant_id, operation.id)
        .await
        .expect("claim")
        .expect("operation should be claimable while order is paid");

    let order_hex = |id: Uuid| -> String {
        id.as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    };
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        format!(
            "UPDATE orders SET status = 'cancelled' WHERE id = X'{order}' AND tenant_id = X'{tenant}'",
            order = order_hex(order_id),
            tenant = order_hex(tenant_id),
        ),
    ))
    .await
    .expect("order cancellation should persist");

    migrations
        .get(10)
        .expect("cancellation cleanup migration should exist")
        .up(&manager)
        .await
        .expect("cancellation cleanup migration should install");

    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("provider operation remains readable after migration upgrade");
    assert_eq!(
        current.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
        "an already executing label operation on a cancelled order must be quarantined during upgrade"
    );
    assert!(current.provider_completed_at.is_some());
    assert_eq!(
        current.error_message.as_deref(),
        Some("order was cancelled while create-label provider execution was in progress")
    );
}

#[tokio::test]
async fn cancelled_order_quarantines_executing_checkout_label() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();

    ensure_test_orders_schema(&db).await;
    insert_test_order(&db, tenant_id, order_id, "paid").await;
    insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;

    let fulfillment_model = fulfillment::Entity::find_by_id(fulfillment_id)
        .one(&db)
        .await
        .expect("load fulfillment")
        .expect("fulfillment exists");
    let mut fulfillment_active: fulfillment::ActiveModel = fulfillment_model.into();
    fulfillment_active.order_id = Set(order_id);
    fulfillment_active
        .update(&db)
        .await
        .expect("bind fulfillment to paid test order");

    let migrations = rustok_fulfillment::migrations::migrations();
    let manager = SchemaManager::new(&db);
    for index in 6..=10 {
        migrations
            .get(index)
            .expect("required provider migration should exist")
            .up(&manager)
            .await
            .expect("required provider migration should install");
    }

    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let pending_operation = journal
        .begin(BeginProviderOperation {
            tenant_id,
            fulfillment_id,
            operation: "create_label".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "cancel-pending-cleanup".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": tenant_id,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "cancel-pending-cleanup",
                "metadata": {}
            }),
        })
        .await
        .expect("pending provider operation");

    let operation = journal
        .begin(BeginProviderOperation {
            tenant_id,
            fulfillment_id,
            operation: "create_label".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "cancel-quarantine".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": tenant_id,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "cancel-quarantine",
                "metadata": {}
            }),
        })
        .await
        .expect("provider operation");

    journal
        .claim_execution(tenant_id, operation.id)
        .await
        .expect("claim")
        .expect("operation should be claimable while order is paid");

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        format!(
            "UPDATE orders SET status = 'cancelled' WHERE id = X'{order}' AND tenant_id = X'{tenant}'",
            order = order_id.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            tenant = tenant_id.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        ),
    ))
    .await
    .expect("order cancellation should persist");

    assert!(
        journal.get(tenant_id, pending_operation.id).await.is_err(),
        "pending checkout label operations must be deleted when the order is cancelled"
    );

    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("provider operation remains readable after cancellation");
    assert_eq!(
        current.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
        "in-flight label execution must be quarantined when the order is cancelled"
    );
    assert!(current.provider_completed_at.is_some());
    assert_eq!(
        current.error_message.as_deref(),
        Some("order was cancelled while create-label provider execution was in progress")
    );
    assert!(
        journal
            .claim_execution(tenant_id, operation.id)
            .await
            .expect("reconciliation operation should be non-claimable")
            .is_none(),
        "a quarantined operation must not become retryable"
    );
}

#[tokio::test]
async fn premature_checkout_label_insert_requires_paid_order() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();

    ensure_test_orders_schema(&db).await;
    insert_test_order(&db, tenant_id, order_id, "pending").await;
    insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;

    let fulfillment_model = fulfillment::Entity::find_by_id(fulfillment_id)
        .one(&db)
        .await
        .expect("load fulfillment")
        .expect("fulfillment exists");
    let mut fulfillment_active: fulfillment::ActiveModel = fulfillment_model.into();
    fulfillment_active.order_id = Set(order_id);
    fulfillment_active
        .update(&db)
        .await
        .expect("bind fulfillment to unpaid test order");

    let migrations = rustok_fulfillment::migrations::migrations();
    let manager = SchemaManager::new(&db);
    for index in 6..=11 {
        migrations
            .get(index)
            .expect("required provider migration should exist")
            .up(&manager)
            .await
            .expect("required provider migration should install");
    }

    let now = Utc::now().fixed_offset();
    let result = rustok_fulfillment::entities::provider_operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        fulfillment_id: Set(fulfillment_id),
        operation: Set("create_label".to_string()),
        provider_id: Set("carrier".to_string()),
        idempotency_key: Set("premature-insert".to_string()),
        status: Set("executing".to_string()),
        request_payload: Set(serde_json::json!({
            "tenant_id": tenant_id,
            "fulfillment_id": fulfillment_id
        })),
        provider_reference: Set(None),
        provider_result: Set(None),
        error_message: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        provider_completed_at: Set(None),
        committed_at: Set(None),
    }
    .insert(&db)
    .await;

    assert!(
        result.is_err(),
        "an executing checkout label operation must not be inserted before payment"
    );
}

#[tokio::test]
async fn premature_checkout_label_migration_quarantines_existing_unpaid_execution() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();

    ensure_test_orders_schema(&db).await;
    insert_test_order(&db, tenant_id, order_id, "pending").await;
    insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;

    let fulfillment_model = fulfillment::Entity::find_by_id(fulfillment_id)
        .one(&db)
        .await
        .expect("load fulfillment")
        .expect("fulfillment exists");
    let mut fulfillment_active: fulfillment::ActiveModel = fulfillment_model.into();
    fulfillment_active.order_id = Set(order_id);
    fulfillment_active
        .update(&db)
        .await
        .expect("bind fulfillment to unpaid test order");

    let migrations = rustok_fulfillment::migrations::migrations();
    let manager = SchemaManager::new(&db);
    migrations
        .get(6)
        .expect("provider journal migration should exist")
        .up(&manager)
        .await
        .expect("provider journal migration should install");

    let now = Utc::now().fixed_offset();
    let operation = rustok_fulfillment::entities::provider_operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        fulfillment_id: Set(fulfillment_id),
        operation: Set("create_label".to_string()),
        provider_id: Set("carrier".to_string()),
        idempotency_key: Set("quarantine-on-upgrade".to_string()),
        status: Set("executing".to_string()),
        request_payload: Set(serde_json::json!({
            "tenant_id": tenant_id,
            "fulfillment_id": fulfillment_id
        })),
        provider_reference: Set(None),
        provider_result: Set(None),
        error_message: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        provider_completed_at: Set(None),
        committed_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("legacy executing operation should be insertable before the new guard");

    for index in 7..=10 {
        migrations
            .get(index)
            .expect("required provider migration should exist")
            .up(&manager)
            .await
            .expect("required provider migration should install");
    }
    migrations
        .get(11)
        .expect("premature insert guard migration should exist")
        .up(&manager)
        .await
        .expect("premature insert guard migration should install");

    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("quarantined operation should remain readable");
    assert_eq!(current.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED);
    assert!(current.provider_completed_at.is_some());
    assert_eq!(
        current.error_message.as_deref(),
        Some("create-label execution started before order payment")
    );
}

#[tokio::test]
async fn premature_checkout_label_rollback_requires_execution_quiescence() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let fulfillment_id = Uuid::new_v4();

    ensure_test_orders_schema(&db).await;
    insert_test_order(&db, tenant_id, order_id, "paid").await;
    insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;

    let fulfillment_model = fulfillment::Entity::find_by_id(fulfillment_id)
        .one(&db)
        .await
        .expect("load fulfillment")
        .expect("fulfillment exists");
    let mut fulfillment_active: fulfillment::ActiveModel = fulfillment_model.into();
    fulfillment_active.order_id = Set(order_id);
    fulfillment_active
        .update(&db)
        .await
        .expect("bind fulfillment to paid test order");

    let migrations = rustok_fulfillment::migrations::migrations();
    let manager = SchemaManager::new(&db);
    for index in 6..=11 {
        migrations
            .get(index)
            .expect("required provider migration should exist")
            .up(&manager)
            .await
            .expect("required provider migration should install");
    }

    let journal = FulfillmentProviderOperationJournal::new(db.clone());
    let operation = journal
        .begin(BeginProviderOperation {
            tenant_id,
            fulfillment_id,
            operation: "create_label".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "rollback-quiescence".to_string(),
            request_payload: serde_json::json!({
                "tenant_id": tenant_id,
                "fulfillment_id": fulfillment_id,
                "idempotency_key": "rollback-quiescence",
                "metadata": {}
            }),
        })
        .await
        .expect("provider operation");

    journal
        .claim_execution(tenant_id, operation.id)
        .await
        .expect("claim")
        .expect("operation should be executing");

    let rollback = migrations
        .get(11)
        .expect("premature insert guard migration should exist")
        .down(&manager)
        .await;
    assert!(
        rollback.is_err(),
        "rollback must be blocked while checkout label execution is in flight"
    );

    let current = journal
        .get(tenant_id, operation.id)
        .await
        .expect("operation remains readable after blocked rollback");
    assert_eq!(
        current.status,
        rustok_fulfillment::PROVIDER_OPERATION_EXECUTING
    );
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

    let oversized_tracking_number = serde_json::json!({
        "provider_id": "carrier",
        "external_reference": "label-1",
        "tracking_number": "T".repeat(101),
        "metadata": {}
    });
    assert!(
        recovery
            .resolve_unknown_as_succeeded(
                tenant_id,
                operation.id,
                Some("label-1".to_string()),
                oversized_tracking_number,
            )
            .await
            .is_err(),
        "manual reconciliation must reject tracking numbers that cannot fit Fulfillment persistence"
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
        current.status, PROVIDER_OPERATION_SUCCEEDED,
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
        current.status, PROVIDER_OPERATION_SUCCEEDED,
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

    let committed = journal
        .get(tenant_id, operation.id)
        .await
        .expect("committed journal");
    assert_eq!(committed.status, PROVIDER_OPERATION_COMMITTED);
    assert!(committed.committed_at.is_some());
}
