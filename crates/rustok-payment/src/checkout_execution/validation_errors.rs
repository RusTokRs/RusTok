fn persisted_provider_result(
    context: &PortContext,
    owner_operation: &'static str,
    operation: &crate::entities::provider_operation::Model,
) -> Result<Option<PaymentProviderOperationResult>, PortError> {
    if operation.status == PROVIDER_OPERATION_EXECUTING {
        return Ok(None);
    }
    if !matches!(
        operation.status.as_str(),
        PROVIDER_OPERATION_COMMITTED
            | PROVIDER_OPERATION_SUCCEEDED
            | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
    ) {
        return Ok(None);
    }
    let value = operation.provider_result.clone().ok_or_else(|| {
        manual_reconciliation(
            context,
            owner_operation,
            CheckoutPaymentExecutionReconciliationReason::MissingNormalizedDurableResult,
        )
    })?;
    let (provider_result_kind, provider_result_collection_length) = match &value {
        Value::Null => ("null", None),
        Value::Bool(_) => ("bool", None),
        Value::Number(_) => ("number", None),
        Value::String(_) => ("string", None),
        Value::Array(items) => ("array", Some(items.len())),
        Value::Object(fields) => ("object", Some(fields.len())),
    };
    serde_json::from_value(value).map(Some).map_err(|_| {
        let context_facts = checkout_payment_execution_context_facts(context);
        tracing::error!(
            operation_id_non_nil = !operation.id.is_nil(),
            provider_result_decode_failed = true,
            provider_result_kind,
            provider_result_collection_length = ?provider_result_collection_length,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            operation = owner_operation,
            code = "payment.provider_invalid_response",
            boundary = PAYMENT_EXECUTION_BOUNDARY,
            "payment provider operation result is malformed"
        );
        manual_reconciliation(
            context,
            owner_operation,
            CheckoutPaymentExecutionReconciliationReason::MalformedDurableResult,
        )
    })
}

fn insert_metadata_string(metadata: &mut Value, key: &str, value: String) -> Result<(), PortError> {
    if metadata.is_null() {
        *metadata = serde_json::json!({});
    }
    let object = metadata.as_object_mut().ok_or_else(|| {
        PortError::validation(
            "payment.provider_metadata_invalid",
            "payment provider metadata must be a JSON object",
        )
    })?;
    if let Some(existing) = object.get(key).and_then(Value::as_str) {
        if existing != value {
            return Err(PortError::conflict(
                "payment.provider_identity_conflict",
                "payment provider identity conflicts with the durable authorize operation",
            ));
        }
        return Ok(());
    }
    object.insert(key.to_string(), Value::String(value));
    Ok(())
}

fn metadata_string<'a>(metadata: &'a Value, key: &str) -> Option<&'a str> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn merge_metadata(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

fn require_checkout_payment_read_admission(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .inspect_err(|error| {
            log_checkout_payment_execution_admission_rejection(
                context,
                owner_operation,
                "policy",
                error,
            );
        })
}

fn require_checkout_payment_write_admission(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .inspect_err(|error| {
            log_checkout_payment_execution_admission_rejection(
                context,
                owner_operation,
                "policy",
                error,
            );
        })?;
    context.require_write_semantics().inspect_err(|error| {
        log_checkout_payment_execution_admission_rejection(
            context,
            owner_operation,
            "write_semantics",
            error,
        );
    })
}

fn log_checkout_payment_execution_admission_rejection(
    context: &PortContext,
    owner_operation: &'static str,
    admission: &'static str,
    error: &PortError,
) {
    let technical_failure = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );
    let context_facts = checkout_payment_execution_context_facts(context);
    let error_facts = checkout_payment_execution_port_error_facts(error);
    if technical_failure {
        tracing::error!(
            owner = "rustok_payment",
            operation = owner_operation,
            admission,
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
            boundary = PAYMENT_EXECUTION_BOUNDARY,
            "payment checkout execution admission failed with safe context"
        );
    } else {
        tracing::warn!(
            owner = "rustok_payment",
            operation = owner_operation,
            admission,
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
            boundary = PAYMENT_EXECUTION_BOUNDARY,
            "payment checkout execution admission was rejected with safe context"
        );
    }
}

fn require_operation_context(
    context: &PortContext,
    owner_operation: &'static str,
    checkout_operation_id: Uuid,
) -> Result<(), PortError> {
    let context_operation = context
        .causation_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    if context_operation != Some(checkout_operation_id) {
        let context_facts = checkout_payment_execution_context_facts(context);
        tracing::warn!(
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            checkout_operation_id_non_nil = !checkout_operation_id.is_nil(),
            causation_matches = false,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            operation = owner_operation,
            code = "payment.checkout_operation_id_invalid",
            boundary = PAYMENT_EXECUTION_BOUNDARY,
            "payment checkout execution causation context is invalid"
        );
        return Err(PortError::validation(
            "payment.checkout_operation_id_invalid",
            "payment request context is invalid",
        ));
    }
    Ok(())
}

