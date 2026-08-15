const PAYMENT_EXECUTION_BOUNDARY: &str = "checkout_payment_execution_port";

#[derive(Debug)]
struct CheckoutPaymentExecutionContextFacts {
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

fn checkout_payment_execution_context_facts(
    context: &PortContext,
) -> CheckoutPaymentExecutionContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    CheckoutPaymentExecutionContextFacts {
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

#[derive(Debug)]
struct CheckoutPaymentExecutionPortErrorFacts {
    error_kind: &'static str,
    message_present: bool,
    message_length: usize,
}

fn checkout_payment_execution_port_error_facts(
    error: &PortError,
) -> CheckoutPaymentExecutionPortErrorFacts {
    let error_kind = match &error.kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    };
    CheckoutPaymentExecutionPortErrorFacts {
        error_kind,
        message_present: !error.message.trim().is_empty(),
        message_length: error.message.chars().count(),
    }
}

#[derive(Debug)]
struct CheckoutPaymentExecutionDiagnosticFacts {
    checkout_operation_id_non_nil: bool,
    cart_id_non_nil: bool,
    order_id_non_nil: bool,
    customer_id_present: bool,
    customer_id_non_nil: Option<bool>,
    collection_id_present: bool,
    collection_id_non_nil: Option<bool>,
    amount_text_length: usize,
    currency_code_length: usize,
    order_plan_hash_length: usize,
    requested_provider_id_present: bool,
    requested_provider_id_length: Option<usize>,
    provider_payment_id_present: bool,
    provider_payment_id_length: Option<usize>,
}

fn checkout_payment_execution_diagnostic_facts(
    identity: &CheckoutPaymentIdentity,
    collection_id: Option<Uuid>,
    requested_provider_id: Option<&str>,
    provider_payment_id: Option<&str>,
) -> CheckoutPaymentExecutionDiagnosticFacts {
    CheckoutPaymentExecutionDiagnosticFacts {
        checkout_operation_id_non_nil: !identity.checkout_operation_id.is_nil(),
        cart_id_non_nil: !identity.cart_id.is_nil(),
        order_id_non_nil: !identity.order_id.is_nil(),
        customer_id_present: identity.customer_id.is_some(),
        customer_id_non_nil: identity.customer_id.map(|value| !value.is_nil()),
        collection_id_present: collection_id.is_some(),
        collection_id_non_nil: collection_id.map(|value| !value.is_nil()),
        amount_text_length: identity.amount.to_string().chars().count(),
        currency_code_length: identity.currency_code.chars().count(),
        order_plan_hash_length: identity.order_plan_hash.chars().count(),
        requested_provider_id_present: requested_provider_id.is_some(),
        requested_provider_id_length: requested_provider_id.map(|value| value.chars().count()),
        provider_payment_id_present: provider_payment_id.is_some(),
        provider_payment_id_length: provider_payment_id.map(|value| value.chars().count()),
    }
}

fn checkout_payment_execution_local_operation(
    operation: &'static str,
    code: &str,
) -> Option<&'static str> {
    match code {
        "payment.checkout_identity_invalid" => Some("validate_checkout_identity"),
        "payment.checkout_currency_invalid" => Some("validate_checkout_currency"),
        "payment.checkout_plan_hash_invalid" => Some("validate_checkout_plan_hash"),
        "payment.checkout_collection_id_invalid" => Some("validate_collection_id"),
        "payment.checkout_collection_operation_conflict" => Some("validate_collection_operation"),
        "payment.checkout_collection_plan_conflict" => Some("validate_collection_plan"),
        "payment.checkout_collection_identity_conflict" => Some("validate_collection_identity"),
        "payment.checkout_collection_identity_missing" => Some("require_collection_identity"),
        "payment.checkout_authorize_state_conflict"
            if operation == AUTHORIZE_CHECKOUT_COLLECTION_OPERATION =>
        {
            Some("validate_authorize_lifecycle")
        }
        "payment.checkout_capture_state_conflict"
            if operation == CAPTURE_CHECKOUT_COLLECTION_OPERATION =>
        {
            Some("validate_capture_lifecycle")
        }
        "payment.checkout_authorize_request_invalid"
            if operation == AUTHORIZE_CHECKOUT_COLLECTION_OPERATION =>
        {
            Some("validate_authorize_request")
        }
        "payment.provider_metadata_invalid" => Some("validate_provider_metadata"),
        "payment.provider_identity_conflict" => Some("validate_provider_identity"),
        "payment.provider_idempotency_key_required" => Some("require_provider_idempotency_key"),
        "payment.provider_request_encoding_failed" => Some("encode_provider_request"),
        "payment.provider_operation_invalid" => Some("select_provider_operation"),
        "payment.database_unavailable" => Some("owner_storage"),
        "payment.checkout_execution_validation" => Some("validate_owner_request"),
        "payment.collection_not_found" => Some("load_collection"),
        "payment.payment_not_found" => Some("load_payment"),
        "payment.refund_not_found" => Some("load_refund"),
        "payment.checkout_execution_state_conflict" => Some("apply_payment_lifecycle"),
        "payment.provider_unavailable" | "payment.provider_rejected" => {
            Some("execute_provider_operation")
        }
        "payment.checkout_execution_manual_reconciliation" => Some("require_manual_reconciliation"),
        "payment.provider_not_configured" => Some("resolve_provider"),
        _ => None,
    }
}
