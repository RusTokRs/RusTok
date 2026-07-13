use rust_decimal::Decimal;
use rustok_payment::dto::CreatePaymentCollectionInput;
use rustok_payment::entities::payment_collection;
use rustok_payment::error::PaymentError;
use rustok_payment::services::PaymentService;
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> PaymentService {
    let db = setup_test_db().await;
    support::ensure_payment_schema(&db).await;
    PaymentService::new(db)
}

fn collection_input(
    cart_id: Uuid,
    customer_id: Uuid,
    order_id: Option<Uuid>,
    amount: &str,
) -> CreatePaymentCollectionInput {
    CreatePaymentCollectionInput {
        cart_id: Some(cart_id),
        order_id,
        customer_id: Some(customer_id),
        currency_code: "usd".to_string(),
        amount: Decimal::from_str(amount).expect("valid amount"),
        metadata: serde_json::json!({"source": "active-cart-race-test"}),
    }
}

#[tokio::test]
async fn duplicate_active_cart_insert_returns_existing_collection() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();

    let first = service
        .create_collection(
            tenant_id,
            collection_input(cart_id, customer_id, None, "49.99"),
        )
        .await
        .expect("first collection should be created");
    let replay = service
        .create_collection(
            tenant_id,
            collection_input(cart_id, customer_id, None, "49.99"),
        )
        .await
        .expect("unique conflict should reuse the active collection");

    assert_eq!(replay.id, first.id);
    assert_eq!(replay.status, "pending");
}

#[tokio::test]
async fn duplicate_active_cart_insert_attaches_requested_order() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();

    let first = service
        .create_collection(
            tenant_id,
            collection_input(cart_id, customer_id, None, "49.99"),
        )
        .await
        .expect("first collection should be created");
    let attached = service
        .create_collection(
            tenant_id,
            collection_input(cart_id, customer_id, Some(order_id), "49.99"),
        )
        .await
        .expect("race recovery should attach the requested order");

    assert_eq!(attached.id, first.id);
    assert_eq!(attached.order_id, Some(order_id));
}

#[tokio::test]
async fn duplicate_active_cart_insert_rejects_incompatible_amount() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();

    service
        .create_collection(
            tenant_id,
            collection_input(cart_id, customer_id, None, "49.99"),
        )
        .await
        .expect("first collection should be created");
    let error = service
        .create_collection(
            tenant_id,
            collection_input(cart_id, customer_id, None, "50.00"),
        )
        .await
        .expect_err("incompatible request must not reuse an active collection");

    assert!(
        matches!(error, PaymentError::Validation(ref message) if message.contains("has amount")),
        "expected amount validation error, got {error:?}"
    );
}

#[tokio::test]
async fn database_rejects_rebinding_collection_to_another_order() {
    let db = setup_test_db().await;
    support::ensure_payment_schema(&db).await;
    let service = PaymentService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    let first_order_id = Uuid::new_v4();
    let second_order_id = Uuid::new_v4();

    let collection = service
        .create_collection(
            tenant_id,
            collection_input(cart_id, customer_id, Some(first_order_id), "49.99"),
        )
        .await
        .expect("collection should be created with an order binding");
    let model = payment_collection::Entity::find_by_id(collection.id)
        .one(&db)
        .await
        .expect("collection query should succeed")
        .expect("collection should exist");
    let mut active: payment_collection::ActiveModel = model.into();
    active.order_id = Set(Some(second_order_id));

    let error = active
        .update(&db)
        .await
        .expect_err("database must reject order rebinding");
    assert!(
        error
            .to_string()
            .contains("payment collection order binding is immutable"),
        "unexpected database error: {error}"
    );
}
