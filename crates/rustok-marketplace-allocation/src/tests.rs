use sea_orm::{
    ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::dto::{
    AllocateMarketplaceOrderLineInput, AllocateMarketplaceOrderLinesInput,
};
use crate::entities::{allocation, allocation_receipt};
use crate::error::MarketplaceAllocationError;
use crate::MarketplaceAllocationService;

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
        .allocate_order_lines_with_receipt(
            tenant_id,
            actor_id,
            "allocate-order-lines",
            request,
        )
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
        .allocate_order_lines_with_receipt(
            tenant_id,
            actor_id,
            "first-allocation",
            request.clone(),
        )
        .await
        .unwrap();

    let mut conflicting = request.clone();
    conflicting.lines[0].seller_id = Uuid::new_v4();
    let conflict = service
        .allocate_order_lines_with_receipt(
            tenant_id,
            actor_id,
            "first-allocation",
            conflicting,
        )
        .await;
    assert!(matches!(
        conflict,
        Err(MarketplaceAllocationError::IdempotencyConflict)
    ));

    let second_key = service
        .allocate_order_lines_with_receipt(
            tenant_id,
            actor_id,
            "different-key",
            request,
        )
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
