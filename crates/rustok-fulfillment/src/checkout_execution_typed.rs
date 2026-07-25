use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use sea_orm::DatabaseConnection;

use crate::checkout_execution::{
    CheckoutFulfillmentExecutionPort, EnsureCheckoutFulfillmentsRequest,
    InProcessCheckoutFulfillmentExecutionPort, ReadCheckoutFulfillmentsRequest,
};
use crate::dto::FulfillmentResponse;
use crate::status::FulfillmentStatusKind;

const MANUAL_RECONCILIATION_CODE: &str =
    "fulfillment.checkout_execution_manual_reconciliation";
const MANUAL_RECONCILIATION_MESSAGE: &str =
    "checkout fulfillment requires manual reconciliation";

/// Mounted in-process fulfillment boundary with fail-closed lifecycle validation.
///
/// The underlying execution adapter still owns persistence and idempotent adoption.
/// This wrapper prevents checkout recovery from accepting cancelled or unknown owner
/// lifecycle states as a successfully-created fulfillment set.
pub struct TypedCheckoutFulfillmentExecutionPort {
    delegate: InProcessCheckoutFulfillmentExecutionPort,
}

impl TypedCheckoutFulfillmentExecutionPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            delegate: InProcessCheckoutFulfillmentExecutionPort::new(db),
        }
    }
}

#[async_trait]
impl CheckoutFulfillmentExecutionPort for TypedCheckoutFulfillmentExecutionPort {
    async fn ensure_checkout_fulfillments(
        &self,
        context: PortContext,
        request: EnsureCheckoutFulfillmentsRequest,
    ) -> Result<Vec<FulfillmentResponse>, PortError> {
        let lifecycle_context = context.clone();
        let fulfillments = self
            .delegate
            .ensure_checkout_fulfillments(context, request)
            .await?;
        validate_checkout_fulfillment_lifecycle(
            &lifecycle_context,
            "ensure_checkout_fulfillments",
            &fulfillments,
        )?;
        Ok(fulfillments)
    }

    async fn read_checkout_fulfillments(
        &self,
        context: PortContext,
        request: ReadCheckoutFulfillmentsRequest,
    ) -> Result<Vec<FulfillmentResponse>, PortError> {
        let lifecycle_context = context.clone();
        let fulfillments = self
            .delegate
            .read_checkout_fulfillments(context, request)
            .await?;
        validate_checkout_fulfillment_lifecycle(
            &lifecycle_context,
            "read_checkout_fulfillments",
            &fulfillments,
        )?;
        Ok(fulfillments)
    }
}

pub fn in_process_checkout_fulfillment_execution_port(
    db: DatabaseConnection,
) -> Arc<dyn CheckoutFulfillmentExecutionPort> {
    Arc::new(TypedCheckoutFulfillmentExecutionPort::new(db))
}

fn validate_checkout_fulfillment_lifecycle(
    context: &PortContext,
    operation: &'static str,
    fulfillments: &[FulfillmentResponse],
) -> Result<(), PortError> {
    for fulfillment in fulfillments {
        match fulfillment.status_kind() {
            FulfillmentStatusKind::Pending
            | FulfillmentStatusKind::Shipped
            | FulfillmentStatusKind::Delivered => {}
            FulfillmentStatusKind::Cancelled => {
                return Err(manual_reconciliation(
                    context,
                    operation,
                    fulfillment,
                    "cancelled_after_payment_capture",
                ));
            }
            FulfillmentStatusKind::Unknown => {
                return Err(manual_reconciliation(
                    context,
                    operation,
                    fulfillment,
                    "unknown_owner_status",
                ));
            }
        }
    }
    Ok(())
}

fn manual_reconciliation(
    context: &PortContext,
    operation: &'static str,
    fulfillment: &FulfillmentResponse,
    cause: &'static str,
) -> PortError {
    tracing::error!(
        owner = "rustok_fulfillment.checkout_execution",
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        channel = ?context.channel,
        operation,
        fulfillment_id = %fulfillment.id,
        order_id = %fulfillment.order_id,
        owner_status = %fulfillment.status,
        cause,
        code = MANUAL_RECONCILIATION_CODE,
        "checkout fulfillment lifecycle requires manual reconciliation"
    );
    PortError::conflict(
        MANUAL_RECONCILIATION_CODE,
        MANUAL_RECONCILIATION_MESSAGE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use std::time::Duration;
    use uuid::Uuid;

    fn context() -> PortContext {
        PortContext::new(
            Uuid::new_v4().to_string(),
            rustok_api::PortActor::service("fulfillment-checkout-lifecycle-test"),
            "en",
            "fulfillment-checkout-lifecycle-test-correlation",
        )
        .with_deadline(Duration::from_secs(2))
    }

    fn fulfillment(status: &str) -> FulfillmentResponse {
        FulfillmentResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            shipping_option_id: None,
            customer_id: None,
            status: status.to_string(),
            carrier: None,
            tracking_number: None,
            delivered_note: None,
            cancellation_reason: None,
            items: Vec::new(),
            metadata: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            shipped_at: None,
            delivered_at: None,
            cancelled_at: None,
        }
    }

    #[test]
    fn checkout_replay_accepts_active_and_completed_fulfillments() {
        let context = context();
        for status in ["pending", "shipped", "delivered"] {
            assert!(
                validate_checkout_fulfillment_lifecycle(
                    &context,
                    "test_checkout_fulfillment_lifecycle",
                    &[fulfillment(status)],
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn checkout_replay_reconciles_cancelled_and_unknown_fulfillments() {
        let context = context();
        for status in ["cancelled", "carrier_custom"] {
            let error = validate_checkout_fulfillment_lifecycle(
                &context,
                "test_checkout_fulfillment_lifecycle",
                &[fulfillment(status)],
            )
            .expect_err("unsafe fulfillment lifecycle must fail closed");
            assert_eq!(error.code, MANUAL_RECONCILIATION_CODE);
            assert_eq!(error.message, MANUAL_RECONCILIATION_MESSAGE);
        }
    }
}
