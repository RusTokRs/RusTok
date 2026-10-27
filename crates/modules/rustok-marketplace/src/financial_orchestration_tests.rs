use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_marketplace_balance::{
    MarketplaceLedgerCommandPort, MarketplaceLedgerTransactionResponse,
    MarketplaceLedgerTransactionStatus, PostMarketplaceOrderLedgerInput,
};
use rustok_marketplace_commission::{
    AssessMarketplaceOrderCommissionsInput, AssessMarketplaceOrderCommissionsResponse,
    MarketplaceCommissionAssessmentResponse, MarketplaceCommissionAssessmentStatus,
    MarketplaceCommissionCommandPort,
};
use uuid::Uuid;

use crate::{
    MarketplaceFinancialOrchestrationError, MarketplaceFinancialOrchestrationService,
    ProcessMarketplaceOrderFinancialsInput,
};

#[tokio::test]
async fn financial_orchestration_uses_stable_child_keys_and_preserves_causation() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let correlation_id = format!("financial-test-{}", Uuid::new_v4());
    let assessed_at = Utc::now().fixed_offset();
    let posted_at = assessed_at;
    let commission = Arc::new(FakeCommissionPort::new(commission_response(
        tenant_id,
        order_id,
        assessed_at,
    )));
    let ledger = Arc::new(FakeLedgerPort::success(ledger_response(
        tenant_id, order_id, posted_at, 1_000,
    )));
    let service = MarketplaceFinancialOrchestrationService::new(commission.clone(), ledger.clone());

    let response = service
        .process_order(
            write_context(
                tenant_id,
                actor_id,
                correlation_id.as_str(),
                "order-financials",
            ),
            ProcessMarketplaceOrderFinancialsInput {
                order_id,
                assessed_at,
                posted_at,
            },
        )
        .await
        .unwrap();

    assert_eq!(response.order_id, order_id);
    let commission_calls = commission.calls();
    let ledger_calls = ledger.calls();
    assert_eq!(commission_calls.len(), 1);
    assert_eq!(ledger_calls.len(), 1);
    assert_eq!(
        commission_calls[0].idempotency_key.as_deref(),
        Some("order-financials:commission:v1")
    );
    assert_eq!(
        ledger_calls[0].idempotency_key.as_deref(),
        Some("order-financials:ledger:v1")
    );
    assert_eq!(
        commission_calls[0].causation_id.as_deref(),
        Some(correlation_id.as_str())
    );
    assert_eq!(
        ledger_calls[0].causation_id.as_deref(),
        Some(correlation_id.as_str())
    );
    assert_eq!(commission_calls[0].deadline_ms, Some(5_000));
    assert_eq!(ledger_calls[0].deadline_ms, Some(5_000));
}

#[tokio::test]
async fn retry_after_ledger_unavailable_reuses_both_child_keys() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    let commission = Arc::new(FakeCommissionPort::new(commission_response(
        tenant_id, order_id, now,
    )));
    let ledger = Arc::new(FakeLedgerPort::fail_once(ledger_response(
        tenant_id, order_id, now, 1_000,
    )));
    let service = MarketplaceFinancialOrchestrationService::new(commission.clone(), ledger.clone());
    let input = ProcessMarketplaceOrderFinancialsInput {
        order_id,
        assessed_at: now,
        posted_at: now,
    };

    let first = service
        .process_order(
            write_context(tenant_id, actor_id, "financial-retry", "retry-order"),
            input.clone(),
        )
        .await;
    assert!(matches!(
        first,
        Err(MarketplaceFinancialOrchestrationError::Ledger {
            retryable: true,
            ..
        })
    ));

    let second = service
        .process_order(
            write_context(tenant_id, actor_id, "financial-retry", "retry-order"),
            input,
        )
        .await
        .unwrap();
    assert_eq!(second.order_id, order_id);

    let commission_keys = commission
        .calls()
        .into_iter()
        .map(|context| context.idempotency_key.unwrap())
        .collect::<Vec<_>>();
    let ledger_keys = ledger
        .calls()
        .into_iter()
        .map(|context| context.idempotency_key.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        commission_keys,
        vec![
            "retry-order:commission:v1".to_string(),
            "retry-order:commission:v1".to_string(),
        ]
    );
    assert_eq!(
        ledger_keys,
        vec![
            "retry-order:ledger:v1".to_string(),
            "retry-order:ledger:v1".to_string(),
        ]
    );
}

