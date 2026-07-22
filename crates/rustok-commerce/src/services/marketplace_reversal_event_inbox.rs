use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, FixedOffset, Utc};
use rustok_api::{PortActor, PortContext};
use rustok_core::generate_id;
use rustok_marketplace::{
    MarketplaceFinancialCommandPort, MarketplaceFinancialOrchestrationError,
    ProcessMarketplaceFinancialReversalInput,
};
use rustok_marketplace_ledger::{
    MarketplaceLedgerReversalKind, MarketplaceLedgerReversalLineInput,
    MarketplaceLedgerReversalResponse, PostMarketplaceLedgerReversalInput,
    MAX_LEDGER_REVERSAL_LINES,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::marketplace_reversal_event_inbox;

const INBOX_LEASE_SECONDS: i64 = 60;
const MAX_SWEEP_ITEMS: u64 = 200;
const ROOT_IDEMPOTENCY_PREFIX: &str = "marketplace-reversal-event";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketplaceReversalEventStatus {
    Received,
    Processing,
    RetryableError,
    OperatorReview,
    Processed,
}

impl MarketplaceReversalEventStatus {
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestMarketplaceReversalEvent {
    pub tenant_id: Uuid,
    pub provider_event_id: Uuid,
    pub event_source: String,
    pub event_id: String,
    pub kind: MarketplaceLedgerReversalKind,
    pub source_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub occurred_at: DateTime<FixedOffset>,
    pub currency_code: String,
    pub currency_exponent: i16,
    pub lines: Vec<MarketplaceLedgerReversalLineInput>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceReversalEventSweepReport {
    pub selected: usize,
    pub processed: usize,
    pub retryable_failures: usize,
    pub operator_review_failures: usize,
    pub failures: Vec<MarketplaceReversalEventSweepFailure>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceReversalEventSweepFailure {
    pub inbox_id: Uuid,
    pub retryable: bool,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum MarketplaceReversalEventInboxError {
    #[error("marketplace reversal event validation failed: {0}")]
    Validation(String),
    #[error("marketplace reversal event conflict: {0}")]
    Conflict(String),
    #[error("marketplace reversal event is busy: {0}")]
    Busy(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Financial(#[from] MarketplaceFinancialOrchestrationError),
}

impl MarketplaceReversalEventInboxError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::Busy(_) | Self::Database(_) => true,
            Self::Financial(error) => error.retryable(),
            Self::Validation(_) | Self::Conflict(_) => false,
        }
    }

    pub fn code(&self) -> String {
        match self {
            Self::Validation(_) => "marketplace_reversal_event.validation".to_string(),
            Self::Conflict(_) => "marketplace_reversal_event.conflict".to_string(),
            Self::Busy(_) => "marketplace_reversal_event.busy".to_string(),
            Self::Database(_) => "marketplace_reversal_event.storage_unavailable".to_string(),
            Self::Financial(MarketplaceFinancialOrchestrationError::Context { code, .. })
            | Self::Financial(MarketplaceFinancialOrchestrationError::Commission { code, .. })
            | Self::Financial(MarketplaceFinancialOrchestrationError::Ledger { code, .. }) => {
                code.clone()
            }
            Self::Financial(MarketplaceFinancialOrchestrationError::Validation(_)) => {
                "marketplace_reversal_event.financial_validation".to_string()
            }
            Self::Financial(MarketplaceFinancialOrchestrationError::Invariant(_)) => {
                "marketplace_reversal_event.financial_invariant".to_string()
            }
        }
    }
}

pub type MarketplaceReversalEventInboxResult<T> =
    Result<T, MarketplaceReversalEventInboxError>;

#[derive(Clone)]
pub struct MarketplaceReversalEventInboxJournal {
    db: DatabaseConnection,
}

impl MarketplaceReversalEventInboxJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn ingest(
        &self,
        input: IngestMarketplaceReversalEvent,
    ) -> MarketplaceReversalEventInboxResult<marketplace_reversal_event_inbox::Model> {
        let input = normalize_input(input)?;
        if let Some(existing) = self.find_existing(&input).await? {
            ensure_same_event(&existing, &input)?;
            return Ok(existing);
        }

        let now = Utc::now().fixed_offset();
        let insert = marketplace_reversal_event_inbox::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(input.tenant_id),
            provider_event_id: Set(input.provider_event_id),
            event_source: Set(input.event_source.clone()),
            event_id: Set(input.event_id.clone()),
            event_hash: Set(input.event_hash.clone()),
            reversal_kind: Set(input.kind.as_str().to_string()),
            source_id: Set(input.source_id),
            order_id: Set(input.order_id),
            payment_collection_id: Set(input.payment_collection_id),
            occurred_at: Set(input.occurred_at),
            currency_code: Set(input.currency_code.clone()),
            currency_exponent: Set(input.currency_exponent),
            total_amount: Set(input.total_amount),
            lines_json: Set(input.lines_json.clone()),
            status: Set(MarketplaceReversalEventStatus::Received.as_str().to_string()),
            attempt_count: Set(0),
            lease_owner: Set(None),
            lease_expires_at: Set(None),
            reversal_id: Set(None),
            ledger_transaction_id: Set(None),
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
                if let Some(existing) = self.find_existing(&input).await? {
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
    ) -> MarketplaceReversalEventInboxResult<marketplace_reversal_event_inbox::Model> {
        marketplace_reversal_event_inbox::Entity::find_by_id(inbox_id)
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                MarketplaceReversalEventInboxError::Conflict(format!(
                    "reversal inbox row {inbox_id} was not found for tenant {tenant_id}"
                ))
            })
    }

    pub async fn find_by_provider_event(
        &self,
        tenant_id: Uuid,
        provider_event_id: Uuid,
    ) -> MarketplaceReversalEventInboxResult<Option<marketplace_reversal_event_inbox::Model>> {
        marketplace_reversal_event_inbox::Entity::find()
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(
                marketplace_reversal_event_inbox::Column::ProviderEventId.eq(provider_event_id),
            )
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn claim(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
        lease_owner: impl Into<String>,
    ) -> MarketplaceReversalEventInboxResult<Option<marketplace_reversal_event_inbox::Model>> {
        let lease_owner = normalize_text(lease_owner.into(), 191, "lease_owner")?;
        let now = Utc::now().fixed_offset();
        let expires_at = now + Duration::seconds(INBOX_LEASE_SECONDS);
        let claimable = Condition::any()
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
        let update = marketplace_reversal_event_inbox::Entity::update_many()
            .col_expr(
                marketplace_reversal_event_inbox::Column::Status,
                Expr::value(MarketplaceReversalEventStatus::Processing.as_str()),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::AttemptCount,
                Expr::col(marketplace_reversal_event_inbox::Column::AttemptCount).add(1),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LeaseOwner,
                Expr::value(Some(lease_owner)),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LeaseExpiresAt,
                Expr::value(Some(expires_at)),
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
        reversal: &MarketplaceLedgerReversalResponse,
    ) -> MarketplaceReversalEventInboxResult<marketplace_reversal_event_inbox::Model> {
        let lease_owner = normalize_text(lease_owner.into(), 191, "lease_owner")?;
        let now = Utc::now().fixed_offset();
        let update = marketplace_reversal_event_inbox::Entity::update_many()
            .col_expr(
                marketplace_reversal_event_inbox::Column::Status,
                Expr::value(MarketplaceReversalEventStatus::Processed.as_str()),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::ReversalId,
                Expr::value(Some(reversal.id)),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LedgerTransactionId,
                Expr::value(Some(reversal.transaction_id)),
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
                marketplace_reversal_event_inbox::Column::ProcessedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(marketplace_reversal_event_inbox::Column::Id.eq(inbox_id))
            .filter(
                marketplace_reversal_event_inbox::Column::Status
                    .eq(MarketplaceReversalEventStatus::Processing.as_str()),
            )
            .filter(marketplace_reversal_event_inbox::Column::LeaseOwner.eq(lease_owner))
            .filter(marketplace_reversal_event_inbox::Column::LeaseExpiresAt.gt(now))
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            let current = self.get(tenant_id, inbox_id).await?;
            if current.status == MarketplaceReversalEventStatus::Processed.as_str()
                && current.reversal_id == Some(reversal.id)
                && current.ledger_transaction_id == Some(reversal.transaction_id)
            {
                return Ok(current);
            }
            return Err(MarketplaceReversalEventInboxError::Conflict(format!(
                "reversal inbox row {inbox_id} lost its processing lease before completion"
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
    ) -> MarketplaceReversalEventInboxResult<marketplace_reversal_event_inbox::Model> {
        let lease_owner = normalize_text(lease_owner.into(), 191, "lease_owner")?;
        let code = normalize_text(code.into(), 100, "error_code")?;
        let message = normalize_text(message.into(), 2000, "error_message")?;
        let status = if retryable {
            MarketplaceReversalEventStatus::RetryableError
        } else {
            MarketplaceReversalEventStatus::OperatorReview
        };
        let now = Utc::now().fixed_offset();
        let update = marketplace_reversal_event_inbox::Entity::update_many()
            .col_expr(
                marketplace_reversal_event_inbox::Column::Status,
                Expr::value(status.as_str()),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LastErrorCode,
                Expr::value(Some(code)),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::LastErrorMessage,
                Expr::value(Some(message)),
            )
            .col_expr(
                marketplace_reversal_event_inbox::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(tenant_id))
            .filter(marketplace_reversal_event_inbox::Column::Id.eq(inbox_id))
            .filter(
                marketplace_reversal_event_inbox::Column::Status
                    .eq(MarketplaceReversalEventStatus::Processing.as_str()),
            )
            .filter(marketplace_reversal_event_inbox::Column::LeaseOwner.eq(lease_owner))
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Err(MarketplaceReversalEventInboxError::Conflict(format!(
                "reversal inbox row {inbox_id} could not persist processing failure"
            )));
        }
        self.get(tenant_id, inbox_id).await
    }

    pub async fn list_recoverable(
        &self,
        limit: u64,
    ) -> MarketplaceReversalEventInboxResult<Vec<marketplace_reversal_event_inbox::Model>> {
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
        marketplace_reversal_event_inbox::Entity::find()
            .filter(recoverable)
            .order_by_asc(marketplace_reversal_event_inbox::Column::UpdatedAt)
            .order_by_asc(marketplace_reversal_event_inbox::Column::Id)
            .limit(limit.clamp(1, MAX_SWEEP_ITEMS))
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    async fn find_existing(
        &self,
        input: &NormalizedReversalEvent,
    ) -> MarketplaceReversalEventInboxResult<Option<marketplace_reversal_event_inbox::Model>> {
        if let Some(model) = marketplace_reversal_event_inbox::Entity::find()
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(input.tenant_id))
            .filter(
                marketplace_reversal_event_inbox::Column::ProviderEventId
                    .eq(input.provider_event_id),
            )
            .one(&self.db)
            .await?
        {
            return Ok(Some(model));
        }
        if let Some(model) = marketplace_reversal_event_inbox::Entity::find()
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(input.tenant_id))
            .filter(
                marketplace_reversal_event_inbox::Column::EventSource.eq(input.event_source.clone()),
            )
            .filter(marketplace_reversal_event_inbox::Column::EventId.eq(input.event_id.clone()))
            .one(&self.db)
            .await?
        {
            return Ok(Some(model));
        }
        marketplace_reversal_event_inbox::Entity::find()
            .filter(marketplace_reversal_event_inbox::Column::TenantId.eq(input.tenant_id))
            .filter(
                marketplace_reversal_event_inbox::Column::ReversalKind.eq(input.kind.as_str()),
            )
            .filter(marketplace_reversal_event_inbox::Column::SourceId.eq(input.source_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }
}

pub struct MarketplaceReversalEventInboxService {
    journal: MarketplaceReversalEventInboxJournal,
    financial_port: Arc<dyn MarketplaceFinancialCommandPort>,
}

impl MarketplaceReversalEventInboxService {
    pub fn new(
        db: DatabaseConnection,
        financial_port: Arc<dyn MarketplaceFinancialCommandPort>,
    ) -> Self {
        Self {
            journal: MarketplaceReversalEventInboxJournal::new(db),
            financial_port,
        }
    }

    pub fn journal(&self) -> &MarketplaceReversalEventInboxJournal {
        &self.journal
    }

    pub async fn ingest_and_process(
        &self,
        input: IngestMarketplaceReversalEvent,
    ) -> MarketplaceReversalEventInboxResult<marketplace_reversal_event_inbox::Model> {
        let event = self.journal.ingest(input).await?;
        self.process(event.tenant_id, event.id).await
    }

    pub async fn process(
        &self,
        tenant_id: Uuid,
        inbox_id: Uuid,
    ) -> MarketplaceReversalEventInboxResult<marketplace_reversal_event_inbox::Model> {
        let current = self.journal.get(tenant_id, inbox_id).await?;
        if current.status == MarketplaceReversalEventStatus::Processed.as_str() {
            return Ok(current);
        }
        if current.status == MarketplaceReversalEventStatus::OperatorReview.as_str() {
            return Err(MarketplaceReversalEventInboxError::Conflict(format!(
                "reversal inbox row {inbox_id} requires operator review"
            )));
        }

        let lease_owner = format!("marketplace-reversal-event:{inbox_id}:{}", Uuid::new_v4());
        let Some(claimed) = self
            .journal
            .claim(tenant_id, inbox_id, lease_owner.as_str())
            .await?
        else {
            let current = self.journal.get(tenant_id, inbox_id).await?;
            if current.status == MarketplaceReversalEventStatus::Processed.as_str() {
                return Ok(current);
            }
            return Err(MarketplaceReversalEventInboxError::Busy(format!(
                "reversal inbox row {inbox_id} is `{}` with lease owner {}",
                current.status,
                current.lease_owner.as_deref().unwrap_or("none")
            )));
        };

        match self.execute(&claimed).await {
            Ok(reversal) => {
                self.journal
                    .mark_processed(tenant_id, inbox_id, lease_owner, &reversal)
                    .await
            }
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
    ) -> MarketplaceReversalEventInboxResult<MarketplaceReversalEventSweepReport> {
        let events = self.journal.list_recoverable(limit).await?;
        let mut report = MarketplaceReversalEventSweepReport {
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

    async fn execute(
        &self,
        event: &marketplace_reversal_event_inbox::Model,
    ) -> MarketplaceReversalEventInboxResult<MarketplaceLedgerReversalResponse> {
        let kind = MarketplaceLedgerReversalKind::parse(event.reversal_kind.as_str()).ok_or_else(
            || {
                MarketplaceReversalEventInboxError::Validation(format!(
                    "unknown reversal kind `{}`",
                    event.reversal_kind
                ))
            },
        )?;
        let lines = serde_json::from_value::<Vec<MarketplaceLedgerReversalLineInput>>(
            event.lines_json.clone(),
        )
        .map_err(|_| {
            MarketplaceReversalEventInboxError::Validation(
                "stored reversal lines are corrupt".to_string(),
            )
        })?;
        let root_key = format!("{ROOT_IDEMPOTENCY_PREFIX}:{}:v1", event.id);
        let mut context = PortContext::new(
            event.tenant_id.to_string(),
            PortActor::service(event.provider_event_id.to_string()),
            "en",
            format!("marketplace-reversal-event:{}", event.id),
        )
        .with_deadline(StdDuration::from_secs(10))
        .with_idempotency_key(root_key);
        context.causation_id = Some(event.provider_event_id.to_string());

        let response = self
            .financial_port
            .process_financial_reversal(
                context,
                ProcessMarketplaceFinancialReversalInput {
                    reversal: PostMarketplaceLedgerReversalInput {
                        kind,
                        source_id: event.source_id,
                        order_id: event.order_id,
                        currency_code: event.currency_code.clone(),
                        reversed_at: event.occurred_at,
                        lines,
                        metadata: serde_json::json!({
                            "payment_provider_event_id": event.provider_event_id,
                            "event_source": event.event_source,
                            "event_id": event.event_id,
                            "payment_collection_id": event.payment_collection_id,
                            "currency_exponent": event.currency_exponent,
                            "normalized_event_hash": event.event_hash,
                        }),
                    },
                },
            )
            .await?;
        let reversal = response.reversal;
        if response.order_id != event.order_id
            || reversal.kind != kind
            || reversal.source_id != event.source_id
            || reversal.order_id != event.order_id
            || reversal.total_amount != event.total_amount
            || reversal.transaction_id != reversal.transaction.id
        {
            return Err(MarketplaceReversalEventInboxError::Conflict(format!(
                "marketplace root returned mismatched reversal evidence for inbox row {}",
                event.id
            )));
        }
        Ok(reversal)
    }
}

#[derive(Clone, Debug)]
struct NormalizedReversalEvent {
    tenant_id: Uuid,
    provider_event_id: Uuid,
    event_source: String,
    event_id: String,
    event_hash: String,
    kind: MarketplaceLedgerReversalKind,
    source_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
    occurred_at: DateTime<FixedOffset>,
    currency_code: String,
    currency_exponent: i16,
    total_amount: i64,
    lines_json: serde_json::Value,
}

fn normalize_input(
    mut input: IngestMarketplaceReversalEvent,
) -> MarketplaceReversalEventInboxResult<NormalizedReversalEvent> {
    if input.tenant_id.is_nil()
        || input.provider_event_id.is_nil()
        || input.source_id.is_nil()
        || input.order_id.is_nil()
        || input.payment_collection_id.is_nil()
    {
        return Err(MarketplaceReversalEventInboxError::Validation(
            "tenant, provider event, source, order, and payment identities must not be nil"
                .to_string(),
        ));
    }
    let event_source = normalize_text(input.event_source, 100, "event_source")?
        .to_ascii_lowercase();
    let event_id = normalize_text(input.event_id, 191, "event_id")?;
    let currency_code = input.currency_code.trim().to_ascii_uppercase();
    if currency_code.len() != 3
        || !currency_code.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return Err(MarketplaceReversalEventInboxError::Validation(
            "currency_code must be a three-letter alphabetic code".to_string(),
        ));
    }
    if !(0..=9).contains(&input.currency_exponent) {
        return Err(MarketplaceReversalEventInboxError::Validation(
            "currency_exponent must be between 0 and 9".to_string(),
        ));
    }
    if input.lines.is_empty() || input.lines.len() > MAX_LEDGER_REVERSAL_LINES {
        return Err(MarketplaceReversalEventInboxError::Validation(format!(
            "reversal lines must contain 1 to {MAX_LEDGER_REVERSAL_LINES} items"
        )));
    }
    input.lines.sort_by_key(|line| {
        (
            line.assessment_id,
            line.allocation_id,
            line.order_line_item_id,
            line.seller_id,
        )
    });
    let total_amount = input.lines.iter().try_fold(0_i64, |total, line| {
        if line.assessment_id.is_nil()
            || line.allocation_id.is_nil()
            || line.order_line_item_id.is_nil()
            || line.seller_id.is_nil()
            || line.commission_amount < 0
            || line.seller_amount < 0
            || line.commission_amount == 0 && line.seller_amount == 0
        {
            return Err(MarketplaceReversalEventInboxError::Validation(
                "reversal lines require non-nil identities and positive economics".to_string(),
            ));
        }
        total
            .checked_add(line.commission_amount)
            .and_then(|value| value.checked_add(line.seller_amount))
            .ok_or_else(|| {
                MarketplaceReversalEventInboxError::Validation(
                    "reversal total overflow".to_string(),
                )
            })
    })?;
    if total_amount <= 0 {
        return Err(MarketplaceReversalEventInboxError::Validation(
            "reversal total must be positive".to_string(),
        ));
    }
    let lines_json = serde_json::to_value(&input.lines).map_err(|_| {
        MarketplaceReversalEventInboxError::Validation(
            "reversal lines could not be serialized".to_string(),
        )
    })?;
    let event_hash = reversal_event_hash(
        input.tenant_id,
        input.provider_event_id,
        event_source.as_str(),
        event_id.as_str(),
        input.kind,
        input.source_id,
        input.order_id,
        input.payment_collection_id,
        input.occurred_at,
        currency_code.as_str(),
        input.currency_exponent,
        total_amount,
        &lines_json,
    )?;
    Ok(NormalizedReversalEvent {
        tenant_id: input.tenant_id,
        provider_event_id: input.provider_event_id,
        event_source,
        event_id,
        event_hash,
        kind: input.kind,
        source_id: input.source_id,
        order_id: input.order_id,
        payment_collection_id: input.payment_collection_id,
        occurred_at: input.occurred_at,
        currency_code,
        currency_exponent: input.currency_exponent,
        total_amount,
        lines_json,
    })
}

fn ensure_same_event(
    existing: &marketplace_reversal_event_inbox::Model,
    input: &NormalizedReversalEvent,
) -> MarketplaceReversalEventInboxResult<()> {
    if existing.event_hash != input.event_hash
        || existing.provider_event_id != input.provider_event_id
        || existing.reversal_kind != input.kind.as_str()
        || existing.source_id != input.source_id
        || existing.order_id != input.order_id
        || existing.payment_collection_id != input.payment_collection_id
        || existing.currency_code != input.currency_code
        || existing.currency_exponent != input.currency_exponent
        || existing.total_amount != input.total_amount
        || existing.lines_json != input.lines_json
    {
        return Err(MarketplaceReversalEventInboxError::Conflict(format!(
            "reversal source identity is already bound to different normalized facts: {}",
            existing.id
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reversal_event_hash(
    tenant_id: Uuid,
    provider_event_id: Uuid,
    event_source: &str,
    event_id: &str,
    kind: MarketplaceLedgerReversalKind,
    source_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
    occurred_at: DateTime<FixedOffset>,
    currency_code: &str,
    currency_exponent: i16,
    total_amount: i64,
    lines_json: &serde_json::Value,
) -> MarketplaceReversalEventInboxResult<String> {
    let payload = serde_json::json!({
        "version": 1,
        "tenant_id": tenant_id,
        "provider_event_id": provider_event_id,
        "event_source": event_source,
        "event_id": event_id,
        "kind": kind,
        "source_id": source_id,
        "order_id": order_id,
        "payment_collection_id": payment_collection_id,
        "occurred_at": occurred_at,
        "currency_code": currency_code,
        "currency_exponent": currency_exponent,
        "total_amount": total_amount,
        "lines": lines_json,
    });
    let encoded = serde_json::to_vec(&payload).map_err(|_| {
        MarketplaceReversalEventInboxError::Validation(
            "normalized reversal facts could not be hashed".to_string(),
        )
    })?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

fn normalize_text(
    value: String,
    max_length: usize,
    field: &str,
) -> MarketplaceReversalEventInboxResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > max_length {
        return Err(MarketplaceReversalEventInboxError::Validation(format!(
            "{field} must contain 1 to {max_length} bytes"
        )));
    }
    Ok(value)
}
