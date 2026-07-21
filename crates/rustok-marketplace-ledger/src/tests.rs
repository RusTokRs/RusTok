use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use rustok_api::{PortActor, PortContext, PortError};
use rustok_marketplace_commission::{
    ListMarketplaceCommissionAssessmentsByOrderRequest,
    ListMarketplaceCommissionAssessmentsBySellerRequest,
    ListMarketplaceCommissionRulesRequest,
    MarketplaceCommissionAssessmentListResponse,
    MarketplaceCommissionAssessmentResponse,
    MarketplaceCommissionAssessmentStatus,
    MarketplaceCommissionReadPort,
    MarketplaceCommissionRuleListResponse,
    ReadMarketplaceCommissionAssessmentRequest,
};
use sea_orm::{
    ConnectOptions, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::dto::{
    MarketplaceLedgerAccountCode, MarketplaceLedgerEntryDirection,
    PostMarketplaceOrderLedgerInput,
};
use crate::entities::{entry, receipt, transaction};
use crate::error::MarketplaceLedgerError;
use crate::MarketplaceLedgerService;

#[tokio::test]
async fn order_posting_is_balanced_atomic_and_replay_safe() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let provider = Arc::new(FakeCommissionReader::new(vec![
        assessment(tenant_id, order_id, seller_id, 10_000, 1_000),
        assessment(tenant_id, order_id, seller_id, 20_000, 2_000),
    ]));
    let service = MarketplaceLedgerService::new(db.clone(), provider.clone());
    let input = PostMarketplaceOrderLedgerInput {
        order_id,
        posted_at: Utc::now().fixed_offset(),
    };

    let posted = service
        .post_order_with_receipt(
            port_context(tenant_id, actor_id, "post-order"),
            tenant_id,
            actor_id,
            "post-order",
            input.clone(),
        )
        .await
        .unwrap();

    assert_eq!(posted.debit_total_amount, 30_000);
    assert_eq!(posted.credit_total_amount, 30_000);
    assert_eq!(posted.entries.len(), 6);
    assert_eq!(
        posted
            .entries
            .iter()
            .filter(|entry| {
                entry.account_code == MarketplaceLedgerAccountCode::MarketplaceClearing
                    && entry.direction == MarketplaceLedgerEntryDirection::Debit
            })
            .map(|entry| entry.amount)
            .sum::<i64>(),
        30_000
    );
    assert_eq!(
        posted
            .entries
            .iter()
            .filter(|entry| {
                entry.account_code
                    == MarketplaceLedgerAccountCode::PlatformCommissionRevenue
                    && entry.direction == MarketplaceLedgerEntryDirection::Credit
            })
            .map(|entry| entry.amount)
            .sum::<i64>(),
        3_000
    );
    assert_eq!(
        posted
            .entries
            .iter()
            .filter(|entry| {
                entry.account_code == MarketplaceLedgerAccountCode::SellerPayable
                    && entry.direction == MarketplaceLedgerEntryDirection::Credit
            })
            .map(|entry| entry.amount)
            .sum::<i64>(),
        27_000
    );
    assert_eq!(provider.read_count(), 1);

    let replayed = service
        .post_order_with_receipt(
            port_context(tenant_id, actor_id, "post-order"),
            tenant_id,
            actor_id,
            "post-order",
            input.clone(),
        )
        .await
        .unwrap();
    assert_eq!(replayed, posted);
    assert_eq!(provider.read_count(), 1, "receipt replay must precede provider reads");

    let conflict = service
        .post_order_with_receipt(
            port_context(tenant_id, actor_id, "post-order"),
            tenant_id,
            actor_id,
            "post-order",
            PostMarketplaceOrderLedgerInput {
                order_id,
                posted_at: input.posted_at + Duration::seconds(1),
            },
        )
        .await;
    assert!(matches!(
        conflict,
        Err(MarketplaceLedgerError::IdempotencyConflict)
    ));
    assert_eq!(provider.read_count(), 1);

    let second_key = service
        .post_order_with_receipt(
            port_context(tenant_id, actor_id, "post-order-second-key"),
            tenant_id,
            actor_id,
            "post-order-second-key",
            input,
        )
        .await;
    assert!(matches!(
        second_key,
        Err(MarketplaceLedgerError::OrderAlreadyPosted(id)) if id == order_id
    ));
    assert_eq!(provider.read_count(), 2);
    assert_eq!(transaction::Entity::find().count(&db).await.unwrap(), 1);
    assert_eq!(entry::Entity::find().count(&db).await.unwrap(), 6);
    assert_eq!(receipt::Entity::find().count(&db).await.unwrap(), 1);
}

