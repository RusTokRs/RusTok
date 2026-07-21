use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use rustok_marketplace::MarketplaceFinancialCommandPort;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::marketplace_reversal_event_inbox;

use super::{
    MarketplaceReversalEventInboxError, MarketplaceReversalEventInboxService,
    MarketplaceReversalEventStatus, MarketplaceReversalEventSweepFailure,
    MarketplaceReversalEventSweepReport,
};

const MAX_OPERATOR_ITEMS: u64 = 100;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceReversalEventOperatorView {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub provider_event_id: Uuid,
    pub event_source: String,
    pub event_id: String,
    pub reversal_kind: String,
    pub source_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub occurred_at: DateTime<FixedOffset>,
    pub currency_code: String,
    pub currency_exponent: i16,
    pub total_amount: i64,
    pub line_count: usize,
    pub status: String,
    pub attempt_count: i32,
    pub reversal_id: Option<Uuid>,
    pub ledger_transaction_id: Option<Uuid>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub processed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Error)]
pub enum MarketplaceReversalOperatorError {
    #[error("marketplace reversal operator request is invalid: {0}")]
    Validation(String),
    #[error("marketplace reversal operator conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Inbox(#[from] MarketplaceReversalEventInboxError),
}

pub type MarketplaceReversalOperatorResult<T> = Result<T, MarketplaceReversalOperatorError>;

pub struct MarketplaceReversalOperatorService {
    db: DatabaseConnection,
    inbox: MarketplaceReversalEventInboxService,
}

impl MarketplaceReversalOperatorService {
    pub fn new(
        db: DatabaseConnection,
        financial_port: Arc<dyn MarketplaceFinancialCommandPort>,
    ) -> Self {
        Self {
            inbox: MarketplaceReversalEventInboxService::new(db.clone(), financial_port),
            db,
        }
    }

    pub async fn get_event(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
    ) -> MarketplaceReversalOperatorResult<MarketplaceReversalEventOperatorView> {
        validate_identity(tenant_id, inbox_id)?;
        self.get_model(tenant_id, inbox_id)
            .await
            .and_then(map_event)
    }

