use rust_decimal::Decimal;
use rustok_order::OrderService;
use rustok_order::dto::{CreateOrderInput, CreateOrderLineItemInput};
use rustok_order::migrations;
use rustok_test_utils::{db::setup_test_db, mock_transactional_event_bus};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

mod support;

async fn setup() -> (sea_orm::DatabaseConnection, OrderService) {
    let db = setup_test_db().await;
    support::ensure_order_schema(&db).await;

    let manager = SchemaManager::new(&db);
    let lifecycle = migrations::migrations()
        .pop()
        .expect("order lifecycle serialization migration should be registered last");
    lifecycle
        .up(&manager)
        .await
        .expect("order lifecycle serialization migration should install on SQLite");

    (
        db.clone(),
        OrderService::new(db, mock_transactional_event_bus()),
    )
}

fn order_input() -> CreateOrderInput {
    CreateOrderInput {
        customer_id: None,
        currency_code: "USD".to_string(),
        shipping_total: Decimal::ZERO,
        line_items: vec![CreateOrderLineItemInput {
            product_id: None,
            variant_id: None,
            shipping_profile_slug: "default".to_string(),
            seller_id: None,
            sku: Some("LIFECYCLE-SKU".to_string()),
            title: "Lifecycle item".to_string(),
            quantity: 1,
            unit_price: Decimal::new(2500, 2),
            metadata: serde_json::json!({}),
        }],
        adjustments: Vec::new(),
        tax_lines: Vec::new(),
        metadata: serde_json::json!({"source":"order-lifecycle-test"}),
    }
}

#[tokio::test]
async fn order_lifecycle_rejects_stale_and_reverse_transitions() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let order = service
        .create_order(tenant_id, actor_id, order_input())
        .await
        .expect("pending order should be created");

    let confirmed = service
        .confirm_order(tenant_id, actor_id, order.id)
        .await
        .expect("pending order should confirm");
    assert_eq!(confirmed.status, "confirmed");
    assert!(confirmed.confirmed_at.is_some());

    let stale_confirm = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE orders
             SET status = 'confirmed', updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            vec![order.id.into()],
        ))
        .await;
    assert!(
        stale_confirm.is_err(),
        "same-state lifecycle writes must fail so stale writers cannot duplicate events"
    );

    let paid = service
        .mark_paid(
            tenant_id,
            actor_id,
            order.id,
            "payment-ref".to_string(),
            "manual".to_string(),
        )
        .await
        .expect("confirmed order should become paid");
    assert_eq!(paid.status, "paid");
    assert!(paid.paid_at.is_some());

    let reverse = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE orders
             SET status = 'confirmed', paid_at = NULL, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            vec![order.id.into()],
        ))
        .await;
    assert!(
        reverse.is_err(),
        "paid orders must not transition backwards"
    );

    let shipped = service
        .ship_order(
            tenant_id,
            actor_id,
            order.id,
            "TRACK-1".to_string(),
            "manual".to_string(),
        )
        .await
        .expect("paid order should ship");
    assert_eq!(shipped.status, "shipped");
    assert!(shipped.shipped_at.is_some());

    let delivered = service
        .deliver_order(tenant_id, actor_id, order.id, None)
        .await
        .expect("shipped order should be delivered");
    assert_eq!(delivered.status, "delivered");
    assert!(delivered.delivered_at.is_some());

    let cancel_delivered = service
        .cancel_order(tenant_id, actor_id, order.id, Some("too late".to_string()))
        .await;
    assert!(
        cancel_delivered.is_err(),
        "delivered orders must remain terminal"
    );
}
