use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rustok_marketplace_ledger::{
    MarketplaceLedgerReversalKind, MarketplaceLedgerReversalLineInput,
};
use rustok_payment::entities::provider_event;
use rustok_payment::{
    PROVIDER_EVENT_PROCESSED, PaymentProviderEventApplyError, PaymentProviderEventContext,
    PaymentProviderProcessedEventObserver, PaymentProviderWebhookResult, PaymentService,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;
use uuid::Uuid;

use super::{
    IngestMarketplaceReversalEvent, MarketplaceReversalEventInboxError,
    MarketplaceReversalEventInboxService,
};

const REFUND_COMPLETED_EVENT: &str = "refund.completed";
const CHARGEBACK_COMPLETED_EVENT: &str = "chargeback.completed";
const MAX_ADAPT_ITEMS: u64 = 200;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceProviderReversalAdaptReport {
    pub selected: usize,
    pub adapted: usize,
    pub ignored: usize,
    pub failed: usize,
    pub failures: Vec<MarketplaceProviderReversalAdaptFailure>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceProviderReversalAdaptFailure {
    pub provider_event_id: Uuid,
    pub retryable: bool,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum MarketplaceProviderReversalEventAdapterError {
    #[error("payment provider event is not eligible for marketplace reversal processing: {0}")]
    Ineligible(String),
    #[error("payment provider event normalized reversal facts are invalid: {0}")]
    Validation(String),
    #[error(transparent)]
    Payment(#[from] rustok_payment::PaymentError),
    #[error(transparent)]
    Inbox(#[from] MarketplaceReversalEventInboxError),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

impl MarketplaceProviderReversalEventAdapterError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::Payment(rustok_payment::PaymentError::Database(_))
            | Self::Database(_)
            | Self::Inbox(MarketplaceReversalEventInboxError::Database(_))
            | Self::Inbox(MarketplaceReversalEventInboxError::Busy(_)) => true,
            Self::Inbox(error) => error.retryable(),
            Self::Ineligible(_) | Self::Validation(_) | Self::Payment(_) => false,
        }
    }

    pub fn code(&self) -> String {
        match self {
            Self::Ineligible(_) => "marketplace_reversal_adapter.ineligible".to_string(),
            Self::Validation(_) => "marketplace_reversal_adapter.validation".to_string(),
            Self::Payment(rustok_payment::PaymentError::Database(_)) | Self::Database(_) => {
                "marketplace_reversal_adapter.storage_unavailable".to_string()
            }
            Self::Payment(_) => "marketplace_reversal_adapter.payment_conflict".to_string(),
            Self::Inbox(error) => error.code(),
        }
    }
}

pub type MarketplaceProviderReversalEventAdapterResult<T> =
    Result<T, MarketplaceProviderReversalEventAdapterError>;

pub struct MarketplaceProviderReversalEventAdapter {
    db: DatabaseConnection,
    payment_service: PaymentService,
    inbox: MarketplaceReversalEventInboxService,
}

impl MarketplaceProviderReversalEventAdapter {
    pub fn new(
        db: DatabaseConnection,
        financial_port: Arc<dyn rustok_marketplace::MarketplaceFinancialCommandPort>,
    ) -> Self {
        Self {
            payment_service: PaymentService::new(db.clone()),
            inbox: MarketplaceReversalEventInboxService::new(db.clone(), financial_port),
            db,
        }
    }

    pub fn inbox(&self) -> &MarketplaceReversalEventInboxService {
        &self.inbox
    }

    pub async fn ingest_provider_event(
        &self,
        event: &provider_event::Model,
    ) -> MarketplaceProviderReversalEventAdapterResult<
        Option<crate::entities::marketplace_reversal_event_inbox::Model>,
    > {
        let Some(event_type) = event.event_type.as_deref() else {
            return Ok(None);
        };
        if !is_supported_event(event_type) {
            return Ok(None);
        }
        if !event.signature_verified || event.status != PROVIDER_EVENT_PROCESSED {
            return Err(MarketplaceProviderReversalEventAdapterError::Ineligible(
                format!(
                    "provider event {} must be signature verified and processed, status={}",
                    event.id, event.status
                ),
            ));
        }
        let normalized = PaymentProviderWebhookResult {
            provider_id: event.provider_id.clone(),
            delivery_id: event.delivery_id.clone(),
            external_reference: event.external_reference.clone(),
            event_type: event_type.to_string(),
            replay_key: event.idempotency_key.clone(),
            metadata: event.event_metadata.clone().ok_or_else(|| {
                MarketplaceProviderReversalEventAdapterError::Validation(format!(
                    "provider event {} has no normalized metadata",
                    event.id
                ))
            })?,
        };
        self.ingest_normalized(
            PaymentProviderEventContext {
                event_id: event.id,
                tenant_id: event.tenant_id,
                provider_id: event.provider_id.clone(),
                delivery_id: event.delivery_id.clone(),
                idempotency_key: event.idempotency_key.clone(),
            },
            normalized,
        )
        .await
        .map(Some)
    }

    pub async fn adapt_pending(
        &self,
        limit: u64,
    ) -> MarketplaceProviderReversalEventAdapterResult<MarketplaceProviderReversalAdaptReport> {
        let candidates = provider_event::Entity::find()
            .filter(provider_event::Column::Status.eq(PROVIDER_EVENT_PROCESSED))
            .filter(
                provider_event::Column::EventType
                    .is_in([REFUND_COMPLETED_EVENT, CHARGEBACK_COMPLETED_EVENT]),
            )
            .filter(sea_orm::sea_query::Expr::cust(
                "NOT EXISTS (SELECT 1 FROM marketplace_reversal_event_inbox mre WHERE mre.tenant_id = payment_provider_events.tenant_id AND mre.provider_event_id = payment_provider_events.id)",
            ))
            .order_by_asc(provider_event::Column::ProcessedAt)
            .order_by_asc(provider_event::Column::Id)
            .limit(limit.clamp(1, MAX_ADAPT_ITEMS))
            .all(&self.db)
            .await?;
        let mut report = MarketplaceProviderReversalAdaptReport {
            selected: candidates.len(),
            ..Default::default()
        };
        for event in candidates {
            match self.ingest_provider_event(&event).await {
                Ok(Some(_)) => report.adapted += 1,
                Ok(None) => report.ignored += 1,
                Err(error) => {
                    report.failed += 1;
                    report.failures.push(MarketplaceProviderReversalAdaptFailure {
                        provider_event_id: event.id,
                        retryable: error.retryable(),
                        message: error.to_string(),
                    });
                }
            }
        }
        Ok(report)
    }

    async fn ingest_normalized(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> MarketplaceProviderReversalEventAdapterResult<
        crate::entities::marketplace_reversal_event_inbox::Model,
    > {
        if !is_supported_event(event.event_type.as_str()) {
            return Err(MarketplaceProviderReversalEventAdapterError::Ineligible(
                format!("unsupported normalized event `{}`", event.event_type),
            ));
        }
        let metadata = event.metadata.as_object().ok_or_else(|| {
            MarketplaceProviderReversalEventAdapterError::Validation(
                "normalized provider event metadata must be an object".to_string(),
            )
        })?;
        let facts = parse_reversal_facts(metadata)?;
        let amount = parse_decimal(metadata, "amount")?;
        let normalized_currency = parse_currency_map(metadata, "currency_code")?;

        let (kind, source_id, collection_id) = match event.event_type.as_str() {
            REFUND_COMPLETED_EVENT => {
                let refund_id = parse_uuid_map(metadata, "refund_id")?;
                let refund = self
                    .payment_service
                    .get_refund(context.tenant_id, refund_id)
                    .await?;
                if refund.status != "refunded"
                    || refund.amount != amount
                    || !refund
                        .currency_code
                        .eq_ignore_ascii_case(normalized_currency.as_str())
                    || facts.source_id != refund.id
                {
                    return Err(MarketplaceProviderReversalEventAdapterError::Validation(format!(
                        "provider event {} does not match authoritative refunded owner state",
                        context.event_id
                    )));
                }
                (
                    MarketplaceLedgerReversalKind::Refund,
                    refund.id,
                    refund.payment_collection_id,
                )
            }
            CHARGEBACK_COMPLETED_EVENT => {
                let collection_id = parse_uuid_map(metadata, "collection_id")?;
                let chargeback_id = parse_uuid_map(metadata, "chargeback_id")?;
                if facts.source_id != chargeback_id {
                    return Err(MarketplaceProviderReversalEventAdapterError::Validation(
                        "chargeback source identity does not match marketplace reversal facts"
                            .to_string(),
                    ));
                }
                (
                    MarketplaceLedgerReversalKind::Chargeback,
                    chargeback_id,
                    collection_id,
                )
            }
            _ => unreachable!("supported event checked above"),
        };

        let collection = self
            .payment_service
            .get_collection(context.tenant_id, collection_id)
            .await?;
        let order_id = collection.order_id.ok_or_else(|| {
            MarketplaceProviderReversalEventAdapterError::Validation(format!(
                "payment collection {} has no order identity",
                collection.id
            ))
        })?;
        if collection
            .provider_id
            .as_deref()
            .is_some_and(|provider| provider != context.provider_id.as_str())
        {
            return Err(MarketplaceProviderReversalEventAdapterError::Validation(format!(
                "provider event {} belongs to another payment provider",
                context.event_id
            )));
        }
        if order_id != facts.order_id
            || !collection
                .currency_code
                .eq_ignore_ascii_case(normalized_currency.as_str())
            || !facts
                .currency_code
                .eq_ignore_ascii_case(normalized_currency.as_str())
        {
            return Err(MarketplaceProviderReversalEventAdapterError::Validation(format!(
                "provider event {} does not match authoritative order/currency identity",
                context.event_id
            )));
        }
        if kind == MarketplaceLedgerReversalKind::Chargeback
            && (collection.status != "captured" || amount > collection.captured_amount)
        {
            return Err(MarketplaceProviderReversalEventAdapterError::Validation(format!(
                "provider event {} does not match authoritative captured payment state",
                context.event_id
            )));
        }
        let expected_minor = decimal_to_minor_exact(amount, facts.currency_exponent)?;
        let line_total = reversal_line_total(&facts.lines)?;
        if expected_minor != line_total {
            return Err(MarketplaceProviderReversalEventAdapterError::Validation(format!(
                "normalized provider amount {expected_minor} does not match marketplace reversal lines {line_total}"
            )));
        }

        self.inbox
            .ingest_and_process(IngestMarketplaceReversalEvent {
                tenant_id: context.tenant_id,
                provider_event_id: context.event_id,
                event_source: context.provider_id,
                event_id: context.delivery_id,
                kind,
                source_id,
                order_id,
                payment_collection_id: collection.id,
                occurred_at: facts.occurred_at,
                currency_code: facts.currency_code,
                currency_exponent: facts.currency_exponent,
                lines: facts.lines,
            })
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl PaymentProviderProcessedEventObserver for MarketplaceProviderReversalEventAdapter {
    async fn observe(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError> {
        if !is_supported_event(event.event_type.as_str()) {
            return Ok(());
        }
        self.ingest_normalized(context, event)
            .await
            .map(|_| ())
            .map_err(|error| {
                PaymentProviderEventApplyError::new(
                    error.code(),
                    error.to_string(),
                    error.retryable(),
                )
            })
    }
}

#[derive(Clone, Debug)]
struct NormalizedMarketplaceReversalFacts {
    source_id: Uuid,
    order_id: Uuid,
    occurred_at: DateTime<FixedOffset>,
    currency_code: String,
    currency_exponent: i16,
    lines: Vec<MarketplaceLedgerReversalLineInput>,
}

fn parse_reversal_facts(
    metadata: &Map<String, Value>,
) -> MarketplaceProviderReversalEventAdapterResult<NormalizedMarketplaceReversalFacts> {
    let facts = metadata
        .get("marketplace_reversal")
        .or_else(|| {
            metadata
                .get("metadata")
                .and_then(Value::as_object)
                .and_then(|domain| domain.get("marketplace_reversal"))
        })
        .and_then(Value::as_object)
        .ok_or_else(|| {
            MarketplaceProviderReversalEventAdapterError::Validation(
                "normalized provider event requires marketplace_reversal facts".to_string(),
            )
        })?;
    let source_id = parse_uuid_map(facts, "source_id")?;
    let order_id = parse_uuid_map(facts, "order_id")?;
    let occurred_at = DateTime::parse_from_rfc3339(required_string(facts, "occurred_at")?.as_str())
        .map_err(|_| {
            MarketplaceProviderReversalEventAdapterError::Validation(
                "marketplace_reversal.occurred_at must be RFC3339".to_string(),
            )
        })?;
    let currency_code = parse_currency_map(facts, "currency_code")?;
    let currency_exponent = facts
        .get("currency_exponent")
        .and_then(Value::as_i64)
        .and_then(|value| i16::try_from(value).ok())
        .filter(|value| (0..=9).contains(value))
        .ok_or_else(|| {
            MarketplaceProviderReversalEventAdapterError::Validation(
                "marketplace_reversal.currency_exponent must be an integer from 0 to 9"
                    .to_string(),
            )
        })?;
    let lines = serde_json::from_value::<Vec<MarketplaceLedgerReversalLineInput>>(
        facts.get("lines").cloned().ok_or_else(|| {
            MarketplaceProviderReversalEventAdapterError::Validation(
                "marketplace_reversal.lines is required".to_string(),
            )
        })?,
    )
    .map_err(|_| {
        MarketplaceProviderReversalEventAdapterError::Validation(
            "marketplace_reversal.lines is invalid".to_string(),
        )
    })?;
    Ok(NormalizedMarketplaceReversalFacts {
        source_id,
        order_id,
        occurred_at,
        currency_code,
        currency_exponent,
        lines,
    })
}

fn is_supported_event(event_type: &str) -> bool {
    matches!(
        event_type,
        REFUND_COMPLETED_EVENT | CHARGEBACK_COMPLETED_EVENT
    )
}

fn reversal_line_total(
    lines: &[MarketplaceLedgerReversalLineInput],
) -> MarketplaceProviderReversalEventAdapterResult<i64> {
    lines.iter().try_fold(0_i64, |total, line| {
        total
            .checked_add(line.commission_amount)
            .and_then(|value| value.checked_add(line.seller_amount))
            .ok_or_else(|| {
                MarketplaceProviderReversalEventAdapterError::Validation(
                    "marketplace reversal line total overflow".to_string(),
                )
            })
    })
}

fn decimal_to_minor_exact(
    amount: Decimal,
    exponent: i16,
) -> MarketplaceProviderReversalEventAdapterResult<i64> {
    if amount <= Decimal::ZERO {
        return Err(MarketplaceProviderReversalEventAdapterError::Validation(
            "normalized provider amount must be positive".to_string(),
        ));
    }
    let factor = 10_i64
        .checked_pow(u32::try_from(exponent).map_err(|_| {
            MarketplaceProviderReversalEventAdapterError::Validation(
                "currency exponent is invalid".to_string(),
            )
        })?)
        .ok_or_else(|| {
            MarketplaceProviderReversalEventAdapterError::Validation(
                "currency exponent overflows minor-unit conversion".to_string(),
            )
        })?;
    let scaled = amount.checked_mul(Decimal::from(factor)).ok_or_else(|| {
        MarketplaceProviderReversalEventAdapterError::Validation(
            "provider amount overflows minor-unit conversion".to_string(),
        )
    })?;
    if scaled.fract() != Decimal::ZERO {
        return Err(MarketplaceProviderReversalEventAdapterError::Validation(
            "provider amount cannot be represented exactly in currency minor units".to_string(),
        ));
    }
    scaled.to_i64().ok_or_else(|| {
        MarketplaceProviderReversalEventAdapterError::Validation(
            "provider amount exceeds supported minor-unit range".to_string(),
        )
    })
}

fn parse_uuid_map(
    metadata: &Map<String, Value>,
    field: &str,
) -> MarketplaceProviderReversalEventAdapterResult<Uuid> {
    Uuid::parse_str(required_string(metadata, field)?.as_str()).map_err(|_| {
        MarketplaceProviderReversalEventAdapterError::Validation(format!(
            "normalized provider event field `{field}` must be a UUID string"
        ))
    })
}

fn parse_decimal(
    metadata: &Map<String, Value>,
    field: &str,
) -> MarketplaceProviderReversalEventAdapterResult<Decimal> {
    Decimal::from_str(required_string(metadata, field)?.as_str()).map_err(|_| {
        MarketplaceProviderReversalEventAdapterError::Validation(format!(
            "normalized provider event field `{field}` must be a decimal string"
        ))
    })
}

fn parse_currency_map(
    metadata: &Map<String, Value>,
    field: &str,
) -> MarketplaceProviderReversalEventAdapterResult<String> {
    let value = required_string(metadata, field)?.to_ascii_uppercase();
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(MarketplaceProviderReversalEventAdapterError::Validation(format!(
            "normalized provider event field `{field}` must be a three-letter code"
        )));
    }
    Ok(value)
}

fn required_string(
    metadata: &Map<String, Value>,
    field: &str,
) -> MarketplaceProviderReversalEventAdapterResult<String> {
    metadata
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            MarketplaceProviderReversalEventAdapterError::Validation(format!(
                "normalized provider event field `{field}` is required"
            ))
        })
}
