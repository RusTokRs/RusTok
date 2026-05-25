use rust_decimal::Decimal;
use rustok_order::contracts::{
    CreateOrderInput, CreateOrderLineItemInput, CreateOrderReturnInput, ListOrderReturnsInput,
};
use rustok_order::services::OrderService;
use sea_orm::Database;
use uuid::Uuid;

mod support;

#[tokio::test]
async fn commerce_test_schema_supports_order_returns_filters() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = OrderService::new(db.clone(), None);

    let order = service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                email: Some("buyer@example.com".to_string()),
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    sku: Some("RET-SKU-1".to_string()),
                    title: "Return Candidate".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(2500, 2),
                    metadata: serde_json::json!({"slot":1}),
                }],
                metadata: serde_json::json!({"source":"commerce-order-returns-bridge-test"}),
            },
        )
        .await
        .unwrap();

    let created = service
        .create_return(
            tenant_id,
            order.id,
            CreateOrderReturnInput {
                reason: Some("damaged".to_string()),
                note: None,
                metadata: serde_json::json!({"source":"commerce-order-returns-bridge-test"}),
            },
        )
        .await
        .unwrap();

    let (filtered, filtered_total) = service
        .list_returns(
            tenant_id,
            ListOrderReturnsInput {
                page: 1,
                per_page: 20,
                order_id: Some(order.id),
                status: Some("pending".to_string()),
            },
        )
        .await
        .unwrap();

    assert_eq!(filtered_total, 1);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, created.id);
    assert_eq!(filtered[0].order_id, order.id);
    assert_eq!(filtered[0].status, "pending");
    assert_eq!(filtered[0].reason.as_deref(), Some("damaged"));
}

#[tokio::test]
async fn commerce_order_returns_listing_ignores_blank_status_filter() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = OrderService::new(db.clone(), None);

    let order = service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                email: Some("buyer@example.com".to_string()),
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    sku: Some("RET-SKU-2".to_string()),
                    title: "Return Candidate 2".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(1500, 2),
                    metadata: serde_json::json!({"slot":2}),
                }],
                metadata: serde_json::json!({"source":"commerce-order-returns-blank-filter-test"}),
            },
        )
        .await
        .unwrap();

    let created = service
        .create_return(
            tenant_id,
            order.id,
            CreateOrderReturnInput {
                reason: Some("wrong-size".to_string()),
                note: None,
                metadata: serde_json::json!({"source":"commerce-order-returns-blank-filter-test"}),
            },
        )
        .await
        .unwrap();

    let (rows, total) = service
        .list_returns(
            tenant_id,
            ListOrderReturnsInput {
                page: 1,
                per_page: 20,
                order_id: Some(order.id),
                status: Some("   ".to_string()),
            },
        )
        .await
        .unwrap();

    assert_eq!(total, 1);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, created.id);
}
