use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    CheckoutOrderCompensationPort, CheckoutOrderCompensationRequest,
    CheckoutOrderCompensationSnapshot, CheckoutOrderIdentityPort,
    CheckoutOrderPaymentSettlementPort, OrderResponse, SettleCheckoutOrderPaymentRequest,
    checkout_compensation, checkout_payment_settlement,
};

const ORDER_COMPENSATION_OWNER: &str = "rustok_order.checkout_compensation";
const ORDER_COMPENSATION_BOUNDARY: &str = "checkout_order_compensation_port";
const COMPENSATE_OPERATION: &str = "compensate_checkout_order";
const ORDER_PAYMENT_SETTLEMENT_OWNER: &str = "rustok_order.checkout_payment_settlement";
const ORDER_PAYMENT_SETTLEMENT_BOUNDARY: &str = "checkout_order_payment_settlement_port";
const SETTLE_PAYMENT_OPERATION: &str = "settle_checkout_payment";

struct OrderCheckoutContextFacts {
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

struct OrderCheckoutPortErrorFacts {
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
            inner: checkout_compensation::in_process_checkout_order_compensation_port(
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
                checkout_compensation::InProcessCheckoutOrderCompensationPort::with_identity_port(
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
        require_order_checkout_write_admission(
            &context,
            ORDER_COMPENSATION_OWNER,
            ORDER_COMPENSATION_BOUNDARY,
            COMPENSATE_OPERATION,
        )?;
        parse_order_tenant_id(
            &context,
            ORDER_COMPENSATION_OWNER,
            ORDER_COMPENSATION_BOUNDARY,
            COMPENSATE_OPERATION,
        )?;
        parse_order_actor_id(
            &context,
            ORDER_COMPENSATION_OWNER,
            ORDER_COMPENSATION_BOUNDARY,
            COMPENSATE_OPERATION,
        )?;
        require_order_checkout_causation(
            &context,
            ORDER_COMPENSATION_OWNER,
            ORDER_COMPENSATION_BOUNDARY,
            COMPENSATE_OPERATION,
            "order.checkout_compensation_causation_invalid",
            request.checkout_operation_id,
        )?;
        self.inner.compensate_checkout_order(context, request).await
    }
}

pub fn in_process_checkout_order_compensation_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CheckoutOrderCompensationPort> {
    Arc::new(InProcessCheckoutOrderCompensationPort::new(db, event_bus))
}

pub struct InProcessCheckoutOrderPaymentSettlementPort {
    inner: Arc<dyn CheckoutOrderPaymentSettlementPort>,
}

impl InProcessCheckoutOrderPaymentSettlementPort {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: checkout_payment_settlement::in_process_checkout_order_payment_settlement_port(
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
                checkout_payment_settlement::InProcessCheckoutOrderPaymentSettlementPort::with_identity_port(
                    db,
                    event_bus,
                    identity_port,
                ),
            ),
        }
    }
}

#[async_trait]
impl CheckoutOrderPaymentSettlementPort for InProcessCheckoutOrderPaymentSettlementPort {
    async fn settle_checkout_payment(
        &self,
        context: PortContext,
        request: SettleCheckoutOrderPaymentRequest,
    ) -> Result<OrderResponse, PortError> {
        require_order_checkout_write_admission(
            &context,
            ORDER_PAYMENT_SETTLEMENT_OWNER,
            ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
            SETTLE_PAYMENT_OPERATION,
        )?;
        parse_order_tenant_id(
            &context,
            ORDER_PAYMENT_SETTLEMENT_OWNER,
            ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
            SETTLE_PAYMENT_OPERATION,
        )?;
        parse_order_actor_id(
            &context,
            ORDER_PAYMENT_SETTLEMENT_OWNER,
            ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
            SETTLE_PAYMENT_OPERATION,
        )?;
        require_order_checkout_causation(
            &context,
            ORDER_PAYMENT_SETTLEMENT_OWNER,
            ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
            SETTLE_PAYMENT_OPERATION,
            "order.checkout_payment_causation_invalid",
            request.checkout_operation_id,
        )?;
        let diagnostic_context = context.clone();
        let result = self.inner.settle_checkout_payment(context, request).await;
        result.map_err(|error| {
            map_checkout_order_payment_settlement_local_port_error(&diagnostic_context, error)
        })
    }
}

pub fn in_process_checkout_order_payment_settlement_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CheckoutOrderPaymentSettlementPort> {
    Arc::new(InProcessCheckoutOrderPaymentSettlementPort::new(
        db, event_bus,
    ))
}

fn order_checkout_context_facts(context: &PortContext) -> OrderCheckoutContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    OrderCheckoutContextFacts {
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

fn order_checkout_port_error_facts(error: &PortError) -> OrderCheckoutPortErrorFacts {
    let error_kind = match &error.kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    };
    OrderCheckoutPortErrorFacts {
        error_kind,
        message_present: !error.message.trim().is_empty(),
        message_length: error.message.chars().count(),
    }
}

fn checkout_order_payment_settlement_local_operation(code: &str) -> Option<&'static str> {
    match code {
        "order.checkout_payment_request_invalid" => Some("validate_request"),
        "order.checkout_payment_identity_missing" => Some("require_durable_checkout_identity"),
        "order.checkout_payment_identity_conflict" => Some("validate_durable_checkout_identity"),
        "order.checkout_payment_state_conflict" => Some("validate_payment_settlement_lifecycle"),
        "order.checkout_payment_reference_conflict" => Some("validate_settled_payment_identity"),
        _ => None,
    }
}

