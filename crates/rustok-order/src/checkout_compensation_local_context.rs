use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::{
    CheckoutOrderCompensationPort, CheckoutOrderCompensationRequest,
    CheckoutOrderCompensationSnapshot, CheckoutOrderIdentityPort, checkout_owner_context_impl,
};

const ORDER_COMPENSATION_OWNER: &str = "rustok_order.checkout_compensation";
const ORDER_COMPENSATION_BOUNDARY: &str = "checkout_order_compensation_port";
const COMPENSATE_OPERATION: &str = "compensate_checkout_order";

struct OrderCompensationContextFacts {
    tenant_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
    channel_present: bool,
    channel_length: Option<usize>,
    locale_length: usize,
    causation_id_present: bool,
    causation_id_length: Option<usize>,
    traceparent_present: bool,
    traceparent_length: Option<usize>,
    idempotency_key_present: bool,
    idempotency_key_length: Option<usize>,
    deadline_ms: Option<u64>,
}

struct OrderCompensationRequestFacts {
    checkout_operation_id_non_nil: bool,
    cart_id_non_nil: bool,
    expected_order_id_present: bool,
    expected_order_id_non_nil: Option<bool>,
    reason_present: bool,
    reason_length: Option<usize>,
}

struct OrderCompensationPortErrorFacts {
    error_kind: &'static str,
    message_present: bool,
    message_length: usize,
}

pub struct InProcessCheckoutOrderCompensationPort {
    inner: Arc<dyn CheckoutOrderCompensationPort>,
}

impl InProcessCheckoutOrderCompensationPort {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: checkout_owner_context_impl::in_process_checkout_order_compensation_port(
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
                checkout_owner_context_impl::InProcessCheckoutOrderCompensationPort::with_identity_port(
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
        let diagnostic_facts = order_compensation_request_facts(&request);
        let result = self.inner.compensate_checkout_order(context, request).await;
        result.map_err(|error| {
            map_checkout_order_compensation_local_port_error(
                &diagnostic_context,
                &diagnostic_facts,
                error,
            )
        })
    }
}

pub fn in_process_checkout_order_compensation_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CheckoutOrderCompensationPort> {
    Arc::new(InProcessCheckoutOrderCompensationPort::new(db, event_bus))
}

fn order_compensation_context_facts(context: &PortContext) -> OrderCompensationContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    OrderCompensationContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
        channel_present: context.channel.is_some(),
        channel_length: context.channel.as_ref().map(|value| value.chars().count()),
        locale_length: context.locale.chars().count(),
        causation_id_present: context.causation_id.is_some(),
        causation_id_length: context
            .causation_id
            .as_ref()
            .map(|value| value.chars().count()),
        traceparent_present: context.traceparent.is_some(),
        traceparent_length: context
            .traceparent
            .as_ref()
            .map(|value| value.chars().count()),
        idempotency_key_present: context.idempotency_key.is_some(),
        idempotency_key_length: context
            .idempotency_key
            .as_ref()
            .map(|value| value.chars().count()),
        deadline_ms: context.deadline_ms,
    }
}

fn order_compensation_request_facts(
    request: &CheckoutOrderCompensationRequest,
) -> OrderCompensationRequestFacts {
    OrderCompensationRequestFacts {
        checkout_operation_id_non_nil: !request.checkout_operation_id.is_nil(),
        cart_id_non_nil: !request.cart_id.is_nil(),
        expected_order_id_present: request.expected_order_id.is_some(),
        expected_order_id_non_nil: request.expected_order_id.map(|value| !value.is_nil()),
        reason_present: request.reason.is_some(),
        reason_length: request.reason.as_ref().map(|value| value.chars().count()),
    }
}

fn order_compensation_port_error_facts(error: &PortError) -> OrderCompensationPortErrorFacts {
    let error_kind = match &error.kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    };
    OrderCompensationPortErrorFacts {
        error_kind,
        message_present: !error.message.trim().is_empty(),
        message_length: error.message.chars().count(),
    }
}

fn order_compensation_local_operation(code: &str) -> Option<&'static str> {
    match code {
        "order.checkout_compensation_identity_invalid" => Some("validate_request"),
        "order.checkout_compensation_identity_conflict" => {
            Some("validate_durable_checkout_identity")
        }
        "order.checkout_compensation_state_conflict" => Some("apply_compensation_state"),
        "order.checkout_compensation_manual_reconciliation" => {
            Some("require_manual_reconciliation")
        }
        _ => None,
    }
}

fn map_checkout_order_compensation_local_port_error(
    context: &PortContext,
    facts: &OrderCompensationRequestFacts,
    error: PortError,
) -> PortError {
    let Some(local_operation) = order_compensation_local_operation(error.code.as_str()) else {
        return error;
    };
    let integrity_failure = matches!(
        local_operation,
        "validate_durable_checkout_identity" | "require_manual_reconciliation"
    );
    let context_facts = order_compensation_context_facts(context);
    let error_facts = order_compensation_port_error_facts(&error);
    if integrity_failure {
        tracing::error!(
            owner = ORDER_COMPENSATION_OWNER,
            operation = COMPENSATE_OPERATION,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            checkout_operation_id_non_nil = facts.checkout_operation_id_non_nil,
            cart_id_non_nil = facts.cart_id_non_nil,
            expected_order_id_present = facts.expected_order_id_present,
            expected_order_id_non_nil = ?facts.expected_order_id_non_nil,
            reason_present = facts.reason_present,
            reason_length = ?facts.reason_length,
            internal_code = %error.code,
            error_message_present = error_facts.message_present,
            error_message_length = error_facts.message_length,
            error_kind = error_facts.error_kind,
            retryable = error.retryable,
            boundary = ORDER_COMPENSATION_BOUNDARY,
            "order checkout compensation local integrity outcome retained bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = ORDER_COMPENSATION_OWNER,
            operation = COMPENSATE_OPERATION,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            checkout_operation_id_non_nil = facts.checkout_operation_id_non_nil,
            cart_id_non_nil = facts.cart_id_non_nil,
            expected_order_id_present = facts.expected_order_id_present,
            expected_order_id_non_nil = ?facts.expected_order_id_non_nil,
            reason_present = facts.reason_present,
            reason_length = ?facts.reason_length,
            internal_code = %error.code,
            error_message_present = error_facts.message_present,
            error_message_length = error_facts.message_length,
            error_kind = error_facts.error_kind,
            retryable = error.retryable,
            boundary = ORDER_COMPENSATION_BOUNDARY,
            "order checkout compensation local outcome retained bounded diagnostics"
        );
    }
    error
}
