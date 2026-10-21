//! Order cancellation orchestration (`DECISIONS/2026-10-21-order-cancellation-orchestration.md`).
//!
//! Written without a Rust toolchain available; run with
//! `cargo test -p rustok-commerce --test order_cancellation_test` before relying on it.

use std::time::Duration;

use rust_decimal::Decimal;
use rustok_api::{PortActor, PortContext};
use rustok_commerce::OrderCancellationOrchestrator;
use rustok_order::dto::{
    CreateOrderInput, CreateOrderLineItemInput, OrderLineFulfillmentRequirement, OrderResponse,
};
use rustok_order::services::OrderService;
use rustok_order::CancelOrderRequest;
use rustok_payment::dto::{
    AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput,
    PaymentCollectionResponse,
};
use rustok_payment::services::PaymentService;
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{Database, DatabaseConnection};
use uuid::Uuid;

mod support;

async fn setup() -> (DatabaseConnection, Uuid, Uuid) {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;
    (db, Uuid::new_v4(), Uuid::new_v4())
}

async fn create_order(db: &DatabaseConnection, tenant_id: Uuid, actor_id: Uuid) -> OrderResponse {
    OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    fulfillment_requirement: OrderLineFulfillmentRequirement::Physical,
                    shipping_profile_slug: Some("default".to_string()),
                    seller_id: None,
                    sku: Some("CANCEL-1".to_string()),
                    title: "Cancellable".to_string(),
                    quantity: 2,
                    unit_price: Decimal::new(2500, 2),
                    metadata: serde_json::json!({}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({"source":"order-cancellation-test"}),
            },
        )
        .await
        .unwrap()
}

async fn create_collection(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    order: &OrderResponse,
    capture: bool,
) -> PaymentCollectionResponse {
    let payments = PaymentService::new(db.clone());
    let collection = payments
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                customer_id: order.customer_id,
                currency_code: "usd".to_string(),
                amount: order.total_amount,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    payments
        .authorize_collection(
            tenant_id,
            collection.id,
            AuthorizePaymentInput {
                provider_id: Some("manual".to_string()),
                provider_payment_id: Some(format!("cancel-test-{}", collection.id)),
                amount: Some(order.total_amount),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    if capture {
        payments
            .capture_collection(
                tenant_id,
                collection.id,
                CapturePaymentInput {
                    amount: Some(order.total_amount),
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .unwrap();
    }
    payments.get_collection(tenant_id, collection.id).await.unwrap()
}

fn context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        "order-cancellation-test",
    )
    .with_idempotency_key(key)
    .with_deadline(Duration::from_secs(30))
}

fn request(order_id: Uuid) -> CancelOrderRequest {
    CancelOrderRequest {
        order_id,
        reason: Some("customer asked".to_string()),
    }
}

#[tokio::test]
async fn cancelling_a_plain_order_is_idempotent() {
    let (db, tenant_id, actor_id) = setup().await;
    let order = create_order(&db, tenant_id, actor_id).await;
    let orchestrator = OrderCancellationOrchestrator::new(db.clone(), mock_transactional_event_bus());

    let first = orchestrator
        .cancel(context(tenant_id, actor_id, "cancel-1"), request(order.id))
        .await
        .unwrap();
    assert_eq!(first.status, "cancelled");

    let replay = orchestrator
        .cancel(context(tenant_id, actor_id, "cancel-2"), request(order.id))
        .await
        .unwrap();
    assert_eq!(replay.status, "cancelled");
}

#[tokio::test]
async fn cancelling_voids_an_authorized_collection() {
    let (db, tenant_id, actor_id) = setup().await;
    let order = create_order(&db, tenant_id, actor_id).await;
    let collection = create_collection(&db, tenant_id, &order, false).await;
    let orchestrator = OrderCancellationOrchestrator::new(db.clone(), mock_transactional_event_bus());

    let cancelled = orchestrator
        .cancel(context(tenant_id, actor_id, "cancel-void"), request(order.id))
        .await
        .unwrap();
    assert_eq!(cancelled.status, "cancelled");

    let after = PaymentService::new(db.clone())
        .get_collection(tenant_id, collection.id)
        .await
        .unwrap();
    assert_eq!(after.status, "cancelled");
}

#[tokio::test]
async fn cancelling_refunds_a_captured_collection_in_full_once() {
    let (db, tenant_id, actor_id) = setup().await;
    let order = create_order(&db, tenant_id, actor_id).await;
    let collection = create_collection(&db, tenant_id, &order, true).await;
    let orchestrator = OrderCancellationOrchestrator::new(db.clone(), mock_transactional_event_bus());

    orchestrator
        .cancel(context(tenant_id, actor_id, "cancel-refund-1"), request(order.id))
        .await
        .unwrap();
    // A repeated command (even with a new idempotency key) must not create a second refund.
    orchestrator
        .cancel(context(tenant_id, actor_id, "cancel-refund-2"), request(order.id))
        .await
        .unwrap();

    let after = PaymentService::new(db.clone())
        .get_collection(tenant_id, collection.id)
        .await
        .unwrap();
    assert_eq!(after.refunds.len(), 1);
    assert_eq!(after.refunds[0].amount, collection.captured_amount);
}

#[tokio::test]
async fn cancel_requires_write_semantics() {
    let (db, tenant_id, actor_id) = setup().await;
    let order = create_order(&db, tenant_id, actor_id).await;
    let orchestrator = OrderCancellationOrchestrator::new(db.clone(), mock_transactional_event_bus());

    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        "order-cancellation-test",
    );
    assert!(orchestrator.cancel(context, request(order.id)).await.is_err());
}
