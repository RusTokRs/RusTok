use rustok_cart::dto::CreateCartInput;
use rustok_cart::error::CartError;
use rustok_cart::services::CartService;
use rustok_test_utils::db::setup_test_db;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

mod support;

async fn setup_with_db() -> (DatabaseConnection, CartService) {
    let db = setup_test_db().await;
    support::ensure_cart_schema(&db).await;
    (db.clone(), CartService::new(db))
}

fn create_cart_input() -> CreateCartInput {
    CreateCartInput {
        customer_id: Some(Uuid::new_v4()),
        email: Some("buyer@example.com".to_string()),
        region_id: None,
        country_code: None,
        locale_code: None,
        selected_shipping_option_id: None,
        currency_code: "USD".to_string(),
        metadata: serde_json::json!({ "source": "cart-lifecycle-cas-test" }),
    }
}

#[tokio::test]
async fn concurrent_begin_checkout_has_exactly_one_winner() {
    let (db, setup_service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let cart = setup_service
        .create_cart(tenant_id, create_cart_input())
        .await
        .expect("cart should be created");

    let first = CartService::new(db.clone());
    let second = CartService::new(db.clone());
    let (first_result, second_result) = tokio::join!(
        first.begin_checkout(tenant_id, cart.id),
        second.begin_checkout(tenant_id, cart.id),
    );

    let results = [first_result, second_result];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);

    let failure = results
        .iter()
        .find_map(|result| result.as_ref().err())
        .expect("one transition should lose the compare-and-set race");
    match failure {
        CartError::InvalidTransition { from, to } => {
            assert_eq!(from, "checking_out");
            assert_eq!(to, "checking_out");
        }
        other => panic!("expected invalid transition, got {other:?}"),
    }

    let persisted = setup_service
        .get_cart(tenant_id, cart.id)
        .await
        .expect("cart should remain readable");
    assert_eq!(persisted.status, "checking_out");
}

#[tokio::test]
async fn concurrent_checkout_and_abandon_commit_only_one_transition() {
    let (db, setup_service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let cart = setup_service
        .create_cart(tenant_id, create_cart_input())
        .await
        .expect("cart should be created");

    let checkout_service = CartService::new(db.clone());
    let abandon_service = CartService::new(db);
    let (checkout_result, abandon_result) = tokio::join!(
        checkout_service.begin_checkout(tenant_id, cart.id),
        abandon_service.abandon_cart(tenant_id, cart.id),
    );

    assert_eq!(
        [checkout_result.is_ok(), abandon_result.is_ok()]
            .into_iter()
            .filter(|succeeded| *succeeded)
            .count(),
        1
    );

    let persisted = setup_service
        .get_cart(tenant_id, cart.id)
        .await
        .expect("cart should remain readable");
    assert!(matches!(
        persisted.status.as_str(),
        "checking_out" | "abandoned"
    ));
}
