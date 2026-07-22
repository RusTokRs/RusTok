use chrono::Utc;
use rustok_marketplace_payout::entities::{operation, operation_transfer, payout};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbErr, Set,
    Statement, TryGetable,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn payout_migrations_repair_canonical_names_and_reapply_cleanly() {
    let db = setup_database().await;
    apply_all(&db).await;
    let manager = SchemaManager::new(&db);

    for table in [
        "marketplace_payouts",
        "marketplace_payout_items",
        "marketplace_payout_receipts",
        "marketplace_payout_operations",
        "marketplace_payout_operation_transfers",
    ] {
        assert!(manager.has_table(table).await.unwrap(), "missing {table}");
    }
    for legacy in ["payouts", "payout_items", "payout_receipts"] {
        assert!(
            !manager.has_table(legacy).await.unwrap(),
            "legacy table {legacy} must be repaired"
        );
    }

    rollback_all(&db).await;
    apply_all(&db).await;
    assert!(
        SchemaManager::new(&db)
            .has_table("marketplace_payout_operations")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn payout_name_repair_preserves_existing_rows() {
    let db = setup_database().await;
    let manager = SchemaManager::new(&db);
    let mut migrations = rustok_marketplace_payout::migrations::migrations();
    let initial = migrations.remove(0);
    let operation_journal = migrations.remove(0);
    initial.up(&manager).await.unwrap();

    let payout_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO payouts (id, tenant_id, seller_id, currency_code, total_amount, status, scheduled_for, metadata, created_at, updated_at) VALUES ('{payout_id}', '{tenant_id}', '{seller_id}', 'USD', 1250, 'scheduled', CURRENT_TIMESTAMP, '{{}}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
    ))
    .await
    .unwrap();

    operation_journal.up(&manager).await.unwrap();

    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            format!("SELECT total_amount FROM marketplace_payouts WHERE id = '{payout_id}'"),
        ))
        .await
        .unwrap()
        .expect("renamed payout row must remain available");
    assert_eq!(row.try_get::<i64>("", "total_amount").unwrap(), 1_250);
}

