use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::checkout_compensation::{
    CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,
    InProcessCheckoutPaymentCompensationPort as PersistentCheckoutPaymentCompensationPort,
};
use crate::providers::PaymentProviderRegistry;
use crate::PaymentCollectionStatusSnapshot;

const PAYMENT_OWNER: &str = "rustok_payment";
const COMPENSATE_CHECKOUT_PAYMENT_OPERATION: &str = "compensate_checkout_payment";
const PAYMENT_COMPENSATION_BOUNDARY: &str = "checkout_payment_compensation_port";

struct CheckoutPaymentCompensationDiagnosticFacts {
    checkout_operation_id: Uuid,
    collection_id: Option<Uuid>,
    reason_length: Option<usize>,
    metadata_kind: &'static str,
    metadata_entry_count: Option<usize>,
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

fn checkout_payment_compensation_diagnostic_facts(
    request: &CheckoutPaymentCompensationRequest,
) -> CheckoutPaymentCompensationDiagnosticFacts {
    CheckoutPaymentCompensationDiagnosticFacts {
        checkout_operation_id: request.checkout_operation_id,
        collection_id: request.collection_id,
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

fn map_checkout_payment_compensation_local_port_error(
    context: &PortContext,
    facts: &CheckoutPaymentCompensationDiagnosticFacts,
    error: PortError,
) -> PortError {
    let local_operation = match (error.code.as_str(), error.message.as_str()) {
        (
            "payment.checkout_compensation_identity_invalid",
            "checkout operation and payment collection identity must be non-nil UUIDs",
        ) => "validate_compensation_identity",
        ("payment.collection_not_found", "payment collection was not found") => {
            "load_collection"
        }
        (
            "payment.checkout_compensation_manual_reconciliation",
            "payment checkout compensation requires manual reconciliation",
        ) => "require_manual_reconciliation",
        (
            "payment.checkout_compensation_state_conflict",
            "payment collection changed while compensation was being applied",
        ) => "apply_compensation_state",
        (
            "payment.checkout_compensation_state_conflict",
            "payment lifecycle conflicts with checkout compensation",
        ) => "apply_payment_lifecycle",
        (
            "payment.checkout_compensation_provider_state_conflict",
            "payment provider cancellation is in an unsupported state",
        ) => "validate_provider_journal_state",
        (
            "payment.checkout_compensation_metadata_invalid",
            "payment compensation metadata must be a JSON object",
        ) => "validate_provider_metadata",
        (
            "payment.checkout_compensation_provider_identity_conflict",
            "payment provider identity conflicts with the durable authorization",
        ) => "validate_provider_identity",
        (
            "payment.checkout_compensation_encoding_failed",
            "payment compensation request could not be encoded",
        ) => "encode_provider_cancel_request",
        (
            "payment.database_unavailable",
            "payment storage is temporarily unavailable",
        ) => "owner_storage",
        (
            "payment.checkout_compensation_validation",
            "payment compensation request is invalid",
        ) => "validate_owner_request",
        ("payment.payment_not_found", "payment was not found") => "load_payment",
        ("payment.refund_not_found", "refund was not found") => "load_refund",
        (
            "payment.provider_unavailable",
            "payment provider is temporarily unavailable",
        ) => "execute_provider_cancel",
        (
            "payment.provider_rejected",
            "payment provider rejected the requested operation",
        ) => "execute_provider_cancel",
        (
            "payment.provider_invalid_response",
            "payment provider response could not be applied safely",
        ) => "normalize_provider_result",
        (
            "payment.provider_not_configured",
            "payment provider is not configured",
        ) => "resolve_provider",
        _ => return error,
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
    if technical_failure {
        tracing::error!(
            error = ?error,
            owner = PAYMENT_OWNER,
            operation = COMPENSATE_CHECKOUT_PAYMENT_OPERATION,
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
            checkout_operation_id = %facts.checkout_operation_id,
            collection_id = ?facts.collection_id,
            reason_length = ?facts.reason_length,
            metadata_kind = facts.metadata_kind,
            metadata_entry_count = ?facts.metadata_entry_count,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = PAYMENT_COMPENSATION_BOUNDARY,
            "payment checkout compensation local technical outcome retained delegated context"
        );
    } else {
        tracing::warn!(
            error = ?error,
            owner = PAYMENT_OWNER,
            operation = COMPENSATE_CHECKOUT_PAYMENT_OPERATION,
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
            checkout_operation_id = %facts.checkout_operation_id,
            collection_id = ?facts.collection_id,
            reason_length = ?facts.reason_length,
            metadata_kind = facts.metadata_kind,
            metadata_entry_count = ?facts.metadata_entry_count,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = PAYMENT_COMPENSATION_BOUNDARY,
            "payment checkout compensation local outcome retained delegated context"
        );
    }
    error
}
