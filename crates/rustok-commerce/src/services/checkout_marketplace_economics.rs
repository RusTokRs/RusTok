use chrono::Utc;
use rustok_marketplace_allocation::{
    AllocateMarketplaceOrderLinesResponse, MarketplaceAllocationStatus,
};
use rustok_marketplace_commission::{
    AssessMarketplaceOrderCommissionsResponse, MarketplaceCommissionAssessmentStatus,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{checkout_marketplace_economics_checkpoint, checkout_operation};

use super::{CheckoutOperationStage, CheckoutOperationStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckoutMarketplaceEconomicsEvidence {
    pub order_id: Uuid,
    pub plan_hash: String,
    pub currency_code: String,
    pub allocation_count: i32,
    pub allocation_total_amount: i64,
    pub allocation_set_hash: String,
    pub assessment_count: i32,
    pub commission_total_amount: i64,
    pub seller_proceeds_total_amount: i64,
    pub assessment_set_hash: String,
}

#[derive(Clone, Debug)]
pub struct RecordCheckoutMarketplaceEconomicsCheckpoint {
    pub tenant_id: Uuid,
    pub checkout_operation_id: Uuid,
    pub lease_owner: String,
    pub evidence: CheckoutMarketplaceEconomicsEvidence,
}

#[derive(Debug, Error)]
pub enum CheckoutMarketplaceEconomicsCheckpointError {
    #[error("marketplace economics checkpoint validation failed: {0}")]
    Validation(String),
    #[error("marketplace economics checkpoint conflict: {0}")]
    Conflict(String),
    #[error("checkout operation {0} was not found for marketplace economics checkpoint")]
    OperationNotFound(Uuid),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type CheckoutMarketplaceEconomicsCheckpointResult<T> =
    Result<T, CheckoutMarketplaceEconomicsCheckpointError>;

#[derive(Clone)]
pub struct CheckoutMarketplaceEconomicsCheckpointJournal {
    db: DatabaseConnection,
}

impl CheckoutMarketplaceEconomicsCheckpointJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> CheckoutMarketplaceEconomicsCheckpointResult<
        Option<checkout_marketplace_economics_checkpoint::Model>,
    > {
        checkout_marketplace_economics_checkpoint::Entity::find_by_id(checkout_operation_id)
            .filter(checkout_marketplace_economics_checkpoint::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn record(
        &self,
        input: RecordCheckoutMarketplaceEconomicsCheckpoint,
    ) -> CheckoutMarketplaceEconomicsCheckpointResult<
        checkout_marketplace_economics_checkpoint::Model,
    > {
        let input = normalize_record_input(input)?;
        if let Some(existing) = self
            .get(input.tenant_id, input.checkout_operation_id)
            .await?
        {
            ensure_same_evidence(&existing, &input.evidence)?;
            return Ok(existing);
        }

        let transaction = self.db.begin().await?;
        let operation = checkout_operation::Entity::find_by_id(input.checkout_operation_id)
            .filter(checkout_operation::Column::TenantId.eq(input.tenant_id))
            .one(&transaction)
            .await?
            .ok_or(CheckoutMarketplaceEconomicsCheckpointError::OperationNotFound(
                input.checkout_operation_id,
            ))?;
        validate_operation_lease(&operation, &input)?;

        if let Some(existing) = checkout_marketplace_economics_checkpoint::Entity::find_by_id(
            input.checkout_operation_id,
        )
        .filter(checkout_marketplace_economics_checkpoint::Column::TenantId.eq(input.tenant_id))
        .one(&transaction)
        .await?
        {
            ensure_same_evidence(&existing, &input.evidence)?;
            transaction.commit().await?;
            return Ok(existing);
        }

        let now = Utc::now().fixed_offset();
        let evidence = input.evidence;
        let model = checkout_marketplace_economics_checkpoint::ActiveModel {
            checkout_operation_id: Set(input.checkout_operation_id),
            tenant_id: Set(input.tenant_id),
            order_id: Set(evidence.order_id),
            plan_hash: Set(evidence.plan_hash),
            currency_code: Set(evidence.currency_code),
            allocation_count: Set(evidence.allocation_count),
            allocation_total_amount: Set(evidence.allocation_total_amount),
            allocation_set_hash: Set(evidence.allocation_set_hash),
            assessment_count: Set(evidence.assessment_count),
            commission_total_amount: Set(evidence.commission_total_amount),
            seller_proceeds_total_amount: Set(evidence.seller_proceeds_total_amount),
            assessment_set_hash: Set(evidence.assessment_set_hash),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
        Ok(model)
    }
}

pub fn build_marketplace_economics_evidence(
    plan_hash: &str,
    allocation: &AllocateMarketplaceOrderLinesResponse,
    commission: &AssessMarketplaceOrderCommissionsResponse,
) -> CheckoutMarketplaceEconomicsCheckpointResult<CheckoutMarketplaceEconomicsEvidence> {
    let plan_hash = normalize_hash(plan_hash, "plan_hash")?;
    if allocation.order_id != commission.order_id {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Validation(
            "allocation and commission responses reference different orders".to_string(),
        ));
    }
    let currency_code = normalize_currency_code(&allocation.currency_code)?;
    if allocation.allocations.is_empty() {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Validation(
            "marketplace economics checkpoint requires at least one allocation".to_string(),
        ));
    }
    if allocation.allocations.len() != commission.assessments.len() {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Validation(format!(
            "allocation count {} does not match assessment count {}",
            allocation.allocations.len(),
            commission.assessments.len()
        )));
    }

    let mut allocation_rows = Vec::with_capacity(allocation.allocations.len());
    let mut allocation_ids = Vec::with_capacity(allocation.allocations.len());
    let mut allocation_total_amount = 0_i64;
    for item in &allocation.allocations {
        if item.order_id != allocation.order_id
            || item.status != MarketplaceAllocationStatus::Allocated
            || normalize_currency_code(&item.currency_code)? != currency_code
        {
            return Err(CheckoutMarketplaceEconomicsCheckpointError::Validation(format!(
                "allocation {} does not match the checkpoint order, currency, or active status",
                item.id
            )));
        }
        allocation_total_amount = allocation_total_amount
            .checked_add(item.total_amount)
            .ok_or_else(|| {
                CheckoutMarketplaceEconomicsCheckpointError::Validation(
                    "allocation total overflow".to_string(),
                )
            })?;
        allocation_ids.push(item.id);
        allocation_rows.push(format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            item.id,
            item.order_line_item_id,
            item.seller_id,
            item.listing_id,
            item.currency_code.to_ascii_uppercase(),
            item.total_amount,
            item.listing_terms_version,
            item.status.as_str(),
        ));
    }

    let mut assessment_rows = Vec::with_capacity(commission.assessments.len());
    let mut assessed_allocation_ids = Vec::with_capacity(commission.assessments.len());
    let mut commission_total_amount = 0_i64;
    let mut seller_proceeds_total_amount = 0_i64;
    for item in &commission.assessments {
        if item.order_id != commission.order_id
            || item.status != MarketplaceCommissionAssessmentStatus::Assessed
            || normalize_currency_code(&item.currency_code)? != currency_code
            || item.commission_amount < 0
            || item.seller_proceeds_amount < 0
            || item
                .commission_amount
                .checked_add(item.seller_proceeds_amount)
                != Some(item.allocation_total_amount)
        {
            return Err(CheckoutMarketplaceEconomicsCheckpointError::Validation(format!(
                "commission assessment {} does not match the checkpoint economics",
                item.id
            )));
        }
        commission_total_amount = commission_total_amount
            .checked_add(item.commission_amount)
            .ok_or_else(|| {
                CheckoutMarketplaceEconomicsCheckpointError::Validation(
                    "commission total overflow".to_string(),
                )
            })?;
        seller_proceeds_total_amount = seller_proceeds_total_amount
            .checked_add(item.seller_proceeds_amount)
            .ok_or_else(|| {
                CheckoutMarketplaceEconomicsCheckpointError::Validation(
                    "seller proceeds total overflow".to_string(),
                )
            })?;
        assessed_allocation_ids.push(item.allocation_id);
        assessment_rows.push(format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            item.id,
            item.allocation_id,
            item.order_line_item_id,
            item.seller_id,
            item.listing_id,
            item.rule_id,
            item.rule_version,
            item.commission_amount,
            item.seller_proceeds_amount,
        ));
    }

    allocation_ids.sort_unstable();
    assessed_allocation_ids.sort_unstable();
    if allocation_ids != assessed_allocation_ids {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Validation(
            "commission assessments do not cover the exact allocation set".to_string(),
        ));
    }
    if commission.commission_total_amount != commission_total_amount
        || commission.seller_proceeds_total_amount != seller_proceeds_total_amount
        || commission_total_amount.checked_add(seller_proceeds_total_amount)
            != Some(allocation_total_amount)
    {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Validation(
            "commission aggregate totals do not reconcile to allocations".to_string(),
        ));
    }

    Ok(CheckoutMarketplaceEconomicsEvidence {
        order_id: allocation.order_id,
        plan_hash,
        currency_code,
        allocation_count: count_to_i32(allocation.allocations.len(), "allocation_count")?,
        allocation_total_amount,
        allocation_set_hash: hash_rows(allocation_rows),
        assessment_count: count_to_i32(commission.assessments.len(), "assessment_count")?,
        commission_total_amount,
        seller_proceeds_total_amount,
        assessment_set_hash: hash_rows(assessment_rows),
    })
}