#[tokio::test]
async fn mismatched_ledger_total_is_rejected_as_invariant_failure() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    let commission = Arc::new(FakeCommissionPort::new(commission_response(
        tenant_id, order_id, now,
    )));
    let ledger = Arc::new(FakeLedgerPort::success(ledger_response(
        tenant_id, order_id, now, 999,
    )));
    let service = MarketplaceFinancialOrchestrationService::new(commission, ledger);

    let result = service
        .process_order(
            write_context(tenant_id, actor_id, "financial-mismatch", "mismatch-order"),
            ProcessMarketplaceOrderFinancialsInput {
                order_id,
                assessed_at: now,
                posted_at: now,
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(MarketplaceFinancialOrchestrationError::Invariant(_))
    ));
}

#[tokio::test]
async fn financial_orchestration_rejects_nil_tenant_or_order_id() {
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    let commission = Arc::new(FakeCommissionPort::new(commission_response(
        Uuid::new_v4(),
        order_id,
        now,
    )));
    let ledger = Arc::new(FakeLedgerPort::success(ledger_response(
        Uuid::new_v4(),
        order_id,
        now,
        1_000,
    )));
    let service = MarketplaceFinancialOrchestrationService::new(commission, ledger);

    // Nil tenant_id rejected
    let nil_tenant_ctx = PortContext::new(
        Uuid::nil().to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        "nil-tenant",
    )
    .with_deadline(std::time::Duration::from_secs(5))
    .with_idempotency_key("nil-tenant-key");

    let err = service
        .process_order(
            nil_tenant_ctx,
            ProcessMarketplaceOrderFinancialsInput {
                order_id,
                assessed_at: now,
                posted_at: now,
            },
        )
        .await;
    assert!(matches!(
        err,
        Err(MarketplaceFinancialOrchestrationError::Validation(msg)) if msg.contains("tenant_id")
    ));

    // Nil order_id rejected
    let valid_ctx = write_context(Uuid::new_v4(), actor_id, "nil-order", "nil-order-key");
    let err = service
        .process_order(
            valid_ctx,
            ProcessMarketplaceOrderFinancialsInput {
                order_id: Uuid::nil(),
                assessed_at: now,
                posted_at: now,
            },
        )
        .await;
    assert!(matches!(
        err,
        Err(MarketplaceFinancialOrchestrationError::Validation(msg)) if msg.contains("order_id")
    ));
}

#[tokio::test]
async fn directory_services_reject_nil_identifiers_and_invalid_currency() {
    let ctx = write_context(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "directory-test",
        "directory-key",
    );

    // Allocation directory
    let allocation_dir =
        crate::MarketplaceAllocationDirectoryService::new(Arc::new(DummyAllocationReader));
    assert!(allocation_dir
        .read_by_order_line(ctx.clone(), Uuid::nil())
        .await
        .is_err());
    assert!(allocation_dir
        .list_by_order(ctx.clone(), Uuid::nil())
        .await
        .is_err());
    assert!(allocation_dir
        .list_by_seller(
            ctx.clone(),
            rustok_marketplace_allocation::ListMarketplaceAllocationsBySellerRequest {
                seller_id: Uuid::nil(),
                page: 1,
                per_page: 10,
                status: None,
            }
        )
        .await
        .is_err());

    // Commission directory
    let commission_dir =
        crate::MarketplaceCommissionDirectoryService::new(Arc::new(DummyCommissionReader));
    assert!(commission_dir
        .read_by_allocation(ctx.clone(), Uuid::nil())
        .await
        .is_err());
    assert!(commission_dir
        .list_by_order(ctx.clone(), Uuid::nil())
        .await
        .is_err());
    assert!(commission_dir
        .list_by_seller(
            ctx.clone(),
            rustok_marketplace_commission::ListMarketplaceCommissionAssessmentsBySellerRequest {
                seller_id: Uuid::nil(),
                page: 1,
                per_page: 10,
                status: None,
            }
        )
        .await
        .is_err());

    // Ledger directory
    let ledger_dir = crate::MarketplaceLedgerDirectoryService::new(Arc::new(DummyLedgerReader));
    assert!(ledger_dir
        .read_by_order(ctx.clone(), Uuid::nil())
        .await
        .is_err());
    assert!(ledger_dir
        .list_seller_entries(
            ctx.clone(),
            rustok_marketplace_balance::ListMarketplaceSellerLedgerEntriesRequest {
                seller_id: Uuid::nil(),
                currency_code: None,
                page: 1,
                per_page: 10,
            }
        )
        .await
        .is_err());
    assert!(ledger_dir
        .read_seller_balance(ctx.clone(), Uuid::nil(), "USD")
        .await
        .is_err());
    assert!(ledger_dir
        .read_seller_balance(ctx.clone(), Uuid::new_v4(), "INVALID")
        .await
        .is_err());
    let normalized = ledger_dir
        .read_seller_balance(ctx.clone(), Uuid::new_v4(), " usd ")
        .await
        .unwrap();
    assert_eq!(normalized.currency_code, "USD");

    // Listing directory
    let listing_dir = crate::MarketplaceListingDirectoryService::new(Arc::new(DummyListingReader));
    assert!(listing_dir
        .read_listing(ctx.clone(), Uuid::nil())
        .await
        .is_err());

    // Payout directory
    let payout_dir = crate::MarketplacePayoutDirectoryService::new(Arc::new(DummyPayoutReader));
    assert!(payout_dir
        .read_payout(ctx.clone(), Uuid::nil())
        .await
        .is_err());
    assert!(payout_dir
        .list_seller_payouts(
            ctx.clone(),
            rustok_marketplace_payout::ListMarketplaceSellerPayoutsRequest {
                seller_id: Uuid::nil(),
                currency_code: None,
                status: None,
                page: 1,
                per_page: 10,
            }
        )
        .await
        .is_err());

    // Seller directory
    let seller_dir = crate::MarketplaceSellerDirectoryService::new(Arc::new(DummySellerReader));
    assert!(seller_dir
        .read_seller(ctx.clone(), Uuid::nil())
        .await
        .is_err());
}

