use std::{path::PathBuf, sync::Arc};

use chrono::Utc;
use rust_decimal::Decimal;
use rustok_order::{
    OrderCheckoutIdentityError, OrderCheckoutIdentityJournal, RecordOrderCheckoutIdentity,
    entities::{order, order_checkout_identity},
};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    Schema, Set,
};
use serde_json::json;
use uuid::Uuid;

struct TestDatabase {
    db: DatabaseConnection,
    path: PathBuf,
}

impl TestDatabase {
    async fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustok-order-checkout-identity-{}.sqlite",
            Uuid::new_v4()
        ));
        let mut options = ConnectOptions::new(format!("sqlite://{}?mode=rwc", path.display()));
        options
            .max_connections(4)
            .min_connections(1)
            .sqlx_logging(false);
        let db = Database::connect(options).await.unwrap();
        db.execute_unprepared("PRAGMA journal_mode = WAL;")
            .await
            .unwrap();
        db.execute_unprepared("PRAGMA busy_timeout = 5000;")
            .await
            .unwrap();

        let backend = DbBackend::Sqlite;
        let schema = Schema::new(backend);
        db.execute(
            backend.build(
                &schema
                    .create_table_from_entity(order::Entity)
                    .if_not_exists()
                    .to_owned(),
            ),
        )
        .await
        .unwrap();
        db.execute(
            backend.build(
                &schema
                    .create_table_from_entity(order_checkout_identity::Entity)
                    .if_not_exists()
                    .to_owned(),
            ),
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE UNIQUE INDEX ux_test_order_checkout_identity_order ON order_checkout_identities (tenant_id, order_id);",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE UNIQUE INDEX ux_test_order_checkout_identity_cart ON order_checkout_identities (tenant_id, source_cart_id);",
        )
        .await
        .unwrap();

        Self { db, path }
    }

    async fn seed_order(&self, tenant_id: Uuid) -> Uuid {
        let order_id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();
        order::ActiveModel {
            id: Set(order_id),
            tenant_id: Set(tenant_id),
            channel_id: Set(None),
            channel_slug: Set(None),
            customer_id: Set(None),
            status: Set("pending".to_string()),
            currency_code: Set("USD".to_string()),
            shipping_total: Set(Decimal::ZERO),
            total_amount: Set(Decimal::ZERO),
            tax_total: Set(Decimal::ZERO),
            tax_included: Set(false),
            metadata: Set(json!({})),
            payment_id: Set(None),
            payment_method: Set(None),
            tracking_number: Set(None),
            carrier: Set(None),
            cancellation_reason: Set(None),
            delivered_signature: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            confirmed_at: Set(None),
            paid_at: Set(None),
            shipped_at: Set(None),
            delivered_at: Set(None),
            cancelled_at: Set(None),
        }
        .insert(&self.db)
        .await
        .unwrap();
        order_id
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("sqlite-shm"));
        let _ = std::fs::remove_file(self.path.with_extension("sqlite-wal"));
    }
}

fn input(
    tenant_id: Uuid,
    checkout_operation_id: Uuid,
    order_id: Uuid,
    source_cart_id: Uuid,
) -> RecordOrderCheckoutIdentity {
    RecordOrderCheckoutIdentity {
        tenant_id,
        checkout_operation_id,
        order_id,
        source_cart_id,
        payment_collection_id: None,
        shipping_option_id: None,
        snapshot_hash: "a".repeat(64),
        request_hash: "b".repeat(64),
    }
}

#[tokio::test]
async fn records_and_reads_typed_checkout_identity() {
    let database = TestDatabase::new().await;
    let tenant_id = Uuid::new_v4();
    let order_id = database.seed_order(tenant_id).await;
    let operation_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let journal = OrderCheckoutIdentityJournal::new(database.db.clone());

    let recorded = journal
        .record(input(tenant_id, operation_id, order_id, cart_id))
        .await
        .unwrap();

    assert_eq!(recorded.checkout_operation_id, operation_id);
    assert_eq!(recorded.order_id, order_id);
    assert_eq!(recorded.source_cart_id, Some(cart_id));
    assert_eq!(recorded.payment_collection_id, None);
    assert_eq!(recorded.shipping_option_id, None);
    assert_eq!(
        journal
            .get_by_operation(tenant_id, operation_id)
            .await
            .unwrap(),
        Some(recorded.clone())
    );
    assert_eq!(
        journal.get_by_order(tenant_id, order_id).await.unwrap(),
        Some(recorded.clone())
    );
    assert_eq!(
        journal.get_by_cart(tenant_id, cart_id).await.unwrap(),
        Some(recorded)
    );
}

#[tokio::test]
async fn concurrent_identical_writers_adopt_one_identity() {
    let database = TestDatabase::new().await;
    let tenant_id = Uuid::new_v4();
    let order_id = database.seed_order(tenant_id).await;
    let operation_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let journal = Arc::new(OrderCheckoutIdentityJournal::new(database.db.clone()));
    let request = input(tenant_id, operation_id, order_id, cart_id);

    let (first, second) = tokio::join!(
        journal.record(request.clone()),
        journal.record(request.clone())
    );

    assert_eq!(first.unwrap(), second.unwrap());
}

#[tokio::test]
async fn conflicting_identity_is_typed_conflict() {
    let database = TestDatabase::new().await;
    let tenant_id = Uuid::new_v4();
    let first_order_id = database.seed_order(tenant_id).await;
    let second_order_id = database.seed_order(tenant_id).await;
    let operation_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let journal = OrderCheckoutIdentityJournal::new(database.db.clone());

    journal
        .record(input(tenant_id, operation_id, first_order_id, cart_id))
        .await
        .unwrap();
    let error = journal
        .record(input(tenant_id, operation_id, second_order_id, cart_id))
        .await
        .unwrap_err();

    assert!(matches!(error, OrderCheckoutIdentityError::Conflict(_)));
}
