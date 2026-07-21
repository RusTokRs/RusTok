use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_marketplace_allocation::{
    ListMarketplaceAllocationsByOrderRequest, MarketplaceAllocationReadPort,
};
use rustok_payment::{
    PaymentError, PaymentProviderEventApplyError, PaymentProviderEventContext,
    PaymentProviderProcessedEventObserver, PaymentProviderWebhookResult, PaymentService,
};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::MarketplaceProviderReversalEventAdapter;

const REFUND_COMPLETED_EVENT: &str = "refund.completed";
const CHARGEBACK_COMPLETED_EVENT: &str = "chargeback.completed";
const ASSOCIATION_READ_DEADLINE: Duration = Duration::from_secs(3);
const MISSING_FACTS_CODE: &str = "marketplace_reversal_adapter.marketplace_facts_missing";
const MISSING_FACTS_MESSAGE: &str =
    "Marketplace reversal facts are missing for a marketplace-associated payment event";

/// Guards the live payment-provider observer path against silently accepting a
/// marketplace refund or chargeback without typed marketplace reversal facts.
///
/// Payment owner lifecycle application runs before this observer. This guard
/// resolves the authoritative order through payment owner state, asks the
/// allocation owner whether the order belongs to the Marketplace family, and
/// fails the payment provider event closed when required facts are absent.
pub struct MarketplaceReversalFactGuardObserver {
    payment_service: PaymentService,
    allocation_reader: Arc<dyn MarketplaceAllocationReadPort>,
    delegate: MarketplaceProviderReversalEventAdapter,
}

impl MarketplaceReversalFactGuardObserver {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        allocation_reader: Arc<dyn MarketplaceAllocationReadPort>,
        delegate: MarketplaceProviderReversalEventAdapter,
    ) -> Self {
        Self {
            payment_service: PaymentService::new(db),
            allocation_reader,
            delegate,
        }
    }

    async fn order_has_marketplace_allocations(
        &self,
        context: &PaymentProviderEventContext,
        event: &PaymentProviderWebhookResult,
    ) -> Result<bool, PaymentProviderEventApplyError> {
        let order_id = self.resolve_authoritative_order_id(context, event).await?;
        let port_context = PortContext::new(
            context.tenant_id.to_string(),
            PortActor::service("commerce.marketplace-reversal-fact-guard"),
            "und",
            format!("marketplace-reversal-facts:{}", context.event_id),
        )
        .with_causation_id(context.event_id.to_string())
        .with_deadline(ASSOCIATION_READ_DEADLINE);
        let allocations = self
            .allocation_reader
            .list_allocations_by_order(
                port_context,
                ListMarketplaceAllocationsByOrderRequest { order_id },
            )
            .await
            .map_err(map_allocation_error)?;
        Ok(!allocations.is_empty())
    }

    async fn resolve_authoritative_order_id(
        &self,
        context: &PaymentProviderEventContext,
        event: &PaymentProviderWebhookResult,
    ) -> Result<Uuid, PaymentProviderEventApplyError> {
        let metadata = event.metadata.as_object().ok_or_else(|| {
            non_retryable(
                "marketplace_reversal_adapter.metadata_invalid",
                "Normalized payment provider event metadata must be an object",
            )
        })?;
        let collection_id = match event.event_type.as_str() {
            REFUND_COMPLETED_EVENT => {
                let refund_id = parse_uuid(metadata, "refund_id")?;
                self.payment_service
                    .get_refund(context.tenant_id, refund_id)
                    .await
                    .map_err(map_payment_error)?
                    .payment_collection_id
            }
            CHARGEBACK_COMPLETED_EVENT => parse_uuid(metadata, "collection_id")?,
            _ => {
                return Err(non_retryable(
                    "marketplace_reversal_adapter.event_unsupported",
                    "Payment provider event is not eligible for marketplace reversal adaptation",
                ));
            }
        };
        let collection = self
            .payment_service
            .get_collection(context.tenant_id, collection_id)
            .await
            .map_err(map_payment_error)?;
        if collection
            .provider_id
            .as_deref()
            .is_some_and(|provider_id| provider_id != context.provider_id)
        {
            return Err(non_retryable(
                "marketplace_reversal_adapter.provider_mismatch",
                "Payment provider event belongs to another payment provider",
            ));
        }
        collection.order_id.ok_or_else(|| {
            non_retryable(
                "marketplace_reversal_adapter.order_identity_missing",
                "Payment collection has no authoritative order identity",
            )
        })
    }
}