#[tokio::test]
async fn payout_name_repair_rejects_mixed_legacy_and_canonical_state() {
    let db = setup_database().await;
    let manager = SchemaManager::new(&db);
    let mut migrations = rustok_marketplace_payout::migrations::migrations();
    let initial = migrations.remove(0);
    let operation_journal = migrations.remove(0);
    initial.up(&manager).await.unwrap();
    db.execute_unprepared("CREATE TABLE marketplace_payouts (id TEXT PRIMARY KEY)")
        .await
        .unwrap();

    let error = operation_journal
        .up(&manager)
        .await
        .expect_err("mixed legacy/canonical names must fail closed");

    assert!(error.to_string().contains("mixed legacy/canonical state"));
    assert!(
        !manager
            .has_table("marketplace_payout_operations")
            .await
            .unwrap()
    );
    assert!(
        !manager
            .has_table("marketplace_payout_legacy_name_repair_marker")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn payout_operation_schema_enforces_identity_capacity_and_tenant_links() {
    let db = setup_database().await;
    apply_all(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();

    let operation = operation::ActiveModel {
        id: Set(operation_id),
        tenant_id: Set(tenant_id),
        actor_id: Set(actor_id),
        seller_id: Set(seller_id),
        currency_code: Set("USD".to_string()),
        idempotency_key: Set("payout-operation".to_string()),
        request_hash: Set("a".repeat(64)),
        request_json: Set(serde_json::json!({"entry_ids": [Uuid::new_v4()]})),
        status: Set(operation::MarketplacePayoutOperationStatus::Pending),
        stage: Set(operation::MarketplacePayoutOperationStage::Created),
        payout_id: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        completed_at: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();
    assert_eq!(operation.id, operation_id);

    let duplicate_key = operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        actor_id: Set(actor_id),
        seller_id: Set(seller_id),
        currency_code: Set("USD".to_string()),
        idempotency_key: Set("payout-operation".to_string()),
        request_hash: Set("b".repeat(64)),
        request_json: Set(serde_json::json!({"entry_ids": [Uuid::new_v4()]})),
        status: Set(operation::MarketplacePayoutOperationStatus::Pending),
        stage: Set(operation::MarketplacePayoutOperationStage::Created),
        payout_id: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        completed_at: Set(None),
    }
    .insert(&db)
    .await;
    assert!(is_constraint_error(&duplicate_key.unwrap_err()));

    let invalid_currency = operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        actor_id: Set(actor_id),
        seller_id: Set(seller_id),
        currency_code: Set("usd".to_string()),
        idempotency_key: Set("invalid-currency".to_string()),
        request_hash: Set("c".repeat(64)),
        request_json: Set(serde_json::json!({})),
        status: Set(operation::MarketplacePayoutOperationStatus::Pending),
        stage: Set(operation::MarketplacePayoutOperationStage::Created),
        payout_id: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        completed_at: Set(None),
    }
    .insert(&db)
    .await;
    assert!(is_constraint_error(&invalid_currency.unwrap_err()));

    operation_transfer::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        operation_id: Set(operation_id),
        sequence_no: Set(0),
        order_id: Set(Uuid::new_v4()),
        transfer_kind: Set(operation_transfer::MarketplacePayoutOperationTransferKind::ReserveHold),
        status: Set(operation_transfer::MarketplacePayoutOperationTransferStatus::Pending),
        idempotency_key: Set(format!("marketplace-payout:{operation_id}:reserve:0:v1")),
        request_hash: Set("d".repeat(64)),
        request_json: Set(serde_json::json!({"lines": []})),
        total_amount: Set(1_000),
        ledger_transfer_id: Set(None),
        ledger_transaction_id: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        completed_at: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();

    let cross_tenant = operation_transfer::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(Uuid::new_v4()),
        operation_id: Set(operation_id),
        sequence_no: Set(1),
        order_id: Set(Uuid::new_v4()),
        transfer_kind: Set(operation_transfer::MarketplacePayoutOperationTransferKind::ReserveHold),
        status: Set(operation_transfer::MarketplacePayoutOperationTransferStatus::Pending),
        idempotency_key: Set(format!("marketplace-payout:{operation_id}:reserve:1:v1")),
        request_hash: Set("e".repeat(64)),
        request_json: Set(serde_json::json!({"lines": []})),
        total_amount: Set(1_000),
        ledger_transfer_id: Set(None),
        ledger_transaction_id: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        completed_at: Set(None),
    }
    .insert(&db)
    .await;
    assert!(is_constraint_error(&cross_tenant.unwrap_err()));

    let invalid_amount = operation_transfer::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        operation_id: Set(operation_id),
        sequence_no: Set(2),
        order_id: Set(Uuid::new_v4()),
        transfer_kind: Set(operation_transfer::MarketplacePayoutOperationTransferKind::ReserveHold),
        status: Set(operation_transfer::MarketplacePayoutOperationTransferStatus::Pending),
        idempotency_key: Set(format!("marketplace-payout:{operation_id}:reserve:2:v1")),
        request_hash: Set("f".repeat(64)),
        request_json: Set(serde_json::json!({"lines": []})),
        total_amount: Set(0),
        ledger_transfer_id: Set(None),
        ledger_transaction_id: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        completed_at: Set(None),
    }
    .insert(&db)
    .await;
    assert!(is_constraint_error(&invalid_amount.unwrap_err()));

    payout::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        seller_id: Set(seller_id),
        currency_code: Set("USD".to_string()),
        total_amount: Set(1_000),
        status: Set("scheduled".to_string()),
        scheduled_for: Set(now),
        destination_reference: Set(None),
        external_reference: Set(None),
        failure_code: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        paid_at: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_payout_operation_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .unwrap();
    db
}

async fn apply_all(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_marketplace_payout::migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
}

async fn rollback_all(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    let mut migrations = rustok_marketplace_payout::migrations::migrations();
    migrations.reverse();
    for migration in migrations {
        migration.down(&manager).await.unwrap();
    }
}

fn is_constraint_error(error: &DbErr) -> bool {
    error.sql_err().is_some() || matches!(error, DbErr::Exec(_) | DbErr::Query(_))
}
