use std::{path::PathBuf, time::Duration};

use chrono::Utc;
use rust_decimal::Decimal;
use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortErrorKind};
use rustok_order::{
    AdoptLegacyCheckoutOrderIdentityRequest, ReadCheckoutOrderIdentityByOperationRequest,
    entities::{order, order_checkout_identity},
    in_process_checkout_order_identity_port,
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
            "rustok-order-checkout-identity-port-{}.sqlite",
            Uuid::new_v4()
        ));
        let mut options = ConnectOptions::new(format!("sqlite://{}?mode=rwc", path.display()));
        options
            .max_connections(2)
            .min_connections(1)
            .sqlx_logging(false);
        let db = Database::connect(options).await.unwrap();
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
            "CREATE UNIQUE INDEX ux_test_identity_order ON order_checkout_identities (tenant_id, order_id);",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE UNIQUE INDEX ux_test_identity_cart ON order_checkout_identities (tenant_id, source_cart_id);",
        )
        .await
        .unwrap();

        Self { db, path }
    }

    async fn seed_legacy_order(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        snapshot_hash: &str,
        request_hash: &str,
    ) -> Uuid {
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
            metadata: Set(json!({
                "checkout": {
                    "operation_id": operation_id,
                    "snapshot_hash": snapshot_hash,
                    "order_request_hash": request_hash,
                }
            })),
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

fn context(tenant_id: Uuid, operation_id: Uuid, action: &str, write: bool) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("order-identity-port-test"),
        PLATFORM_FALLBACK_LOCALE,
        format!("order-identity-port-test:{operation_id}:{action}"),
    )
    .with_deadline(Duration::from_secs(3));
    if write {
        context.with_idempotency_key(format!("order-identity-port-test:{operation_id}:{action}"))
    } else {
        context
    }
}

#[tokio::test]
async fn adopts_legacy_metadata_inside_order_owner_and_reads_typed_identity() {
    let database = TestDatabase::new().await;
    let tenant_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let snapshot_hash = "a".repeat(64);
    let request_hash = "b".repeat(64);
    let order_id = database
        .seed_legacy_order(
            tenant_id,
            operation_id,
            snapshot_hash.as_str(),
            request_hash.as_str(),
        )
        .await;
    let port = in_process_checkout_order_identity_port(database.db.clone());

    let adopted = port
        .adopt_legacy(
            context(tenant_id, operation_id, "adopt", true),
            AdoptLegacyCheckoutOrderIdentityRequest {
                checkout_operation_id: operation_id,
                cart_id,
            },
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(adopted.order_id, order_id);
    assert_eq!(adopted.source_cart_id, Some(cart_id));
    assert_eq!(
        adopted.snapshot_hash.as_deref(),
        Some(snapshot_hash.as_str())
    );
    assert_eq!(adopted.request_hash.as_deref(), Some(request_hash.as_str()));
    assert_eq!(
        port.read_by_operation(
            context(tenant_id, operation_id, "read", false),
            ReadCheckoutOrderIdentityByOperationRequest {
                checkout_operation_id: operation_id,
            },
        )
        .await
        .unwrap(),
        Some(adopted)
    );
}

#[tokio::test]
async fn rejects_rebinding_adopted_operation_to_another_cart() {
    let database = TestDatabase::new().await;
    let tenant_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let first_cart_id = Uuid::new_v4();
    let snapshot_hash = "c".repeat(64);
    let request_hash = "d".repeat(64);
    database
        .seed_legacy_order(
            tenant_id,
            operation_id,
            snapshot_hash.as_str(),
            request_hash.as_str(),
        )
        .await;
    let port = in_process_checkout_order_identity_port(database.db.clone());

    port.adopt_legacy(
        context(tenant_id, operation_id, "first-adopt", true),
        AdoptLegacyCheckoutOrderIdentityRequest {
            checkout_operation_id: operation_id,
            cart_id: first_cart_id,
        },
    )
    .await
    .unwrap();
    let error = port
        .adopt_legacy(
            context(tenant_id, operation_id, "second-adopt", true),
            AdoptLegacyCheckoutOrderIdentityRequest {
                checkout_operation_id: operation_id,
                cart_id: Uuid::new_v4(),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, PortErrorKind::Conflict);
    assert_eq!(error.code, "order.checkout_identity_cart_conflict");
}
