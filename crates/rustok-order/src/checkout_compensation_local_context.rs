use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::{
    CheckoutOrderCompensationPort, CheckoutOrderCompensationRequest,
    CheckoutOrderCompensationSnapshot, CheckoutOrderIdentityPort, checkout_owner_context,
};

const ORDER_COMPENSATION_OWNER: &str = "rustok_order.checkout_compensation";
const ORDER_COMPENSATION_BOUNDARY: &str = "checkout_order_compensation_port";
const COMPENSATE_OPERATION: &str = "compensate_checkout_order";

pub struct InProcessCheckoutOrderCompensationPort {
    inner: Arc<dyn CheckoutOrderCompensationPort>,
}

impl InProcessCheckoutOrderCompensationPort {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: checkout_owner_context::in_process_checkout_order_compensation_port(
                db, event_bus,
            ),
        }
    }

    pub fn with_identity_port(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        identity_port: Arc<dyn CheckoutOrderIdentityPort>,
    ) -> Self {
        Self {
            inner: Arc::new(
                checkout_owner_context::InProcessCheckoutOrderCompensationPort::with_identity_port(
                    db,
                    event_bus,
                    identity_port,
                ),
            ),
        }
    }
}

#[async_trait]
impl CheckoutOrderCompensationPort for InProcessCheckoutOrderCompensationPort {
    async fn compensate_checkout_order(
        &self,
        context: PortContext,
        request: CheckoutOrderCompensationRequest,
    ) -> Result<Option<CheckoutOrderCompensationSnapshot>, PortError> {
        let diagnostic_context = context.clone();
        let result = self.inner.compensate_checkout_order(context, request).await;
        result.map_err(|error| {
            map_checkout_order_compensation_local_port_error(&diagnostic_context, error)
        })
    }
}

pub fn in_process_checkout_order_compensation_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CheckoutOrderCompensationPort> {
    Arc::new(InProcessCheckoutOrderCompensationPort::new(db, event_bus))
}

fn map_checkout_order_compensation_local_port_error(
    context: &PortContext,
    error: PortError,
) -> PortError {
    let local_operation = match (error.code.as_str(), error.message.as_str()) {
        (
            "order.checkout_compensation_identity_invalid",
            "checkout compensation request is invalid",
        ) => "validate_request",
        (
            "order.checkout_compensation_identity_conflict",
            "checkout order identity conflicts with the compensation request",
        ) => "validate_durable_checkout_identity",
        (
            "order.checkout_compensation_state_conflict",
            "checkout order changed while compensation was being applied",
        ) => "adopt_cancelled_after_transition_race",
        (
            "order.checkout_compensation_manual_reconciliation",
            "checkout requires manual reconciliation",
        ) => "require_manual_reconciliation",
        _ => return error,
    };
    let integrity_failure = matches!(
        local_operation,
        "validate_durable_checkout_identity" | "require_manual_reconciliation"
    );
    if integrity_failure {
        tracing::error!(
            error = ?error,
            owner = ORDER_COMPENSATION_OWNER,
            operation = COMPENSATE_OPERATION,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = ORDER_COMPENSATION_BOUNDARY,
            "order checkout compensation local integrity outcome retained delegated context"
        );
    } else {
        tracing::warn!(
            error = ?error,
            owner = ORDER_COMPENSATION_OWNER,
            operation = COMPENSATE_OPERATION,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = ORDER_COMPENSATION_BOUNDARY,
            "order checkout compensation local outcome retained delegated context"
        );
    }
    error
}
