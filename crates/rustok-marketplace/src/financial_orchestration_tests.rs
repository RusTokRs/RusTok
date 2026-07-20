use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_marketplace_commission::{
    AssessMarketplaceOrderCommissionsInput, AssessMarketplaceOrderCommissionsResponse,
    MarketplaceCommissionAssessmentResponse, MarketplaceCommissionAssessmentStatus,
    MarketplaceCommissionCommandPort,
};
use rustok_marketplace_ledger::{
    MarketplaceLedgerCommandPort, MarketplaceLedgerTransactionResponse,
    MarketplaceLedgerTransactionStatus, PostMarketplaceOrderLedgerInput,
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
        tenant_id, order_id, assessed_at,
    )));
    let ledger = Arc::new(FakeLedgerPort::success(ledger_response(
        tenant_id, order_id, posted_at, 1_000,
    )));
    let service = MarketplaceFinancialOrchestrationService::new(
        commission.clone(),
        ledger.clone(),
    );

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
    let service = MarketplaceFinancialOrchestrationService::new(
        commission.clone(),
        ledger.clone(),
    );
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
