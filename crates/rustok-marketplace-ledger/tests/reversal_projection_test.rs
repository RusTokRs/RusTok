use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use rustok_api::{PortActor, PortContext, PortError};
use rustok_marketplace_commission::{
    ListMarketplaceCommissionAssessmentsByOrderRequest,
    ListMarketplaceCommissionAssessmentsBySellerRequest, ListMarketplaceCommissionRulesRequest,
    MarketplaceCommissionAssessmentListResponse, MarketplaceCommissionAssessmentResponse,
    MarketplaceCommissionAssessmentStatus, MarketplaceCommissionReadPort,
    MarketplaceCommissionRuleListResponse, ReadMarketplaceCommissionAssessmentRequest,
};
use rustok_marketplace_ledger::dto::{
    MarketplaceLedgerAccountCode, MarketplaceLedgerEntryDirection,
    MarketplaceLedgerReversalKind, MarketplaceLedgerReversalLineInput,
    MarketplaceSellerBalanceBucket, PostMarketplaceLedgerReversalInput,
    PostMarketplaceOrderLedgerInput, ReadMarketplaceSellerBalanceRequest,
    RebuildMarketplaceSellerBalanceInput,
};
use rustok_marketplace_ledger::entities::{entry, reversal, seller_balance, transaction};
use rustok_marketplace_ledger::{MarketplaceLedgerError, MarketplaceLedgerService};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, IntoActiveModel,
    PaginatorTrait, Set,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn refund_and_chargeback_reversals_are_append_only_and_rebuildable() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let assessment = assessment(tenant_id, order_id, seller_id, 10_000, 1_000);
    let service = MarketplaceLedgerService::new(
        db.clone(),
        Arc::new(FakeCommissionReader::new(vec![assessment.clone()])),
    );
    let posted_at = Utc::now().fixed_offset();
    let original = service
        .post_order_with_receipt(
            port_context(tenant_id, actor_id, "post-order"),
            tenant_id,
            actor_id,
            "post-order",
            PostMarketplaceOrderLedgerInput {
                order_id,
                posted_at,
            },
        )
        .await
        .unwrap();
    let original_seller_entry = original
        .entries
        .iter()
        .find(|item| item.account_code == MarketplaceLedgerAccountCode::SellerPayable)
        .cloned()
        .unwrap();

    let refund_source_id = Uuid::new_v4();
    let refund_input = reversal_input(
        MarketplaceLedgerReversalKind::Refund,
        refund_source_id,
        &assessment,
        500,
        4_500,
        posted_at + Duration::seconds(1),
    );
    let refund = service
        .post_reversal_with_receipt(
            port_context(tenant_id, actor_id, "refund-reversal"),
            tenant_id,
            actor_id,
            "refund-reversal",
            refund_input.clone(),
        )
        .await
        .unwrap();
    assert_eq!(refund.total_amount, 5_000);
    assert_eq!(refund.transaction.debit_total_amount, 5_000);
    assert_eq!(refund.transaction.credit_total_amount, 5_000);
    assert_eq!(refund.entries.len(), 3);
    assert_eq!(
        refund
            .entries
            .iter()
            .filter(|item| {
                item.entry.account_code
                    == MarketplaceLedgerAccountCode::PlatformCommissionRevenue
                    && item.entry.direction == MarketplaceLedgerEntryDirection::Debit
            })
            .map(|item| item.entry.amount)
            .sum::<i64>(),
        500
    );
    assert_eq!(
        refund
            .entries
            .iter()
            .filter(|item| {
                item.entry.account_code == MarketplaceLedgerAccountCode::SellerPayable
                    && item.entry.direction == MarketplaceLedgerEntryDirection::Debit
            })
            .map(|item| item.entry.amount)
            .sum::<i64>(),
        4_500
    );
    assert_eq!(
        refund
            .entries
            .iter()
            .filter(|item| {
                item.entry.account_code == MarketplaceLedgerAccountCode::MarketplaceClearing
                    && item.entry.direction == MarketplaceLedgerEntryDirection::Credit
            })
            .map(|item| item.entry.amount)
            .sum::<i64>(),
        5_000
    );

    let replay = service
        .post_reversal_with_receipt(
            port_context(tenant_id, actor_id, "refund-reversal"),
            tenant_id,
            actor_id,
            "refund-reversal",
            refund_input,
        )
        .await
        .unwrap();
    assert_eq!(replay, refund);

    let chargeback = service
        .post_reversal_with_receipt(
            port_context(tenant_id, actor_id, "chargeback-reversal"),
            tenant_id,
            actor_id,
            "chargeback-reversal",
            reversal_input(
                MarketplaceLedgerReversalKind::Chargeback,
                Uuid::new_v4(),
                &assessment,
                250,
                2_250,
                posted_at + Duration::seconds(2),
            ),
        )
        .await
        .unwrap();
    assert_eq!(chargeback.total_amount, 2_500);

    let balance = service
        .read_seller_balance_projection(
            tenant_id,
            ReadMarketplaceSellerBalanceRequest {
                seller_id,
                currency_code: "usd".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(balance.pending_amount, 2_250);
    assert_eq!(balance.available_amount, 0);
    assert_eq!(balance.reserved_amount, 0);
    assert_eq!(balance.paid_amount, 0);
    assert_eq!(balance.negative_amount, 0);
    assert_eq!(balance.source_entry_count, 3);

    let stored = entry::Entity::find_by_id(original_seller_entry.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.amount, 9_000, "original entry must remain immutable");

    let projection = seller_balance::Entity::find_by_id(balance.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut corrupted = projection.into_active_model();
    corrupted.pending_amount = Set(123);
    corrupted.negative_amount = Set(999);
    corrupted.update(&db).await.unwrap();
    let rebuilt = service
        .rebuild_seller_balance_projection(
            tenant_id,
            RebuildMarketplaceSellerBalanceInput {
                seller_id,
                currency_code: "USD".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(rebuilt.pending_amount, 2_250);
    assert_eq!(rebuilt.negative_amount, 0);

    assert_eq!(transaction::Entity::find().count(&db).await.unwrap(), 3);
    assert_eq!(reversal::Entity::find().count(&db).await.unwrap(), 2);
}

#[tokio::test]
async fn reversal_rejects_duplicate_source_and_cumulative_over_reversal() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let assessment = assessment(tenant_id, order_id, seller_id, 10_000, 1_000);
    let service = MarketplaceLedgerService::new(
        db.clone(),
        Arc::new(FakeCommissionReader::new(vec![assessment.clone()])),
    );
    let posted_at = Utc::now().fixed_offset();
    service
        .post_order_with_receipt(
            port_context(tenant_id, actor_id, "post-order"),
            tenant_id,
            actor_id,
            "post-order",
            PostMarketplaceOrderLedgerInput {
                order_id,
                posted_at,
            },
        )
        .await
        .unwrap();

    let source_id = Uuid::new_v4();
    let first = reversal_input(
        MarketplaceLedgerReversalKind::Refund,
        source_id,
        &assessment,
        800,
        7_200,
        posted_at + Duration::seconds(1),
    );
    service
        .post_reversal_with_receipt(
            port_context(tenant_id, actor_id, "first-refund"),
            tenant_id,
            actor_id,
            "first-refund",
            first.clone(),
        )
        .await
        .unwrap();

    let duplicate_source = service
        .post_reversal_with_receipt(
            port_context(tenant_id, actor_id, "duplicate-source"),
            tenant_id,
            actor_id,
            "duplicate-source",
            first,
        )
        .await;
    assert!(matches!(
        duplicate_source,
        Err(MarketplaceLedgerError::ReversalAlreadyPosted(id)) if id == source_id
    ));

    let over_reversal = service
        .post_reversal_with_receipt(
            port_context(tenant_id, actor_id, "over-reversal"),
            tenant_id,
            actor_id,
            "over-reversal",
            reversal_input(
                MarketplaceLedgerReversalKind::Chargeback,
                Uuid::new_v4(),
                &assessment,
                300,
                2_000,
                posted_at + Duration::seconds(2),
            ),
        )
        .await;
    assert!(matches!(
        over_reversal,
        Err(MarketplaceLedgerError::Validation(_))
    ));
    assert_eq!(transaction::Entity::find().count(&db).await.unwrap(), 2);
    assert_eq!(reversal::Entity::find().count(&db).await.unwrap(), 1);
}

fn reversal_input(
    kind: MarketplaceLedgerReversalKind,
    source_id: Uuid,
    assessment: &MarketplaceCommissionAssessmentResponse,
    commission_amount: i64,
    seller_amount: i64,
    reversed_at: chrono::DateTime<chrono::FixedOffset>,
) -> PostMarketplaceLedgerReversalInput {
    PostMarketplaceLedgerReversalInput {
        kind,
        source_id,
        order_id: assessment.order_id,
        currency_code: assessment.currency_code.clone(),
        reversed_at,
        lines: vec![MarketplaceLedgerReversalLineInput {
            assessment_id: assessment.id,
            allocation_id: assessment.allocation_id,
            order_line_item_id: assessment.order_line_item_id,
            seller_id: assessment.seller_id,
            commission_amount,
            seller_amount,
            seller_balance_bucket: MarketplaceSellerBalanceBucket::Pending,
        }],
        metadata: serde_json::json!({"test": true}),
    }
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_ledger_reversal_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    let manager = SchemaManager::new(&db);
    for migration in rustok_marketplace_ledger::migrations::migrations() {
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
        format!("ledger-reversal-test-{}", Uuid::new_v4()),
    )
    .with_deadline(std::time::Duration::from_secs(5))
    .with_idempotency_key(key)
}

struct FakeCommissionReader {
    assessments: Vec<MarketplaceCommissionAssessmentResponse>,
}

impl FakeCommissionReader {
    fn new(assessments: Vec<MarketplaceCommissionAssessmentResponse>) -> Self {
        Self { assessments }
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
