use rust_decimal::Decimal;
use rustok_payment::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
    CompleteRefundInput, CreatePaymentCollectionInput, CreateRefundInput,
};
use rustok_payment::error::PaymentError;
use rustok_payment::services::PaymentService;
use rustok_test_utils::db::setup_test_db;
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> PaymentService {
    let db = setup_test_db().await;
    support::ensure_payment_schema(&db).await;
    PaymentService::new(db)
}

fn create_collection_input() -> CreatePaymentCollectionInput {
    CreatePaymentCollectionInput {
        cart_id: Some(Uuid::new_v4()),
        order_id: None,
        customer_id: Some(Uuid::new_v4()),
        currency_code: "usd".to_string(),
        amount: Decimal::from_str("99.99").expect("valid decimal"),
        metadata: serde_json::json!({ "source": "payment-test" }),
    }
}

#[tokio::test]
async fn create_and_authorize_payment_collection() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_collection(tenant_id, create_collection_input())
        .await
        .unwrap();
    assert_eq!(created.status, "pending");

    let authorized = service
        .authorize_collection(
            tenant_id,
            created.id,
            AuthorizePaymentInput {
                provider_id: None,
                provider_payment_id: None,
                amount: None,
                metadata: serde_json::json!({ "step": "authorized" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(authorized.status, "authorized");
    assert_eq!(authorized.provider_id.as_deref(), Some("manual"));
    assert_eq!(authorized.payments.len(), 1);
    assert_eq!(authorized.payments[0].provider_id, "manual");
}

#[tokio::test]
async fn capture_authorized_payment_collection() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_collection(tenant_id, create_collection_input())
        .await
        .unwrap();
    service
        .authorize_collection(
            tenant_id,
            created.id,
            AuthorizePaymentInput {
                provider_id: None,
                provider_payment_id: None,
                amount: Some(Decimal::from_str("49.99").expect("valid decimal")),
                metadata: serde_json::json!({ "step": "authorized" }),
            },
        )
        .await
        .unwrap();

    let captured = service
        .capture_collection(
            tenant_id,
            created.id,
            CapturePaymentInput {
                amount: Some(Decimal::from_str("49.99").expect("valid decimal")),
                metadata: serde_json::json!({ "step": "captured" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(captured.status, "captured");
    assert_eq!(
        captured.captured_amount,
        Decimal::from_str("49.99").expect("valid decimal")
    );
    assert_eq!(captured.payments[0].status, "captured");
}

#[tokio::test]
async fn cancel_pending_payment_collection() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_collection(tenant_id, create_collection_input())
        .await
        .unwrap();
    let cancelled = service
        .cancel_collection(
            tenant_id,
            created.id,
            CancelPaymentInput {
                reason: Some("user-abandoned-checkout".to_string()),
                metadata: serde_json::json!({ "step": "cancelled" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(cancelled.status, "cancelled");
    assert_eq!(
        cancelled.cancellation_reason.as_deref(),
        Some("user-abandoned-checkout")
    );
}

#[tokio::test]
async fn capture_requires_authorized_state() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_collection(tenant_id, create_collection_input())
        .await
        .unwrap();
    let error = service
        .capture_collection(
            tenant_id,
            created.id,
            CapturePaymentInput {
                amount: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap_err();

    match error {
        PaymentError::InvalidTransition { from, to } => {
            assert_eq!(from, "pending");
            assert_eq!(to, "captured");
        }
        other => panic!("expected invalid transition, got {other:?}"),
    }
}

#[tokio::test]
async fn find_reusable_collection_by_cart_returns_latest_active_collection() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();

    let first = service
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: Some(cart_id),
                order_id: None,
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("99.99").expect("valid decimal"),
                metadata: serde_json::json!({ "attempt": 1 }),
            },
        )
        .await
        .unwrap();
    service
        .cancel_collection(
            tenant_id,
            first.id,
            CancelPaymentInput {
                reason: Some("retry".to_string()),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let second = service
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: Some(cart_id),
                order_id: None,
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("99.99").expect("valid decimal"),
                metadata: serde_json::json!({ "attempt": 2 }),
            },
        )
        .await
        .unwrap();

    let reusable = service
        .find_reusable_collection_by_cart(tenant_id, cart_id)
        .await
        .unwrap()
        .expect("expected reusable collection");
    assert_eq!(reusable.id, second.id);
    assert_eq!(reusable.status, "pending");
}

#[tokio::test]
async fn refund_lifecycle_tracks_pending_completed_and_cancelled_records() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_collection(tenant_id, create_collection_input())
        .await
        .unwrap();
    service
        .authorize_collection(
            tenant_id,
            created.id,
            AuthorizePaymentInput {
                provider_id: Some("manual".to_string()),
                provider_payment_id: Some("refund-test-1".to_string()),
                amount: None,
                metadata: serde_json::json!({ "step": "authorized" }),
            },
        )
        .await
        .unwrap();
    service
        .capture_collection(
            tenant_id,
            created.id,
            CapturePaymentInput {
                amount: Some(Decimal::from_str("40.00").expect("valid decimal")),
                metadata: serde_json::json!({ "step": "captured" }),
            },
        )
        .await
        .unwrap();

    let pending = service
        .create_refund(
            tenant_id,
            created.id,
            CreateRefundInput {
                amount: Decimal::from_str("15.00").expect("valid decimal"),
                reason: Some("customer-request".to_string()),
                metadata: serde_json::json!({ "step": "refund-created" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(pending.status, "pending");

    let completed = service
        .complete_refund(
            tenant_id,
            pending.id,
            CompleteRefundInput {
                metadata: serde_json::json!({ "step": "refund-completed" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.status, "refunded");

    let second = service
        .create_refund(
            tenant_id,
            created.id,
            CreateRefundInput {
                amount: Decimal::from_str("10.00").expect("valid decimal"),
                reason: Some("operator-cancel".to_string()),
                metadata: serde_json::json!({ "step": "refund-created-2" }),
            },
        )
        .await
        .unwrap();
    let cancelled = service
        .cancel_refund(
            tenant_id,
            second.id,
            CancelRefundInput {
                reason: Some("review-failed".to_string()),
                metadata: serde_json::json!({ "step": "refund-cancelled" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status, "cancelled");

    let collection = service.get_collection(tenant_id, created.id).await.unwrap();
    assert_eq!(
        collection.refunded_amount,
        Decimal::from_str("15.00").expect("valid decimal")
    );
    assert_eq!(collection.refunds.len(), 2);

    let (refunds, total) = service
        .list_refunds(
            tenant_id,
            rustok_payment::dto::ListRefundsInput {
                page: 1,
                per_page: 20,
                payment_collection_id: Some(created.id),
                status: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(total, 2);
    assert_eq!(refunds[0].id, second.id);
    assert_eq!(refunds[1].id, pending.id);
}

#[tokio::test]
async fn refund_amount_cannot_exceed_remaining_captured_total() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_collection(tenant_id, create_collection_input())
        .await
        .unwrap();
    service
        .authorize_collection(
            tenant_id,
            created.id,
            AuthorizePaymentInput {
                provider_id: None,
                provider_payment_id: None,
                amount: Some(Decimal::from_str("20.00").expect("valid decimal")),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    service
        .capture_collection(
            tenant_id,
            created.id,
            CapturePaymentInput {
                amount: Some(Decimal::from_str("20.00").expect("valid decimal")),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    service
        .create_refund(
            tenant_id,
            created.id,
            CreateRefundInput {
                amount: Decimal::from_str("12.00").expect("valid decimal"),
                reason: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let error = service
        .create_refund(
            tenant_id,
            created.id,
            CreateRefundInput {
                amount: Decimal::from_str("9.00").expect("valid decimal"),
                reason: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap_err();

    match error {
        PaymentError::Validation(message) => {
            assert!(message.contains("remaining refundable amount"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}