fn map_checkout_order_payment_settlement_local_port_error(
    context: &PortContext,
    error: PortError,
) -> PortError {
    let Some(local_operation) =
        checkout_order_payment_settlement_local_operation(error.code.as_str())
    else {
        return error;
    };
    let integrity_failure = matches!(
        local_operation,
        "require_durable_checkout_identity"
            | "validate_durable_checkout_identity"
            | "validate_settled_payment_identity"
    );
    let context_facts = order_checkout_context_facts(context);
    let error_facts = order_checkout_port_error_facts(&error);
    if integrity_failure {
        tracing::error!(
            owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
            operation = SETTLE_PAYMENT_OPERATION,
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
            internal_code = %error.code,
            error_message_present = error_facts.message_present,
            error_message_length = error_facts.message_length,
            error_kind = error_facts.error_kind,
            retryable = error.retryable,
            boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
            "order checkout payment settlement local integrity outcome retained bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
            operation = SETTLE_PAYMENT_OPERATION,
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
            internal_code = %error.code,
            error_message_present = error_facts.message_present,
            error_message_length = error_facts.message_length,
            error_kind = error_facts.error_kind,
            retryable = error.retryable,
            boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
            "order checkout payment settlement local outcome retained bounded diagnostics"
        );
    }
    error
}

fn require_order_checkout_write_admission(
    context: &PortContext,
    owner: &'static str,
    boundary: &'static str,
    operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .inspect_err(|error| {
            log_order_checkout_admission_rejection(
                context, owner, boundary, operation, "policy", error,
            );
        })?;
    context.require_write_semantics().inspect_err(|error| {
        log_order_checkout_admission_rejection(
            context,
            owner,
            boundary,
            operation,
            "write_semantics",
            error,
        );
    })
}

struct CheckoutContextRejectionEvidence {
    parse_failed: bool,
    expected_checkout_operation_id: Option<Uuid>,
}

fn log_order_checkout_admission_rejection(
    context: &PortContext,
    owner: &'static str,
    boundary: &'static str,
    operation: &'static str,
    admission_phase: &'static str,
    error: &PortError,
) {
    let technical_failure = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );
    let context_facts = order_checkout_context_facts(context);
    let error_facts = order_checkout_port_error_facts(error);
    if technical_failure {
        tracing::error!(
            owner,
            operation,
            admission_phase,
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
            internal_code = %error.code,
            error_message_present = error_facts.message_present,
            error_message_length = error_facts.message_length,
            error_kind = error_facts.error_kind,
            retryable = error.retryable,
            boundary,
            "order checkout owner admission failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner,
            operation,
            admission_phase,
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
            internal_code = %error.code,
            error_message_present = error_facts.message_present,
            error_message_length = error_facts.message_length,
            error_kind = error_facts.error_kind,
            retryable = error.retryable,
            boundary,
            "order checkout owner admission was rejected with bounded diagnostics"
        );
    }
}

fn parse_order_tenant_id(
    context: &PortContext,
    owner: &'static str,
    boundary: &'static str,
    operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let error = PortError::validation(
            "order.tenant_id_invalid",
            "order request context is invalid",
        );
        log_order_checkout_context_rejection(
            context,
            owner,
            boundary,
            operation,
            "tenant_id",
            &error,
            CheckoutContextRejectionEvidence {
                parse_failed: true,
                expected_checkout_operation_id: None,
            },
        );
        error
    })
}

fn parse_order_actor_id(
    context: &PortContext,
    owner: &'static str,
    boundary: &'static str,
    operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.actor.id).map_err(|_| {
        let error =
            PortError::validation("order.actor_id_invalid", "order request context is invalid");
        log_order_checkout_context_rejection(
            context,
            owner,
            boundary,
            operation,
            "actor_id",
            &error,
            CheckoutContextRejectionEvidence {
                parse_failed: true,
                expected_checkout_operation_id: None,
            },
        );
        error
    })
}

fn require_order_checkout_causation(
    context: &PortContext,
    owner: &'static str,
    boundary: &'static str,
    operation: &'static str,
    code: &'static str,
    checkout_operation_id: Uuid,
) -> Result<(), PortError> {
    let (context_operation, parse_failed) = match context.causation_id.as_deref() {
        Some(value) => match Uuid::parse_str(value) {
            Ok(value) => (Some(value), false),
            Err(_) => (None, true),
        },
        None => (None, false),
    };
    if context_operation != Some(checkout_operation_id) {
        let error = PortError::validation(code, "checkout operation context is invalid");
        log_order_checkout_context_rejection(
            context,
            owner,
            boundary,
            operation,
            "causation_id",
            &error,
            CheckoutContextRejectionEvidence {
                parse_failed,
                expected_checkout_operation_id: Some(checkout_operation_id),
            },
        );
        return Err(error);
    }
    Ok(())
}

fn log_order_checkout_context_rejection(
    context: &PortContext,
    owner: &'static str,
    boundary: &'static str,
    operation: &'static str,
    validation_phase: &'static str,
    error: &PortError,
    evidence: CheckoutContextRejectionEvidence,
) {
    let context_facts = order_checkout_context_facts(context);
    let error_facts = order_checkout_port_error_facts(error);
    let expected_checkout_operation_id_present = evidence.expected_checkout_operation_id.is_some();
    let expected_checkout_operation_id_non_nil = evidence
        .expected_checkout_operation_id
        .map(|value| !value.is_nil());
    tracing::warn!(
        parse_failed = evidence.parse_failed,
        owner,
        operation,
        validation_phase,
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
        expected_checkout_operation_id_present,
        expected_checkout_operation_id_non_nil = ?expected_checkout_operation_id_non_nil,
        causation_matches = false,
        internal_code = %error.code,
        error_message_present = error_facts.message_present,
        error_message_length = error_facts.message_length,
        error_kind = error_facts.error_kind,
        retryable = error.retryable,
        boundary,
        "order checkout owner context validation was rejected with bounded diagnostics"
    );
}
