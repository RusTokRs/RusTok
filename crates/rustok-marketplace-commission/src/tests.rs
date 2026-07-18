use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use rustok_api::{PortActor, PortContext, PortError};
use rustok_marketplace_allocation::{
    ListMarketplaceAllocationsByOrderRequest, ListMarketplaceAllocationsBySellerRequest,
    MarketplaceAllocationListResponse, MarketplaceAllocationReadPort,
    MarketplaceAllocationStatus, MarketplaceOrderAllocationResponse,
    ReadMarketplaceAllocationByLineRequest,
};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::dto::{
    AssessMarketplaceOrderCommissionsInput, CreateMarketplaceCommissionRuleVersionInput,
    MarketplaceCommissionRuleStatus,
};
use crate::error::MarketplaceCommissionError;
use crate::MarketplaceCommissionService;

#[tokio::test]
async fn assessment_selects_listing_then_seller_rule_and_replays_before_provider_read() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let listing_id = Uuid::new_v4();
    let other_listing_id = Uuid::new_v4();
    let allocations = vec![
        allocation(tenant_id, order_id, seller_id, listing_id, 10_000),
        allocation(tenant_id, order_id, seller_id, other_listing_id, 20_000),
    ];
    let provider = Arc::new(FakeAllocationReader::new(allocations));
    let service = MarketplaceCommissionService::new(db, provider.clone());
    let effective_from = Utc::now().fixed_offset() - Duration::hours(1);

    service
        .create_rule_version_with_receipt(
            tenant_id,
            actor_id,
            "global-rule-v1",
            rule_input(Uuid::new_v4(), None, None, 100, 0, effective_from),
        )
        .await
        .unwrap();
    service
        .create_rule_version_with_receipt(
            tenant_id,
            actor_id,
            "seller-rule-v1",
            rule_input(
                Uuid::new_v4(),
                Some(seller_id),
                None,
                500,
                0,
                effective_from,
            ),
        )
        .await
        .unwrap();
    service
        .create_rule_version_with_receipt(
            tenant_id,
            actor_id,
            "listing-rule-v1",
            rule_input(
                Uuid::new_v4(),
                Some(seller_id),
                Some(listing_id),
                1_000,
                0,
                effective_from,
            ),
        )
        .await
        .unwrap();

    let assessed_at = Utc::now().fixed_offset();
    let input = AssessMarketplaceOrderCommissionsInput {
        order_id,
        assessed_at,
    };
    let created = service
        .assess_order_with_receipt(
            port_context(tenant_id, actor_id, "assess-order"),
            tenant_id,
            actor_id,
            "assess-order",
            input.clone(),
        )
        .await
        .unwrap();
    assert_eq!(created.assessments.len(), 2);
    assert_eq!(created.assessments[0].rate_bps, 1_000);
    assert_eq!(created.assessments[0].commission_amount, 1_000);
    assert_eq!(created.assessments[1].rate_bps, 500);
    assert_eq!(created.assessments[1].commission_amount, 1_000);
    assert_eq!(created.commission_total_amount, 2_000);
    assert_eq!(created.seller_proceeds_total_amount, 28_000);
    assert_eq!(provider.read_count(), 1);

    let replayed = service
        .assess_order_with_receipt(
            port_context(tenant_id, actor_id, "assess-order"),
            tenant_id,
            actor_id,
            "assess-order",
            input,
        )
        .await
        .unwrap();
    assert_eq!(replayed, created);
    assert_eq!(provider.read_count(), 1, "replay must precede provider reads");

    let conflict = service
        .assess_order_with_receipt(
            port_context(tenant_id, actor_id, "assess-order"),
            tenant_id,
            actor_id,
            "assess-order",
            AssessMarketplaceOrderCommissionsInput {
                order_id,
                assessed_at: assessed_at + Duration::seconds(1),
            },
        )
        .await;
    assert!(matches!(
        conflict,
        Err(MarketplaceCommissionError::IdempotencyConflict)
    ));
    assert_eq!(provider.read_count(), 1);
}