pub fn validate_marketplace_economics_checkpoint(
    checkpoint: &checkout_marketplace_economics_checkpoint::Model,
    tenant_id: Uuid,
    checkout_operation_id: Uuid,
    order_id: Uuid,
    plan_hash: &str,
    currency_code: &str,
    expected_marketplace_line_count: usize,
) -> CheckoutMarketplaceEconomicsCheckpointResult<()> {
    let expected_count = count_to_i32(expected_marketplace_line_count, "marketplace_line_count")?;
    let plan_hash = normalize_hash(plan_hash, "plan_hash")?;
    let currency_code = normalize_currency_code(currency_code)?;
    if checkpoint.tenant_id != tenant_id
        || checkpoint.checkout_operation_id != checkout_operation_id
        || checkpoint.order_id != order_id
        || checkpoint.plan_hash != plan_hash
        || checkpoint.currency_code != currency_code
        || checkpoint.allocation_count != expected_count
        || checkpoint.assessment_count != expected_count
        || checkpoint.commission_total_amount < 0
        || checkpoint.seller_proceeds_total_amount < 0
        || checkpoint
            .commission_total_amount
            .checked_add(checkpoint.seller_proceeds_total_amount)
            != Some(checkpoint.allocation_total_amount)
    {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Conflict(format!(
            "checkpoint for checkout operation {checkout_operation_id} does not match the immutable checkout plan"
        )));
    }
    Ok(())
}

