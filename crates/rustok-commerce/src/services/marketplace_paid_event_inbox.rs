use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset, Utc};
use rust_decimal::Decimal;
use rustok_core::generate_id;
use rustok_marketplace_ledger::MarketplaceLedgerCommandPort;
use rustok_order::{OrderError, OrderResponse, OrderService};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::{PaymentCollectionResponse, PaymentError, PaymentService};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, sea_query::Expr,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::marketplace_paid_event_inbox;

use super::{
    CheckoutMarketplaceFinancialError, CheckoutMarketplaceFinancialStage,
    CheckoutOperationError, CheckoutOperationJournal, CheckoutOperationStage,
    CheckoutOrderPlanError, CheckoutOrderPlanJournal, CheckoutPaymentCapturedState,
};

const INBOX_LEASE_SECONDS: i64 = 60;
const MAX_SWEEP_ITEMS: u64 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketplacePaidEventStatus {
    Received,
    Processing,
    RetryableError,
    OperatorReview,
    Processed,
}

impl MarketplacePaidEventStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Processing => "processing",
            Self::RetryableError => "retryable_error",
            Self::OperatorReview => "operator_review",
            Self::Processed => "processed",
        }
    }
}

#[derive(Clone, Debug)]
pub struct IngestMarketplacePaidEvent {
    pub tenant_id: Uuid,
    pub event_source: String,
    pub event_id: String,
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub captured_at: DateTime<FixedOffset>,
    pub currency_code: String,
    pub captured_amount: Decimal,
}

#[derive(Clone, Debug, Default)]
pub struct MarketplacePaidEventSweepReport {
    pub selected: usize,
    pub processed: usize,
    pub retryable_failures: usize,
    pub operator_review_failures: usize,
    pub failures: Vec<MarketplacePaidEventSweepFailure>,
}

