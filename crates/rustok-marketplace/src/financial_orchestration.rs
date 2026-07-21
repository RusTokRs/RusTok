use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_marketplace_commission::{
    AssessMarketplaceOrderCommissionsInput, AssessMarketplaceOrderCommissionsResponse,
    MarketplaceCommissionCommandPort,
};
use rustok_marketplace_ledger::{
    MarketplaceLedgerCommandPort, MarketplaceLedgerReversalResponse,
    MarketplaceLedgerTransactionResponse, PostMarketplaceLedgerReversalInput,
    PostMarketplaceOrderLedgerInput,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const MAX_CHILD_IDEMPOTENCY_KEY_BYTES: usize = 191;
const COMMISSION_STAGE_SUFFIX: &str = ":commission:v1";
const LEDGER_STAGE_SUFFIX: &str = ":ledger:v1";
const LEDGER_REVERSAL_STAGE_SUFFIX: &str = ":ledger-reversal:v1";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProcessMarketplaceOrderFinancialsInput {
    pub order_id: Uuid,
    pub assessed_at: DateTime<FixedOffset>,
    pub posted_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProcessMarketplaceOrderFinancialsResponse {
    pub order_id: Uuid,
    pub commission: AssessMarketplaceOrderCommissionsResponse,
    pub ledger: MarketplaceLedgerTransactionResponse,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProcessMarketplaceFinancialReversalInput {
    pub reversal: PostMarketplaceLedgerReversalInput,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProcessMarketplaceFinancialReversalResponse {
    pub order_id: Uuid,
    pub reversal: MarketplaceLedgerReversalResponse,
}

#[derive(Debug, Error)]
pub enum MarketplaceFinancialOrchestrationError {
    #[error("marketplace financial context `{code}` failed: {message}")]
    Context {
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("marketplace commission stage `{code}` failed: {message}")]
    Commission {
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("marketplace ledger stage `{code}` failed: {message}")]
    Ledger {
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("marketplace financial orchestration validation failed: {0}")]
    Validation(String),
    #[error("marketplace financial orchestration invariant failed: {0}")]
    Invariant(String),
}

impl MarketplaceFinancialOrchestrationError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::Context { retryable, .. }
            | Self::Commission { retryable, .. }
            | Self::Ledger { retryable, .. } => *retryable,
            Self::Validation(_) | Self::Invariant(_) => false,
        }
    }
}

pub type MarketplaceFinancialOrchestrationResult<T> =
    Result<T, MarketplaceFinancialOrchestrationError>;

#[async_trait]
pub trait MarketplaceFinancialCommandPort: Send + Sync {
    async fn process_order_financials(
        &self,
        context: PortContext,
        request: ProcessMarketplaceOrderFinancialsInput,
    ) -> MarketplaceFinancialOrchestrationResult<ProcessMarketplaceOrderFinancialsResponse>;

    async fn process_financial_reversal(
        &self,
        _context: PortContext,
        _request: ProcessMarketplaceFinancialReversalInput,
    ) -> MarketplaceFinancialOrchestrationResult<ProcessMarketplaceFinancialReversalResponse> {
        Err(MarketplaceFinancialOrchestrationError::Validation(
            "marketplace financial provider does not support append-only reversals".to_string(),
        ))
    }
}

pub struct MarketplaceFinancialOrchestrationService {
    commission_port: Arc<dyn MarketplaceCommissionCommandPort>,
    ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
}

impl MarketplaceFinancialOrchestrationService {
    pub fn new(
        commission_port: Arc<dyn MarketplaceCommissionCommandPort>,
        ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
    ) -> Self {
        Self {
            commission_port,
            ledger_port,
        }
    }

    pub async fn process_order(
        &self,
        context: PortContext,
        input: ProcessMarketplaceOrderFinancialsInput,
    ) -> MarketplaceFinancialOrchestrationResult<ProcessMarketplaceOrderFinancialsResponse> {
        context
            .require_policy(PortCallPolicy::write())
            .map_err(map_context_error)?;
        validate_input(&input)?;
        let root_key = root_idempotency_key(&context)?;
        let commission_key = child_key(root_key, COMMISSION_STAGE_SUFFIX)?;
        let ledger_key = child_key(root_key, LEDGER_STAGE_SUFFIX)?;

        let commission = self
            .commission_port
            .assess_order(
                child_context(&context, commission_key),
                AssessMarketplaceOrderCommissionsInput {
                    order_id: input.order_id,
                    assessed_at: input.assessed_at,
                },
            )
            .await
            .map_err(map_commission_error)?;
        validate_commission_result(input.order_id, &commission)?;

        let ledger = self
            .ledger_port
            .post_order_commissions(
                child_context(&context, ledger_key),
                PostMarketplaceOrderLedgerInput {
                    order_id: input.order_id,
                    posted_at: input.posted_at,
                },
            )
            .await
            .map_err(map_ledger_error)?;
        validate_ledger_result(input.order_id, &commission, &ledger)?;

        Ok(ProcessMarketplaceOrderFinancialsResponse {
            order_id: input.order_id,
            commission,
            ledger,
        })
    }

    pub async fn process_reversal(
        &self,
        context: PortContext,
        input: ProcessMarketplaceFinancialReversalInput,
    ) -> MarketplaceFinancialOrchestrationResult<ProcessMarketplaceFinancialReversalResponse> {
        context
            .require_policy(PortCallPolicy::write())
            .map_err(map_context_error)?;
        validate_reversal_input(&input.reversal)?;
        let ledger_key = child_key(root_idempotency_key(&context)?, LEDGER_REVERSAL_STAGE_SUFFIX)?;
        let expected = input.reversal.clone();
        let reversal = self
            .ledger_port
            .post_financial_reversal(child_context(&context, ledger_key), input.reversal)
            .await
            .map_err(map_ledger_error)?;
        validate_reversal_result(&expected, &reversal)?;
        Ok(ProcessMarketplaceFinancialReversalResponse {
            order_id: expected.order_id,
            reversal,
        })
    }
}

#[async_trait]
impl MarketplaceFinancialCommandPort for MarketplaceFinancialOrchestrationService {
    async fn process_order_financials(
        &self,
        context: PortContext,
        request: ProcessMarketplaceOrderFinancialsInput,
    ) -> MarketplaceFinancialOrchestrationResult<ProcessMarketplaceOrderFinancialsResponse> {
        self.process_order(context, request).await
    }

    async fn process_financial_reversal(
        &self,
        context: PortContext,
        request: ProcessMarketplaceFinancialReversalInput,
    ) -> MarketplaceFinancialOrchestrationResult<ProcessMarketplaceFinancialReversalResponse> {
        self.process_reversal(context, request).await
    }
}

fn root_idempotency_key(
    context: &PortContext,
) -> MarketplaceFinancialOrchestrationResult<&str> {
    context
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            MarketplaceFinancialOrchestrationError::Validation(
                "root idempotency key is required".to_string(),
            )
        })
}

