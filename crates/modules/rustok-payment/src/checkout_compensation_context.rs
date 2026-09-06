use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::PaymentCollectionStatusSnapshot;
use crate::checkout_compensation_persistent::{
    CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,
    InProcessCheckoutPaymentCompensationPort as PersistentCheckoutPaymentCompensationPort,
};
use crate::providers::PaymentProviderRegistry;

const PAYMENT_OWNER: &str = "rustok_payment";
const COMPENSATE_CHECKOUT_PAYMENT_OPERATION: &str = "compensate_checkout_payment";
const PAYMENT_COMPENSATION_BOUNDARY: &str = "checkout_payment_compensation_port";

struct CheckoutPaymentCompensationContextFacts {
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

struct CheckoutPaymentCompensationDiagnosticFacts {
    checkout_operation_id_non_nil: bool,
    collection_id_present: bool,
    collection_id_non_nil: Option<bool>,
    reason_present: bool,
    reason_length: Option<usize>,
    metadata_kind: &'static str,
    metadata_entry_count: Option<usize>,
}

struct CheckoutPaymentCompensationPortErrorFacts {
    error_kind: &'static str,
    message_present: bool,
    message_length: usize,
}

pub struct InProcessCheckoutPaymentCompensationPort {
    inner: PersistentCheckoutPaymentCompensationPort,
}

impl InProcessCheckoutPaymentCompensationPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: PersistentCheckoutPaymentCompensationPort::new(db),
        }
    }

    pub fn with_provider_registry(
        db: DatabaseConnection,
        provider_registry: PaymentProviderRegistry,
    ) -> Self {
        Self {
            inner: PersistentCheckoutPaymentCompensationPort::with_provider_registry(
                db,
                provider_registry,
            ),
        }
    }
}

pub fn in_process_checkout_payment_compensation_port(
    db: DatabaseConnection,
) -> Arc<dyn CheckoutPaymentCompensationPort> {
    Arc::new(InProcessCheckoutPaymentCompensationPort::new(db))
}

#[async_trait]
impl CheckoutPaymentCompensationPort for InProcessCheckoutPaymentCompensationPort {
    async fn compensate_checkout_payment(
        &self,
        context: PortContext,
        request: CheckoutPaymentCompensationRequest,
    ) -> Result<Option<PaymentCollectionStatusSnapshot>, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = checkout_payment_compensation_diagnostic_facts(&request);
        let result = self
            .inner
            .compensate_checkout_payment(context, request)
            .await;
        result.map_err(|error| {
            map_checkout_payment_compensation_local_port_error(
                &diagnostic_context,
                &diagnostic_facts,
                error,
            )
        })
    }
}