#[derive(Clone, Debug)]
pub struct MarketplacePaidEventSweepFailure {
    pub inbox_id: Uuid,
    pub retryable: bool,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum MarketplacePaidEventInboxError {
    #[error("marketplace paid-event validation failed: {0}")]
    Validation(String),
    #[error("marketplace paid-event is not ready: {0}")]
    NotReady(String),
    #[error("marketplace paid-event conflict: {0}")]
    Conflict(String),
    #[error("marketplace paid-event is busy: {0}")]
    Busy(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    Plan(#[from] CheckoutOrderPlanError),
    #[error(transparent)]
    Order(#[from] OrderError),
    #[error(transparent)]
    Payment(#[from] PaymentError),
    #[error(transparent)]
    Financial(#[from] CheckoutMarketplaceFinancialError),
}

impl MarketplacePaidEventInboxError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::NotReady(_) | Self::Busy(_) | Self::Database(_) => true,
            Self::Operation(CheckoutOperationError::Database(_))
            | Self::Plan(CheckoutOrderPlanError::Database(_))
            | Self::Order(OrderError::Database(_))
            | Self::Payment(PaymentError::Database(_)) => true,
            Self::Operation(CheckoutOperationError::NotFound(_))
            | Self::Plan(CheckoutOrderPlanError::NotFound(_))
            | Self::Order(OrderError::OrderNotFound(_))
            | Self::Payment(PaymentError::PaymentCollectionNotFound(_)) => true,
            Self::Financial(error) => error.retryable(),
            _ => false,
        }
    }

    pub fn code(&self) -> String {
        match self {
            Self::Validation(_) => "marketplace_paid_event.validation".to_string(),
            Self::NotReady(_) => "marketplace_paid_event.not_ready".to_string(),
            Self::Conflict(_) => "marketplace_paid_event.conflict".to_string(),
            Self::Busy(_) => "marketplace_paid_event.busy".to_string(),
            Self::Database(_)
            | Self::Operation(CheckoutOperationError::Database(_))
            | Self::Plan(CheckoutOrderPlanError::Database(_))
            | Self::Order(OrderError::Database(_))
            | Self::Payment(PaymentError::Database(_)) => {
                "marketplace_paid_event.storage_unavailable".to_string()
            }
            Self::Financial(error) => error.code(),
            _ => "marketplace_paid_event.authoritative_state_conflict".to_string(),
        }
    }
}

pub type MarketplacePaidEventInboxResult<T> = Result<T, MarketplacePaidEventInboxError>;

#[derive(Clone)]
pub struct MarketplacePaidEventInboxJournal {
    db: DatabaseConnection,
}

impl MarketplacePaidEventInboxJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn ingest(
        &self,
        input: IngestMarketplacePaidEvent,
    ) -> MarketplacePaidEventInboxResult<marketplace_paid_event_inbox::Model> {
        let input = normalize_input(input)?;
        if let Some(existing) = self
            .find_by_source_event(
                input.tenant_id,
                input.event_source.as_str(),
                input.event_id.as_str(),
            )
            .await?
        {
            ensure_same_event(&existing, &input)?;
            return Ok(existing);
        }

        let now = Utc::now().fixed_offset();
        let insert = marketplace_paid_event_inbox::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(input.tenant_id),
            event_source: Set(input.event_source.clone()),
            event_id: Set(input.event_id.clone()),
            event_hash: Set(input.event_hash.clone()),
            checkout_operation_id: Set(input.checkout_operation_id),
            order_id: Set(input.order_id),
            payment_collection_id: Set(input.payment_collection_id),
            captured_at: Set(input.captured_at),
            currency_code: Set(input.currency_code.clone()),
            captured_amount: Set(input.captured_amount),
            status: Set(MarketplacePaidEventStatus::Received.as_str().to_string()),
            attempt_count: Set(0),
            lease_owner: Set(None),
            lease_expires_at: Set(None),
            last_error_code: Set(None),
            last_error_message: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            processed_at: Set(None),
        }
        .insert(&self.db)
        .await;
        match insert {
            Ok(model) => Ok(model),
            Err(error) => {
                if let Some(existing) = self
                    .find_by_source_event(
                        input.tenant_id,
                        input.event_source.as_str(),
                        input.event_id.as_str(),
                    )
                    .await?
                {
                    ensure_same_event(&existing, &input)?;
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
        inbox_id: Uuid,
    ) -> MarketplacePaidEventInboxResult<marketplace_paid_event_inbox::Model> {
        marketplace_paid_event_inbox::Entity::find_by_id(inbox_id)
            .filter(marketplace_paid_event_inbox::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                MarketplacePaidEventInboxError::Conflict(format!(
                    "inbox row {inbox_id} was not found for tenant {tenant_id}"
                ))
            })
    }

    pub async fn claim(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
        lease_owner: impl Into<String>,
    ) -> MarketplacePaidEventInboxResult<Option<marketplace_paid_event_inbox::Model>> {
        let lease_owner = normalize_text(lease_owner.into(), 191, "lease_owner")?;
        let now = Utc::now().fixed_offset();
        let expires_at = now + Duration::seconds(INBOX_LEASE_SECONDS);
        let claimable = Condition::any()
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
        let update = marketplace_paid_event_inbox::Entity::update_many()
            .col_expr(
                marketplace_paid_event_inbox::Column::Status,
                Expr::value(MarketplacePaidEventStatus::Processing.as_str()),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::AttemptCount,
                Expr::col(marketplace_paid_event_inbox::Column::AttemptCount).add(1),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::LeaseOwner,
                Expr::value(Some(lease_owner)),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::LeaseExpiresAt,
                Expr::value(Some(expires_at)),
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
            .filter(claimable)
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, inbox_id).await.map(Some)
    }

    pub async fn mark_processed(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
        lease_owner: impl Into<String>,
    ) -> MarketplacePaidEventInboxResult<marketplace_paid_event_inbox::Model> {
        let lease_owner = normalize_text(lease_owner.into(), 191, "lease_owner")?;
        let now = Utc::now().fixed_offset();
        let update = marketplace_paid_event_inbox::Entity::update_many()
            .col_expr(
                marketplace_paid_event_inbox::Column::Status,
                Expr::value(MarketplacePaidEventStatus::Processed.as_str()),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
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
                marketplace_paid_event_inbox::Column::ProcessedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_paid_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(marketplace_paid_event_inbox::Column::Id.eq(inbox_id))
            .filter(
                marketplace_paid_event_inbox::Column::Status
                    .eq(MarketplacePaidEventStatus::Processing.as_str()),
            )
            .filter(marketplace_paid_event_inbox::Column::LeaseOwner.eq(lease_owner))
            .filter(marketplace_paid_event_inbox::Column::LeaseExpiresAt.gt(now))
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            let current = self.get(tenant_id, inbox_id).await?;
            if current.status == MarketplacePaidEventStatus::Processed.as_str() {
                return Ok(current);
            }
            return Err(MarketplacePaidEventInboxError::Conflict(format!(
                "inbox row {inbox_id} lost its processing lease before completion"
            )));
        }
        self.get(tenant_id, inbox_id).await
    }

    pub async fn mark_failure(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
        lease_owner: impl Into<String>,
        retryable: bool,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> MarketplacePaidEventInboxResult<marketplace_paid_event_inbox::Model> {
        let lease_owner = normalize_text(lease_owner.into(), 191, "lease_owner")?;
        let code = normalize_text(code.into(), 100, "error_code")?;
        let message = normalize_text(message.into(), 2000, "error_message")?;
        let now = Utc::now().fixed_offset();
        let status = if retryable {
            MarketplacePaidEventStatus::RetryableError
        } else {
            MarketplacePaidEventStatus::OperatorReview
        };
        let update = marketplace_paid_event_inbox::Entity::update_many()
            .col_expr(
                marketplace_paid_event_inbox::Column::Status,
                Expr::value(status.as_str()),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::LastErrorCode,
                Expr::value(Some(code)),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::LastErrorMessage,
                Expr::value(Some(message)),
            )
            .col_expr(
                marketplace_paid_event_inbox::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_paid_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(marketplace_paid_event_inbox::Column::Id.eq(inbox_id))
            .filter(
                marketplace_paid_event_inbox::Column::Status
                    .eq(MarketplacePaidEventStatus::Processing.as_str()),
            )
            .filter(marketplace_paid_event_inbox::Column::LeaseOwner.eq(lease_owner))
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Err(MarketplacePaidEventInboxError::Conflict(format!(
                "inbox row {inbox_id} could not persist processing failure"
            )));
        }
        self.get(tenant_id, inbox_id).await
    }

    pub async fn list_recoverable(
        &self,
        limit: u64,
    ) -> MarketplacePaidEventInboxResult<Vec<marketplace_paid_event_inbox::Model>> {
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
        marketplace_paid_event_inbox::Entity::find()
            .filter(recoverable)
            .order_by_asc(marketplace_paid_event_inbox::Column::UpdatedAt)
            .order_by_asc(marketplace_paid_event_inbox::Column::Id)
            .limit(limit.clamp(1, MAX_SWEEP_ITEMS))
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    async fn find_by_source_event(
        &self,
        tenant_id: Uuid,
        event_source: &str,
        event_id: &str,
    ) -> MarketplacePaidEventInboxResult<Option<marketplace_paid_event_inbox::Model>> {
        marketplace_paid_event_inbox::Entity::find()
            .filter(marketplace_paid_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(marketplace_paid_event_inbox::Column::EventSource.eq(event_source))
            .filter(marketplace_paid_event_inbox::Column::EventId.eq(event_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }
}

pub struct MarketplacePaidEventInboxService {
    journal: MarketplacePaidEventInboxJournal,
    operation_journal: CheckoutOperationJournal,
    plan_journal: CheckoutOrderPlanJournal,
    order_service: OrderService,
    payment_service: PaymentService,
    financial_stage: CheckoutMarketplaceFinancialStage,
}

impl MarketplacePaidEventInboxService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
    ) -> Self {
        Self {
            journal: MarketplacePaidEventInboxJournal::new(db.clone()),
            operation_journal: CheckoutOperationJournal::new(db.clone()),
            plan_journal: CheckoutOrderPlanJournal::new(db.clone()),
            order_service: OrderService::new(db.clone(), event_bus),
            payment_service: PaymentService::new(db.clone()),
            financial_stage: CheckoutMarketplaceFinancialStage::new(db, ledger_port),
        }
    }

    pub async fn ingest_and_process(
        &self,
        input: IngestMarketplacePaidEvent,
    ) -> MarketplacePaidEventInboxResult<marketplace_paid_event_inbox::Model> {
        let event = self.journal.ingest(input).await?;
        self.process(event.tenant_id, event.id).await
    }

    pub async fn process(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
    ) -> MarketplacePaidEventInboxResult<marketplace_paid_event_inbox::Model> {
        let current = self.journal.get(tenant_id, inbox_id).await?;
        if current.status == MarketplacePaidEventStatus::Processed.as_str() {
            return Ok(current);
        }
        if current.status == MarketplacePaidEventStatus::OperatorReview.as_str() {
            return Err(MarketplacePaidEventInboxError::Conflict(format!(
                "inbox row {inbox_id} requires operator review"
            )));
        }

        let lease_owner = format!("marketplace-paid-event:{inbox_id}:{}", Uuid::new_v4());
        let Some(claimed) = self
            .journal
            .claim(tenant_id, inbox_id, lease_owner.as_str())
            .await?
        else {
            let current = self.journal.get(tenant_id, inbox_id).await?;
            if current.status == MarketplacePaidEventStatus::Processed.as_str() {
                return Ok(current);
            }
            return Err(MarketplacePaidEventInboxError::Busy(format!(
                "inbox row {inbox_id} is status `{}` with lease owner {}",
                current.status,
                current.lease_owner.as_deref().unwrap_or("none")
            )));
        };

        match self
            .load_and_post(tenant_id, lease_owner.as_str(), &claimed)
            .await
        {
            Ok(()) => self
                .journal
                .mark_processed(tenant_id, inbox_id, lease_owner)
                .await,
            Err(error) => {
                let retryable = error.retryable();
                let code = error.code();
                let message = error.to_string();
                self.journal
                    .mark_failure(
                        tenant_id,
                        inbox_id,
                        lease_owner,
                        retryable,
                        code,
                        message,
                    )
                    .await?;
                Err(error)
            }
        }
    }

    pub async fn sweep(
        &self,
        limit: u64,
    ) -> MarketplacePaidEventInboxResult<MarketplacePaidEventSweepReport> {
        let events = self.journal.list_recoverable(limit).await?;
        let mut report = MarketplacePaidEventSweepReport {
            selected: events.len(),
            ..Default::default()
        };
        for event in events {
            match self.process(event.tenant_id, event.id).await {
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

    async fn load_and_post(
        &self,
        tenant_id: Uuid,
        lease_owner: &str,
        event: &marketplace_paid_event_inbox::Model,
    ) -> MarketplacePaidEventInboxResult<()> {
        let operation = self
            .operation_journal
            .get(tenant_id, event.checkout_operation_id)
            .await?;
        if operation.order_id != Some(event.order_id)
            || operation.payment_collection_id != Some(event.payment_collection_id)
        {
            return Err(MarketplacePaidEventInboxError::Conflict(format!(
                "paid event {} does not match checkout operation order/payment identity",
                event.id
            )));
        }
        if !matches!(
            operation.stage.as_str(),
            stage if stage == CheckoutOperationStage::PaymentCaptured.as_str()
                || stage == CheckoutOperationStage::FulfillmentCreated.as_str()
                || stage == CheckoutOperationStage::CartCompleted.as_str()
                || stage == CheckoutOperationStage::Completed.as_str()
        ) {
            return Err(MarketplacePaidEventInboxError::NotReady(format!(
                "checkout operation {} is still at `{}`",
                operation.id, operation.stage
            )));
        }
        if matches!(operation.status.as_str(), "failed" | "compensated") {
            return Err(MarketplacePaidEventInboxError::Conflict(format!(
                "checkout operation {} is terminal with status `{}`",
                operation.id, operation.status
            )));
        }

        let plan = self
            .plan_journal
            .get(tenant_id, event.checkout_operation_id)
            .await?;
        let order = self
            .order_service
            .get_order_with_locale_fallback(
                tenant_id,
                event.order_id,
                plan.payload.context.locale.as_str(),
                Some(plan.payload.context.default_locale.as_str()),
            )
            .await?;
        let payment = self
            .payment_service
            .get_collection(tenant_id, event.payment_collection_id)
            .await?;
        validate_authoritative_state(event, &operation, &plan, &order, &payment)?;

        self.financial_stage
            .post_after_capture_if_present(
                tenant_id,
                event.checkout_operation_id,
                lease_owner,
                &CheckoutPaymentCapturedState {
                    operation_id: event.checkout_operation_id,
                    order,
                    plan,
                    payment_collection: payment,
                },
            )
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
struct NormalizedPaidEvent {
    tenant_id: Uuid,
    event_source: String,
    event_id: String,
    event_hash: String,
    checkout_operation_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
    captured_at: DateTime<FixedOffset>,
    currency_code: String,
    captured_amount: Decimal,
}

fn normalize_input(
    input: IngestMarketplacePaidEvent,
) -> MarketplacePaidEventInboxResult<NormalizedPaidEvent> {
    if input.tenant_id.is_nil()
        || input.checkout_operation_id.is_nil()
        || input.order_id.is_nil()
        || input.payment_collection_id.is_nil()
    {
        return Err(MarketplacePaidEventInboxError::Validation(
            "tenant, checkout operation, order, and payment identities must not be nil".to_string(),
        ));
    }
    let event_source = normalize_text(input.event_source, 100, "event_source")?
        .to_ascii_lowercase();
    let event_id = normalize_text(input.event_id, 191, "event_id")?;
    let currency_code = input.currency_code.trim().to_ascii_uppercase();
    if currency_code.len() != 3
        || !currency_code.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return Err(MarketplacePaidEventInboxError::Validation(
            "currency_code must be a three-letter alphabetic code".to_string(),
        ));
    }
    if input.captured_amount <= Decimal::ZERO {
        return Err(MarketplacePaidEventInboxError::Validation(
            "captured_amount must be positive".to_string(),
        ));
    }
    let event_hash = paid_event_hash(
        input.tenant_id,
        event_source.as_str(),
        event_id.as_str(),
        input.checkout_operation_id,
        input.order_id,
        input.payment_collection_id,
        input.captured_at,
        currency_code.as_str(),
        input.captured_amount,
    );
    Ok(NormalizedPaidEvent {
        tenant_id: input.tenant_id,
        event_source,
        event_id,
        event_hash,
        checkout_operation_id: input.checkout_operation_id,
        order_id: input.order_id,
        payment_collection_id: input.payment_collection_id,
        captured_at: input.captured_at,
        currency_code,
        captured_amount: input.captured_amount,
    })
}

fn ensure_same_event(
    existing: &marketplace_paid_event_inbox::Model,
    input: &NormalizedPaidEvent,
) -> MarketplacePaidEventInboxResult<()> {
    if existing.event_hash != input.event_hash
        || existing.checkout_operation_id != input.checkout_operation_id
        || existing.order_id != input.order_id
        || existing.payment_collection_id != input.payment_collection_id
        || existing.captured_at != input.captured_at
        || existing.currency_code != input.currency_code
        || existing.captured_amount != input.captured_amount
    {
        return Err(MarketplacePaidEventInboxError::Conflict(format!(
            "event `{}/{}` is already bound to different normalized payment facts",
            input.event_source, input.event_id
        )));
    }
    Ok(())
}

fn validate_authoritative_state(
    event: &marketplace_paid_event_inbox::Model,
    operation: &crate::entities::checkout_operation::Model,
    plan: &super::CheckoutOrderPlanRecord,
    order: &OrderResponse,
    payment: &PaymentCollectionResponse,
) -> MarketplacePaidEventInboxResult<()> {
    let operation_id = event.checkout_operation_id.to_string();
    let order_operation_id = order
        .metadata
        .get("checkout")
        .and_then(|value| value.get("operation_id"))
        .and_then(Value::as_str);
    let payment_checkout = payment.metadata.get("checkout");
    if operation.tenant_id != event.tenant_id
        || plan.tenant_id != event.tenant_id
        || plan.checkout_operation_id != event.checkout_operation_id
        || order.tenant_id != event.tenant_id
        || order.id != event.order_id
        || order_operation_id != Some(operation_id.as_str())
        || payment.tenant_id != event.tenant_id
        || payment.id != event.payment_collection_id
        || payment.order_id != Some(event.order_id)
        || payment.status != "captured"
        || payment.captured_at.map(|value| value.fixed_offset()) != Some(event.captured_at)
        || !payment
            .currency_code
            .eq_ignore_ascii_case(event.currency_code.as_str())
        || payment.captured_amount != event.captured_amount
        || payment_checkout
            .and_then(|value| value.get("operation_id"))
            .and_then(Value::as_str)
            != Some(operation_id.as_str())
        || payment_checkout
            .and_then(|value| value.get("order_plan_hash"))
            .and_then(Value::as_str)
            != Some(plan.plan_hash.as_str())
    {
        return Err(MarketplacePaidEventInboxError::Conflict(format!(
            "paid event {} does not match authoritative checkout, order, plan, or payment state",
            event.id
        )));
    }
    Ok(())
}

fn normalize_text(
    value: String,
    max_len: usize,
    field: &str,
) -> MarketplacePaidEventInboxResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > max_len {
        return Err(MarketplacePaidEventInboxError::Validation(format!(
            "{field} must contain 1 to {max_len} bytes"
        )));
    }
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
fn paid_event_hash(
    tenant_id: Uuid,
    event_source: &str,
    event_id: &str,
    checkout_operation_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
    captured_at: DateTime<FixedOffset>,
    currency_code: &str,
    captured_amount: Decimal,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        tenant_id.to_string(),
        event_source.to_string(),
        event_id.to_string(),
        checkout_operation_id.to_string(),
        order_id.to_string(),
        payment_collection_id.to_string(),
        captured_at.timestamp_micros().to_string(),
        currency_code.to_string(),
        captured_amount.normalize().to_string(),
    ] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}
