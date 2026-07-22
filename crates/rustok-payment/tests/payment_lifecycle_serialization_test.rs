use rust_decimal::Decimal;
use rustok_payment::PaymentService;
use rustok_payment::dto::{AuthorizePaymentInput, CreatePaymentCollectionInput};
use rustok_payment::entities::payment;
use rustok_payment::migrations;
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

mod support;

async fn setup() -> (DatabaseConnection, PaymentService) {
    let db = setup_test_db().await;
    support::ensure_payment_schema(&db).await;

    let manager = SchemaManager::new(&db);
    let mut registered = migrations::migrations();
    let lifecycle = registered
        .pop()
        .expect("payment lifecycle serialization migration should be registered last");
    let state_guards = registered
        .pop()
        .expect("payment state invariant migration should precede lifecycle serialization");

    state_guards
        .up(&manager)
        .await
        .expect("payment state guards should install on SQLite");
    lifecycle
        .up(&manager)
        .await
        .expect("payment lifecycle serialization migration should install on SQLite");

    (db.clone(), PaymentService::new(db))
}

fn collection_input() -> CreatePaymentCollectionInput {
    CreatePaymentCollectionInput {
        cart_id: Some(Uuid::new_v4()),
        order_id: None,
        customer_id: None,
        currency_code: "USD".to_string(),
        amount: Decimal::new(5000, 2),
        metadata: serde_json::json!({}),
    }
}

#[tokio::test]
async fn lifecycle_guards_cancel_child_payment_and_reject_stale_or_rebound_updates() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();

    let collection = service
        .create_collection(tenant_id, collection_input())
        .await
        .expect("collection should be created");
    let authorized = service
        .authorize_collection(
            tenant_id,
            collection.id,
            AuthorizePaymentInput {
                provider_id: Some("manual".to_string()),
                provider_payment_id: Some("serialized-payment".to_string()),
                amount: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("collection should be authorized");
    let payment_id = authorized.payments[0].id;

    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE payment_collections
         SET status = 'cancelled',
             cancellation_reason = 'direct cancellation',
             cancelled_at = CURRENT_TIMESTAMP,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
        vec![collection.id.into()],
    ))
    .await
    .expect("collection cancellation should synchronize its payment row");

    let cancelled_payment = payment::Entity::find_by_id(payment_id)
        .one(&db)
        .await
        .expect("payment query should succeed")
        .expect("payment should exist");
    assert_eq!(cancelled_payment.status, "cancelled");
    assert_eq!(
        cancelled_payment.error_message.as_deref(),
        Some("direct cancellation")
    );
    assert!(cancelled_payment.cancelled_at.is_some());

    let stale_capture = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE payment_collections
             SET status = 'captured',
                 captured_amount = authorized_amount,
                 captured_at = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            vec![collection.id.into()],
        ))
        .await;
    assert!(
        stale_capture.is_err(),
        "a stale writer must not revive a cancelled collection"
    );

    let attachable = service
        .create_collection(tenant_id, collection_input())
        .await
        .expect("second collection should be created");
    let first_order_id = Uuid::new_v4();
    service
        .attach_order_to_collection(
            tenant_id,
            attachable.id,
            first_order_id,
            serde_json::json!({}),
        )
        .await
        .expect("first order attachment should succeed");

    let rebound = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE payment_collections SET order_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            vec![Uuid::new_v4().into(), attachable.id.into()],
        ))
        .await;
    assert!(
        rebound.is_err(),
        "payment collection order ownership must be immutable after attachment"
    );
}