#[tokio::test]
async fn reversed_assessment_is_rejected_before_receipt_admission() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let mut reversed = assessment(tenant_id, order_id, seller_id, 10_000, 1_000);
    reversed.status = MarketplaceCommissionAssessmentStatus::Reversed;
    let provider = Arc::new(FakeCommissionReader::new(vec![reversed]));
    let service = MarketplaceLedgerService::new(db.clone(), provider.clone());

    let result = service
        .post_order_with_receipt(
            port_context(tenant_id, actor_id, "reversed-assessment"),
            tenant_id,
            actor_id,
            "reversed-assessment",
            PostMarketplaceOrderLedgerInput {
                order_id,
                posted_at: Utc::now().fixed_offset(),
            },
        )
        .await;

    assert!(matches!(result, Err(MarketplaceLedgerError::Validation(_))));
    assert_eq!(provider.read_count(), 1);
    assert_eq!(transaction::Entity::find().count(&db).await.unwrap(), 0);
    assert_eq!(receipt::Entity::find().count(&db).await.unwrap(), 0);
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_ledger_{}?mode=memory&cache=shared",
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

fn assessment(
    tenant_id: Uuid,
    order_id: Uuid,
    seller_id: Uuid,
    allocation_total: i64,
    commission: i64,
) -> MarketplaceCommissionAssessmentResponse {
    let now = Utc::now().fixed_offset();
    MarketplaceCommissionAssessmentResponse {
        id: Uuid::new_v4(),
        tenant_id,
        allocation_id: Uuid::new_v4(),
        order_id,
        order_line_item_id: Uuid::new_v4(),
        seller_id,
        listing_id: Uuid::new_v4(),
        rule_id: Uuid::new_v4(),
        rule_key: Uuid::new_v4(),
        rule_version: 1,
        currency_code: "USD".to_string(),
        allocation_total_amount: allocation_total,
        rate_bps: 1_000,
        fixed_amount: 0,
        commission_amount: commission,
        seller_proceeds_amount: allocation_total - commission,
        status: MarketplaceCommissionAssessmentStatus::Assessed,
        metadata: serde_json::json!({}),
        assessed_at: now,
        created_at: now,
    }
}

fn port_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("ledger-test-{}", Uuid::new_v4()),
    )
    .with_deadline(std::time::Duration::from_secs(5))
    .with_idempotency_key(key)
}

struct FakeCommissionReader {
    assessments: Vec<MarketplaceCommissionAssessmentResponse>,
    reads: AtomicUsize,
}

impl FakeCommissionReader {
    fn new(assessments: Vec<MarketplaceCommissionAssessmentResponse>) -> Self {
        Self {
            assessments,
            reads: AtomicUsize::new(0),
        }
    }

    fn read_count(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl MarketplaceCommissionReadPort for FakeCommissionReader {
    async fn read_assessment(
        &self,
        _context: PortContext,
        request: ReadMarketplaceCommissionAssessmentRequest,
    ) -> Result<MarketplaceCommissionAssessmentResponse, PortError> {
        self.assessments
            .iter()
            .find(|assessment| assessment.allocation_id == request.allocation_id)
            .cloned()
            .ok_or_else(|| PortError::not_found("test.not_found", "assessment not found"))
    }

    async fn list_assessments_by_order(
        &self,
        _context: PortContext,
        request: ListMarketplaceCommissionAssessmentsByOrderRequest,
    ) -> Result<Vec<MarketplaceCommissionAssessmentResponse>, PortError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .assessments
            .iter()
            .filter(|assessment| assessment.order_id == request.order_id)
            .cloned()
            .collect())
    }

    async fn list_assessments_by_seller(
        &self,
        _context: PortContext,
        request: ListMarketplaceCommissionAssessmentsBySellerRequest,
    ) -> Result<MarketplaceCommissionAssessmentListResponse, PortError> {
        let items = self
            .assessments
            .iter()
            .filter(|assessment| assessment.seller_id == request.seller_id)
            .cloned()
            .collect::<Vec<_>>();
        Ok(MarketplaceCommissionAssessmentListResponse {
            total: items.len() as u64,
            items,
            page: request.page.max(1),
            per_page: request.per_page.max(1),
        })
    }

    async fn list_rules(
        &self,
        _context: PortContext,
        request: ListMarketplaceCommissionRulesRequest,
    ) -> Result<MarketplaceCommissionRuleListResponse, PortError> {
        Ok(MarketplaceCommissionRuleListResponse {
            items: Vec::new(),
            total: 0,
            page: request.page.max(1),
            per_page: request.per_page.max(1),
        })
    }
}
