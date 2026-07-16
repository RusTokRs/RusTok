use rust_decimal::Decimal;
use rustok_payment::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
    CompleteRefundInput, CreatePaymentCollectionInput, CreateRefundInput, ListRefundsInput,
    PaymentCollectionResponse,
};
use rustok_payment::error::PaymentError;
use rustok_payment::services::{PaymentRefundCreationService, PaymentService};
use rustok_test_utils::db::setup_test_db;
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use uuid::Uuid;

#[path = "support/order_schema.rs"]
mod order_support;
mod support;

struct PaymentHarness {
    db: DatabaseConnection,
    payment: PaymentService,
    refunds: PaymentRefundCreationService,
}

impl PaymentHarness {
    async fn new() -> Self {
        let db = setup_test_db().await;
        support::ensure_payment_schema(&db).await;
        Self {
            payment: PaymentService::new(db.clone()),
            refunds: PaymentRefundCreationService::new(db.clone()),
            db,
        }
    }

    async fn create_collection(
        &self,
        tenant_id: Uuid,
        input: CreatePaymentCollectionInput,
    ) -> PaymentCollectionResponse {
        self.payment
            .create_collection(tenant_id, input)
            .await
            .expect("payment collection should be created")
    }

    async fn capture_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        amount: Decimal,
    ) -> PaymentCollectionResponse {
        self.payment
            .authorize_collection(
                tenant_id,
                collection_id,
                AuthorizePaymentInput {
                    provider_id: Some("manual".to_string()),
                    provider_payment_id: None,
                    amount: Some(amount),
                    metadata: serde_json::json!({"test": "payment-service"}),
                },
            )
            .await
            .expect("payment collection should authorize");
        self.payment
            .capture_collection(
                tenant_id,
                collection_id,
                CapturePaymentInput {
                    amount: Some(amount),
                    metadata: serde_json::json!({"test": "payment-service"}),
                },
            )
            .await
            .expect("payment collection should capture")
    }

    async fn create_refund(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        key: &str,
        amount: Decimal,
    ) -> rustok_payment::RefundResponse {
        self.refunds
            .create_or_replay(
                tenant_id,
                collection_id,
                key,
                CreateRefundInput {
                    amount,
                    reason: Some("payment-service-test".to_string()),
                    metadata: serde_json::json!({"key": key}),
                },
            )
            .await
            .expect("refund should create or replay")
    }
}

fn amount(value: &str) -> Decimal {
    Decimal::from_str(value).expect("valid decimal")
}

fn create_collection_input() -> CreatePaymentCollectionInput {
    CreatePaymentCollectionInput {
        cart_id: Some(Uuid::new_v4()),
        order_id: None,
        customer_id: Some(Uuid::new_v4()),
        currency_code: "usd".to_string(),
        amount: amount("99.99"),
        metadata: serde_json::json!({"source": "payment-test"}),
    }
}

#[tokio::test]
async fn create_and_authorize_payment_collection() {
    let harness = PaymentHarness::new().await;
    let tenant_id = Uuid::new_v4();
    let created = harness
        .create_collection(tenant_id, create_collection_input())
        .await;
    assert_eq!(created.status, "pending");

    let authorized = harness
        .payment
        .authorize_collection(
            tenant_id,
            created.id,
            AuthorizePaymentInput {
                provider_id: None,
                provider_payment_id: None,
                amount: None,
                metadata: serde_json::json!({"step": "authorized"}),
            },
        )
        .await
        .unwrap();
    assert_eq!(authorized.status, "authorized");
    assert_eq!(authorized.provider_id.as_deref(), Some("manual"));
    assert_eq!(authorized.payments.len(), 1);
}

#[tokio::test]
async fn capture_authorized_payment_collection() {
    let harness = PaymentHarness::new().await;
    let tenant_id = Uuid::new_v4();
    let created = harness
        .create_collection(tenant_id, create_collection_input())
        .await;
    let captured = harness
        .capture_collection(tenant_id, created.id, amount("49.99"))
        .await;
    assert_eq!(captured.status, "captured");
    assert_eq!(captured.captured_amount, amount("49.99"));
    assert_eq!(captured.payments[0].status, "captured");
}