fn checkout_payment_compensation_context_facts(
    context: &PortContext,
) -> CheckoutPaymentCompensationContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    CheckoutPaymentCompensationContextFacts {
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

fn checkout_payment_compensation_diagnostic_facts(
    request: &CheckoutPaymentCompensationRequest,
) -> CheckoutPaymentCompensationDiagnosticFacts {
    CheckoutPaymentCompensationDiagnosticFacts {
        checkout_operation_id_non_nil: !request.checkout_operation_id.is_nil(),
        collection_id_present: request.collection_id.is_some(),
        collection_id_non_nil: request.collection_id.map(|value| !value.is_nil()),
        reason_present: request.reason.is_some(),
        reason_length: request.reason.as_ref().map(|value| value.chars().count()),
        metadata_kind: payment_metadata_kind(&request.metadata),
        metadata_entry_count: match &request.metadata {
            Value::Object(entries) => Some(entries.len()),
            Value::Array(entries) => Some(entries.len()),
            _ => None,
        },
    }
}

fn payment_metadata_kind(metadata: &Value) -> &'static str {
    match metadata {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn checkout_payment_compensation_port_error_facts(
    error: &PortError,
) -> CheckoutPaymentCompensationPortErrorFacts {
    let error_kind = match &error.kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    };
    CheckoutPaymentCompensationPortErrorFacts {
        error_kind,
        message_present: !error.message.trim().is_empty(),
        message_length: error.message.chars().count(),
    }
}

fn checkout_payment_compensation_local_operation(code: &str) -> Option<&'static str> {
    match code {
        "port.idempotency_key_required" => Some("admit_write_idempotency"),
        "port.deadline_required" => Some("admit_deadline"),
        "payment.checkout_compensation_identity_invalid" => Some("validate_compensation_identity"),
        "payment.collection_not_found" => Some("load_collection"),
        "payment.checkout_compensation_manual_reconciliation" => {
            Some("require_manual_reconciliation")
        }
        "payment.checkout_compensation_state_conflict" => Some("apply_compensation_state"),
        "payment.checkout_compensation_provider_state_conflict" => {
            Some("validate_provider_journal_state")
        }
        "payment.checkout_compensation_metadata_invalid" => Some("validate_provider_metadata"),
        "payment.checkout_compensation_provider_identity_conflict" => {
            Some("validate_provider_identity")
        }
        "payment.checkout_compensation_encoding_failed" => Some("encode_provider_cancel_request"),
        "payment.database_unavailable" => Some("owner_storage"),
        "payment.checkout_compensation_validation" => Some("validate_owner_request"),
        "payment.payment_not_found" => Some("load_payment"),
        "payment.refund_not_found" => Some("load_refund"),
        "payment.provider_unavailable" | "payment.provider_rejected" => {
            Some("execute_provider_cancel")
        }
        "payment.provider_invalid_response" => Some("normalize_provider_result"),
        "payment.provider_not_configured" => Some("resolve_provider"),
        _ => None,
    }
}

fn map_checkout_payment_compensation_local_port_error(
    context: &PortContext,
    facts: &CheckoutPaymentCompensationDiagnosticFacts,
    error: PortError,
) -> PortError {
    let Some(local_operation) = checkout_payment_compensation_local_operation(error.code.as_str())
    else {
        return error;
    };
    let integrity_failure = matches!(
        local_operation,
        "require_manual_reconciliation" | "validate_provider_journal_state"
    );
    let technical_failure = integrity_failure
        || matches!(
            &error.kind,
            PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
        );
    let context_facts = checkout_payment_compensation_context_facts(context);
    let error_facts = checkout_payment_compensation_port_error_facts(&error);
    if technical_failure {
        tracing::error!(
            owner = PAYMENT_OWNER,
            operation = COMPENSATE_CHECKOUT_PAYMENT_OPERATION,
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
            collection_id_present = facts.collection_id_present,
            collection_id_non_nil = ?facts.collection_id_non_nil,
            reason_present = facts.reason_present,
            reason_length = ?facts.reason_length,
            metadata_kind = facts.metadata_kind,
            metadata_entry_count = ?facts.metadata_entry_count,
            internal_code = %error.code,
            error_message_present = error_facts.message_present,
            error_message_length = error_facts.message_length,
            error_kind = error_facts.error_kind,
            retryable = error.retryable,
            boundary = PAYMENT_COMPENSATION_BOUNDARY,
            "payment checkout compensation local technical outcome retained safe context"
        );
    } else {
        tracing::warn!(
            owner = PAYMENT_OWNER,
            operation = COMPENSATE_CHECKOUT_PAYMENT_OPERATION,
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
            collection_id_present = facts.collection_id_present,
            collection_id_non_nil = ?facts.collection_id_non_nil,
            reason_present = facts.reason_present,
            reason_length = ?facts.reason_length,
            metadata_kind = facts.metadata_kind,
            metadata_entry_count = ?facts.metadata_entry_count,
            internal_code = %error.code,
            error_message_present = error_facts.message_present,
            error_message_length = error_facts.message_length,
            error_kind = error_facts.error_kind,
            retryable = error.retryable,
            boundary = PAYMENT_COMPENSATION_BOUNDARY,
            "payment checkout compensation local outcome retained safe context"
        );
    }
    error
}
