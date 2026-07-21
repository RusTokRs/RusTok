use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_marketplace::{
    MarketplaceFinancialOrchestrationError, MarketplaceFinancialOrchestrationService,
    ProcessMarketplaceFinancialReversalInput,
};
use rustok_marketplace_commission::{
    AssessMarketplaceOrderCommissionsInput, AssessMarketplaceOrderCommissionsResponse,
    CreateMarketplaceCommissionRuleVersionInput, MarketplaceCommissionCommandPort,
    MarketplaceCommissionRuleResponse,
};
use rustok_marketplace_ledger::{
    MarketplaceLedgerCommandPort, MarketplaceLedgerReversalKind,
    MarketplaceLedgerReversalLineInput, MarketplaceLedgerReversalResponse,
    MarketplaceLedgerTransactionResponse, MarketplaceLedgerTransactionStatus,
    MarketplaceSellerBalanceBucket, PostMarketplaceLedgerReversalInput,
    PostMarketplaceOrderLedgerInput,
};
use uuid::Uuid;

#[tokio::test]
async fn reversal_orchestration_uses_stable_child_identity_and_skips_commission_stage() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let source_id = Uuid::new_v4();
    let assessment_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    let commission = Arc::new(RejectingCommissionPort::default());
    let ledger = Arc::new(CapturingReversalLedgerPort::new(reversal_response(
        tenant_id,
        order_id,
        source_id,
        now,
        1_000,
    )));
    let service = MarketplaceFinancialOrchestrationService::new(commission.clone(), ledger.clone());
    let correlation_id = format!("reversal-orchestration-{}", Uuid::new_v4());
    let input = ProcessMarketplaceFinancialReversalInput {
        reversal: PostMarketplaceLedgerReversalInput {
            kind: MarketplaceLedgerReversalKind::Refund,
            source_id,
            order_id,
            currency_code: "USD".to_string(),
            reversed_at: now,
            lines: vec![MarketplaceLedgerReversalLineInput {
                assessment_id,
                allocation_id: Uuid::new_v4(),
                order_line_item_id: Uuid::new_v4(),
                seller_id: Uuid::new_v4(),
                commission_amount: 100,
                seller_amount: 900,
                seller_balance_bucket: MarketplaceSellerBalanceBucket::Pending,
            }],
            metadata: serde_json::json!({"normalized_by": "test"}),
        },
    };

    let response = service
        .process_reversal(
            write_context(
                tenant_id,
                actor_id,
                correlation_id.as_str(),
                "refund-event:provider:evt-1",
            ),
            input.clone(),
        )
        .await
        .unwrap();

    assert_eq!(response.order_id, order_id);
    assert_eq!(response.reversal.source_id, source_id);
    assert_eq!(commission.call_count(), 0);
    let calls = ledger.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].0.idempotency_key.as_deref(),
        Some("refund-event:provider:evt-1:ledger-reversal:v1")
    );
    assert_eq!(
        calls[0].0.causation_id.as_deref(),
        Some(correlation_id.as_str())
    );
    assert_eq!(calls[0].0.deadline_ms, Some(5_000));
    assert_eq!(calls[0].1, input.reversal);
}

#[tokio::test]
async fn mismatched_reversal_result_is_rejected_as_invariant_failure() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let source_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    let commission = Arc::new(RejectingCommissionPort::default());
    let mut response = reversal_response(tenant_id, order_id, source_id, now, 1_000);
    response.total_amount = 999;
    let service = MarketplaceFinancialOrchestrationService::new(
        commission,
        Arc::new(CapturingReversalLedgerPort::new(response)),
    );

    let result = service
        .process_reversal(
            write_context(tenant_id, actor_id, "mismatch", "mismatch-reversal"),
            ProcessMarketplaceFinancialReversalInput {
                reversal: PostMarketplaceLedgerReversalInput {
                    kind: MarketplaceLedgerReversalKind::Chargeback,
                    source_id,
                    order_id,
                    currency_code: "USD".to_string(),
                    reversed_at: now,
                    lines: vec![MarketplaceLedgerReversalLineInput {
                        assessment_id: Uuid::new_v4(),
                        allocation_id: Uuid::new_v4(),
                        order_line_item_id: Uuid::new_v4(),
                        seller_id: Uuid::new_v4(),
                        commission_amount: 100,
                        seller_amount: 900,
                        seller_balance_bucket: MarketplaceSellerBalanceBucket::Pending,
                    }],
                    metadata: serde_json::json!({}),
                },
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(MarketplaceFinancialOrchestrationError::Invariant(_))
    ));
}

