use std::{sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_marketplace_ledger::{
    MarketplaceLedgerCommandPort, MarketplaceLedgerTransactionResponse,
    MarketplaceLedgerTransactionStatus, PostMarketplaceOrderLedgerInput,
};
use sea_orm::DatabaseConnection;
use thiserror::Error;
use uuid::Uuid;

use super::checkout_marketplace_financial_legacy::{
    BeginMarketplaceFinancialOperation, MarketplaceFinancialOperationError,
    MarketplaceFinancialOperationJournal, MarketplaceFinancialOperationStatus,
};
use super::{
    CheckoutMarketplaceEconomicsCheckpointError,
    CheckoutMarketplaceEconomicsCheckpointJournal, CheckoutPaymentCapturedState,
    validate_marketplace_economics_checkpoint,
};

const LEDGER_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum CheckoutMarketplaceFinancialError {
    #[error("marketplace post-capture financial validation failed: {0}")]
    Validation(String),
    #[error("marketplace post-capture financial invariant failed: {0}")]
    Invariant(String),
    #[error("marketplace post-capture financial operation is busy: {0}")]
    Busy(String),
    #[error("marketplace ledger boundary `{code}` failed: {message}")]
    Boundary {
        code: String,
        message: String,
        retryable: bool,
    },
    #[error(transparent)]
    Journal(#[from] MarketplaceFinancialOperationError),
    #[error(transparent)]
    EconomicsCheckpoint(#[from] CheckoutMarketplaceEconomicsCheckpointError),
}

impl CheckoutMarketplaceFinancialError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::Busy(_) => true,
            Self::Boundary { retryable, .. } => *retryable,
            Self::Journal(MarketplaceFinancialOperationError::Database(_))
            | Self::EconomicsCheckpoint(
                CheckoutMarketplaceEconomicsCheckpointError::Database(_),
            ) => true,
            Self::Journal(_)
            | Self::EconomicsCheckpoint(_)
            | Self::Validation(_)
            | Self::Invariant(_) => false,
        }
    }

    pub fn code(&self) -> String {
        match self {
            Self::Boundary { code, .. } => code.clone(),
            Self::Busy(_) => "marketplace_financial.busy".to_string(),
            Self::Journal(MarketplaceFinancialOperationError::Database(_)) => {
                "marketplace_financial.storage_unavailable".to_string()
            }
            Self::Journal(_) => "marketplace_financial.journal_conflict".to_string(),
            Self::EconomicsCheckpoint(
                CheckoutMarketplaceEconomicsCheckpointError::Database(_),
            ) => "marketplace_financial.checkpoint_unavailable".to_string(),
            Self::EconomicsCheckpoint(_) => {
                "marketplace_financial.checkpoint_conflict".to_string()
            }
            Self::Validation(_) => "marketplace_financial.validation".to_string(),
            Self::Invariant(_) => "marketplace_financial.invariant".to_string(),
        }
    }
}

pub type CheckoutMarketplaceFinancialResult<T> =
    Result<T, CheckoutMarketplaceFinancialError>;

pub struct CheckoutMarketplaceFinancialStage {
    journal: MarketplaceFinancialOperationJournal,
    economics_journal: CheckoutMarketplaceEconomicsCheckpointJournal,
    ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
}

impl CheckoutMarketplaceFinancialStage {
    pub fn new(
        db: DatabaseConnection,
        ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
    ) -> Self {
        Self {
            journal: MarketplaceFinancialOperationJournal::new(db.clone()),
            economics_journal: CheckoutMarketplaceEconomicsCheckpointJournal::new(db),
            ledger_port,
        }
    }