struct DummyAllocationReader;
#[async_trait]
impl rustok_marketplace_allocation::MarketplaceAllocationReadPort for DummyAllocationReader {
    async fn read_allocation_by_line(
        &self,
        _: PortContext,
        _: rustok_marketplace_allocation::ReadMarketplaceAllocationByLineRequest,
    ) -> Result<rustok_marketplace_allocation::MarketplaceOrderAllocationResponse, PortError> {
        unreachable!()
    }
    async fn list_allocations_by_order(
        &self,
        _: PortContext,
        _: rustok_marketplace_allocation::ListMarketplaceAllocationsByOrderRequest,
    ) -> Result<Vec<rustok_marketplace_allocation::MarketplaceOrderAllocationResponse>, PortError> {
        unreachable!()
    }
    async fn list_allocations_by_seller(
        &self,
        _: PortContext,
        _: rustok_marketplace_allocation::ListMarketplaceAllocationsBySellerRequest,
    ) -> Result<rustok_marketplace_allocation::MarketplaceAllocationListResponse, PortError> {
        unreachable!()
    }
}

struct DummyCommissionReader;
#[async_trait]
impl rustok_marketplace_commission::MarketplaceCommissionReadPort for DummyCommissionReader {
    async fn read_assessment(
        &self,
        _: PortContext,
        _: rustok_marketplace_commission::ReadMarketplaceCommissionAssessmentRequest,
    ) -> Result<rustok_marketplace_commission::MarketplaceCommissionAssessmentResponse, PortError> {
        unreachable!()
    }
    async fn list_assessments_by_order(
        &self,
        _: PortContext,
        _: rustok_marketplace_commission::ListMarketplaceCommissionAssessmentsByOrderRequest,
    ) -> Result<Vec<rustok_marketplace_commission::MarketplaceCommissionAssessmentResponse>, PortError> {
        unreachable!()
    }
    async fn list_assessments_by_seller(
        &self,
        _: PortContext,
        _: rustok_marketplace_commission::ListMarketplaceCommissionAssessmentsBySellerRequest,
    ) -> Result<rustok_marketplace_commission::MarketplaceCommissionAssessmentListResponse, PortError> {
        unreachable!()
    }
    async fn list_rules(
        &self,
        _: PortContext,
        _: rustok_marketplace_commission::ListMarketplaceCommissionRulesRequest,
    ) -> Result<rustok_marketplace_commission::MarketplaceCommissionRuleListResponse, PortError> {
        unreachable!()
    }
}