#[tokio::test]
async fn rule_versions_are_immutable_and_latest_version_wins_ties() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let listing_id = Uuid::new_v4();
    let provider = Arc::new(FakeAllocationReader::new(vec![allocation(
        tenant_id,
        order_id,
        seller_id,
        listing_id,
        10_000,
    )]));
    let service = MarketplaceCommissionService::new(db, provider);
    let rule_key = Uuid::new_v4();
    let effective_from = Utc::now().fixed_offset() - Duration::hours(1);

    let version_one = service
        .create_rule_version_with_receipt(
            tenant_id,
            actor_id,
            "rule-version-one",
            rule_input(
                rule_key,
                Some(seller_id),
                None,
                200,
                0,
                effective_from,
            ),
        )
        .await
        .unwrap();
    let version_two = service
        .create_rule_version_with_receipt(
            tenant_id,
            actor_id,
            "rule-version-two",
            rule_input(
                rule_key,
                Some(seller_id),
                None,
                300,
                0,
                effective_from,
            ),
        )
        .await
        .unwrap();
    assert_eq!(version_one.version, 1);
    assert_eq!(version_two.version, 2);
    assert_ne!(version_one.id, version_two.id);

    let assessed = service
        .assess_order_with_receipt(
            port_context(tenant_id, actor_id, "assess-latest-rule"),
            tenant_id,
            actor_id,
            "assess-latest-rule",
            AssessMarketplaceOrderCommissionsInput {
                order_id,
                assessed_at: Utc::now().fixed_offset(),
            },
        )
        .await
        .unwrap();
    assert_eq!(assessed.assessments[0].rule_version, 2);
    assert_eq!(assessed.assessments[0].rate_bps, 300);
}

fn rule_input(
    rule_key: Uuid,
    seller_id: Option<Uuid>,
    listing_id: Option<Uuid>,
    rate_bps: i32,
    fixed_amount: i64,
    effective_from: chrono::DateTime<chrono::FixedOffset>,
) -> CreateMarketplaceCommissionRuleVersionInput {
    CreateMarketplaceCommissionRuleVersionInput {
        rule_key,
        seller_id,
        listing_id,
        rate_bps,
        fixed_amount,
        currency_code: (fixed_amount > 0).then(|| "usd".to_string()),
        priority: 0,
        effective_from,
        effective_until: None,
        status: MarketplaceCommissionRuleStatus::Active,
        metadata: serde_json::json!({}),
    }
}

fn allocation(
    tenant_id: Uuid,
    order_id: Uuid,
    seller_id: Uuid,
    listing_id: Uuid,
    total_amount: i64,
) -> MarketplaceOrderAllocationResponse {
    let now = Utc::now().fixed_offset();
    MarketplaceOrderAllocationResponse {
        id: Uuid::new_v4(),
        tenant_id,
        order_id,
        order_line_item_id: Uuid::new_v4(),
        seller_id,
        listing_id,
        master_product_id: Uuid::new_v4(),
        master_variant_id: Uuid::new_v4(),
        quantity: 1,
        currency_code: "USD".to_string(),
        unit_amount: total_amount,
        subtotal_amount: total_amount,
        discount_amount: 0,
        tax_amount: 0,
        total_amount,
        listing_terms_version: 1,
        pricing_reference: None,
        inventory_reference: None,
        fulfillment_profile_slug: None,
        status: MarketplaceAllocationStatus::Allocated,
        metadata: serde_json::json!({}),
        created_at: now,
        updated_at: now,
    }
}

fn port_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("commission-test-{}", Uuid::new_v4()),
    )
    .with_deadline(std::time::Duration::from_secs(5))
    .with_idempotency_key(key)
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_commission_{}?mode=memory&cache=shared",
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

struct FakeAllocationReader {
    allocations: Vec<MarketplaceOrderAllocationResponse>,
    reads: AtomicUsize,
}

impl FakeAllocationReader {
    fn new(allocations: Vec<MarketplaceOrderAllocationResponse>) -> Self {
        Self {
            allocations,
            reads: AtomicUsize::new(0),
        }
    }

    fn read_count(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl MarketplaceAllocationReadPort for FakeAllocationReader {
    async fn read_allocation_by_line(
        &self,
        _context: PortContext,
        request: ReadMarketplaceAllocationByLineRequest,
    ) -> Result<MarketplaceOrderAllocationResponse, PortError> {
        self.allocations
            .iter()
            .find(|allocation| allocation.order_line_item_id == request.order_line_item_id)
            .cloned()
            .ok_or_else(|| PortError::not_found("test.not_found", "allocation not found"))
    }

    async fn list_allocations_by_order(
        &self,
        _context: PortContext,
        request: ListMarketplaceAllocationsByOrderRequest,
    ) -> Result<Vec<MarketplaceOrderAllocationResponse>, PortError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .allocations
            .iter()
            .filter(|allocation| allocation.order_id == request.order_id)
            .cloned()
            .collect())
    }

    async fn list_allocations_by_seller(
        &self,
        _context: PortContext,
        request: ListMarketplaceAllocationsBySellerRequest,
    ) -> Result<MarketplaceAllocationListResponse, PortError> {
        let items = self
            .allocations
            .iter()
            .filter(|allocation| allocation.seller_id == request.seller_id)
            .cloned()
            .collect::<Vec<_>>();
        Ok(MarketplaceAllocationListResponse {
            total: items.len() as u64,
            items,
            page: request.page.max(1),
            per_page: request.per_page.max(1),
        })
    }
}