    pub async fn post_after_capture_if_present(
        &self,
        tenant_id: Uuid,
        _actor_id: Uuid,
        checkout_lease_owner: &str,
        captured: &CheckoutPaymentCapturedState,
    ) -> CheckoutMarketplaceFinancialResult<()> {
        if captured.plan.payload.marketplace_lines.is_empty() {
            return Ok(());
        }
        validate_captured_payment(captured)?;
        let payment = &captured.payment_collection;
        let posted_at = payment
            .captured_at
            .ok_or_else(|| {
                CheckoutMarketplaceFinancialError::Invariant(format!(
                    "captured payment collection {} has no captured_at timestamp",
                    payment.id
                ))
            })?
            .fixed_offset();

        let checkpoint = self
            .economics_journal
            .get(tenant_id, captured.operation_id)
            .await?
            .ok_or_else(|| {
                CheckoutMarketplaceFinancialError::Invariant(format!(
                    "checkout operation {} has no pre-capture marketplace economics checkpoint",
                    captured.operation_id
                ))
            })?;
        validate_marketplace_economics_checkpoint(
            &checkpoint,
            tenant_id,
            captured.operation_id,
            captured.order.id,
            captured.plan.plan_hash.as_str(),
            captured.order.currency_code.as_str(),
            captured.plan.payload.marketplace_lines.len(),
        )?;

        let operation = self
            .journal
            .begin(BeginMarketplaceFinancialOperation {
                tenant_id,
                checkout_operation_id: captured.operation_id,
                order_id: captured.order.id,
                payment_collection_id: payment.id,
                plan_hash: captured.plan.plan_hash.clone(),
                currency_code: captured.order.currency_code.clone(),
                posted_at,
            })
            .await?;
        if operation.status == MarketplaceFinancialOperationStatus::Completed.as_str() {
            validate_completed_against_checkpoint(&operation, &checkpoint)?;
            return Ok(());
        }
        if operation.status == MarketplaceFinancialOperationStatus::OperatorReview.as_str() {
            return Err(CheckoutMarketplaceFinancialError::Invariant(format!(
                "financial operation {} requires operator review",
                captured.operation_id
            )));
        }

        let lease_owner = format!("{checkout_lease_owner}:marketplace-finance");
        let Some(claimed) = self
            .journal
            .claim(tenant_id, captured.operation_id, lease_owner.as_str())
            .await?
        else {
            let current = self.journal.get(tenant_id, captured.operation_id).await?;
            if current.status == MarketplaceFinancialOperationStatus::Completed.as_str() {
                validate_completed_against_checkpoint(&current, &checkpoint)?;
                return Ok(());
            }
            if current.status == MarketplaceFinancialOperationStatus::OperatorReview.as_str() {
                return Err(CheckoutMarketplaceFinancialError::Invariant(format!(
                    "financial operation {} requires operator review",
                    captured.operation_id
                )));
            }
            return Err(CheckoutMarketplaceFinancialError::Busy(format!(
                "operation {} is status `{}` with lease owner {}",
                current.checkout_operation_id,
                current.status,
                current.lease_owner.as_deref().unwrap_or("none")
            )));
        };

        let mut context = PortContext::new(
            tenant_id.to_string(),
            PortActor::service(captured.operation_id.to_string()),
            captured.plan.payload.context.locale.clone(),
            format!("checkout-marketplace-ledger-{}", captured.operation_id),
        )
        .with_deadline(LEDGER_DEADLINE)
        .with_idempotency_key(claimed.idempotency_key.clone());
        if let Some(channel) = captured.plan.payload.channel_slug.clone() {
            context = context.with_channel(channel);
        }

        let ledger = match self
            .ledger_port
            .post_order_commissions(
                context,
                PostMarketplaceOrderLedgerInput {
                    order_id: captured.order.id,
                    posted_at,
                },
            )
            .await
        {
            Ok(ledger) => ledger,
            Err(error) => {
                let mapped = map_port_error(error);
                if mapped.retryable() {
                    self.journal
                        .mark_retryable_error(
                            tenant_id,
                            captured.operation_id,
                            lease_owner,
                            mapped.code(),
                            mapped.to_string(),
                        )
                        .await?;
                } else {
                    self.journal
                        .mark_operator_review(
                            tenant_id,
                            captured.operation_id,
                            lease_owner,
                            mapped.code(),
                            mapped.to_string(),
                        )
                        .await?;
                }
                return Err(mapped);
            }
        };

        if let Err(error) = validate_ledger(&ledger, tenant_id, captured, &checkpoint) {
            self.journal
                .mark_operator_review(
                    tenant_id,
                    captured.operation_id,
                    lease_owner,
                    error.code(),
                    error.to_string(),
                )
                .await?;
            return Err(error);
        }
        self.journal
            .complete_with_ledger(
                tenant_id,
                captured.operation_id,
                lease_owner,
                &ledger,
            )
            .await?;
        Ok(())
    }
}

