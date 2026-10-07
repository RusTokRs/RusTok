use chrono::{DateTime, Duration, FixedOffset, Utc};
use rustok_marketplace_ledger::MarketplaceLedgerTransactionResponse;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, Set,
    sea_query::{Expr, ExprTrait},
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::marketplace_financial_operation;

const FINANCIAL_LEASE_SECONDS: i64 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketplaceFinancialOperationStatus {
    Pending,
    Executing,
    RetryableError,
    OperatorReview,
    Completed,
}

impl MarketplaceFinancialOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Executing => "executing",
            Self::RetryableError => "retryable_error",
            Self::OperatorReview => "operator_review",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Debug)]
pub struct BeginMarketplaceFinancialOperation {
    pub tenant_id: Uuid,
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub plan_hash: String,
    pub currency_code: String,
    pub posted_at: DateTime<FixedOffset>,
}

#[derive(Debug, Error)]
pub enum MarketplaceFinancialOperationError {
    #[error("marketplace financial operation validation failed: {0}")]
    Validation(String),
    #[error("marketplace financial operation {0} was not found")]
    NotFound(Uuid),
    #[error("marketplace financial operation conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type MarketplaceFinancialOperationResult<T> = Result<T, MarketplaceFinancialOperationError>;

#[derive(Clone)]
pub struct MarketplaceFinancialOperationJournal {
    db: DatabaseConnection,
}

impl MarketplaceFinancialOperationJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn begin(
        &self,
        input: BeginMarketplaceFinancialOperation,
    ) -> MarketplaceFinancialOperationResult<marketplace_financial_operation::Model> {
        let normalized = normalize_begin_input(input)?;
        if let Some(existing) = self
            .get_optional(normalized.tenant_id, normalized.checkout_operation_id)
            .await?
        {
            ensure_same_operation(&existing, &normalized)?;
            return Ok(existing);
        }

        let now = Utc::now().fixed_offset();
        let insert = marketplace_financial_operation::ActiveModel {
            checkout_operation_id: Set(normalized.checkout_operation_id),
            tenant_id: Set(normalized.tenant_id),
            order_id: Set(normalized.order_id),
            payment_collection_id: Set(normalized.payment_collection_id),
            plan_hash: Set(normalized.plan_hash.clone()),
            currency_code: Set(normalized.currency_code.clone()),
            idempotency_key: Set(normalized.idempotency_key.clone()),
            request_hash: Set(normalized.request_hash.clone()),
            status: Set(MarketplaceFinancialOperationStatus::Pending
                .as_str()
                .to_string()),
            stage: Set("admitted".to_string()),
            attempt_count: Set(0),
            lease_owner: Set(None),
            lease_expires_at: Set(None),
            ledger_transaction_id: Set(None),
            ledger_debit_total_amount: Set(None),
            ledger_credit_total_amount: Set(None),
            last_error_code: Set(None),
            last_error_message: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            completed_at: Set(None),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(model) => Ok(model),
            Err(error) => {
                if let Some(existing) = self
                    .get_optional(normalized.tenant_id, normalized.checkout_operation_id)
                    .await?
                {
                    ensure_same_operation(&existing, &normalized)?;
                    Ok(existing)
                } else {
                    Err(error.into())
                }
            }
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> MarketplaceFinancialOperationResult<marketplace_financial_operation::Model> {
        self.get_optional(tenant_id, checkout_operation_id)
            .await?
            .ok_or(MarketplaceFinancialOperationError::NotFound(
                checkout_operation_id,
            ))
    }

    pub async fn claim(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
        lease_owner: impl Into<String>,
    ) -> MarketplaceFinancialOperationResult<Option<marketplace_financial_operation::Model>> {
        let lease_owner = normalize_lease_owner(lease_owner.into())?;
        let now = Utc::now().fixed_offset();
        let expires_at = now + Duration::seconds(FINANCIAL_LEASE_SECONDS);
        let claimable = Condition::any()
            .add(marketplace_financial_operation::Column::Status.is_in([
                MarketplaceFinancialOperationStatus::Pending.as_str(),
                MarketplaceFinancialOperationStatus::RetryableError.as_str(),
            ]))
            .add(
                Condition::all()
                    .add(
                        marketplace_financial_operation::Column::Status
                            .eq(MarketplaceFinancialOperationStatus::Executing.as_str()),
                    )
                    .add(marketplace_financial_operation::Column::LeaseExpiresAt.lte(now)),
            );
        let update = marketplace_financial_operation::Entity::update_many()
            .col_expr(
                marketplace_financial_operation::Column::Status,
                Expr::value(MarketplaceFinancialOperationStatus::Executing.as_str()),
            )
            .col_expr(
                marketplace_financial_operation::Column::LeaseOwner,
                Expr::value(Some(lease_owner)),
            )
            .col_expr(
                marketplace_financial_operation::Column::LeaseExpiresAt,
                Expr::value(Some(expires_at)),
            )
            .col_expr(
                marketplace_financial_operation::Column::AttemptCount,
                Expr::col(marketplace_financial_operation::Column::AttemptCount).add(1),
            )
            .col_expr(
                marketplace_financial_operation::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_financial_operation::Column::LastErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_financial_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(marketplace_financial_operation::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_financial_operation::Column::CheckoutOperationId
                    .eq(checkout_operation_id),
            )
            .filter(claimable)
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, checkout_operation_id).await.map(Some)
    }

    pub async fn complete_with_ledger(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
        lease_owner: impl Into<String>,
        ledger: &MarketplaceLedgerTransactionResponse,
    ) -> MarketplaceFinancialOperationResult<marketplace_financial_operation::Model> {
        let lease_owner = normalize_lease_owner(lease_owner.into())?;
        let now = Utc::now().fixed_offset();
        let update = marketplace_financial_operation::Entity::update_many()
            .col_expr(
                marketplace_financial_operation::Column::Status,
                Expr::value(MarketplaceFinancialOperationStatus::Completed.as_str()),
            )
            .col_expr(
                marketplace_financial_operation::Column::Stage,
                Expr::value("ledger_posted"),
            )
            .col_expr(
                marketplace_financial_operation::Column::LedgerTransactionId,
                Expr::value(Some(ledger.id)),
            )
            .col_expr(
                marketplace_financial_operation::Column::LedgerDebitTotalAmount,
                Expr::value(Some(ledger.debit_total_amount)),
            )
            .col_expr(
                marketplace_financial_operation::Column::LedgerCreditTotalAmount,
                Expr::value(Some(ledger.credit_total_amount)),
            )
            .col_expr(
                marketplace_financial_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_financial_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                marketplace_financial_operation::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                marketplace_financial_operation::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_financial_operation::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_financial_operation::Column::CheckoutOperationId
                    .eq(checkout_operation_id),
            )
            .filter(
                marketplace_financial_operation::Column::Status
                    .eq(MarketplaceFinancialOperationStatus::Executing.as_str()),
            )
            .filter(marketplace_financial_operation::Column::Stage.eq("admitted"))
            .filter(marketplace_financial_operation::Column::LeaseOwner.eq(lease_owner))
            .filter(marketplace_financial_operation::Column::LeaseExpiresAt.gt(now))
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            let current = self.get(tenant_id, checkout_operation_id).await?;
            if current.status == MarketplaceFinancialOperationStatus::Completed.as_str() {
                validate_completed_operation(&current, ledger)?;
                return Ok(current);
            }
            return Err(MarketplaceFinancialOperationError::Conflict(format!(
                "operation {checkout_operation_id} lost its active lease before ledger completion"
            )));
        }
        self.get(tenant_id, checkout_operation_id).await
    }

    pub async fn mark_retryable_error(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
        lease_owner: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> MarketplaceFinancialOperationResult<marketplace_financial_operation::Model> {
        self.release_with_error(
            tenant_id,
            checkout_operation_id,
            lease_owner.into(),
            MarketplaceFinancialOperationStatus::RetryableError,
            code.into(),
            message.into(),
        )
        .await
    }

    pub async fn mark_operator_review(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
        lease_owner: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> MarketplaceFinancialOperationResult<marketplace_financial_operation::Model> {
        self.release_with_error(
            tenant_id,
            checkout_operation_id,
            lease_owner.into(),
            MarketplaceFinancialOperationStatus::OperatorReview,
            code.into(),
            message.into(),
        )
        .await
    }

    async fn get_optional(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> MarketplaceFinancialOperationResult<Option<marketplace_financial_operation::Model>> {
        marketplace_financial_operation::Entity::find_by_id(checkout_operation_id)
            .filter(marketplace_financial_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    async fn release_with_error(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
        lease_owner: String,
        next_status: MarketplaceFinancialOperationStatus,
        code: String,
        message: String,
    ) -> MarketplaceFinancialOperationResult<marketplace_financial_operation::Model> {
        let lease_owner = normalize_lease_owner(lease_owner)?;
        let code = normalize_error(code, 100, "error code")?;
        let message = normalize_error(message, 2000, "error message")?;
        let now = Utc::now().fixed_offset();
        let update = marketplace_financial_operation::Entity::update_many()
            .col_expr(
                marketplace_financial_operation::Column::Status,
                Expr::value(next_status.as_str()),
            )
            .col_expr(
                marketplace_financial_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_financial_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                marketplace_financial_operation::Column::LastErrorCode,
                Expr::value(Some(code)),
            )
            .col_expr(
                marketplace_financial_operation::Column::LastErrorMessage,
                Expr::value(Some(message)),
            )
            .col_expr(
                marketplace_financial_operation::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_financial_operation::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_financial_operation::Column::CheckoutOperationId
                    .eq(checkout_operation_id),
            )
            .filter(
                marketplace_financial_operation::Column::Status
                    .eq(MarketplaceFinancialOperationStatus::Executing.as_str()),
            )
            .filter(marketplace_financial_operation::Column::LeaseOwner.eq(lease_owner))
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Err(MarketplaceFinancialOperationError::Conflict(format!(
                "operation {checkout_operation_id} could not release its lease"
            )));
        }
        self.get(tenant_id, checkout_operation_id).await
    }
}

#[derive(Clone)]
struct NormalizedBeginInput {
    tenant_id: Uuid,
    checkout_operation_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
    plan_hash: String,
    currency_code: String,
    idempotency_key: String,
    request_hash: String,
}

fn normalize_begin_input(
    input: BeginMarketplaceFinancialOperation,
) -> MarketplaceFinancialOperationResult<NormalizedBeginInput> {
    if input.tenant_id.is_nil()
        || input.checkout_operation_id.is_nil()
        || input.order_id.is_nil()
        || input.payment_collection_id.is_nil()
    {
        return Err(MarketplaceFinancialOperationError::Validation(
            "tenant, checkout operation, order, and payment identities must not be nil".to_string(),
        ));
    }
    let plan_hash = input.plan_hash.trim().to_string();
    if plan_hash.is_empty() || plan_hash.len() > 128 {
        return Err(MarketplaceFinancialOperationError::Validation(
            "plan_hash must contain 1 to 128 bytes".to_string(),
        ));
    }
    let currency_code = input.currency_code.trim().to_ascii_uppercase();
    if currency_code.len() != 3 || !currency_code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(MarketplaceFinancialOperationError::Validation(
            "currency_code must be a three-letter alphabetic code".to_string(),
        ));
    }
    let idempotency_key = format!(
        "checkout:{}:marketplace-ledger:v1",
        input.checkout_operation_id
    );
    let request_hash = hash_request(
        input.tenant_id,
        input.checkout_operation_id,
        input.order_id,
        input.payment_collection_id,
        plan_hash.as_str(),
        currency_code.as_str(),
        input.posted_at,
    );
    Ok(NormalizedBeginInput {
        tenant_id: input.tenant_id,
        checkout_operation_id: input.checkout_operation_id,
        order_id: input.order_id,
        payment_collection_id: input.payment_collection_id,
        plan_hash,
        currency_code,
        idempotency_key,
        request_hash,
    })
}

fn ensure_same_operation(
    existing: &marketplace_financial_operation::Model,
    input: &NormalizedBeginInput,
) -> MarketplaceFinancialOperationResult<()> {
    if existing.tenant_id != input.tenant_id
        || existing.checkout_operation_id != input.checkout_operation_id
        || existing.order_id != input.order_id
        || existing.payment_collection_id != input.payment_collection_id
        || existing.plan_hash != input.plan_hash
        || existing.currency_code != input.currency_code
        || existing.idempotency_key != input.idempotency_key
        || existing.request_hash != input.request_hash
    {
        return Err(MarketplaceFinancialOperationError::Conflict(format!(
            "checkout operation {} is already bound to different post-capture financial identity",
            input.checkout_operation_id
        )));
    }
    Ok(())
}

fn validate_completed_operation(
    operation: &marketplace_financial_operation::Model,
    ledger: &MarketplaceLedgerTransactionResponse,
) -> MarketplaceFinancialOperationResult<()> {
    if operation.ledger_transaction_id != Some(ledger.id)
        || operation.ledger_debit_total_amount != Some(ledger.debit_total_amount)
        || operation.ledger_credit_total_amount != Some(ledger.credit_total_amount)
    {
        return Err(MarketplaceFinancialOperationError::Conflict(format!(
            "completed financial operation {} contains different ledger evidence",
            operation.checkout_operation_id
        )));
    }
    Ok(())
}

fn normalize_lease_owner(value: String) -> MarketplaceFinancialOperationResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > 191 {
        return Err(MarketplaceFinancialOperationError::Validation(
            "financial operation lease owner must contain 1 to 191 bytes".to_string(),
        ));
    }
    Ok(value)
}

fn normalize_error(
    value: String,
    max_len: usize,
    field: &str,
) -> MarketplaceFinancialOperationResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > max_len {
        return Err(MarketplaceFinancialOperationError::Validation(format!(
            "{field} must contain 1 to {max_len} bytes"
        )));
    }
    Ok(value)
}

fn hash_request(
    tenant_id: Uuid,
    checkout_operation_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
    plan_hash: &str,
    currency_code: &str,
    posted_at: DateTime<FixedOffset>,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        tenant_id.to_string(),
        checkout_operation_id.to_string(),
        order_id.to_string(),
        payment_collection_id.to_string(),
        plan_hash.to_string(),
        currency_code.to_string(),
        posted_at.to_rfc3339(),
    ] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}
