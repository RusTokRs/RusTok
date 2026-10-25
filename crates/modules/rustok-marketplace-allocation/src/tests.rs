use sea_orm::{
    ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::MarketplaceAllocationService;
use crate::dto::{AllocateMarketplaceOrderLineInput, AllocateMarketplaceOrderLinesInput};
use crate::entities::{allocation, allocation_receipt};
use crate::error::MarketplaceAllocationError;

#[tokio::test]
async fn allocation_batch_commits_once_and_replays_saved_response() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let request = allocation_request(order_id, 2);
    let service = MarketplaceAllocationService::new(db.clone());

    let created = service
        .allocate_order_lines_with_receipt(
            tenant_id,
            actor_id,
            "allocate-order-lines",
            request.clone(),
        )
        .await
        .unwrap();
    let replayed = service
        .allocate_order_lines_with_receipt(tenant_id, actor_id, "allocate-order-lines", request)
        .await
        .unwrap();

    assert_eq!(created, replayed);
    assert_eq!(created.allocations.len(), 2);
    assert_eq!(
        allocation::Entity::find()
            .filter(allocation::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        allocation_receipt::Entity::find()
            .filter(allocation_receipt::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn conflicting_receipt_and_reallocation_are_rejected_without_partial_rows() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let request = allocation_request(order_id, 2);
    let service = MarketplaceAllocationService::new(db.clone());

    service
        .allocate_order_lines_with_receipt(tenant_id, actor_id, "first-allocation", request.clone())
        .await
        .unwrap();

    let mut conflicting = request.clone();
    conflicting.lines[0].seller_id = Uuid::new_v4();
    let conflict = service
        .allocate_order_lines_with_receipt(tenant_id, actor_id, "first-allocation", conflicting)
        .await;
    assert!(matches!(
        conflict,
        Err(MarketplaceAllocationError::IdempotencyConflict)
    ));

    let second_key = service
        .allocate_order_lines_with_receipt(tenant_id, actor_id, "different-key", request)
        .await;
    assert!(matches!(
        second_key,
        Err(MarketplaceAllocationError::LineAlreadyAllocated(_))
    ));
    assert_eq!(
        allocation::Entity::find()
            .filter(allocation::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .unwrap(),
        2
    );
    assert!(
        allocation_receipt::Entity::find()
            .filter(allocation_receipt::Column::TenantId.eq(tenant_id))
            .filter(allocation_receipt::Column::IdempotencyKey.eq("different-key"))
            .one(&db)
            .await
            .unwrap()
            .is_none(),
        "pending receipt must roll back when any order line is already allocated"
    );
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_allocations_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    let manager = SchemaManager::new(&db);
    for migration in crate::migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    db
}

fn allocation_request(order_id: Uuid, line_count: usize) -> AllocateMarketplaceOrderLinesInput {
    AllocateMarketplaceOrderLinesInput {
        order_id,
        currency_code: "usd".to_string(),
        lines: (0..line_count)
            .map(|index| AllocateMarketplaceOrderLineInput {
                order_line_item_id: Uuid::new_v4(),
                seller_id: Uuid::new_v4(),
                listing_id: Uuid::new_v4(),
                master_product_id: Uuid::new_v4(),
                master_variant_id: Uuid::new_v4(),
                quantity: 2,
                unit_amount: 1_000 + index as i64,
                subtotal_amount: 2_000 + (index as i64 * 2),
                discount_amount: 100,
                tax_amount: 190,
                total_amount: 2_090 + (index as i64 * 2),
                listing_terms_version: 1,
                pricing_reference: Some("price-list-v1".to_string()),
                inventory_reference: Some("inventory-item-v1".to_string()),
                fulfillment_profile_slug: Some("standard".to_string()),
                metadata: serde_json::json!({"source": "checkout"}),
            })
            .collect(),
    }
}

#[tokio::test]
async fn input_validation_and_arithmetic_invariants_are_enforced() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let service = MarketplaceAllocationService::new(db);

    // Nil tenant or actor
    let req = allocation_request(order_id, 1);
    assert!(
        service
            .allocate_order_lines_with_receipt(Uuid::nil(), actor_id, "key1", req.clone())
            .await
            .is_err()
    );
    assert!(
        service
            .allocate_order_lines_with_receipt(tenant_id, Uuid::nil(), "key2", req.clone())
            .await
            .is_err()
    );

    // Nil order_id
    let mut bad_order = req.clone();
    bad_order.order_id = Uuid::nil();
    assert!(
        service
            .allocate_order_lines_with_receipt(tenant_id, actor_id, "key3", bad_order)
            .await
            .is_err()
    );

    // Invalid currency
    let mut bad_curr = req.clone();
    bad_curr.currency_code = "US".to_string();
    assert!(
        service
            .allocate_order_lines_with_receipt(tenant_id, actor_id, "key4", bad_curr)
            .await
            .is_err()
    );

    // Zero quantity
    let mut bad_qty = req.clone();
    bad_qty.lines[0].quantity = 0;
    assert!(
        service
            .allocate_order_lines_with_receipt(tenant_id, actor_id, "key5", bad_qty)
            .await
            .is_err()
    );

    // Subtotal mismatch (unit 1000 * qty 2 != subtotal 1500)
    let mut bad_subtotal = req.clone();
    bad_subtotal.lines[0].subtotal_amount = 1500;
    assert!(
        service
            .allocate_order_lines_with_receipt(tenant_id, actor_id, "key6", bad_subtotal)
            .await
            .is_err()
    );

    // Discount exceeding subtotal
    let mut bad_disc = req.clone();
    bad_disc.lines[0].discount_amount = 3000;
    assert!(
        service
            .allocate_order_lines_with_receipt(tenant_id, actor_id, "key7", bad_disc)
            .await
            .is_err()
    );

    // Read methods reject nil UUIDs
    assert!(service.get_by_order_line(Uuid::nil(), Uuid::new_v4()).await.is_err());
    assert!(service.get_by_order_line(tenant_id, Uuid::nil()).await.is_err());
    assert!(service.list_by_order(Uuid::nil(), Uuid::new_v4()).await.is_err());
    assert!(service.list_by_order(tenant_id, Uuid::nil()).await.is_err());
    assert!(service.list_by_seller(Uuid::nil(), crate::dto::ListMarketplaceAllocationsBySellerRequest {
        seller_id: Uuid::new_v4(),
        page: 1,
        per_page: 10,
        status: None,
    }).await.is_err());
    assert!(service.list_by_seller(tenant_id, crate::dto::ListMarketplaceAllocationsBySellerRequest {
        seller_id: Uuid::nil(),
        page: 1,
        per_page: 10,
        status: None,
    }).await.is_err());
}

