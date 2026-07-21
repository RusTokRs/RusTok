use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use rust_decimal::Decimal;
use rustok_marketplace_ledger::MarketplaceLedgerCommandPort;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{marketplace_financial_operation, marketplace_paid_event_inbox};

use super::{
    MarketplaceFinancialOperationStatus, MarketplacePaidEventInboxError,
    MarketplacePaidEventInboxService, MarketplacePaidEventStatus,
    MarketplacePaidEventSweepFailure, MarketplacePaidEventSweepReport,
};

const MAX_OPERATOR_ITEMS: u64 = 100;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceFinancialOperationOperatorView {
    pub checkout_operation_id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub currency_code: String,
    pub status: String,
    pub stage: String,
    pub attempt_count: i32,
    pub ledger_transaction_id: Option<Uuid>,
    pub ledger_debit_total_amount: Option<i64>,
    pub ledger_credit_total_amount: Option<i64>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub completed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MarketplacePaidEventOperatorView {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub event_source: String,
    pub event_id: String,
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub captured_at: DateTime<FixedOffset>,
    pub currency_code: String,
    pub captured_amount: Decimal,
    pub status: String,
    pub attempt_count: i32,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub processed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Error)]
pub enum MarketplaceFinancialOperatorError {
    #[error("marketplace financial operator request is invalid: {0}")]
    Validation(String),
    #[error("marketplace financial operator conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Inbox(#[from] MarketplacePaidEventInboxError),
}

pub type MarketplaceFinancialOperatorResult<T> =
    Result<T, MarketplaceFinancialOperatorError>;

pub struct MarketplaceFinancialOperatorService {
    db: DatabaseConnection,
    inbox: MarketplacePaidEventInboxService,
}