fn parse_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let context_facts = checkout_payment_execution_context_facts(context);
        tracing::warn!(
            tenant_id_parse_failed = true,
            tenant_id_length = context_facts.tenant_id_length,
            correlation_id = %context.correlation_id,
            operation = owner_operation,
            code = "payment.tenant_id_invalid",
            boundary = PAYMENT_EXECUTION_BOUNDARY,
            "payment checkout execution tenant context is invalid"
        );
        PortError::validation(
            "payment.tenant_id_invalid",
            "payment request context is invalid",
        )
    })
}

#[derive(Clone, Copy, Debug)]
enum CheckoutPaymentExecutionReconciliationReason {
    MissingNormalizedDurableResult,
    MalformedDurableResult,
    InvalidSuccessfulProviderResponse,
    UnknownProviderOutcome,
    MissingDurableAuthorizeProviderIdentity,
    IncompleteAuthorizeOperation,
    MissingDurableProviderPaymentIdentity,
    CommitCheckpointFailed,
    UnknownCollectionLifecycleBeforeAuthorization,
    AuthorizationLocalPersistenceIncomplete,
    UnknownCollectionLifecycleBeforeCapture,
    CaptureLocalPersistenceIncomplete,
    ProviderOperationInProgressOrReconciliationRequired,
    ProviderFailureCheckpointFailed,
    ProviderResultEncodingFailed,
    ProviderSuccessCheckpointFailed,
}

impl CheckoutPaymentExecutionReconciliationReason {
    fn label(self) -> &'static str {
        match self {
            Self::MissingNormalizedDurableResult => "missing_normalized_durable_result",
            Self::MalformedDurableResult => "malformed_durable_result",
            Self::InvalidSuccessfulProviderResponse => "invalid_successful_provider_response",
            Self::UnknownProviderOutcome => "unknown_provider_outcome",
            Self::MissingDurableAuthorizeProviderIdentity => {
                "missing_durable_authorize_provider_identity"
            }
            Self::IncompleteAuthorizeOperation => "incomplete_authorize_operation",
            Self::MissingDurableProviderPaymentIdentity => {
                "missing_durable_provider_payment_identity"
            }
            Self::CommitCheckpointFailed => "commit_checkpoint_failed",
            Self::UnknownCollectionLifecycleBeforeAuthorization => {
                "unknown_collection_lifecycle_before_authorization"
            }
            Self::AuthorizationLocalPersistenceIncomplete => {
                "authorization_local_persistence_incomplete"
            }
            Self::UnknownCollectionLifecycleBeforeCapture => {
                "unknown_collection_lifecycle_before_capture"
            }
            Self::CaptureLocalPersistenceIncomplete => "capture_local_persistence_incomplete",
            Self::ProviderOperationInProgressOrReconciliationRequired => {
                "provider_operation_in_progress_or_reconciliation_required"
            }
            Self::ProviderFailureCheckpointFailed => "provider_failure_checkpoint_failed",
            Self::ProviderResultEncodingFailed => "provider_result_encoding_failed",
            Self::ProviderSuccessCheckpointFailed => "provider_success_checkpoint_failed",
        }
    }
}

fn manual_reconciliation(
    context: &PortContext,
    owner_operation: &'static str,
    reason: CheckoutPaymentExecutionReconciliationReason,
) -> PortError {
    let context_facts = checkout_payment_execution_context_facts(context);
    let reconciliation_reason = reason.label();
    tracing::error!(
        reconciliation_reason,
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        channel_present = context_facts.channel_present,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        idempotency_key_present = context_facts.idempotency_key_present,
        deadline_ms = ?context_facts.deadline_ms,
        operation = owner_operation,
        code = "payment.checkout_execution_manual_reconciliation",
        boundary = PAYMENT_EXECUTION_BOUNDARY,
        "payment checkout execution requires manual reconciliation"
    );
    PortError::new(
        PortErrorKind::Conflict,
        "payment.checkout_execution_manual_reconciliation",
        "payment checkout execution requires manual reconciliation",
        false,
    )
}

#[derive(Debug)]
struct CheckoutPaymentExecutionPaymentErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