fn validate_captured_payment(
    captured: &CheckoutPaymentCapturedState,
) -> CheckoutMarketplaceFinancialResult<()> {
    let payment = &captured.payment_collection;
    if payment.status != "captured"
        || payment.captured_at.is_none()
        || payment.order_id != Some(captured.order.id)
        || payment.captured_amount != captured.order.total_amount
        || !payment
            .currency_code
            .eq_ignore_ascii_case(&captured.order.currency_code)
    {
        return Err(CheckoutMarketplaceFinancialError::Validation(format!(
            "payment collection {} is not a fully captured result for order {}",
            payment.id, captured.order.id
        )));
    }
    Ok(())
}

fn validate_ledger(
    ledger: &MarketplaceLedgerTransactionResponse,
    tenant_id: Uuid,
    captured: &CheckoutPaymentCapturedState,
    checkpoint: &crate::entities::checkout_marketplace_economics_checkpoint::Model,
) -> CheckoutMarketplaceFinancialResult<()> {
    if ledger.tenant_id != tenant_id
        || ledger.order_id != captured.order.id
        || ledger.source_id != captured.order.id
        || ledger.status != MarketplaceLedgerTransactionStatus::Posted
        || !ledger
            .currency_code
            .eq_ignore_ascii_case(&checkpoint.currency_code)
        || ledger.debit_total_amount != ledger.credit_total_amount
        || ledger.debit_total_amount != checkpoint.allocation_total_amount
    {
        return Err(CheckoutMarketplaceFinancialError::Invariant(format!(
            "ledger transaction {} does not reconcile to captured marketplace economics",
            ledger.id
        )));
    }
    Ok(())
}

fn validate_completed_against_checkpoint(
    operation: &crate::entities::marketplace_financial_operation::Model,
    checkpoint: &crate::entities::checkout_marketplace_economics_checkpoint::Model,
) -> CheckoutMarketplaceFinancialResult<()> {
    if operation.status != MarketplaceFinancialOperationStatus::Completed.as_str()
        || operation.stage != "ledger_posted"
        || operation.ledger_transaction_id.is_none()
        || operation.ledger_debit_total_amount != Some(checkpoint.allocation_total_amount)
        || operation.ledger_credit_total_amount != Some(checkpoint.allocation_total_amount)
        || operation.completed_at.is_none()
    {
        return Err(CheckoutMarketplaceFinancialError::Invariant(format!(
            "completed financial operation {} does not reconcile to the pre-capture checkpoint",
            operation.checkout_operation_id
        )));
    }
    Ok(())
}

fn map_port_error(error: PortError) -> CheckoutMarketplaceFinancialError {
    let message = match error.kind {
        PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict => {
            error.message
        }
        PortErrorKind::Forbidden => "marketplace ledger permission denied".to_string(),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            "marketplace ledger owner is temporarily unavailable".to_string()
        }
        PortErrorKind::InvariantViolation => {
            "marketplace ledger receipt requires operator review".to_string()
        }
    };
    CheckoutMarketplaceFinancialError::Boundary {
        code: error.code,
        message,
        retryable: error.retryable,
    }
}