fn validate_operation_lease(
    operation: &checkout_operation::Model,
    input: &RecordCheckoutMarketplaceEconomicsCheckpoint,
) -> CheckoutMarketplaceEconomicsCheckpointResult<()> {
    let now = Utc::now().fixed_offset();
    if operation.status != CheckoutOperationStatus::Executing.as_str()
        || operation.stage != CheckoutOperationStage::PaymentReady.as_str()
        || operation.order_id != Some(input.evidence.order_id)
        || operation.lease_owner.as_deref() != Some(input.lease_owner.as_str())
        || operation.lease_expires_at.is_none_or(|expires_at| expires_at <= now)
    {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Conflict(format!(
            "checkout operation {} is not actively leased at payment_ready for this economics checkpoint",
            operation.id
        )));
    }
    Ok(())
}

fn normalize_record_input(
    mut input: RecordCheckoutMarketplaceEconomicsCheckpoint,
) -> CheckoutMarketplaceEconomicsCheckpointResult<RecordCheckoutMarketplaceEconomicsCheckpoint> {
    input.lease_owner = input.lease_owner.trim().to_string();
    if input.tenant_id.is_nil()
        || input.checkout_operation_id.is_nil()
        || input.evidence.order_id.is_nil()
        || input.lease_owner.is_empty()
        || input.lease_owner.len() > 191
    {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Validation(
            "checkpoint tenant, operation, order, and lease identity must be valid".to_string(),
        ));
    }
    input.evidence.plan_hash = normalize_hash(&input.evidence.plan_hash, "plan_hash")?;
    input.evidence.currency_code = normalize_currency_code(&input.evidence.currency_code)?;
    input.evidence.allocation_set_hash =
        normalize_hash(&input.evidence.allocation_set_hash, "allocation_set_hash")?;
    input.evidence.assessment_set_hash =
        normalize_hash(&input.evidence.assessment_set_hash, "assessment_set_hash")?;
    Ok(input)
}

fn ensure_same_evidence(
    existing: &checkout_marketplace_economics_checkpoint::Model,
    evidence: &CheckoutMarketplaceEconomicsEvidence,
) -> CheckoutMarketplaceEconomicsCheckpointResult<()> {
    if existing.order_id != evidence.order_id
        || existing.plan_hash != evidence.plan_hash
        || existing.currency_code != evidence.currency_code
        || existing.allocation_count != evidence.allocation_count
        || existing.allocation_total_amount != evidence.allocation_total_amount
        || existing.allocation_set_hash != evidence.allocation_set_hash
        || existing.assessment_count != evidence.assessment_count
        || existing.commission_total_amount != evidence.commission_total_amount
        || existing.seller_proceeds_total_amount != evidence.seller_proceeds_total_amount
        || existing.assessment_set_hash != evidence.assessment_set_hash
    {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Conflict(format!(
            "checkout operation {} is already bound to different marketplace economics evidence",
            existing.checkout_operation_id
        )));
    }
    Ok(())
}

fn normalize_hash(
    value: &str,
    field: &str,
) -> CheckoutMarketplaceEconomicsCheckpointResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Validation(format!(
            "{field} must contain 1 to 128 bytes"
        )));
    }
    Ok(value.to_string())
}

fn normalize_currency_code(
    value: &str,
) -> CheckoutMarketplaceEconomicsCheckpointResult<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(CheckoutMarketplaceEconomicsCheckpointError::Validation(
            "currency_code must be a three-letter alphabetic code".to_string(),
        ));
    }
    Ok(value)
}

fn count_to_i32(
    value: usize,
    field: &str,
) -> CheckoutMarketplaceEconomicsCheckpointResult<i32> {
    i32::try_from(value).map_err(|_| {
        CheckoutMarketplaceEconomicsCheckpointError::Validation(format!(
            "{field} exceeds supported range"
        ))
    })
}

fn hash_rows(mut rows: Vec<String>) -> String {
    rows.sort_unstable();
    let mut hasher = Sha256::new();
    for row in rows {
        hasher.update((row.len() as u64).to_be_bytes());
        hasher.update(row.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}
