use chrono::{DateTime, Duration, FixedOffset, Utc};
use rustok_core::generate_id;
use rustok_payment::entities::provider_event;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, sea_query::Expr,
};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::marketplace_reversal_adaptation_failure;

const MAX_OPERATOR_ITEMS: u64 = 100;
const MAX_BACKOFF_SECONDS: i64 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketplaceReversalAdaptationFailureStatus {
    RetryableError,
    OperatorReview,
    Resolved,
}

impl MarketplaceReversalAdaptationFailureStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RetryableError => "retryable_error",
            Self::OperatorReview => "operator_review",
            Self::Resolved => "resolved",
        }
    }
}

#[derive(Debug, Error)]
pub enum MarketplaceReversalAdaptationFailureError {
    #[error("marketplace reversal adaptation failure validation failed: {0}")]
    Validation(String),
    #[error("marketplace reversal adaptation failure conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type MarketplaceReversalAdaptationFailureResult<T> =
    Result<T, MarketplaceReversalAdaptationFailureError>;

#[derive(Clone)]
pub struct MarketplaceReversalAdaptationFailureJournal {
    db: DatabaseConnection,
}

impl MarketplaceReversalAdaptationFailureJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn record_failure(
        &self,
        event: &provider_event::Model,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
        retryable: bool,
    ) -> MarketplaceReversalAdaptationFailureResult<marketplace_reversal_adaptation_failure::Model>
    {
        let event_type = normalize_required(
            event.event_type.clone().unwrap_or_default(),
            100,
            "event_type",
        )?;
        let event_source = normalize_required(event.provider_id.clone(), 100, "event_source")?;
        let event_id = normalize_required(event.delivery_id.clone(), 191, "event_id")?;
        let error_code = normalize_required(error_code.into(), 100, "error_code")?;
        let error_message = normalize_required(error_message.into(), 2000, "error_message")?;

        if let Some(existing) = self
            .find_by_provider_event(event.tenant_id, event.id)
            .await?
        {
            return self
                .update_failure(existing, error_code, error_message, retryable)
                .await;
        }

        let now = Utc::now().fixed_offset();
        let status = if retryable {
            MarketplaceReversalAdaptationFailureStatus::RetryableError
        } else {
            MarketplaceReversalAdaptationFailureStatus::OperatorReview
        };
        let next_retry_at = retryable.then(|| now + retry_backoff(1));
        let insert = marketplace_reversal_adaptation_failure::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(event.tenant_id),
            provider_event_id: Set(event.id),
            event_source: Set(event_source),
            event_id: Set(event_id),
            event_type: Set(event_type),
            status: Set(status.as_str().to_string()),
            retryable: Set(retryable),
            attempt_count: Set(1),
            last_error_code: Set(error_code.clone()),
            last_error_message: Set(error_message.clone()),
            next_retry_at: Set(next_retry_at),
            created_at: Set(now),
            updated_at: Set(now),
            resolved_at: Set(None),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(model) => Ok(model),
            Err(error) => {
                if let Some(existing) = self
                    .find_by_provider_event(event.tenant_id, event.id)
                    .await?
                {
                    self.update_failure(existing, error_code, error_message, retryable)
                        .await
                } else {
                    Err(error.into())
                }
            }
        }
    }

    pub async fn mark_resolved(
        &self,
        tenant_id: Uuid,
        provider_event_id: Uuid,
    ) -> MarketplaceReversalAdaptationFailureResult<
        Option<marketplace_reversal_adaptation_failure::Model>,
    > {
        validate_identity(tenant_id, provider_event_id)?;
        let now = Utc::now().fixed_offset();
        let update = marketplace_reversal_adaptation_failure::Entity::update_many()
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::Status,
                Expr::value(MarketplaceReversalAdaptationFailureStatus::Resolved.as_str()),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::Retryable,
                Expr::value(false),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::NextRetryAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::ResolvedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_reversal_adaptation_failure::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_reversal_adaptation_failure::Column::ProviderEventId
                    .eq(provider_event_id),
            )
            .filter(
                marketplace_reversal_adaptation_failure::Column::Status
                    .ne(MarketplaceReversalAdaptationFailureStatus::Resolved.as_str()),
            )
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return self.find_by_provider_event(tenant_id, provider_event_id).await;
        }
        self.find_by_provider_event(tenant_id, provider_event_id).await
    }

    pub async fn reset_for_retry(
        &self,
        tenant_id: Uuid,
        failure_id: Uuid,
    ) -> MarketplaceReversalAdaptationFailureResult<marketplace_reversal_adaptation_failure::Model>
    {
        validate_identity(tenant_id, failure_id)?;
        let now = Utc::now().fixed_offset();
        let update = marketplace_reversal_adaptation_failure::Entity::update_many()
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::Status,
                Expr::value(MarketplaceReversalAdaptationFailureStatus::RetryableError.as_str()),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::Retryable,
                Expr::value(true),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::NextRetryAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_reversal_adaptation_failure::Column::TenantId.eq(tenant_id))
            .filter(marketplace_reversal_adaptation_failure::Column::Id.eq(failure_id))
            .filter(
                marketplace_reversal_adaptation_failure::Column::Status.is_in([
                    MarketplaceReversalAdaptationFailureStatus::RetryableError.as_str(),
                    MarketplaceReversalAdaptationFailureStatus::OperatorReview.as_str(),
                ]),
            )
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Err(MarketplaceReversalAdaptationFailureError::Conflict(format!(
                "adaptation failure {failure_id} is not retryable"
            )));
        }
        self.get(tenant_id, failure_id).await
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        failure_id: Uuid,
    ) -> MarketplaceReversalAdaptationFailureResult<marketplace_reversal_adaptation_failure::Model>
    {
        validate_identity(tenant_id, failure_id)?;
        marketplace_reversal_adaptation_failure::Entity::find_by_id(failure_id)
            .filter(marketplace_reversal_adaptation_failure::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                MarketplaceReversalAdaptationFailureError::Conflict(format!(
                    "adaptation failure {failure_id} was not found"
                ))
            })
    }

    pub async fn list_operator_review(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> MarketplaceReversalAdaptationFailureResult<
        Vec<marketplace_reversal_adaptation_failure::Model>,
    > {
        if tenant_id.is_nil() {
            return Err(MarketplaceReversalAdaptationFailureError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        marketplace_reversal_adaptation_failure::Entity::find()
            .filter(marketplace_reversal_adaptation_failure::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_reversal_adaptation_failure::Column::Status
                    .eq(MarketplaceReversalAdaptationFailureStatus::OperatorReview.as_str()),
            )
            .order_by_asc(marketplace_reversal_adaptation_failure::Column::UpdatedAt)
            .order_by_asc(marketplace_reversal_adaptation_failure::Column::Id)
            .limit(limit.clamp(1, MAX_OPERATOR_ITEMS))
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn find_by_provider_event(
        &self,
        tenant_id: Uuid,
        provider_event_id: Uuid,
    ) -> MarketplaceReversalAdaptationFailureResult<
        Option<marketplace_reversal_adaptation_failure::Model>,
    > {
        validate_identity(tenant_id, provider_event_id)?;
        marketplace_reversal_adaptation_failure::Entity::find()
            .filter(marketplace_reversal_adaptation_failure::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_reversal_adaptation_failure::Column::ProviderEventId
                    .eq(provider_event_id),
            )
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    async fn update_failure(
        &self,
        existing: marketplace_reversal_adaptation_failure::Model,
        error_code: String,
        error_message: String,
        retryable: bool,
    ) -> MarketplaceReversalAdaptationFailureResult<marketplace_reversal_adaptation_failure::Model>
    {
        if existing.status == MarketplaceReversalAdaptationFailureStatus::Resolved.as_str() {
            return Ok(existing);
        }
        let now = Utc::now().fixed_offset();
        let attempt_count = existing.attempt_count.saturating_add(1).max(1);
        let status = if retryable {
            MarketplaceReversalAdaptationFailureStatus::RetryableError
        } else {
            MarketplaceReversalAdaptationFailureStatus::OperatorReview
        };
        let next_retry_at = retryable.then(|| now + retry_backoff(attempt_count));
        let update = marketplace_reversal_adaptation_failure::Entity::update_many()
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::Status,
                Expr::value(status.as_str()),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::Retryable,
                Expr::value(retryable),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::AttemptCount,
                Expr::value(attempt_count),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::LastErrorCode,
                Expr::value(error_code),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::LastErrorMessage,
                Expr::value(error_message),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::NextRetryAt,
                Expr::value(next_retry_at),
            )
            .col_expr(
                marketplace_reversal_adaptation_failure::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_reversal_adaptation_failure::Column::Id.eq(existing.id))
            .filter(
                marketplace_reversal_adaptation_failure::Column::Status
                    .ne(MarketplaceReversalAdaptationFailureStatus::Resolved.as_str()),
            )
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return self.get(existing.tenant_id, existing.id).await;
        }
        self.get(existing.tenant_id, existing.id).await
    }
}

fn retry_backoff(attempt_count: i32) -> Duration {
    let exponent = u32::try_from(attempt_count.saturating_sub(1).clamp(0, 6)).unwrap_or(0);
    let seconds = 10_i64
        .checked_mul(2_i64.saturating_pow(exponent))
        .unwrap_or(MAX_BACKOFF_SECONDS)
        .min(MAX_BACKOFF_SECONDS);
    Duration::seconds(seconds)
}

fn validate_identity(
    tenant_id: Uuid,
    id: Uuid,
) -> MarketplaceReversalAdaptationFailureResult<()> {
    if tenant_id.is_nil() || id.is_nil() {
        return Err(MarketplaceReversalAdaptationFailureError::Validation(
            "tenant_id and identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn normalize_required(
    value: String,
    max_length: usize,
    field: &str,
) -> MarketplaceReversalAdaptationFailureResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > max_length {
        return Err(MarketplaceReversalAdaptationFailureError::Validation(format!(
            "{field} must contain 1 to {max_length} bytes"
        )));
    }
    Ok(value)
}
