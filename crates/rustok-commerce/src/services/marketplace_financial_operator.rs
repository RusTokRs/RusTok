use std::sync::Arc;

use chrono::Utc;
use rustok_marketplace_ledger::MarketplaceLedgerCommandPort;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait, sea_query::Expr,
};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{marketplace_financial_operation, marketplace_paid_event_inbox};

use super::{
    MarketplaceFinancialOperationStatus, MarketplacePaidEventInboxError,
    MarketplacePaidEventInboxService, MarketplacePaidEventStatus,
};

const MAX_OPERATOR_ITEMS: u64 = 100;

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

    pub async fn list_financial_operator_review(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> MarketplaceFinancialOperatorResult<Vec<marketplace_financial_operation::Model>> {
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
            .map_err(Into::into)
    }

    pub async fn list_paid_event_operator_review(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> MarketplaceFinancialOperatorResult<Vec<marketplace_paid_event_inbox::Model>> {
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
            .map_err(Into::into)
    }

    pub async fn retry_financial_operation(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> MarketplaceFinancialOperatorResult<marketplace_financial_operation::Model> {
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
    ) -> MarketplaceFinancialOperatorResult<marketplace_paid_event_inbox::Model> {
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
            .map_err(Into::into)
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