#[async_trait]
impl PaymentProviderProcessedEventObserver for MarketplaceReversalFactGuardObserver {
    async fn observe(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError> {
        if !is_supported_event(event.event_type.as_str())
            || has_marketplace_reversal(&event.metadata)
        {
            return self.delegate.observe(context, event).await;
        }

        if !self
            .order_has_marketplace_allocations(&context, &event)
            .await?
        {
            return Ok(());
        }

        tracing::warn!(
            provider_event_id = %context.event_id,
            tenant_id = %context.tenant_id,
            provider_id = %context.provider_id,
            delivery_id = %context.delivery_id,
            event_type = %event.event_type,
            "marketplace-associated reversal event is missing typed marketplace facts"
        );
        Err(non_retryable(MISSING_FACTS_CODE, MISSING_FACTS_MESSAGE))
    }
}

fn is_supported_event(event_type: &str) -> bool {
    matches!(
        event_type,
        REFUND_COMPLETED_EVENT | CHARGEBACK_COMPLETED_EVENT
    )
}

fn has_marketplace_reversal(metadata: &Value) -> bool {
    metadata.as_object().is_some_and(|object| {
        object.contains_key("marketplace_reversal")
            || object
                .get("metadata")
                .and_then(Value::as_object)
                .is_some_and(|domain| domain.contains_key("marketplace_reversal"))
    })
}

fn parse_uuid(
    metadata: &Map<String, Value>,
    field: &str,
) -> Result<Uuid, PaymentProviderEventApplyError> {
    let value = metadata
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            non_retryable(
                "marketplace_reversal_adapter.identity_missing",
                format!("Normalized payment provider event requires `{field}`"),
            )
        })?;
    Uuid::parse_str(value).map_err(|_| {
        non_retryable(
            "marketplace_reversal_adapter.identity_invalid",
            format!("Normalized payment provider event `{field}` must be a UUID"),
        )
    })
}

fn map_payment_error(error: PaymentError) -> PaymentProviderEventApplyError {
    match error {
        PaymentError::Database(_) | PaymentError::ProviderUnavailable { .. } => retryable(
            "marketplace_reversal_adapter.payment_owner_unavailable",
            "Payment owner is temporarily unavailable while resolving marketplace association",
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => retryable(
            "marketplace_reversal_adapter.payment_owner_record_missing",
            "Payment owner record is not available yet",
        ),
        PaymentError::Validation(_)
        | PaymentError::InvalidTransition { .. }
        | PaymentError::ProviderRejected { .. }
        | PaymentError::ProviderInvalidResponse { .. }
        | PaymentError::ProviderOutcomeUnknown { .. }
        | PaymentError::ProviderConfiguration { .. } => non_retryable(
            "marketplace_reversal_adapter.payment_owner_conflict",
            "Payment owner could not confirm marketplace reversal association",
        ),
    }
}

fn map_allocation_error(error: PortError) -> PaymentProviderEventApplyError {
    if error.retryable {
        retryable(
            "marketplace_reversal_adapter.allocation_owner_unavailable",
            "Marketplace allocation owner is temporarily unavailable",
        )
    } else {
        non_retryable(
            "marketplace_reversal_adapter.allocation_owner_conflict",
            "Marketplace allocation owner could not confirm order association",
        )
    }
}

fn retryable(
    code: impl Into<String>,
    message: impl Into<String>,
) -> PaymentProviderEventApplyError {
    PaymentProviderEventApplyError::new(code, message, true)
}

fn non_retryable(
    code: impl Into<String>,
    message: impl Into<String>,
) -> PaymentProviderEventApplyError {
    PaymentProviderEventApplyError::new(code, message, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marketplace_reversal_fact_detection_accepts_both_normalized_locations() {
        assert!(has_marketplace_reversal(&serde_json::json!({
            "marketplace_reversal": {}
        })));
        assert!(has_marketplace_reversal(&serde_json::json!({
            "metadata": {"marketplace_reversal": {}}
        })));
        assert!(!has_marketplace_reversal(&serde_json::json!({
            "metadata": {}
        })));
    }

    #[test]
    fn missing_facts_error_is_stable_and_non_retryable() {
        let error = non_retryable(MISSING_FACTS_CODE, MISSING_FACTS_MESSAGE);
        assert_eq!(error.code, MISSING_FACTS_CODE);
        assert_eq!(error.message, MISSING_FACTS_MESSAGE);
        assert!(!error.retryable);
    }
}
