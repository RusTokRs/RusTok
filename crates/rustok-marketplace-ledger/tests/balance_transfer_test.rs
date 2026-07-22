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
    MarketplaceLedgerAccountCode, MarketplaceSellerBalanceTransferKind,
    MarketplaceSellerBalanceTransferLineInput, PostMarketplaceOrderLedgerInput,
    PostMarketplaceSellerBalanceTransferInput, ReadMarketplaceSellerBalanceRequest,
};
use rustok_marketplace_ledger::entities::{
    balance_transfer, balance_transfer_line, entry_balance_bucket, transaction,
};
use rustok_marketplace_ledger::{MarketplaceLedgerError, MarketplaceLedgerService};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, EntityTrait, PaginatorTrait};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn transfers_move_seller_payable_between_buckets_and_replay_exactly() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let first = assessment(tenant_id, order_id, seller_id, 10_000, 1_000);
    let second = assessment(tenant_id, order_id, seller_id, 5_000, 500);
    let service = MarketplaceLedgerService::new(
        db.clone(),
        Arc::new(FakeCommissionReader::new(vec![first.clone(), second])),
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
    let original_credit = original
        .entries
        .iter()
        .find(|item| {
            item.account_code == MarketplaceLedgerAccountCode::SellerPayable
                && item.assessment_id == first.id
        })
        .cloned()
        .unwrap();

    let pending_release_input = transfer_input(
        MarketplaceSellerBalanceTransferKind::PendingRelease,
        Uuid::new_v4(),
        seller_id,
        original_credit.id,
        6_000,
        posted_at + Duration::seconds(1),
    );
    let pending_release = service
        .post_balance_transfer_with_receipt(
            port_context(tenant_id, actor_id, "pending-release"),
            tenant_id,
            actor_id,
            "pending-release",
            pending_release_input.clone(),
        )
        .await
        .unwrap();
    let replay = service
        .post_balance_transfer_with_receipt(
            port_context(tenant_id, actor_id, "pending-release"),
            tenant_id,
            actor_id,
            "pending-release",
            pending_release_input.clone(),
        )
        .await
        .unwrap();
    assert_eq!(pending_release, replay);

    let duplicate_source = service
        .post_balance_transfer_with_receipt(
            port_context(tenant_id, actor_id, "pending-release-other-key"),
            tenant_id,
            actor_id,
            "pending-release-other-key",
            pending_release_input,
        )
        .await;
    assert!(matches!(
        duplicate_source,
        Err(MarketplaceLedgerError::BalanceTransferAlreadyPosted(_))
    ));

    let available_credit = pending_release.lines[0].credit_entry.id;
    let reserve_hold = service
        .post_balance_transfer_with_receipt(
            port_context(tenant_id, actor_id, "reserve-hold"),
            tenant_id,
            actor_id,
            "reserve-hold",
            transfer_input(
                MarketplaceSellerBalanceTransferKind::ReserveHold,
                Uuid::new_v4(),
                seller_id,
                available_credit,
                4_000,
                posted_at + Duration::seconds(2),
            ),
        )
        .await
        .unwrap();
    let reserved_credit = reserve_hold.lines[0].credit_entry.id;

    service
        .post_balance_transfer_with_receipt(
            port_context(tenant_id, actor_id, "reserve-release"),
            tenant_id,
            actor_id,
            "reserve-release",
            transfer_input(
                MarketplaceSellerBalanceTransferKind::ReserveRelease,
                Uuid::new_v4(),
                seller_id,
                reserved_credit,
                1_000,
                posted_at + Duration::seconds(3),
            ),
        )
        .await
        .unwrap();

    let settlement = service
        .post_balance_transfer_with_receipt(
            port_context(tenant_id, actor_id, "payout-settlement"),
            tenant_id,
            actor_id,
            "payout-settlement",
            transfer_input(
                MarketplaceSellerBalanceTransferKind::PayoutSettlement,
                Uuid::new_v4(),
                seller_id,
                reserved_credit,
                2_000,
                posted_at + Duration::seconds(4),
            ),
        )
        .await
        .unwrap();
    let paid_credit = settlement.lines[0].credit_entry.id;

    service
        .post_balance_transfer_with_receipt(
            port_context(tenant_id, actor_id, "payout-reversal"),
            tenant_id,
            actor_id,
            "payout-reversal",
            transfer_input(
                MarketplaceSellerBalanceTransferKind::PayoutReversal,
                Uuid::new_v4(),
                seller_id,
                paid_credit,
                500,
                posted_at + Duration::seconds(5),
            ),
        )
        .await
        .unwrap();

    let balance = service
        .read_seller_balance_projection(
            tenant_id,
            ReadMarketplaceSellerBalanceRequest {
                seller_id,
                currency_code: "USD".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(balance.pending_amount, 7_500);
    assert_eq!(balance.available_amount, 3_500);
    assert_eq!(balance.reserved_amount, 1_000);
    assert_eq!(balance.paid_amount, 1_500);
    assert_eq!(balance.negative_amount, 0);

    assert_eq!(transaction::Entity::find().count(&db).await.unwrap(), 6);
    assert_eq!(balance_transfer::Entity::find().count(&db).await.unwrap(), 5);
    assert_eq!(balance_transfer_line::Entity::find().count(&db).await.unwrap(), 5);
    assert_eq!(entry_balance_bucket::Entity::find().count(&db).await.unwrap(), 10);
}

#[tokio::test]
async fn cumulative_reference_capacity_rejects_reusing_an_original_credit() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let first = assessment(tenant_id, order_id, seller_id, 10_000, 1_000);
    let second = assessment(tenant_id, order_id, seller_id, 5_000, 500);
    let service = MarketplaceLedgerService::new(
        db.clone(),
        Arc::new(FakeCommissionReader::new(vec![first.clone(), second])),
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
    let reference_entry_id = original
        .entries
        .iter()
        .find(|item| {
            item.account_code == MarketplaceLedgerAccountCode::SellerPayable
                && item.assessment_id == first.id
        })
        .unwrap()
        .id;

    service
        .post_balance_transfer_with_receipt(
            port_context(tenant_id, actor_id, "release-one"),
            tenant_id,
            actor_id,
            "release-one",
            transfer_input(
                MarketplaceSellerBalanceTransferKind::PendingRelease,
                Uuid::new_v4(),
                seller_id,
                reference_entry_id,
                6_000,
                posted_at + Duration::seconds(1),
            ),
        )
        .await
        .unwrap();
    let over_capacity = service
        .post_balance_transfer_with_receipt(
            port_context(tenant_id, actor_id, "release-two"),
            tenant_id,
            actor_id,
            "release-two",
            transfer_input(
                MarketplaceSellerBalanceTransferKind::PendingRelease,
                Uuid::new_v4(),
                seller_id,
                reference_entry_id,
                4_000,
                posted_at + Duration::seconds(2),
            ),
        )
        .await;
    assert!(matches!(
        over_capacity,
        Err(MarketplaceLedgerError::Validation(message))
            if message.contains("cumulative transfer amount")
    ));
    assert_eq!(balance_transfer::Entity::find().count(&db).await.unwrap(), 1);
}

fn transfer_input(
    kind: MarketplaceSellerBalanceTransferKind,
    source_id: Uuid,
    seller_id: Uuid,
    reference_entry_id: Uuid,
    amount: i64,
    transferred_at: chrono::DateTime<chrono::FixedOffset>,
) -> PostMarketplaceSellerBalanceTransferInput {
    PostMarketplaceSellerBalanceTransferInput {
        kind,
        source_id,
        seller_id,
        currency_code: "USD".to_string(),
        transferred_at,
        lines: vec![MarketplaceSellerBalanceTransferLineInput {
            reference_entry_id,
            amount,
        }],
        metadata: serde_json::json!({"test": true}),
    }
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_ledger_balance_transfer_{}?mode=memory&cache=shared",
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
        format!("ledger-balance-transfer-test-{}", Uuid::new_v4()),
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