#[tokio::test]
async fn cancel_pending_payment_collection() {
    let harness = PaymentHarness::new().await;
    let tenant_id = Uuid::new_v4();
    let created = harness
        .create_collection(tenant_id, create_collection_input())
        .await;
    let cancelled = harness
        .payment
        .cancel_collection(
            tenant_id,
            created.id,
            CancelPaymentInput {
                reason: Some("user-abandoned-checkout".to_string()),
                metadata: serde_json::json!({"step": "cancelled"}),
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
    let harness = PaymentHarness::new().await;
    let tenant_id = Uuid::new_v4();
    let created = harness
        .create_collection(tenant_id, create_collection_input())
        .await;
    let error = harness
        .payment
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
    assert!(matches!(
        error,
        PaymentError::InvalidTransition { ref from, ref to }
            if from == "pending" && to == "captured"
    ));
}

#[tokio::test]
async fn find_reusable_collection_by_cart_returns_latest_active_collection() {
    let harness = PaymentHarness::new().await;
    let tenant_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let first = harness
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: Some(cart_id),
                order_id: None,
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                amount: amount("99.99"),
                metadata: serde_json::json!({"attempt": 1}),
            },
        )
        .await;
    harness
        .payment
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
    let second = harness
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: Some(cart_id),
                order_id: None,
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                amount: amount("99.99"),
                metadata: serde_json::json!({"attempt": 2}),
            },
        )
        .await;

    let reusable = harness
        .payment
        .find_reusable_collection_by_cart(tenant_id, cart_id)
        .await
        .unwrap()
        .expect("expected reusable collection");
    assert_eq!(reusable.id, second.id);
}

#[tokio::test]
async fn refund_lifecycle_tracks_pending_completed_and_cancelled_records() {
    let harness = PaymentHarness::new().await;
    let tenant_id = Uuid::new_v4();
    let collection = harness
        .create_collection(tenant_id, create_collection_input())
        .await;
    harness
        .capture_collection(tenant_id, collection.id, amount("40.00"))
        .await;

    let pending = harness
        .create_refund(
            tenant_id,
            collection.id,
            "payment-service:refund:completed",
            amount("15.00"),
        )
        .await;
    let completed = harness
        .payment
        .complete_refund(
            tenant_id,
            pending.id,
            CompleteRefundInput {
                metadata: serde_json::json!({"step": "refund-completed"}),
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.status, "refunded");

    let second = harness
        .create_refund(
            tenant_id,
            collection.id,
            "payment-service:refund:cancelled",
            amount("10.00"),
        )
        .await;
    let cancelled = harness
        .payment
        .cancel_refund(
            tenant_id,
            second.id,
            CancelRefundInput {
                reason: Some("review-failed".to_string()),
                metadata: serde_json::json!({"step": "refund-cancelled"}),
            },
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status, "cancelled");

    let collection = harness
        .payment
        .get_collection(tenant_id, collection.id)
        .await
        .unwrap();
    assert_eq!(collection.refunded_amount, amount("15.00"));
    assert_eq!(collection.refunds.len(), 2);
}

#[tokio::test]
async fn refund_amount_cannot_exceed_remaining_captured_total() {
    let harness = PaymentHarness::new().await;
    let tenant_id = Uuid::new_v4();
    let collection = harness
        .create_collection(tenant_id, create_collection_input())
        .await;
    harness
        .capture_collection(tenant_id, collection.id, amount("20.00"))
        .await;
    harness
        .create_refund(
            tenant_id,
            collection.id,
            "payment-service:capacity:first",
            amount("12.00"),
        )
        .await;

    let error = harness
        .refunds
        .create_or_replay(
            tenant_id,
            collection.id,
            "payment-service:capacity:second",
            CreateRefundInput {
                amount: amount("9.00"),
                reason: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        PaymentError::Validation(ref message)
            if message.contains("remaining refundable amount")
    ));
}

#[tokio::test]
async fn list_refunds_validates_and_normalizes_status_filter() {
    let harness = PaymentHarness::new().await;
    let tenant_id = Uuid::new_v4();
    let invalid = harness
        .payment
        .list_refunds(
            tenant_id,
            ListRefundsInput {
                page: 1,
                per_page: 20,
                payment_collection_id: None,
                order_id: None,
                status: Some("processing".to_string()),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        invalid,
        PaymentError::Validation(ref message)
            if message.contains("invalid refund status filter")
    ));

    let collection = harness
        .create_collection(tenant_id, create_collection_input())
        .await;
    harness
        .capture_collection(tenant_id, collection.id, amount("20.00"))
        .await;
    harness
        .create_refund(
            tenant_id,
            collection.id,
            "payment-service:status-filter",
            amount("5.00"),
        )
        .await;
    let (items, total) = harness
        .payment
        .list_refunds(
            tenant_id,
            ListRefundsInput {
                page: 1,
                per_page: 20,
                payment_collection_id: Some(collection.id),
                order_id: None,
                status: Some(" PENDING ".to_string()),
            },
        )
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].status, "pending");
}