fn reversal_response(
    tenant_id: Uuid,
    order_id: Uuid,
    source_id: Uuid,
    posted_at: chrono::DateTime<chrono::FixedOffset>,
    total: i64,
) -> MarketplaceLedgerReversalResponse {
    MarketplaceLedgerReversalResponse {
        id: Uuid::new_v4(),
        tenant_id,
        kind: MarketplaceLedgerReversalKind::Refund,
        source_id,
        order_id,
        currency_code: "USD".to_string(),
        total_amount: total,
        reversed_transaction_id: Uuid::new_v4(),
        reversed_at: posted_at,
        metadata: serde_json::json!({}),
        created_at: posted_at,
        transaction: MarketplaceLedgerTransactionResponse {
            id: Uuid::new_v4(),
            tenant_id,
            source_kind: "refund_reversal".to_string(),
            source_id,
            order_id,
            currency_code: "USD".to_string(),
            debit_total_amount: total,
            credit_total_amount: total,
            status: MarketplaceLedgerTransactionStatus::Posted,
            posted_at,
            metadata: serde_json::json!({}),
            created_at: posted_at,
            entries: Vec::new(),
        },
        entries: Vec::new(),
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

#[derive(Default)]
struct RejectingCommissionPort {
    calls: Mutex<usize>,
}

impl RejectingCommissionPort {
    fn call_count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

#[async_trait]
impl MarketplaceCommissionCommandPort for RejectingCommissionPort {
    async fn create_rule_version(
        &self,
        _context: PortContext,
        _request: CreateMarketplaceCommissionRuleVersionInput,
    ) -> Result<MarketplaceCommissionRuleResponse, PortError> {
        *self.calls.lock().unwrap() += 1;
        Err(PortError::validation(
            "test.unexpected_commission_call",
            "reversal orchestration must not invoke commission rule creation",
        ))
    }

    async fn assess_order(
        &self,
        _context: PortContext,
        _request: AssessMarketplaceOrderCommissionsInput,
    ) -> Result<AssessMarketplaceOrderCommissionsResponse, PortError> {
        *self.calls.lock().unwrap() += 1;
        Err(PortError::validation(
            "test.unexpected_commission_call",
            "reversal orchestration must not reassess commissions",
        ))
    }
}

struct CapturingReversalLedgerPort {
    response: MarketplaceLedgerReversalResponse,
    calls: Mutex<Vec<(PortContext, PostMarketplaceLedgerReversalInput)>>,
}

impl CapturingReversalLedgerPort {
    fn new(response: MarketplaceLedgerReversalResponse) -> Self {
        Self {
            response,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<(PortContext, PostMarketplaceLedgerReversalInput)> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl MarketplaceLedgerCommandPort for CapturingReversalLedgerPort {
    async fn post_order_commissions(
        &self,
        _context: PortContext,
        _request: PostMarketplaceOrderLedgerInput,
    ) -> Result<MarketplaceLedgerTransactionResponse, PortError> {
        Err(PortError::validation(
            "test.unexpected_order_posting",
            "reversal orchestration must not post order commissions",
        ))
    }

    async fn post_financial_reversal(
        &self,
        context: PortContext,
        request: PostMarketplaceLedgerReversalInput,
    ) -> Result<MarketplaceLedgerReversalResponse, PortError> {
        self.calls.lock().unwrap().push((context, request));
        Ok(self.response.clone())
    }
}
