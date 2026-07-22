use chrono::Utc;
use rustok_marketplace_payout::entities::{payout, provider_operation};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbErr, Set,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn provider_operation_schema_enforces_identity_tenant_and_outcome_invariants() {
    let db = setup_database().await;
    apply_all(&db).await;
    let manager = SchemaManager::new(&db);
    assert!(manager
        .has_table("marketplace_payout_provider_operations")
        .await
        .unwrap());

    let tenant_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let payout_id = insert_payout(&db, tenant_id, seller_id).await;
    let now = Utc::now().fixed_offset();

    provider_operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        payout_id: Set(payout_id),
        operation: Set(provider_operation::MarketplacePayoutProviderOperationKind::Submit),
        provider_id: Set("gateway".to_string()),
        idempotency_key: Set("submit-payout".to_string()),
        request_hash: Set("a".repeat(64)),
        request_json: Set(serde_json::json!({"amount": 1000})),
        status: Set(provider_operation::MarketplacePayoutProviderOperationStatus::Pending),
        provider_reference: Set(None),
        provider_result_json: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        provider_completed_at: Set(None),
        committed_at: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();

    let duplicate_key_payout = insert_payout(&db, tenant_id, seller_id).await;
    let duplicate_key = provider_operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        payout_id: Set(duplicate_key_payout),
        operation: Set(provider_operation::MarketplacePayoutProviderOperationKind::Lookup),
        provider_id: Set("gateway".to_string()),
        idempotency_key: Set("submit-payout".to_string()),
        request_hash: Set("b".repeat(64)),
        request_json: Set(serde_json::json!({})),
        status: Set(provider_operation::MarketplacePayoutProviderOperationStatus::Pending),
        provider_reference: Set(None),
        provider_result_json: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        provider_completed_at: Set(None),
        committed_at: Set(None),
    }
    .insert(&db)
    .await;
    assert!(is_constraint_error(&duplicate_key.unwrap_err()));

    let duplicate_submit = provider_operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        payout_id: Set(payout_id),
        operation: Set(provider_operation::MarketplacePayoutProviderOperationKind::Submit),
        provider_id: Set("other-gateway".to_string()),
        idempotency_key: Set("other-submit-key".to_string()),
        request_hash: Set("c".repeat(64)),
        request_json: Set(serde_json::json!({})),
        status: Set(provider_operation::MarketplacePayoutProviderOperationStatus::Pending),
        provider_reference: Set(None),
        provider_result_json: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        provider_completed_at: Set(None),
        committed_at: Set(None),
    }
    .insert(&db)
    .await;
    assert!(is_constraint_error(&duplicate_submit.unwrap_err()));

    let cross_tenant = provider_operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(Uuid::new_v4()),
        payout_id: Set(payout_id),
        operation: Set(provider_operation::MarketplacePayoutProviderOperationKind::Lookup),
        provider_id: Set("gateway".to_string()),
        idempotency_key: Set("cross-tenant".to_string()),
        request_hash: Set("d".repeat(64)),
        request_json: Set(serde_json::json!({})),
        status: Set(provider_operation::MarketplacePayoutProviderOperationStatus::Pending),
        provider_reference: Set(None),
        provider_result_json: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        provider_completed_at: Set(None),
        committed_at: Set(None),
    }
    .insert(&db)
    .await;
    assert!(is_constraint_error(&cross_tenant.unwrap_err()));

    let second_payout = insert_payout(&db, tenant_id, seller_id).await;
    let false_commit = provider_operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        payout_id: Set(second_payout),
        operation: Set(provider_operation::MarketplacePayoutProviderOperationKind::Submit),
        provider_id: Set("gateway".to_string()),
        idempotency_key: Set("false-commit".to_string()),
        request_hash: Set("e".repeat(64)),
        request_json: Set(serde_json::json!({})),
        status: Set(provider_operation::MarketplacePayoutProviderOperationStatus::Committed),
        provider_reference: Set(Some("transfer-1".to_string())),
        provider_result_json: Set(None),
        attempt_count: Set(1),
        revision: Set(1),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        provider_completed_at: Set(None),
        committed_at: Set(Some(now)),
    }
    .insert(&db)
    .await;
    assert!(is_constraint_error(&false_commit.unwrap_err()));

    let third_payout = insert_payout(&db, tenant_id, seller_id).await;
    provider_operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        payout_id: Set(third_payout),
        operation: Set(provider_operation::MarketplacePayoutProviderOperationKind::Submit),
        provider_id: Set("gateway".to_string()),
        idempotency_key: Set("committed-submit".to_string()),
        request_hash: Set("f".repeat(64)),
        request_json: Set(serde_json::json!({"amount": 1000})),
        status: Set(provider_operation::MarketplacePayoutProviderOperationStatus::Committed),
        provider_reference: Set(Some("transfer-2".to_string())),
        provider_result_json: Set(Some(serde_json::json!({
            "provider_id": "gateway",
            "status": "paid",
            "external_reference": "transfer-2"
        }))),
        attempt_count: Set(1),
        revision: Set(2),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        provider_completed_at: Set(Some(now)),
        committed_at: Set(Some(now)),
    }
    .insert(&db)
    .await
    .unwrap();
}

async fn insert_payout(db: &DatabaseConnection, tenant_id: Uuid, seller_id: Uuid) -> Uuid {
    let payout_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    payout::ActiveModel {
        id: Set(payout_id),
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
    .insert(db)
    .await
    .unwrap();
    payout_id
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_payout_provider_operation_{}?mode=memory&cache=shared",
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

fn is_constraint_error(error: &DbErr) -> bool {
    error.sql_err().is_some() || matches!(error, DbErr::Exec(_) | DbErr::Query(_))
}