fn validate_input(
    input: &ProcessMarketplaceOrderFinancialsInput,
) -> MarketplaceFinancialOrchestrationResult<()> {
    if input.order_id.is_nil() {
        return Err(MarketplaceFinancialOrchestrationError::Validation(
            "order_id must not be nil".to_string(),
        ));
    }
    if input.posted_at < input.assessed_at {
        return Err(MarketplaceFinancialOrchestrationError::Validation(
            "posted_at must not be earlier than assessed_at".to_string(),
        ));
    }
    Ok(())
}

fn validate_reversal_input(
    input: &PostMarketplaceLedgerReversalInput,
) -> MarketplaceFinancialOrchestrationResult<()> {
    if input.source_id.is_nil() || input.order_id.is_nil() {
        return Err(MarketplaceFinancialOrchestrationError::Validation(
            "reversal source_id and order_id must not be nil".to_string(),
        ));
    }
    let currency = input.currency_code.trim();
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(MarketplaceFinancialOrchestrationError::Validation(
            "reversal currency_code must contain exactly three ASCII letters".to_string(),
        ));
    }
    if input.lines.is_empty() {
        return Err(MarketplaceFinancialOrchestrationError::Validation(
            "reversal requires at least one normalized line".to_string(),
        ));
    }
    let total = input.lines.iter().try_fold(0_i64, |total, line| {
        if line.commission_amount < 0 || line.seller_amount < 0 {
            return Err(MarketplaceFinancialOrchestrationError::Validation(
                "reversal line amounts must not be negative".to_string(),
            ));
        }
        total
            .checked_add(line.commission_amount)
            .and_then(|value| value.checked_add(line.seller_amount))
            .ok_or_else(|| {
                MarketplaceFinancialOrchestrationError::Validation(
                    "reversal total overflow".to_string(),
                )
            })
    })?;
    if total <= 0 {
        return Err(MarketplaceFinancialOrchestrationError::Validation(
            "reversal total must be positive".to_string(),
        ));
    }
    Ok(())
}

fn child_key(
    root_key: &str,
    suffix: &str,
) -> MarketplaceFinancialOrchestrationResult<String> {
    let key = format!("{root_key}{suffix}");
    if key.len() > MAX_CHILD_IDEMPOTENCY_KEY_BYTES {
        return Err(MarketplaceFinancialOrchestrationError::Validation(format!(
            "root idempotency key is too long for child stage `{suffix}`"
        )));
    }
    Ok(key)
}

fn child_context(context: &PortContext, idempotency_key: String) -> PortContext {
    let mut child = context.clone().with_idempotency_key(idempotency_key);
    child.causation_id = Some(context.correlation_id.clone());
    child
}

