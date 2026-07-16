use rust_decimal::Decimal;
use rustok_migrations::Migrator;
use rustok_payment::{
    AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput, CreateRefundInput,
    PaymentService,
};
use rustok_test_utils::db::setup_test_db_with_migrations;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn legacy_refund_creation_without_identity_is_rejected_by_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant_id = Uuid::new_v4();
    let payment = PaymentService::new(db);
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

    let error = payment
        .create_refund(
            tenant_id,
            collection.id,
            CreateRefundInput {
                amount: Decimal::new(1_000, 2),
                reason: Some("legacy bypass".to_string()),
                metadata: json!({}),
            },
        )
        .await
        .expect_err("identity-less refund creation must be rejected");

    assert!(
        error.to_string().contains("refund creation identity is required"),
        "unexpected error: {error}"
    );
}