fn checkout_payment_execution_payment_error_facts(
    error: &PaymentError,
) -> CheckoutPaymentExecutionPaymentErrorFacts {
    let (
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    ) = match error {
        PaymentError::Validation(value) => ("validation", 1, value.chars().count(), 0, 0, false),
        PaymentError::PaymentCollectionNotFound(id) => (
            "payment_collection_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        PaymentError::PaymentNotFound(id) => (
            "payment_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        PaymentError::RefundNotFound(id) => (
            "refund_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        PaymentError::InvalidTransition { from, to } => (
            "invalid_transition",
            2,
            from.chars().count() + to.chars().count(),
            0,
            0,
            false,
        ),
        PaymentError::ProviderUnavailable {
            provider_id,
            operation,
        } => (
            "provider_unavailable",
            2,
            provider_id.chars().count() + operation.chars().count(),
            0,
            0,
            false,
        ),
        PaymentError::ProviderRejected {
            provider_id,
            operation,
        } => (
            "provider_rejected",
            2,
            provider_id.chars().count() + operation.chars().count(),
            0,
            0,
            false,
        ),
        PaymentError::ProviderInvalidResponse {
            provider_id,
            operation,
        } => (
            "provider_invalid_response",
            2,
            provider_id.chars().count() + operation.chars().count(),
            0,
            0,
            false,
        ),
        PaymentError::ProviderOutcomeUnknown {
            provider_id,
            operation,
        } => (
            "provider_outcome_unknown",
            2,
            provider_id.chars().count() + operation.chars().count(),
            0,
            0,
            false,
        ),
        PaymentError::ProviderConfiguration { provider_id } => (
            "provider_configuration",
            1,
            provider_id.chars().count(),
            0,
            0,
            false,
        ),
        PaymentError::Database(_) => ("database", 0, 0, 0, 0, true),
    };
    CheckoutPaymentExecutionPaymentErrorFacts {
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    }
}

fn stable_payment_error_code(error: &PaymentError) -> &'static str {
    match error {
        PaymentError::Database(_) => "payment.database_unavailable",
        PaymentError::Validation(_) => "payment.validation",
        PaymentError::PaymentCollectionNotFound(_) => "payment.collection_not_found",
        PaymentError::PaymentNotFound(_) => "payment.payment_not_found",
        PaymentError::RefundNotFound(_) => "payment.refund_not_found",
        PaymentError::InvalidTransition { .. } => "payment.invalid_transition",
        PaymentError::ProviderUnavailable { .. } => "payment.provider_unavailable",
        PaymentError::ProviderRejected { .. } => "payment.provider_rejected",
        PaymentError::ProviderInvalidResponse { .. } => "payment.provider_invalid_response",
        PaymentError::ProviderOutcomeUnknown { .. } => "payment.provider_outcome_unknown",
        PaymentError::ProviderConfiguration { .. } => "payment.provider_not_configured",
    }
}

fn payment_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PaymentError,
) -> PortError {
    let code = stable_payment_error_code(&error);
    let context_facts = checkout_payment_execution_context_facts(context);
    let error_facts = checkout_payment_execution_payment_error_facts(&error);
    tracing::error!(
        owner_error_variant = error_facts.error_variant,
        owner_error_text_field_count = error_facts.text_field_count,
        owner_error_text_total_length = error_facts.text_total_length,
        owner_error_uuid_field_count = error_facts.uuid_field_count,
        owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
        owner_error_opaque_payload_present = error_facts.opaque_payload_present,
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        channel_present = context_facts.channel_present,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        idempotency_key_present = context_facts.idempotency_key_present,
        deadline_ms = ?context_facts.deadline_ms,
        operation = owner_operation,
        code,
        boundary = PAYMENT_EXECUTION_BOUNDARY,
        "payment checkout execution owner operation failed"
    );
    match error {
        PaymentError::Database(_) => PortError::unavailable(
            "payment.database_unavailable",
            "payment storage is temporarily unavailable",
        ),
        PaymentError::Validation(_) => PortError::validation(
            "payment.checkout_execution_validation",
            "checkout payment request is invalid",
        ),
        PaymentError::PaymentCollectionNotFound(_) => PortError::not_found(
            "payment.collection_not_found",
            "payment collection was not found",
        ),
        PaymentError::PaymentNotFound(_) => {
            PortError::not_found("payment.payment_not_found", "payment was not found")
        }
        PaymentError::RefundNotFound(_) => {
            PortError::not_found("payment.refund_not_found", "refund was not found")
        }
        PaymentError::InvalidTransition { .. } => PortError::conflict(
            "payment.checkout_execution_state_conflict",
            "payment lifecycle conflicts with checkout execution",
        ),
        PaymentError::ProviderUnavailable { .. } => PortError::unavailable(
            "payment.provider_unavailable",
            "payment provider is temporarily unavailable",
        ),
        PaymentError::ProviderRejected { .. } => PortError::conflict(
            "payment.provider_rejected",
            "payment provider rejected the requested operation",
        ),
        PaymentError::ProviderInvalidResponse { .. } => manual_reconciliation(
            context,
            owner_operation,
            CheckoutPaymentExecutionReconciliationReason::InvalidSuccessfulProviderResponse,
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => manual_reconciliation(
            context,
            owner_operation,
            CheckoutPaymentExecutionReconciliationReason::UnknownProviderOutcome,
        ),
        PaymentError::ProviderConfiguration { .. } => PortError::invariant_violation(
            "payment.provider_not_configured",
            "payment provider is not configured",
        ),
    }
}