impl MarketplaceFinancialOperatorService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
    ) -> Self {
        Self {
            inbox: MarketplacePaidEventInboxService::new(
                db.clone(),
                event_bus,
                ledger_port,
            ),
            db,
        }
    }

    pub async fn get_financial_operation(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> MarketplaceFinancialOperatorResult<MarketplaceFinancialOperationOperatorView> {
        self.get_financial_operation_model(tenant_id, checkout_operation_id)
            .await
            .map(map_financial_operation)
    }

    pub async fn get_paid_event(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
    ) -> MarketplaceFinancialOperatorResult<MarketplacePaidEventOperatorView> {
        validate_identity(tenant_id, inbox_id)?;
        marketplace_paid_event_inbox::Entity::find_by_id(inbox_id)
            .filter(marketplace_paid_event_inbox::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .map(map_paid_event)
            .ok_or_else(|| {
                MarketplaceFinancialOperatorError::Conflict(format!(
                    "paid-event inbox row {inbox_id} was not found"
                ))
            })
    }

    pub async fn list_financial_operator_review(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> MarketplaceFinancialOperatorResult<Vec<MarketplaceFinancialOperationOperatorView>> {
        if tenant_id.is_nil() {
            return Err(MarketplaceFinancialOperatorError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        marketplace_financial_operation::Entity::find()
            .filter(marketplace_financial_operation::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_financial_operation::Column::Status
                    .eq(MarketplaceFinancialOperationStatus::OperatorReview.as_str()),
            )
            .order_by_asc(marketplace_financial_operation::Column::UpdatedAt)
            .order_by_asc(marketplace_financial_operation::Column::CheckoutOperationId)
            .limit(limit.clamp(1, MAX_OPERATOR_ITEMS))
            .all(&self.db)
            .await
            .map(|models| models.into_iter().map(map_financial_operation).collect())
            .map_err(Into::into)
    }

    pub async fn list_paid_event_operator_review(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> MarketplaceFinancialOperatorResult<Vec<MarketplacePaidEventOperatorView>> {
        if tenant_id.is_nil() {
            return Err(MarketplaceFinancialOperatorError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        marketplace_paid_event_inbox::Entity::find()
            .filter(marketplace_paid_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_paid_event_inbox::Column::Status
                    .eq(MarketplacePaidEventStatus::OperatorReview.as_str()),
            )
            .order_by_asc(marketplace_paid_event_inbox::Column::UpdatedAt)
            .order_by_asc(marketplace_paid_event_inbox::Column::Id)
            .limit(limit.clamp(1, MAX_OPERATOR_ITEMS))
            .all(&self.db)
            .await
            .map(|models| models.into_iter().map(map_paid_event).collect())
            .map_err(Into::into)
    }

    pub async fn sweep_tenant(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> MarketplaceFinancialOperatorResult<MarketplacePaidEventSweepReport> {
        if tenant_id.is_nil() {
            return Err(MarketplaceFinancialOperatorError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        let now = Utc::now().fixed_offset();
        let recoverable = Condition::any()
            .add(
                marketplace_paid_event_inbox::Column::Status.is_in([
                    MarketplacePaidEventStatus::Received.as_str(),
                    MarketplacePaidEventStatus::RetryableError.as_str(),
                ]),
            )
            .add(
                Condition::all()
                    .add(
                        marketplace_paid_event_inbox::Column::Status
                            .eq(MarketplacePaidEventStatus::Processing.as_str()),
                    )
                    .add(marketplace_paid_event_inbox::Column::LeaseExpiresAt.lte(now)),
            );
        let events = marketplace_paid_event_inbox::Entity::find()
            .filter(marketplace_paid_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(recoverable)
            .order_by_asc(marketplace_paid_event_inbox::Column::UpdatedAt)
            .order_by_asc(marketplace_paid_event_inbox::Column::Id)
            .limit(limit.clamp(1, MAX_OPERATOR_ITEMS))
            .all(&self.db)
            .await?;
        let mut report = MarketplacePaidEventSweepReport {
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
                    report.failures.push(MarketplacePaidEventSweepFailure {
                        inbox_id: event.id,
                        retryable,
                        message: error.to_string(),
                    });
                }
            }
        }
        Ok(report)
    }

    pub async fn retry_financial_operation(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> MarketplaceFinancialOperatorResult<MarketplaceFinancialOperationOperatorView> {
        validate_identity(tenant_id, checkout_operation_id)?;
        let now = Utc::now().fixed_offset();
        let update = marketplace_financial_operation::Entity::update_many()
            .col_expr(
                marketplace_financial_operation::Column::Status,
                Expr::value(MarketplaceFinancialOperationStatus::RetryableError.as_str()),
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
                Expr::value(now),
            )
            .filter(marketplace_financial_operation::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_financial_operation::Column::CheckoutOperationId
                    .eq(checkout_operation_id),
            )
            .filter(
                marketplace_financial_operation::Column::Status
                    .eq(MarketplaceFinancialOperationStatus::OperatorReview.as_str()),
            )
            .filter(marketplace_financial_operation::Column::Stage.eq("admitted"))
            .filter(marketplace_financial_operation::Column::LedgerTransactionId.is_null())
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Err(MarketplaceFinancialOperatorError::Conflict(format!(
                "financial operation {checkout_operation_id} is not safely retryable from operator_review"
            )));
        }
        self.get_financial_operation(tenant_id, checkout_operation_id)
            .await
    }

    pub async fn retry_paid_event(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
    ) -> MarketplaceFinancialOperatorResult<MarketplacePaidEventOperatorView> {
        validate_identity(tenant_id, inbox_id)?;
        let transaction = self.db.begin().await?;
        let event = marketplace_paid_event_inbox::Entity::find_by_id(inbox_id)
            .filter(marketplace_paid_event_inbox::Column::TenantId.eq(tenant_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                MarketplaceFinancialOperatorError::Conflict(format!(
                    "paid-event inbox row {inbox_id} was not found"
                ))
            })?;
        if event.status != MarketplacePaidEventStatus::OperatorReview.as_str() {
            return Err(MarketplaceFinancialOperatorError::Conflict(format!(
                "paid-event inbox row {inbox_id} is `{}` rather than operator_review",
                event.status
            )));
        }

        let now = Utc::now().fixed_offset();
        marketplace_financial_operation::Entity::update_many()
            .col_expr(
                marketplace_financial_operation::Column::Status,
                Expr::value(MarketplaceFinancialOperationStatus::RetryableError.as_str()),
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
                Expr::value(now),
            )
            .filter(marketplace_financial_operation::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_financial_operation::Column::CheckoutOperationId
                    .eq(event.checkout_operation_id),
            )
            .filter(
                marketplace_financial_operation::Column::Status
                    .eq(MarketplaceFinancialOperationStatus::OperatorReview.as_str()),
            )
            .filter(marketplace_financial_operation::Column::Stage.eq("admitted"))
            .filter(marketplace_financial_operation::Column::LedgerTransactionId.is_null())
            .exec(&transaction)
            .await?;

        let inbox_update = marketplace_paid_event_inbox::Entity::update_many()
            .col_expr(
                marketplace_paid_event_inbox::Column::Status,
                Expr::value(MarketplacePaidEventStatus::RetryableError.as_str()),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::LastErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_paid_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(marketplace_paid_event_inbox::Column::Id.eq(inbox_id))
            .filter(
                marketplace_paid_event_inbox::Column::Status
                    .eq(MarketplacePaidEventStatus::OperatorReview.as_str()),
            )
            .exec(&transaction)
            .await?;
        if inbox_update.rows_affected != 1 {
            return Err(MarketplaceFinancialOperatorError::Conflict(format!(
                "paid-event inbox row {inbox_id} could not be reset for retry"
            )));
        }
        transaction.commit().await?;
        self.inbox
            .process(tenant_id, inbox_id)
            .await
            .map(map_paid_event)
            .map_err(Into::into)
    }

    async fn get_financial_operation_model(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> MarketplaceFinancialOperatorResult<marketplace_financial_operation::Model> {
        validate_identity(tenant_id, checkout_operation_id)?;
        marketplace_financial_operation::Entity::find_by_id(checkout_operation_id)
            .filter(marketplace_financial_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                MarketplaceFinancialOperatorError::Conflict(format!(
                    "financial operation {checkout_operation_id} was not found"
                ))
            })
    }
}

fn map_financial_operation(
    model: marketplace_financial_operation::Model,
) -> MarketplaceFinancialOperationOperatorView {
    MarketplaceFinancialOperationOperatorView {
        checkout_operation_id: model.checkout_operation_id,
        tenant_id: model.tenant_id,
        order_id: model.order_id,
        payment_collection_id: model.payment_collection_id,
        currency_code: model.currency_code,
        status: model.status,
        stage: model.stage,
        attempt_count: model.attempt_count,
        ledger_transaction_id: model.ledger_transaction_id,
        ledger_debit_total_amount: model.ledger_debit_total_amount,
        ledger_credit_total_amount: model.ledger_credit_total_amount,
        last_error_code: model.last_error_code,
        last_error_message: model.last_error_message,
        created_at: model.created_at,
        updated_at: model.updated_at,
        completed_at: model.completed_at,
    }
}

fn map_paid_event(model: marketplace_paid_event_inbox::Model) -> MarketplacePaidEventOperatorView {
    MarketplacePaidEventOperatorView {
        id: model.id,
        tenant_id: model.tenant_id,
        event_source: model.event_source,
        event_id: model.event_id,
        checkout_operation_id: model.checkout_operation_id,
        order_id: model.order_id,
        payment_collection_id: model.payment_collection_id,
        captured_at: model.captured_at,
        currency_code: model.currency_code,
        captured_amount: model.captured_amount,
        status: model.status,
        attempt_count: model.attempt_count,
        last_error_code: model.last_error_code,
        last_error_message: model.last_error_message,
        created_at: model.created_at,
        updated_at: model.updated_at,
        processed_at: model.processed_at,
    }
}

fn validate_identity(tenant_id: Uuid, object_id: Uuid) -> MarketplaceFinancialOperatorResult<()> {
    if tenant_id.is_nil() || object_id.is_nil() {
        return Err(MarketplaceFinancialOperatorError::Validation(
            "tenant and object identities must not be nil".to_string(),
        ));
    }
    Ok(())
}
