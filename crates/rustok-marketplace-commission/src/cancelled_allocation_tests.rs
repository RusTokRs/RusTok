use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_marketplace_allocation::{
    ListMarketplaceAllocationsByOrderRequest, ListMarketplaceAllocationsBySellerRequest,
    MarketplaceAllocationListResponse, MarketplaceAllocationReadPort,
    MarketplaceAllocationStatus, MarketplaceOrderAllocationResponse,
    ReadMarketplaceAllocationByLineRequest,
};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, EntityTrait, PaginatorTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::dto::AssessMarketplaceOrderCommissionsInput;
use crate::entities::{assessment, receipt};
use crate::error::MarketplaceCommissionError;
use crate::MarketplaceCommissionService;

#[tokio::test]
async fn cancelled_allocation_is_rejected_before_receipt_admission() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let provider = Arc::new(CancelledAllocationReader {
        allocation: cancelled_allocation(tenant_id, order_id),
        reads: AtomicUsize::new(0),
    });
    let service = MarketplaceCommissionService::new(db.clone(), provider.clone());

    let result = service
        .assess_order_with_receipt(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::user(actor_id.to_string()),
                "en",
                format!("cancelled-allocation-{}", Uuid::new_v4()),
            )
            .with_deadline(std::time::Duration::from_secs(5))
            .with_idempotency_key("cancelled-allocation"),
            tenant_id,
            actor_id,
            "cancelled-allocation",
            AssessMarketplaceOrderCommissionsInput {
                order_id,
                assessed_at: Utc::now().fixed_offset(),
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(MarketplaceCommissionError::AllocationBoundary {
            ref code,
            retryable: false,
            ..
        }) if code == "marketplace_commission.allocation_not_assessable"
    ));
    assert_eq!(provider.reads.load(Ordering::SeqCst), 1);
    assert_eq!(assessment::Entity::find().count(&db).await.unwrap(), 0);
    assert_eq!(receipt::Entity::find().count(&db).await.unwrap(), 0);
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_commission_cancelled_{}?mode=memory&cache=shared",
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

fn cancelled_allocation(
    tenant_id: Uuid,
    order_id: Uuid,
) -> MarketplaceOrderAllocationResponse {
    let now = Utc::now().fixed_offset();
    MarketplaceOrderAllocationResponse {
        id: Uuid::new_v4(),
        tenant_id,
        order_id,
        order_line_item_id: Uuid::new_v4(),
        seller_id: Uuid::new_v4(),
        listing_id: Uuid::new_v4(),
        master_product_id: Uuid::new_v4(),
        master_variant_id: Uuid::new_v4(),
        quantity: 1,
        currency_code: "USD".to_string(),
        unit_amount: 1_000,
        subtotal_amount: 1_000,
        discount_amount: 0,
        tax_amount: 0,
        total_amount: 1_000,
        listing_terms_version: 1,
        pricing_reference: None,
        inventory_reference: None,
        fulfillment_profile_slug: None,
        status: MarketplaceAllocationStatus::Cancelled,
        metadata: serde_json::json!({}),
        created_at: now,
        updated_at: now,
    }
}

struct CancelledAllocationReader {
    allocation: MarketplaceOrderAllocationResponse,
    reads: AtomicUsize,
}

#[async_trait]
impl MarketplaceAllocationReadPort for CancelledAllocationReader {
    async fn read_allocation_by_line(
        &self,
        _context: PortContext,
        _request: ReadMarketplaceAllocationByLineRequest,
    ) -> Result<MarketplaceOrderAllocationResponse, PortError> {
        Ok(self.allocation.clone())
    }

    async fn list_allocations_by_order(
        &self,
        _context: PortContext,
        _request: ListMarketplaceAllocationsByOrderRequest,
    ) -> Result<Vec<MarketplaceOrderAllocationResponse>, PortError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(vec![self.allocation.clone()])
    }

    async fn list_allocations_by_seller(
        &self,
        _context: PortContext,
        request: ListMarketplaceAllocationsBySellerRequest,
    ) -> Result<MarketplaceAllocationListResponse, PortError> {
        Ok(MarketplaceAllocationListResponse {
            items: vec![self.allocation.clone()],
            total: 1,
            page: request.page.max(1),
            per_page: request.per_page.max(1),
        })
    }
}