    pub async fn list_operator_review(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> MarketplaceReversalOperatorResult<Vec<MarketplaceReversalEventOperatorView>> {
        if tenant_id.is_nil() {
            return Err(MarketplaceReversalOperatorError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        let models = marketplace_reversal_event_inbox::Entity::find()
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_reversal_event_inbox::Column::Status
                    .eq(MarketplaceReversalEventStatus::OperatorReview.as_str()),
            )
            .order_by_asc(marketplace_reversal_event_inbox::Column::UpdatedAt)
            .order_by_asc(marketplace_reversal_event_inbox::Column::Id)
            .limit(limit.clamp(1, MAX_OPERATOR_ITEMS))
            .all(&self.db)
            .await?;
        models.into_iter().map(map_event).collect()
    }

    pub async fn retry_event(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
    ) -> MarketplaceReversalOperatorResult<MarketplaceReversalEventOperatorView> {
        validate_identity(tenant_id, inbox_id)?;
        let now = Utc::now().fixed_offset();
        let update = marketplace_reversal_event_inbox::Entity::update_many()
            .col_expr(
                marketplace_reversal_event_inbox::Column::Status,
                Expr::value(MarketplaceReversalEventStatus::RetryableError.as_str()),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LastErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(marketplace_reversal_event_inbox::Column::Id.eq(inbox_id))
            .filter(
                marketplace_reversal_event_inbox::Column::Status
                    .eq(MarketplaceReversalEventStatus::OperatorReview.as_str()),
            )
            .filter(marketplace_reversal_event_inbox::Column::ReversalId.is_null())
            .filter(marketplace_reversal_event_inbox::Column::LedgerTransactionId.is_null())
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Err(MarketplaceReversalOperatorError::Conflict(format!(
                "reversal inbox row {inbox_id} is not safely retryable from operator_review"
            )));
        }
        self.inbox.process(tenant_id, inbox_id).await?;
        self.get_event(tenant_id, inbox_id).await
    }

    pub async fn sweep_tenant(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> MarketplaceReversalOperatorResult<MarketplaceReversalEventSweepReport> {
        if tenant_id.is_nil() {
            return Err(MarketplaceReversalOperatorError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        let now = Utc::now().fixed_offset();
        let recoverable = Condition::any()
            .add(
                marketplace_reversal_event_inbox::Column::Status.is_in([
                    MarketplaceReversalEventStatus::Received.as_str(),
                    MarketplaceReversalEventStatus::RetryableError.as_str(),
                ]),
            )
            .add(
                Condition::all()
                    .add(
                        marketplace_reversal_event_inbox::Column::Status
                            .eq(MarketplaceReversalEventStatus::Processing.as_str()),
                    )
                    .add(marketplace_reversal_event_inbox::Column::LeaseExpiresAt.lte(now)),
            );
        let events = marketplace_reversal_event_inbox::Entity::find()
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(recoverable)
            .order_by_asc(marketplace_reversal_event_inbox::Column::UpdatedAt)
            .order_by_asc(marketplace_reversal_event_inbox::Column::Id)
            .limit(limit.clamp(1, MAX_OPERATOR_ITEMS))
            .all(&self.db)
            .await?;
        let mut report = MarketplaceReversalEventSweepReport {
            selected: events.len(),
            ..Default::default()
        };
        for event in events {
            match self.inbox.process(tenant_id, event.id).await {
                Ok(_) => report.processed += 1,
                Err(error) => {
                    let retryable = error.retryable();
                    if retryable {
                        report.retryable_failures += 1;
                    } else {
                        report.operator_review_failures += 1;
                    }
                    report.failures.push(MarketplaceReversalEventSweepFailure {
                        inbox_id: event.id,
                        retryable,
                        message: error.to_string(),
                    });
                }
            }
        }
        Ok(report)
    }

    async fn get_model(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
    ) -> MarketplaceReversalOperatorResult<marketplace_reversal_event_inbox::Model> {
        marketplace_reversal_event_inbox::Entity::find_by_id(inbox_id)
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                MarketplaceReversalOperatorError::Conflict(format!(
                    "reversal inbox row {inbox_id} was not found"
                ))
            })
    }
}

fn validate_identity(
    tenant_id: Uuid,
    inbox_id: Uuid,
) -> MarketplaceReversalOperatorResult<()> {
    if tenant_id.is_nil() || inbox_id.is_nil() {
        return Err(MarketplaceReversalOperatorError::Validation(
            "tenant_id and inbox_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn map_event(
    model: marketplace_reversal_event_inbox::Model,
) -> MarketplaceReversalOperatorResult<MarketplaceReversalEventOperatorView> {
    let line_count = model
        .lines_json
        .as_array()
        .map(Vec::len)
        .ok_or_else(|| {
            MarketplaceReversalOperatorError::Conflict(format!(
                "reversal inbox row {} has corrupt line evidence",
                model.id
            ))
        })?;
    Ok(MarketplaceReversalEventOperatorView {
        id: model.id,
        tenant_id: model.tenant_id,
        provider_event_id: model.provider_event_id,
        event_source: model.event_source,
        event_id: model.event_id,
        reversal_kind: model.reversal_kind,
        source_id: model.source_id,
        order_id: model.order_id,
        payment_collection_id: model.payment_collection_id,
        occurred_at: model.occurred_at,
        currency_code: model.currency_code,
        currency_exponent: model.currency_exponent,
        total_amount: model.total_amount,
        line_count,
        status: model.status,
        attempt_count: model.attempt_count,
        reversal_id: model.reversal_id,
        ledger_transaction_id: model.ledger_transaction_id,
        last_error_code: model.last_error_code,
        last_error_message: model.last_error_message,
        created_at: model.created_at,
        updated_at: model.updated_at,
        processed_at: model.processed_at,
    })
}