struct DummyLedgerReader;
#[async_trait]
impl rustok_marketplace_balance::MarketplaceLedgerReadPort for DummyLedgerReader {
    async fn read_order_ledger(
        &self,
        _: PortContext,
        _: rustok_marketplace_balance::ReadMarketplaceOrderLedgerRequest,
    ) -> Result<rustok_marketplace_balance::MarketplaceLedgerTransactionResponse, PortError> {
        unreachable!()
    }
    async fn list_seller_entries(
        &self,
        _: PortContext,
        _: rustok_marketplace_balance::ListMarketplaceSellerLedgerEntriesRequest,
    ) -> Result<rustok_marketplace_balance::MarketplaceLedgerEntryListResponse, PortError> {
        unreachable!()
    }
    async fn read_seller_balance(
        &self,
        _: PortContext,
        req: rustok_marketplace_balance::ReadMarketplaceSellerBalanceRequest,
    ) -> Result<rustok_marketplace_balance::MarketplaceSellerBalanceResponse, PortError> {
        let now = chrono::Utc::now().fixed_offset();
        Ok(rustok_marketplace_balance::MarketplaceSellerBalanceResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            seller_id: req.seller_id,
            currency_code: req.currency_code,
            pending_amount: 0,
            available_amount: 0,
            reserved_amount: 0,
            paid_amount: 0,
            negative_amount: 0,
            source_entry_count: 0,
            last_entry_id: None,
            last_entry_created_at: None,
            rebuilt_at: now,
            updated_at: now,
        })
    }
}

struct DummyListingReader;
#[async_trait]
impl rustok_marketplace_listing::MarketplaceListingReadPort for DummyListingReader {
    async fn read_listing(
        &self,
        _: PortContext,
        _: rustok_marketplace_listing::ReadMarketplaceListingRequest,
    ) -> Result<rustok_marketplace_listing::MarketplaceListingResponse, PortError> {
        unreachable!()
    }
    async fn list_listings(
        &self,
        _: PortContext,
        _: rustok_marketplace_listing::ListMarketplaceListingsInput,
    ) -> Result<rustok_marketplace_listing::MarketplaceListingListResponse, PortError> {
        unreachable!()
    }
    async fn list_eligibility(
        &self,
        _: PortContext,
        _: rustok_marketplace_listing::MarketplaceListingEligibilityRequest,
    ) -> Result<Vec<rustok_marketplace_listing::MarketplaceListingEligibilityProjection>, PortError> {
        unreachable!()
    }
    async fn list_listing_events(
        &self,
        _: PortContext,
        _: rustok_marketplace_listing::ListMarketplaceListingEventsRequest,
    ) -> Result<Vec<rustok_marketplace_listing::MarketplaceListingEventResponse>, PortError> {
        unreachable!()
    }
}

struct DummyPayoutReader;
#[async_trait]
impl rustok_marketplace_payout::MarketplacePayoutReadPort for DummyPayoutReader {
    async fn read_payout(
        &self,
        _: PortContext,
        _: rustok_marketplace_payout::ReadMarketplacePayoutRequest,
    ) -> Result<rustok_marketplace_payout::MarketplacePayoutResponse, PortError> {
        unreachable!()
    }
    async fn list_seller_payouts(
        &self,
        _: PortContext,
        _: rustok_marketplace_payout::ListMarketplaceSellerPayoutsRequest,
    ) -> Result<rustok_marketplace_payout::MarketplacePayoutListResponse, PortError> {
        unreachable!()
    }
}

struct DummySellerReader;
#[async_trait]
impl rustok_marketplace_seller::MarketplaceSellerReadPort for DummySellerReader {
    async fn read_seller(
        &self,
        _: PortContext,
        _: rustok_marketplace_seller::ReadMarketplaceSellerRequest,
    ) -> Result<rustok_marketplace_seller::MarketplaceSellerResponse, PortError> {
        unreachable!()
    }
    async fn list_sellers(
        &self,
        _: PortContext,
        _: rustok_marketplace_seller::ListMarketplaceSellersInput,
    ) -> Result<rustok_marketplace_seller::MarketplaceSellerListResponse, PortError> {
        unreachable!()
    }
    async fn read_membership(
        &self,
        _: PortContext,
        _: rustok_marketplace_seller::ReadMarketplaceSellerMembershipRequest,
    ) -> Result<rustok_marketplace_seller::MarketplaceSellerMemberResponse, PortError> {
        unreachable!()
    }
    async fn list_members(
        &self,
        _: PortContext,
        _: rustok_marketplace_seller::ListMarketplaceSellerMembersRequest,
    ) -> Result<Vec<rustok_marketplace_seller::MarketplaceSellerMemberResponse>, PortError> {
        unreachable!()
    }
    async fn list_seller_events(
        &self,
        _: PortContext,
        _: rustok_marketplace_seller::ListMarketplaceSellerEventsRequest,
    ) -> Result<Vec<rustok_marketplace_seller::MarketplaceSellerEventResponse>, PortError> {
        unreachable!()
    }
}

