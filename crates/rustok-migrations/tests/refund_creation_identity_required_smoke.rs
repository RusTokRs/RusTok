use chrono::Utc;
use rust_decimal::Decimal;
use rustok_migrations::Migrator;
use rustok_payment::entities::refund;
use rustok_payment::{
    AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput, PaymentService,
};
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn refund_insert_without_creation_identity_is_rejected_by_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let payment = PaymentService::new(db.clone());
    let collection = payment
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: None,
                customer_id: None,
                currency_code: "USD".to_string(),
                amount: Decimal::new(10_000, 2),
                metadata: json!({"test": "refund-identity-required"}),
            },
        )
        .await
        .expect("collection fixture must be created");
    payment
        .authorize_collection(
            tenant_id,
            collection.id,
            AuthorizePaymentInput {
                provider_id: Some("manual".to_string()),
                provider_payment_id: None,
                amount: Some(collection.amount),
                metadata: json!({}),
            },
        )
        .await
        .expect("collection fixture must be authorized");
    payment
        .capture_collection(
            tenant_id,
            collection.id,
            CapturePaymentInput {
                amount: Some(collection.amount),
                metadata: json!({}),
            },
        )
        .await
        .expect("collection fixture must be captured");

    let now = Utc::now();
    let error = refund::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        payment_collection_id: Set(collection.id),
        status: Set("pending".to_string()),
        currency_code: Set("USD".to_string()),
        amount: Set(Decimal::new(1_000, 2)),
        reason: Set(Some("legacy bypass".to_string())),
        metadata: Set(json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        refunded_at: Set(None),
        cancelled_at: Set(None),
    }
    .insert(&db)
    .await
    .expect_err("identity-less refund insert must be rejected");

    assert!(
        error
            .to_string()
            .contains("refund creation identity is required"),
        "unexpected error: {error}"
    );
}
