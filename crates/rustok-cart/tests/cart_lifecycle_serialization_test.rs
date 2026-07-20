use rustok_cart::CartService;
use rustok_cart::dto::CreateCartInput;
use rustok_cart::migrations;
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::SchemaManager;

mod support;

async fn setup() -> (sea_orm::DatabaseConnection, CartService) {
    let db = setup_test_db().await;
    support::ensure_cart_schema(&db).await;

    let manager = SchemaManager::new(&db);
    let lifecycle = migrations::migrations()
        .pop()
        .expect("cart lifecycle serialization migration should be registered last");
    lifecycle
        .up(&manager)
        .await
        .expect("cart lifecycle serialization migration should install on SQLite");

    (db.clone(), CartService::new(db))
}

fn cart_input() -> CreateCartInput {
    CreateCartInput {
        customer_id: None,
        email: Some("buyer@example.com".to_string()),
        region_id: None,
        country_code: None,
        locale_code: Some("en".to_string()),
        selected_shipping_option_id: None,
        currency_code: "USD".to_string(),
        metadata: serde_json::json!({"source":"cart-lifecycle-test"}),
    }
}

#[tokio::test]
async fn checkout_lock_rejects_stale_begin_and_completed_cart_is_terminal() {
    let (db, service) = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, cart_input())
        .await
        .expect("active cart should be created");
    assert_eq!(cart.status, "active");

    let checking_out = service
        .begin_checkout(tenant_id, cart.id)
        .await
        .expect("first checkout lock should succeed");
    assert_eq!(checking_out.status, "checking_out");

    let stale_begin = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE carts SET status = 'checking_out', updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            vec![cart.id.into()],
        ))
        .await;
    assert!(
        stale_begin.is_err(),
        "a second stale checkout lock must be rejected"
    );

    let active = service
        .release_checkout(tenant_id, cart.id)
        .await
        .expect("checkout lock should be releasable");
    assert_eq!(active.status, "active");
    assert!(active.completed_at.is_none());

    let checking_out = service
        .begin_checkout(tenant_id, cart.id)
        .await
        .expect("checkout should be restartable after release");
    assert_eq!(checking_out.status, "checking_out");

    let completed = service
        .complete_cart(tenant_id, cart.id)
        .await
        .expect("checking-out cart should complete");
    assert_eq!(completed.status, "completed");
    assert!(completed.completed_at.is_some());

    let reopen = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE carts SET status = 'active', completed_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            vec![cart.id.into()],
        ))
        .await;
    assert!(reopen.is_err(), "completed carts must remain terminal");

    let release_completed = service.release_checkout(tenant_id, cart.id).await;
    assert!(
        release_completed.is_err(),
        "completed carts must not release back to active"
    );
}
