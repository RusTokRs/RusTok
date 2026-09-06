fn shipping_option_query_context(
    tenant_id: Uuid,
    query_field: &'static str,
    shipping_option_id: Option<Uuid>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> PortContext {
    let locale = requested_locale.or(tenant_default_locale).unwrap_or("en");
    let resource = shipping_option_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| tenant_id.to_string());
    with_current_graphql_public_channel(
        PortContext::new(
            tenant_id.to_string(),
            PortActor::service("rustok-commerce.graphql-query-shipping-options"),
            locale,
            format!("graphql-fulfillment:{query_field}:{resource}"),
        )
        .with_deadline(std::time::Duration::from_secs(2)),
    )
}

fn fulfillment_query_context(
    tenant_id: Uuid,
    query_field: &'static str,
    operation: &'static str,
    fulfillment_id: Option<Uuid>,
    order_id: Option<Uuid>,
) -> PortContext {
    let resource = fulfillment_id.or(order_id).unwrap_or(tenant_id);
    with_current_graphql_public_channel(
        PortContext::new(
            tenant_id.to_string(),
            PortActor::service("rustok-commerce.graphql-query-fulfillments"),
            "en",
            format!("graphql-fulfillment-lifecycle:{query_field}:{operation}:{resource}"),
        )
        .with_deadline(std::time::Duration::from_secs(2)),
    )
}

