use rust_decimal::Decimal;
use rustok_migrations::Migrator;
use rustok_payment::{
    AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput, CreateRefundInput,
    PaymentRefundCreationService, PaymentService,
};
use rustok_test_utils::db::setup_test_db_with_migrations;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn refund_creation_key_replays_and_rejects_payload_conflicts() {
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
                metadata: json!({"test": "refund-creation-identity"}),
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

    let refunds = PaymentRefundCreationService::new(db.clone());
    let input = CreateRefundInput {
        amount: Decimal::new(2_500, 2),
        reason: Some("customer request".to_string()),
        metadata: json!({"case": "same-request"}),
    };
    let first = refunds
        .create_or_replay(tenant_id, collection.id, "refund-request-1", input.clone())
        .await
        .expect("first refund reservation must succeed");
    let replay = refunds
        .create_or_replay(tenant_id, collection.id, "refund-request-1", input)
        .await
        .expect("same refund request must replay");
    assert_eq!(first.id, replay.id);

    let conflict = refunds
        .create_or_replay(
            tenant_id,
            collection.id,
            "refund-request-1",
            CreateRefundInput {
                amount: Decimal::new(3_000, 2),
                reason: Some("customer request".to_string()),
                metadata: json!({"case": "different-request"}),
            },
        )
        .await
        .expect_err("same key with another payload must conflict");
    assert!(conflict.to_string().contains("already bound"));
}

#[tokio::test]
async fn concurrent_same_refund_creation_key_returns_one_identity() {
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
                metadata: json!({}),
            },
        )
        .await
        .unwrap();
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
        .unwrap();
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
        .unwrap();

    let left = PaymentRefundCreationService::new(db.clone());
    let right = PaymentRefundCreationService::new(db);
    let input = CreateRefundInput {
        amount: Decimal::new(1_000, 2),
        reason: None,
        metadata: json!({"concurrent": true}),
    };
    let (left_result, right_result) = tokio::join!(
        left.create_or_replay(tenant_id, collection.id, "refund-race-1", input.clone(),),
        right.create_or_replay(tenant_id, collection.id, "refund-race-1", input,),
    );
    let left_refund = left_result.expect("left caller must create or replay");
    let right_refund = right_result.expect("right caller must create or replay");
    assert_eq!(left_refund.id, right_refund.id);
}
