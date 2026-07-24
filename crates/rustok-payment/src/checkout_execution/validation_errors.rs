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
            "payment provider operation has no normalized durable result",
        )
    })?;
    serde_json::from_value(value).map(Some).map_err(|error| {
        tracing::error!(
            operation_id = %operation.id,
            error = ?error,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "payment.provider_invalid_response",
            "payment provider operation result is malformed"
        );
        manual_reconciliation(
            context,
            owner_operation,
            "payment provider operation result is malformed",
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
        tracing::warn!(
            causation_id = ?context.causation_id,
            checkout_operation_id = %checkout_operation_id,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "payment.checkout_operation_id_invalid",
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
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            internal_tenant_id = %context.tenant_id,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "payment.tenant_id_invalid",
            "payment checkout execution tenant context is invalid"
        );
        PortError::validation(
            "payment.tenant_id_invalid",
            "payment request context is invalid",
        )
    })
}

fn manual_reconciliation(
    context: &PortContext,
    owner_operation: &'static str,
    internal_message: &'static str,
) -> PortError {
    tracing::error!(
        internal_message,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code = "payment.checkout_execution_manual_reconciliation",
        "payment checkout execution requires manual reconciliation"
    );
    PortError::new(
        PortErrorKind::Conflict,
        "payment.checkout_execution_manual_reconciliation",
        "payment checkout execution requires manual reconciliation",
        false,
    )
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
    tracing::error!(
        error = ?error,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code,
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
            "payment provider returned an invalid successful response",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => manual_reconciliation(
            context,
            owner_operation,
            "payment provider operation outcome is unknown",
        ),
        PaymentError::ProviderConfiguration { .. } => PortError::invariant_violation(
            "payment.provider_not_configured",
            "payment provider is not configured",
        ),
    }
}