fn validate_commission_result(
    order_id: Uuid,
    commission: &AssessMarketplaceOrderCommissionsResponse,
) -> MarketplaceFinancialOrchestrationResult<()> {
    if commission.order_id != order_id {
        return Err(MarketplaceFinancialOrchestrationError::Invariant(format!(
            "commission result order {} does not match requested order {order_id}",
            commission.order_id
        )));
    }
    if commission.assessments.is_empty() {
        return Err(MarketplaceFinancialOrchestrationError::Invariant(
            "commission stage returned no assessments".to_string(),
        ));
    }
    let expected_total = commission
        .commission_total_amount
        .checked_add(commission.seller_proceeds_total_amount)
        .ok_or_else(|| {
            MarketplaceFinancialOrchestrationError::Invariant(
                "commission stage totals overflow".to_string(),
            )
        })?;
    let assessment_total = commission.assessments.iter().try_fold(
        0_i64,
        |total, assessment| {
            total
                .checked_add(assessment.allocation_total_amount)
                .ok_or_else(|| {
                    MarketplaceFinancialOrchestrationError::Invariant(
                        "commission assessment total overflow".to_string(),
                    )
                })
        },
    )?;
    if expected_total != assessment_total {
        return Err(MarketplaceFinancialOrchestrationError::Invariant(
            "commission aggregate totals do not match assessment allocations".to_string(),
        ));
    }
    Ok(())
}

fn validate_ledger_result(
    order_id: Uuid,
    commission: &AssessMarketplaceOrderCommissionsResponse,
    ledger: &MarketplaceLedgerTransactionResponse,
) -> MarketplaceFinancialOrchestrationResult<()> {
    if ledger.order_id != order_id || ledger.source_id != order_id {
        return Err(MarketplaceFinancialOrchestrationError::Invariant(format!(
            "ledger result does not belong to requested order {order_id}"
        )));
    }
    if ledger.debit_total_amount != ledger.credit_total_amount {
        return Err(MarketplaceFinancialOrchestrationError::Invariant(
            "ledger transaction is not balanced".to_string(),
        ));
    }
    let expected_total = commission
        .commission_total_amount
        .checked_add(commission.seller_proceeds_total_amount)
        .ok_or_else(|| {
            MarketplaceFinancialOrchestrationError::Invariant(
                "financial total overflow".to_string(),
            )
        })?;
    if ledger.debit_total_amount != expected_total {
        return Err(MarketplaceFinancialOrchestrationError::Invariant(format!(
            "ledger total {} does not match commission-derived total {expected_total}",
            ledger.debit_total_amount
        )));
    }
    Ok(())
}

fn validate_reversal_result(
    input: &PostMarketplaceLedgerReversalInput,
    reversal: &MarketplaceLedgerReversalResponse,
) -> MarketplaceFinancialOrchestrationResult<()> {
    if reversal.kind != input.kind
        || reversal.source_id != input.source_id
        || reversal.order_id != input.order_id
        || !reversal
            .currency_code
            .eq_ignore_ascii_case(input.currency_code.as_str())
    {
        return Err(MarketplaceFinancialOrchestrationError::Invariant(
            "ledger reversal identity does not match normalized request".to_string(),
        ));
    }
    if reversal.transaction.source_kind != input.kind.source_kind()
        || reversal.transaction.source_id != input.source_id
        || reversal.transaction.order_id != input.order_id
        || reversal.transaction_id != reversal.transaction.id
        || reversal.transaction.id.is_nil()
        || reversal.reversed_transaction_id == reversal.transaction.id
    {
        return Err(MarketplaceFinancialOrchestrationError::Invariant(
            "ledger reversal transaction identity is invalid".to_string(),
        ));
    }
    if reversal.transaction.debit_total_amount != reversal.transaction.credit_total_amount
        || reversal.transaction.debit_total_amount != reversal.total_amount
    {
        return Err(MarketplaceFinancialOrchestrationError::Invariant(
            "ledger reversal transaction is not balanced".to_string(),
        ));
    }
    let expected_total = input.lines.iter().try_fold(0_i64, |total, line| {
        total
            .checked_add(line.commission_amount)
            .and_then(|value| value.checked_add(line.seller_amount))
            .ok_or_else(|| {
                MarketplaceFinancialOrchestrationError::Invariant(
                    "normalized reversal total overflow".to_string(),
                )
            })
    })?;
    if expected_total != reversal.total_amount {
        return Err(MarketplaceFinancialOrchestrationError::Invariant(format!(
            "ledger reversal total {} does not match normalized total {expected_total}",
            reversal.total_amount
        )));
    }
    Ok(())
}

fn map_context_error(error: PortError) -> MarketplaceFinancialOrchestrationError {
    MarketplaceFinancialOrchestrationError::Context {
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

fn map_commission_error(error: PortError) -> MarketplaceFinancialOrchestrationError {
    MarketplaceFinancialOrchestrationError::Commission {
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

fn map_ledger_error(error: PortError) -> MarketplaceFinancialOrchestrationError {
    MarketplaceFinancialOrchestrationError::Ledger {
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}
