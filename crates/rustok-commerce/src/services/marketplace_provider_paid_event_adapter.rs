use std::{str::FromStr, sync::Arc};

use rust_decimal::Decimal;
use rustok_marketplace_ledger::MarketplaceLedgerCommandPort;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::{
    PaymentService, PROVIDER_EVENT_PROCESSED, entities::provider_event,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use super::{
    IngestMarketplacePaidEvent, MarketplacePaidEventInboxError,
    MarketplacePaidEventInboxService,
};

const PAYMENT_CAPTURED_EVENT: &str = "payment.captured";

#[derive(Debug, Error)]
pub enum MarketplaceProviderPaidEventAdapterError {
    #[error("payment provider event is not eligible for marketplace financial processing: {0}")]
    Ineligible(String),
    #[error("payment provider event normalized facts are invalid: {0}")]
    Validation(String),
    #[error(transparent)]
    Payment(#[from] rustok_payment::PaymentError),
    #[error(transparent)]
    Inbox(#[from] MarketplacePaidEventInboxError),
}

pub type MarketplaceProviderPaidEventAdapterResult<T> =
    Result<T, MarketplaceProviderPaidEventAdapterError>;

pub struct MarketplaceProviderPaidEventAdapter {
    payment_service: PaymentService,
    inbox: MarketplacePaidEventInboxService,
}

impl MarketplaceProviderPaidEventAdapter {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
    ) -> Self {
        Self {
            payment_service: PaymentService::new(db.clone()),
            inbox: MarketplacePaidEventInboxService::new(db, event_bus, ledger_port),
        }
    }

    /// Admits an already signature-verified and owner-processed payment provider event.
    /// Non-capture events are intentionally ignored; malformed or unprocessed capture
    /// events fail closed before the commerce inbox is touched.
    pub async fn ingest_provider_event(
        &self,
        event: &provider_event::Model,
    ) -> MarketplaceProviderPaidEventAdapterResult<
        Option<crate::entities::marketplace_paid_event_inbox::Model>,
    > {
        if event.event_type.as_deref() != Some(PAYMENT_CAPTURED_EVENT) {
            return Ok(None);
        }
        if !event.signature_verified || event.status != PROVIDER_EVENT_PROCESSED {
            return Err(MarketplaceProviderPaidEventAdapterError::Ineligible(format!(
                "provider event {} must be signature verified and processed, status={}",
                event.id, event.status
            )));
        }
        let metadata = event.event_metadata.as_ref().ok_or_else(|| {
            MarketplaceProviderPaidEventAdapterError::Validation(format!(
                "provider event {} has no normalized metadata",
                event.id
            ))
        })?;
        let collection_id = parse_uuid(metadata, "collection_id")?;
        let normalized_amount = parse_decimal(metadata, "amount")?;
        let normalized_currency = parse_currency(metadata, "currency_code")?;

        let payment = self
            .payment_service
            .get_collection(event.tenant_id, collection_id)
            .await?;
        let checkout = payment
            .metadata
            .get("checkout")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                MarketplaceProviderPaidEventAdapterError::Validation(format!(
                    "payment collection {} has no checkout identity",
                    payment.id
                ))
            })?;
        let checkout_operation_id = checkout
            .get("operation_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| {
                MarketplaceProviderPaidEventAdapterError::Validation(format!(
                    "payment collection {} has no valid checkout operation id",
                    payment.id
                ))
            })?;
        let order_id = payment.order_id.ok_or_else(|| {
            MarketplaceProviderPaidEventAdapterError::Validation(format!(
                "payment collection {} has no order id",
                payment.id
            ))
        })?;
        let captured_at = payment.captured_at.ok_or_else(|| {
            MarketplaceProviderPaidEventAdapterError::Validation(format!(
                "payment collection {} has no captured_at timestamp",
                payment.id
            ))
        })?;
        if payment.status != "captured"
            || payment.captured_amount != normalized_amount
            || !payment
                .currency_code
                .eq_ignore_ascii_case(normalized_currency.as_str())
        {
            return Err(MarketplaceProviderPaidEventAdapterError::Validation(format!(
                "provider event {} does not match authoritative captured payment collection {}",
                event.id, payment.id
            )));
        }

        self.inbox
            .ingest_and_process(IngestMarketplacePaidEvent {
                tenant_id: event.tenant_id,
                event_source: event.provider_id.clone(),
                event_id: event.delivery_id.clone(),
                checkout_operation_id,
                order_id,
                payment_collection_id: payment.id,
                captured_at: captured_at.fixed_offset(),
                currency_code: payment.currency_code,
                captured_amount: payment.captured_amount,
            })
            .await
            .map(Some)
            .map_err(Into::into)
    }

    pub fn inbox(&self) -> &MarketplacePaidEventInboxService {
        &self.inbox
    }
}

fn parse_uuid(
    metadata: &Value,
    field: &str,
) -> MarketplaceProviderPaidEventAdapterResult<Uuid> {
    metadata
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| {
            MarketplaceProviderPaidEventAdapterError::Validation(format!(
                "normalized provider event field `{field}` must be a UUID string"
            ))
        })
}

fn parse_decimal(
    metadata: &Value,
    field: &str,
) -> MarketplaceProviderPaidEventAdapterResult<Decimal> {
    let value = metadata.get(field).and_then(Value::as_str).ok_or_else(|| {
        MarketplaceProviderPaidEventAdapterError::Validation(format!(
            "normalized provider event field `{field}` must be a decimal string"
        ))
    })?;
    let amount = Decimal::from_str(value).map_err(|_| {
        MarketplaceProviderPaidEventAdapterError::Validation(format!(
            "normalized provider event field `{field}` is not a decimal"
        ))
    })?;
    if amount <= Decimal::ZERO {
        return Err(MarketplaceProviderPaidEventAdapterError::Validation(format!(
            "normalized provider event field `{field}` must be positive"
        )));
    }
    Ok(amount)
}

fn parse_currency(
    metadata: &Value,
    field: &str,
) -> MarketplaceProviderPaidEventAdapterResult<String> {
    let value = metadata
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .map(str::to_ascii_uppercase)
        .ok_or_else(|| {
            MarketplaceProviderPaidEventAdapterError::Validation(format!(
                "normalized provider event field `{field}` must be a currency string"
            ))
        })?;
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(MarketplaceProviderPaidEventAdapterError::Validation(format!(
            "normalized provider event field `{field}` must be a three-letter code"
        )));
    }
    Ok(value)
}