fn write_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    correlation_id: &str,
    idempotency_key: &str,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        correlation_id,
    )
    .with_deadline(std::time::Duration::from_secs(5))
    .with_idempotency_key(idempotency_key)
}

fn commission_response(
    tenant_id: Uuid,
    order_id: Uuid,
    assessed_at: chrono::DateTime<chrono::FixedOffset>,
) -> AssessMarketplaceOrderCommissionsResponse {
    AssessMarketplaceOrderCommissionsResponse {
        order_id,
        assessments: vec![MarketplaceCommissionAssessmentResponse {
            id: Uuid::new_v4(),
            tenant_id,
            allocation_id: Uuid::new_v4(),
            order_id,
            order_line_item_id: Uuid::new_v4(),
            seller_id: Uuid::new_v4(),
            listing_id: Uuid::new_v4(),
            rule_id: Uuid::new_v4(),
            rule_key: Uuid::new_v4(),
            rule_version: 1,
            currency_code: "USD".to_string(),
            allocation_total_amount: 1_000,
            rate_bps: 1_000,
            fixed_amount: 0,
            commission_amount: 100,
            seller_proceeds_amount: 900,
            status: MarketplaceCommissionAssessmentStatus::Assessed,
            metadata: serde_json::json!({}),
            assessed_at,
            created_at: assessed_at,
        }],
        commission_total_amount: 100,
        seller_proceeds_total_amount: 900,
    }
}

fn ledger_response(
    tenant_id: Uuid,
    order_id: Uuid,
    posted_at: chrono::DateTime<chrono::FixedOffset>,
    total: i64,
) -> MarketplaceLedgerTransactionResponse {
    MarketplaceLedgerTransactionResponse {
        id: Uuid::new_v4(),
        tenant_id,
        source_kind: "commission_assessment_batch".to_string(),
        source_id: order_id,
        order_id,
        currency_code: "USD".to_string(),
        debit_total_amount: total,
        credit_total_amount: total,
        status: MarketplaceLedgerTransactionStatus::Posted,
        posted_at,
        metadata: serde_json::json!({}),
        created_at: posted_at,
        entries: Vec::new(),
    }
}

struct FakeCommissionPort {
    calls: Mutex<Vec<PortContext>>,
    response: AssessMarketplaceOrderCommissionsResponse,
}

impl FakeCommissionPort {
    fn new(response: AssessMarketplaceOrderCommissionsResponse) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            response,
        }
    }

    fn calls(&self) -> Vec<PortContext> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl MarketplaceCommissionCommandPort for FakeCommissionPort {
    async fn create_rule_version(
        &self,
        _context: PortContext,
        _request: rustok_marketplace_commission::CreateMarketplaceCommissionRuleVersionInput,
    ) -> Result<rustok_marketplace_commission::MarketplaceCommissionRuleResponse, PortError> {
        Err(PortError::validation(
            "test.unsupported",
            "rule creation is not used by orchestration fixtures",
        ))
    }

    async fn assess_order(
        &self,
        context: PortContext,
        _request: AssessMarketplaceOrderCommissionsInput,
    ) -> Result<AssessMarketplaceOrderCommissionsResponse, PortError> {
        self.calls.lock().unwrap().push(context);
        Ok(self.response.clone())
    }
}

struct FakeLedgerPort {
    calls: Mutex<Vec<PortContext>>,
    response: MarketplaceLedgerTransactionResponse,
    fail_remaining: Mutex<usize>,
}

impl FakeLedgerPort {
    fn success(response: MarketplaceLedgerTransactionResponse) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            response,
            fail_remaining: Mutex::new(0),
        }
    }

    fn fail_once(response: MarketplaceLedgerTransactionResponse) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            response,
            fail_remaining: Mutex::new(1),
        }
    }

    fn calls(&self) -> Vec<PortContext> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl MarketplaceLedgerCommandPort for FakeLedgerPort {
    async fn post_order_commissions(
        &self,
        context: PortContext,
        _request: PostMarketplaceOrderLedgerInput,
    ) -> Result<MarketplaceLedgerTransactionResponse, PortError> {
        self.calls.lock().unwrap().push(context);
        let mut remaining = self.fail_remaining.lock().unwrap();
        if *remaining > 0 {
            *remaining -= 1;
            return Err(PortError::new(
                PortErrorKind::Unavailable,
                "test.ledger_unavailable",
                "ledger temporarily unavailable",
                true,
            ));
        }
        Ok(self.response.clone())
    }
}
