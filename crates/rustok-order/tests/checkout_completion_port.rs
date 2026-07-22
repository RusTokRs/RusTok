use std::{path::PathBuf, sync::Arc, time::Duration};

use chrono::Utc;
use rust_decimal::Decimal;
use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortErrorKind};
use rustok_order::{
    CheckoutCompletionPort, CheckoutResultByOperationRequest, CheckoutResultRequest,
    CompleteCheckoutPortRequest,
    entities::{
        order, order_adjustment, order_checkout_identity, order_line_item,
        order_line_item_translation, order_tax_line,
    },
    in_process_checkout_completion_port,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
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
            "rustok-order-checkout-completion-{}.sqlite",
            Uuid::new_v4()
        ));
        let mut options = ConnectOptions::new(format!("sqlite://{}?mode=rwc", path.display()));
        options
            .max_connections(4)
            .min_connections(1)
            .sqlx_logging(false);
        let db = Database::connect(options).await.unwrap();
        db.execute_unprepared("PRAGMA busy_timeout = 5000;")
            .await
            .unwrap();

        let backend = DbBackend::Sqlite;
        let schema = Schema::new(backend);
        for statement in [
            schema
                .create_table_from_entity(order::Entity)
                .if_not_exists()
                .to_owned(),
            schema
                .create_table_from_entity(order_line_item::Entity)
                .if_not_exists()
                .to_owned(),
            schema
                .create_table_from_entity(order_line_item_translation::Entity)
                .if_not_exists()
                .to_owned(),
            schema
                .create_table_from_entity(order_adjustment::Entity)
                .if_not_exists()
                .to_owned(),
            schema
                .create_table_from_entity(order_tax_line::Entity)
                .if_not_exists()
                .to_owned(),
            schema
                .create_table_from_entity(order_checkout_identity::Entity)
                .if_not_exists()
                .to_owned(),
        ] {
            db.execute(backend.build(&statement)).await.unwrap();
        }
        db.execute_unprepared(
            "CREATE UNIQUE INDEX ux_test_completion_identity_order ON order_checkout_identities (tenant_id, order_id);",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE UNIQUE INDEX ux_test_completion_identity_cart ON order_checkout_identities (tenant_id, source_cart_id);",
        )
        .await
        .unwrap();

        Self { db, path }
    }

    async fn seed_completed_identity(&self) -> SeededIdentity {
        let tenant_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        let cart_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let payment_collection_id = Uuid::new_v4();
        let shipping_option_id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();

        order::ActiveModel {
            id: Set(order_id),
            tenant_id: Set(tenant_id),
            channel_id: Set(None),
            channel_slug: Set(None),
            customer_id: Set(None),
            status: Set("confirmed".to_string()),
            currency_code: Set("USD".to_string()),
            shipping_total: Set(Decimal::new(500, 2)),
            total_amount: Set(Decimal::new(2500, 2)),
            tax_total: Set(Decimal::new(200, 2)),
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
            confirmed_at: Set(Some(now)),
            paid_at: Set(None),
            shipped_at: Set(None),
            delivered_at: Set(None),
            cancelled_at: Set(None),
        }
        .insert(&self.db)
        .await
        .unwrap();

        order_checkout_identity::ActiveModel {
            checkout_operation_id: Set(operation_id),
            tenant_id: Set(tenant_id),
            order_id: Set(order_id),
            source_cart_id: Set(Some(cart_id)),
            payment_collection_id: Set(Some(payment_collection_id)),
            shipping_option_id: Set(Some(shipping_option_id)),
            snapshot_hash: Set(Some("a".repeat(64))),
            request_hash: Set(Some("b".repeat(64))),
            created_at: Set(now),
        }
        .insert(&self.db)
        .await
        .unwrap();

        SeededIdentity {
            tenant_id,
            operation_id,
            cart_id,
            order_id,
            payment_collection_id,
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

struct SeededIdentity {
    tenant_id: Uuid,
    operation_id: Uuid,
    cart_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
}

fn read_context(tenant_id: Uuid, correlation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("checkout-completion-test"),
        PLATFORM_FALLBACK_LOCALE,
        correlation,
    )
    .with_deadline(Duration::from_secs(3))
}

fn write_context(tenant_id: Uuid, operation_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(Uuid::new_v4().to_string()),
        PLATFORM_FALLBACK_LOCALE,
        format!("checkout-completion-test:{operation_id}"),
    )
    .with_causation_id(operation_id.to_string())
    .with_idempotency_key(format!("checkout:{operation_id}"))
    .with_deadline(Duration::from_secs(3))
}

fn port(database: &TestDatabase) -> Arc<dyn CheckoutCompletionPort> {
    in_process_checkout_completion_port(
        database.db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.db.clone()))),
    )
}

#[tokio::test]
async fn reads_checkout_result_by_cart_and_operation() {
    let database = TestDatabase::new().await;
    let seeded = database.seed_completed_identity().await;
    let port = port(&database);

    let by_cart = port
        .read_checkout_result(
            read_context(seeded.tenant_id, "checkout-result-by-cart"),
            CheckoutResultRequest {
                cart_id: seeded.cart_id,
            },
        )
        .await
        .unwrap();
    let by_operation = port
        .read_checkout_result_by_operation(
            read_context(seeded.tenant_id, "checkout-result-by-operation"),
            CheckoutResultByOperationRequest {
                checkout_operation_id: seeded.operation_id,
            },
        )
        .await
        .unwrap();

    assert_eq!(by_cart, by_operation);
    assert_eq!(by_cart.order_id, seeded.order_id);
    assert_eq!(
        by_cart.payment_collection_id,
        Some(seeded.payment_collection_id)
    );
    assert_eq!(by_cart.status, "confirmed");
}

#[tokio::test]
async fn conflicting_completion_replay_returns_typed_conflict() {
    let database = TestDatabase::new().await;
    let seeded = database.seed_completed_identity().await;
    let port = port(&database);

    let error = port
        .complete_checkout(
            write_context(seeded.tenant_id, seeded.operation_id),
            CompleteCheckoutPortRequest {
                cart_id: seeded.cart_id,
                customer_id: None,
                payment_collection_id: Some(seeded.payment_collection_id),
                shipping_option_id: None,
                channel_id: None,
                channel_slug: None,
                locale: None,
                fallback_locale: None,
                currency_code: "USD".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: Vec::new(),
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: json!({}),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, PortErrorKind::Conflict);
    assert_eq!(error.code, "order.checkout_request_conflict");
}