fn with_current_graphql_public_channel(context: PortContext) -> PortContext {
    let call_context =
        crate::graphql_runtime::fulfillment_read_call_context_for_current_graphql_scope();
    match call_context.channel() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

#[allow(clippy::too_many_arguments)]
fn map_shipping_option_lookup_port_error(
    error: PortError,
    context: &PortContext,
    query_field: &'static str,
    operation: &'static str,
    shipping_option_id: Uuid,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> FulfillmentError {
    let error_kind = port_error_kind_name(&error.kind);
    let technical = is_technical_port_error(&error.kind);
    let optional_not_found = matches!(&error.kind, PortErrorKind::NotFound);
    let (message, code, retryable) = public_fulfillment_port_policy(&error.kind);
    log_shipping_option_port_error(
        &error,
        context,
        query_field,
        operation,
        Some(shipping_option_id),
        requested_locale,
        tenant_default_locale,
        error_kind,
        if optional_not_found {
            "OPTIONAL_NONE"
        } else {
            code
        },
        if optional_not_found { false } else { retryable },
        technical,
    );

    if optional_not_found {
        FulfillmentError::ShippingOptionNotFound(shipping_option_id)
    } else {
        FulfillmentError::Public(BoundaryError::Public {
            message,
            code,
            retryable,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn map_shipping_option_port_error(
    error: PortError,
    context: &PortContext,
    query_field: &'static str,
    operation: &'static str,
    shipping_option_id: Option<Uuid>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> BoundaryError {
    let (message, code, retryable) = public_fulfillment_port_policy(&error.kind);
    let error_kind = port_error_kind_name(&error.kind);
    let technical = is_technical_port_error(&error.kind);
    log_shipping_option_port_error(
        &error,
        context,
        query_field,
        operation,
        shipping_option_id,
        requested_locale,
        tenant_default_locale,
        error_kind,
        code,
        retryable,
        technical,
    );

    BoundaryError::Public {
        message,
        code,
        retryable,
    }
}

fn map_fulfillment_port_error(
    error: PortError,
    context: &PortContext,
    query_field: &'static str,
    operation: &'static str,
    fulfillment_id: Option<Uuid>,
    order_id: Option<Uuid>,
) -> FulfillmentError {
    let (message, public_code, public_retryable) = public_fulfillment_port_policy(&error.kind);
    log_fulfillment_port_error(
        &error,
        context,
        query_field,
        operation,
        fulfillment_id,
        order_id,
        message,
        public_code,
        public_retryable,
    );

    if matches!(&error.kind, PortErrorKind::NotFound) {
        FulfillmentError::FulfillmentNotFound(fulfillment_id.or(order_id).unwrap_or_else(Uuid::nil))
    } else {
        FulfillmentError::Public(BoundaryError::Public {
            message,
            code: public_code,
            retryable: public_retryable,
        })
    }
}

fn public_fulfillment_port_policy(kind: &PortErrorKind) -> (&'static str, &'static str, bool) {
    match kind {
        PortErrorKind::Validation => (
            "Fulfillment query is invalid",
            "FULFILLMENT_REQUEST_INVALID",
            false,
        ),
        PortErrorKind::NotFound => (
            "Fulfillment resource was not found",
            "FULFILLMENT_RESOURCE_NOT_FOUND",
            false,
        ),
        PortErrorKind::Conflict => (
            "Fulfillment state conflicts with this query",
            "FULFILLMENT_STATE_CONFLICT",
            false,
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            "Fulfillment data is temporarily unavailable",
            "FULFILLMENT_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        PortErrorKind::Forbidden => (
            "Fulfillment query is not permitted",
            "FULFILLMENT_ACCESS_DENIED",
            false,
        ),
        PortErrorKind::InvariantViolation => (
            "Fulfillment query could not be completed safely",
            "FULFILLMENT_OPERATION_FAILED",
            false,
        ),
    }
}

fn port_error_kind_name(kind: &PortErrorKind) -> &'static str {
    match kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable | PortErrorKind::Timeout => "unavailable",
        PortErrorKind::InvariantViolation => "invariant",
    }
}

fn is_technical_port_error(kind: &PortErrorKind) -> bool {
    matches!(
        kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    )
}

struct FulfillmentQueryDiagnosticError;

impl std::fmt::Debug for FulfillmentQueryDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

struct FulfillmentQueryContextFacts {
    tenant_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
    correlation_id_length: usize,
    context_locale_length: usize,
    channel_present: bool,
    channel_length: Option<usize>,
    deadline_ms: Option<u64>,
}

fn fulfillment_query_context_facts(context: &PortContext) -> FulfillmentQueryContextFacts {
    let actor_kind = match &context.actor.kind {
        ::rustok_api::PortActorKind::User => "user",
        ::rustok_api::PortActorKind::Service => "service",
        ::rustok_api::PortActorKind::System => "system",
    };
    FulfillmentQueryContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
        correlation_id_length: context.correlation_id.chars().count(),
        context_locale_length: context.locale.chars().count(),
        channel_present: context.channel.is_some(),
        channel_length: context.channel.as_ref().map(|value| value.chars().count()),
        deadline_ms: context.deadline_ms,
    }
}

fn optional_uuid_shape(value: Option<Uuid>) -> &'static str {
    match value {
        None => "absent",
        Some(value) if value.is_nil() => "nil",
        Some(_) => "non_nil",
    }
}

fn text_presence_shape(value: &str) -> &'static str {
    if value.is_empty() { "empty" } else { "present" }
}

#[allow(clippy::too_many_arguments)]
fn log_shipping_option_port_error(
    error: &PortError,
    context: &PortContext,
    query_field: &'static str,
    operation: &'static str,
    shipping_option_id: Option<Uuid>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
    error_kind: &'static str,
    public_code: &'static str,
    public_retryable: bool,
    technical: bool,
) {
    let facts = fulfillment_query_context_facts(context);
    let shipping_option_id_shape = optional_uuid_shape(shipping_option_id);
    let owner_message_presence = text_presence_shape(&error.message);
    let owner_message_length = error.message.chars().count();
    let diagnostic_error = FulfillmentQueryDiagnosticError;
    if technical {
        tracing::error!(
            error = ?diagnostic_error,
            owner = "rustok_fulfillment",
            tenant_id_length = facts.tenant_id_length,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            correlation_id_length = facts.correlation_id_length,
            context_locale_length = facts.context_locale_length,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            deadline_ms = ?facts.deadline_ms,
            query_field,
            operation,
            shipping_option_id_shape,
            requested_locale_length = requested_locale.map(str::len),
            tenant_default_locale_length = tenant_default_locale.map(str::len),
            error_kind,
            owner_code = %error.code,
            owner_kind = error_kind,
            owner_message_presence,
            owner_message_length,
            owner_retryable = error.retryable,
            public_code,
            public_retryable,
            boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
            "commerce GraphQL query shipping-option owner read failed"
        );
    } else {
        tracing::warn!(
            error = ?diagnostic_error,
            owner = "rustok_fulfillment",
            tenant_id_length = facts.tenant_id_length,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            correlation_id_length = facts.correlation_id_length,
            context_locale_length = facts.context_locale_length,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            deadline_ms = ?facts.deadline_ms,
            query_field,
            operation,
            shipping_option_id_shape,
            requested_locale_length = requested_locale.map(str::len),
            tenant_default_locale_length = tenant_default_locale.map(str::len),
            error_kind,
            owner_code = %error.code,
            owner_kind = error_kind,
            owner_message_presence,
            owner_message_length,
            owner_retryable = error.retryable,
            public_code,
            public_retryable,
            boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
            "commerce GraphQL query shipping-option owner read was rejected"
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn log_fulfillment_port_error(
    error: &PortError,
    context: &PortContext,
    query_field: &'static str,
    operation: &'static str,
    fulfillment_id: Option<Uuid>,
    order_id: Option<Uuid>,
    public_message: &'static str,
    public_code: &'static str,
    public_retryable: bool,
) {
    let facts = fulfillment_query_context_facts(context);
    let error_kind = port_error_kind_name(&error.kind);
    let fulfillment_id_shape = optional_uuid_shape(fulfillment_id);
    let order_id_shape = optional_uuid_shape(order_id);
    let owner_message_presence = text_presence_shape(&error.message);
    let owner_message_length = error.message.chars().count();
    let public_message_presence = text_presence_shape(public_message);
    let public_message_length = public_message.chars().count();
    let diagnostic_error = FulfillmentQueryDiagnosticError;
    if is_technical_port_error(&error.kind) {
        tracing::error!(
            error = ?diagnostic_error,
            owner = "rustok_fulfillment",
            tenant_id_length = facts.tenant_id_length,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            correlation_id_length = facts.correlation_id_length,
            context_locale_length = facts.context_locale_length,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            deadline_ms = ?facts.deadline_ms,
            query_field,
            operation,
            fulfillment_id_shape,
            order_id_shape,
            error_kind,
            owner_code = %error.code,
            owner_kind = error_kind,
            owner_message_presence,
            owner_message_length,
            owner_retryable = error.retryable,
            public_message_presence,
            public_message_length,
            public_code,
            public_retryable,
            boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
            "commerce GraphQL query fulfillment owner read failed"
        );
    } else {
        tracing::warn!(
            error = ?diagnostic_error,
            owner = "rustok_fulfillment",
            tenant_id_length = facts.tenant_id_length,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            correlation_id_length = facts.correlation_id_length,
            context_locale_length = facts.context_locale_length,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            deadline_ms = ?facts.deadline_ms,
            query_field,
            operation,
            fulfillment_id_shape,
            order_id_shape,
            error_kind,
            owner_code = %error.code,
            owner_kind = error_kind,
            owner_message_presence,
            owner_message_length,
            owner_retryable = error.retryable,
            public_message_presence,
            public_message_length,
            public_code,
            public_retryable,
            boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
            "commerce GraphQL query fulfillment owner read was rejected"
        );
    }
}
