use std::{path::PathBuf, sync::Arc};

use chrono::{Duration, Utc};
use rustok_commerce::{
    CheckoutMarketplaceEconomicsCheckpointError, CheckoutMarketplaceEconomicsCheckpointJournal,
    CheckoutMarketplaceEconomicsEvidence, CheckoutOperationStage, CheckoutOperationStatus,
    RecordCheckoutMarketplaceEconomicsCheckpoint,
    entities::{checkout_marketplace_economics_checkpoint, checkout_operation},
};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, Schema, Set,
};
use uuid::Uuid;

struct TestDatabase {
    db: DatabaseConnection,
    path: PathBuf,
}

impl TestDatabase {
    async fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustok-checkout-marketplace-economics-{}.sqlite",
            Uuid::new_v4()
        ));
        let mut options = ConnectOptions::new(format!("sqlite://{}?mode=rwc", path.display()));
        options
            .max_connections(4)
            .min_connections(1)
            .sqlx_logging(false);
        let db = Database::connect(options).await.unwrap();
        db.execute_unprepared("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")
            .await
            .unwrap();

        let backend = DbBackend::Sqlite;
        let schema = Schema::new(backend);
        db.execute(backend.build(
            &schema
                .create_table_from_entity(checkout_operation::Entity)
                .if_not_exists()
                .to_owned(),
        ))
        .await
        .unwrap();
        db.execute(backend.build(
            &schema
                .create_table_from_entity(checkout_marketplace_economics_checkpoint::Entity)
                .if_not_exists()
                .to_owned(),
        ))
        .await
        .unwrap();

        Self { db, path }
    }

    async fn seed_operation(&self) -> SeededOperation {
        let now = Utc::now().fixed_offset();
        let tenant_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let lease_owner = format!("checkpoint-test-{operation_id}");
        checkout_operation::ActiveModel {
            id: Set(operation_id),
            tenant_id: Set(tenant_id),
            cart_id: Set(Uuid::new_v4()),
            idempotency_key: Set(format!("checkout:{operation_id}")),
            request_hash: Set("1".repeat(64)),
            snapshot_hash: Set(Some("2".repeat(64))),
            status: Set(CheckoutOperationStatus::Executing.as_str().to_string()),
            stage: Set(CheckoutOperationStage::PaymentReady.as_str().to_string()),
            order_id: Set(Some(order_id)),
            payment_collection_id: Set(None),
            attempt_count: Set(1),
            lease_owner: Set(Some(lease_owner.clone())),
            lease_expires_at: Set(Some(
                (Utc::now() + Duration::minutes(5)).fixed_offset(),
            )),
            last_error_code: Set(None),
            last_error_message: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            completed_at: Set(None),
        }
        .insert(&self.db)
        .await
        .unwrap();

        SeededOperation {
            tenant_id,
            operation_id,
            order_id,
            lease_owner,
        }
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("sqlite-shm"));
        let _ = std::fs::remove_file(self.path.with_extension("sqlite-wal"));
    }
}

struct SeededOperation {
    tenant_id: Uuid,
    operation_id: Uuid,
    order_id: Uuid,
    lease_owner: String,
}

fn evidence(order_id: Uuid) -> CheckoutMarketplaceEconomicsEvidence {
    CheckoutMarketplaceEconomicsEvidence {
        order_id,
        plan_hash: "3".repeat(64),
        currency_code: "USD".to_string(),
        allocation_count: 1,
        allocation_total_amount: 1_000,
        allocation_set_hash: "4".repeat(64),
        assessment_count: 1,
        commission_total_amount: 100,
        seller_proceeds_total_amount: 900,
        assessment_set_hash: "5".repeat(64),
    }
}

fn input(
    operation: &SeededOperation,
    evidence: CheckoutMarketplaceEconomicsEvidence,
) -> RecordCheckoutMarketplaceEconomicsCheckpoint {
    RecordCheckoutMarketplaceEconomicsCheckpoint {
        tenant_id: operation.tenant_id,
        checkout_operation_id: operation.operation_id,
        lease_owner: operation.lease_owner.clone(),
        evidence,
    }
}

#[tokio::test]
async fn concurrent_identical_checkpoint_writers_adopt_one_row() {
    let database = TestDatabase::new().await;
    let operation = database.seed_operation().await;
    let journal = Arc::new(CheckoutMarketplaceEconomicsCheckpointJournal::new(
        database.db.clone(),
    ));
    let request = input(&operation, evidence(operation.order_id));

    let (first, second) = tokio::join!(
        journal.record(request.clone()),
        journal.record(request.clone())
    );
    let first = first.unwrap();
    let second = second.unwrap();

    assert_eq!(first, second);
    assert_eq!(
        checkout_marketplace_economics_checkpoint::Entity::find()
            .all(&database.db)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn concurrent_conflicting_checkpoint_writer_returns_typed_conflict() {
    let database = TestDatabase::new().await;
    let operation = database.seed_operation().await;
    let journal = Arc::new(CheckoutMarketplaceEconomicsCheckpointJournal::new(
        database.db.clone(),
    ));
    let first = input(&operation, evidence(operation.order_id));
    let mut conflicting_evidence = evidence(operation.order_id);
    conflicting_evidence.commission_total_amount = 125;
    conflicting_evidence.seller_proceeds_total_amount = 875;
    conflicting_evidence.assessment_set_hash = "6".repeat(64);
    let second = input(&operation, conflicting_evidence);

    let (first, second) = tokio::join!(journal.record(first), journal.record(second));
    let outcomes = [first, second];

    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| matches!(
                result,
                Err(CheckoutMarketplaceEconomicsCheckpointError::Conflict(_))
            ))
            .count(),
        1
    );
    assert_eq!(
        checkout_marketplace_economics_checkpoint::Entity::find()
            .all(&database.db)
            .await
            .unwrap()
            .len(),
        1
    );
}
