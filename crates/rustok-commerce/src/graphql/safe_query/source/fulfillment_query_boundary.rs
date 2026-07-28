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
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.graphql-query-shipping-options"),
        locale,
        format!("graphql-fulfillment:{query_field}:{resource}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

fn fulfillment_query_context(
    tenant_id: Uuid,
    query_field: &'static str,
    operation: &'static str,
    fulfillment_id: Option<Uuid>,
    order_id: Option<Uuid>,
) -> PortContext {
    let resource = fulfillment_id.or(order_id).unwrap_or(tenant_id);
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.graphql-query-fulfillments"),
        "en",
        format!("graphql-fulfillment-lifecycle:{query_field}:{operation}:{resource}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
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
        if optional_not_found { "OPTIONAL_NONE" } else { code },
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
    let (public_message, public_code, public_retryable) =
        public_fulfillment_port_policy(&error.kind);
    log_fulfillment_port_error(
        &error,
        context,
        query_field,
        operation,
        fulfillment_id,
        order_id,
        public_message,
        public_code,
        public_retryable,
    );

    if matches!(&error.kind, PortErrorKind::NotFound) {
        FulfillmentError::FulfillmentNotFound(
            fulfillment_id.or(order_id).unwrap_or_else(Uuid::nil),
        )
    } else {
        FulfillmentError::Public(BoundaryError::Public {
            message: public_message,
            code: public_code,
            retryable: public_retryable,
        })
    }
}

fn public_fulfillment_port_policy(
    kind: &PortErrorKind,
) -> (&'static str, &'static str, bool) {
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
        PortErrorKind::Unavailable
            | PortErrorKind::Timeout
            | PortErrorKind::InvariantViolation
    )
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
    if technical {
        tracing::error!(
            error = ?error,
            owner = "rustok_fulfillment",
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            context_locale_length = context.locale.len(),
            deadline_ms = ?context.deadline_ms,
            query_field,
            operation,
            shipping_option_id = ?shipping_option_id,
            requested_locale_length = requested_locale.map(str::len),
            tenant_default_locale_length = tenant_default_locale.map(str::len),
            error_kind,
            owner_code = %error.code,
            owner_kind = ?error.kind,
            owner_retryable = error.retryable,
            public_code,
            public_retryable,
            boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
            "commerce GraphQL query shipping-option owner read failed"
        );
    } else {
        tracing::warn!(
            owner = "rustok_fulfillment",
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            context_locale_length = context.locale.len(),
            deadline_ms = ?context.deadline_ms,
            query_field,
            operation,
            shipping_option_id = ?shipping_option_id,
            requested_locale_length = requested_locale.map(str::len),
            tenant_default_locale_length = tenant_default_locale.map(str::len),
            error_kind,
            owner_code = %error.code,
            owner_kind = ?error.kind,
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
    let error_kind = port_error_kind_name(&error.kind);
    if is_technical_port_error(&error.kind) {
        tracing::error!(
            error = ?error,
            owner = "rustok_fulfillment",
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            context_locale_length = context.locale.len(),
            deadline_ms = ?context.deadline_ms,
            query_field,
            operation,
            fulfillment_id = ?fulfillment_id,
            order_id = ?order_id,
            error_kind,
            owner_code = %error.code,
            owner_kind = ?error.kind,
            owner_retryable = error.retryable,
            public_message,
            public_code,
            public_retryable,
            boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
            "commerce GraphQL query fulfillment owner read failed"
        );
    } else {
        tracing::warn!(
            owner = "rustok_fulfillment",
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            context_locale_length = context.locale.len(),
            deadline_ms = ?context.deadline_ms,
            query_field,
            operation,
            fulfillment_id = ?fulfillment_id,
            order_id = ?order_id,
            error_kind,
            owner_code = %error.code,
            owner_kind = ?error.kind,
            owner_retryable = error.retryable,
            public_message,
            public_code,
            public_retryable,
            boundary = GRAPHQL_QUERY_FULFILLMENT_BOUNDARY,
            "commerce GraphQL query fulfillment owner read was rejected"
        );
    }
}