#[tokio::test]
async fn list_refunds_supports_order_and_collection_filters() {
    let harness = PaymentHarness::new().await;
    order_support::ensure_order_schema(&harness.db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_service = rustok_order::services::OrderService::new(
        harness.db.clone(),
        rustok_test_utils::mock_transactional_event_bus(),
    );
    let first_order = create_order(&order_service, tenant_id, actor_id, "ORDER-FILTER-1").await;
    let second_order = create_order(&order_service, tenant_id, actor_id, "ORDER-FILTER-2").await;
    let first_collection = harness
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(first_order.id),
                customer_id: first_order.customer_id,
                currency_code: "usd".to_string(),
                amount: first_order.total_amount,
                metadata: serde_json::json!({}),
            },
        )
        .await;
    let second_collection = harness
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(second_order.id),
                customer_id: second_order.customer_id,
                currency_code: "usd".to_string(),
                amount: second_order.total_amount,
                metadata: serde_json::json!({}),
            },
        )
        .await;
    harness
        .capture_collection(tenant_id, first_collection.id, first_collection.amount)
        .await;
    harness
        .capture_collection(tenant_id, second_collection.id, second_collection.amount)
        .await;
    harness
        .create_refund(
            tenant_id,
            first_collection.id,
            "payment-service:order-filter:first",
            amount("5.00"),
        )
        .await;
    harness
        .create_refund(
            tenant_id,
            second_collection.id,
            "payment-service:order-filter:second",
            amount("7.00"),
        )
        .await;

    let (items, total) = harness
        .payment
        .list_refunds(
            tenant_id,
            ListRefundsInput {
                page: 1,
                per_page: 20,
                payment_collection_id: None,
                order_id: Some(first_order.id),
                status: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].payment_collection_id, first_collection.id);

    let (intersection, total) = harness
        .payment
        .list_refunds(
            tenant_id,
            ListRefundsInput {
                page: 1,
                per_page: 20,
                payment_collection_id: Some(first_collection.id),
                order_id: Some(second_order.id),
                status: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(total, 0);
    assert!(intersection.is_empty());

    let (unknown, total) = harness
        .payment
        .list_refunds(
            tenant_id,
            ListRefundsInput {
                page: 1,
                per_page: 20,
                payment_collection_id: None,
                order_id: Some(Uuid::new_v4()),
                status: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(total, 0);
    assert!(unknown.is_empty());
}

async fn create_order(
    service: &rustok_order::services::OrderService,
    tenant_id: Uuid,
    actor_id: Uuid,
    sku: &str,
) -> rustok_order::dto::OrderResponse {
    service
        .create_order(
            tenant_id,
            actor_id,
            rustok_order::dto::CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![rustok_order::dto::CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some(sku.to_string()),
                    title: sku.to_string(),
                    quantity: 1,
                    unit_price: amount("20.00"),
                    metadata: serde_json::json!({}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("order fixture should be created")
}
