pub fn in_process_checkout_payment_execution_port(
    db: DatabaseConnection,
) -> Arc<dyn CheckoutPaymentExecutionPort> {
    Arc::new(InProcessCheckoutPaymentExecutionPort::new(db))
}

#[derive(Debug)]
struct CheckoutPaymentExecutionDiagnosticFacts {
    checkout_operation_id: Uuid,
    cart_id: Uuid,
    order_id: Uuid,
    customer_id: Option<Uuid>,
    collection_id: Option<Uuid>,
    amount: Decimal,
    currency_code_length: usize,
    order_plan_hash_length: usize,
    requested_provider_id_length: Option<usize>,
    provider_payment_id_length: Option<usize>,
}

#[async_trait]
impl CheckoutPaymentExecutionPort for InProcessCheckoutPaymentExecutionPort {
    async fn prepare_checkout_collection(
        &self,
        context: PortContext,
        request: PrepareCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        let owner_operation = PREPARE_CHECKOUT_COLLECTION_OPERATION;
        require_checkout_payment_write_admission(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        require_operation_context(
            &context,
            owner_operation,
            request.identity.checkout_operation_id,
        )?;
        let diagnostic_context = context.clone();
        let diagnostic_facts = checkout_payment_execution_diagnostic_facts(
            &request.identity,
            None,
            None,
            None,
        );
        let result = self.prepare(&context, owner_operation, tenant_id, request).await;
        result.map_err(|error| {
            map_checkout_payment_execution_local_port_error(
                &diagnostic_context,
                owner_operation,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn authorize_checkout_collection(
        &self,
        context: PortContext,
        request: AuthorizeCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        let owner_operation = AUTHORIZE_CHECKOUT_COLLECTION_OPERATION;
        require_checkout_payment_write_admission(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        require_operation_context(
            &context,
            owner_operation,
            request.identity.checkout_operation_id,
        )?;
        let diagnostic_context = context.clone();
        let diagnostic_facts = checkout_payment_execution_diagnostic_facts(
            &request.identity,
            Some(request.collection_id),
            request.provider_id.as_deref(),
            request.provider_payment_id.as_deref(),
        );
        let result = self
            .authorize(&context, owner_operation, tenant_id, request)
            .await;
        result.map_err(|error| {
            map_checkout_payment_execution_local_port_error(
                &diagnostic_context,
                owner_operation,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn capture_checkout_collection(
        &self,
        context: PortContext,
        request: CaptureCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        let owner_operation = CAPTURE_CHECKOUT_COLLECTION_OPERATION;
        require_checkout_payment_write_admission(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        require_operation_context(
            &context,
            owner_operation,
            request.identity.checkout_operation_id,
        )?;
        let diagnostic_context = context.clone();
        let diagnostic_facts = checkout_payment_execution_diagnostic_facts(
            &request.identity,
            Some(request.collection_id),
            None,
            None,
        );
        let result = self.capture(&context, owner_operation, tenant_id, request).await;
        result.map_err(|error| {
            map_checkout_payment_execution_local_port_error(
                &diagnostic_context,
                owner_operation,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn read_checkout_collection(
        &self,
        context: PortContext,
        request: ReadCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        let owner_operation = READ_CHECKOUT_COLLECTION_OPERATION;
        require_checkout_payment_read_admission(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        require_operation_context(
            &context,
            owner_operation,
            request.identity.checkout_operation_id,
        )?;
        let diagnostic_context = context.clone();
        let diagnostic_facts = checkout_payment_execution_diagnostic_facts(
            &request.identity,
            Some(request.collection_id),
            None,
            None,
        );
        let result = self.read(&context, owner_operation, tenant_id, request).await;
        result.map_err(|error| {
            map_checkout_payment_execution_local_port_error(
                &diagnostic_context,
                owner_operation,
                &diagnostic_facts,
                error,
            )
        })
    }
}

impl InProcessCheckoutPaymentExecutionPort {
    async fn read(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        request: ReadCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        validate_identity(&request.identity)?;
        let collection = self
            .payment_service
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?;
        validate_collection(&collection, tenant_id, &request.identity)?;
        Ok(collection)
    }
}

fn checkout_payment_execution_diagnostic_facts(
    identity: &CheckoutPaymentIdentity,
    collection_id: Option<Uuid>,
    requested_provider_id: Option<&str>,
    provider_payment_id: Option<&str>,
) -> CheckoutPaymentExecutionDiagnosticFacts {
    CheckoutPaymentExecutionDiagnosticFacts {
        checkout_operation_id: identity.checkout_operation_id,
        cart_id: identity.cart_id,
        order_id: identity.order_id,
        customer_id: identity.customer_id,
        collection_id,
        amount: identity.amount,
        currency_code_length: identity.currency_code.chars().count(),
        order_plan_hash_length: identity.order_plan_hash.chars().count(),
        requested_provider_id_length: requested_provider_id.map(|value| value.chars().count()),
        provider_payment_id_length: provider_payment_id.map(|value| value.chars().count()),
    }
}

fn map_checkout_payment_execution_local_port_error(
    context: &PortContext,
    operation: &'static str,
    facts: &CheckoutPaymentExecutionDiagnosticFacts,
    error: PortError,
) -> PortError {
    let local_operation = match (error.code.as_str(), error.message.as_str()) {
        (
            "payment.checkout_identity_invalid",
            "checkout payment identity contains invalid UUID or amount fields",
        ) => "validate_checkout_identity",
        (
            "payment.checkout_currency_invalid",
            "checkout payment currency must be a three-letter alphabetic code",
        ) => "validate_checkout_currency",
        (
            "payment.checkout_plan_hash_invalid",
            "checkout payment order plan hash must be a 64-character hexadecimal value",
        ) => "validate_checkout_plan_hash",
        (
            "payment.checkout_collection_id_invalid",
            "checkout payment collection identity must be a non-nil UUID",
        ) => "validate_collection_id",
        (
            "payment.checkout_collection_operation_conflict",
            "payment collection belongs to another checkout operation",
        ) => "validate_collection_operation",
        (
            "payment.checkout_collection_plan_conflict",
            "payment collection belongs to another checkout order plan",
        ) => "validate_collection_plan",
        (
            "payment.checkout_collection_identity_conflict",
            "payment collection does not match the checkout identity",
        ) => "validate_collection_identity",
        (
            "payment.checkout_collection_identity_missing",
            "payment collection has no checkout identity",
        ) => "require_collection_identity",
        (
            "payment.checkout_collection_identity_conflict",
            "payment collection has mismatched checkout identity",
        ) => "validate_collection_identity",
        (
            "payment.checkout_authorize_state_conflict",
            "cancelled payment collection cannot be authorized",
        ) if operation == AUTHORIZE_CHECKOUT_COLLECTION_OPERATION => {
            "validate_authorize_lifecycle"
        }
        (
            "payment.checkout_capture_state_conflict",
            "payment collection lifecycle does not allow capture",
        ) if operation == CAPTURE_CHECKOUT_COLLECTION_OPERATION => "validate_capture_lifecycle",
        (
            "payment.checkout_authorize_request_invalid",
            "checkout payment authorization request is invalid",
        ) if operation == AUTHORIZE_CHECKOUT_COLLECTION_OPERATION => {
            "validate_authorize_request"
        }
        (
            "payment.provider_metadata_invalid",
            "payment provider metadata must be a JSON object",
        ) => "validate_provider_metadata",
        (
            "payment.provider_identity_conflict",
            "payment provider identity conflicts with the durable authorize operation",
        ) => "validate_provider_identity",
        (
            "payment.provider_idempotency_key_required",
            "payment provider operation requires an idempotency key",
        ) => "require_provider_idempotency_key",
        (
            "payment.provider_request_encoding_failed",
            "payment provider request could not be encoded",
        ) => "encode_provider_request",
        (
            "payment.provider_operation_invalid",
            "unsupported checkout payment provider operation",
        ) => "select_provider_operation",
        (
            "payment.database_unavailable",
            "payment storage is temporarily unavailable",
        ) => "owner_storage",
        (
            "payment.checkout_execution_validation",
            "checkout payment request is invalid",
        ) => "validate_owner_request",
        ("payment.collection_not_found", "payment collection was not found") => {
            "load_collection"
        }
        ("payment.payment_not_found", "payment was not found") => "load_payment",
        ("payment.refund_not_found", "refund was not found") => "load_refund",
        (
            "payment.checkout_execution_state_conflict",
            "payment lifecycle conflicts with checkout execution",
        ) => "apply_payment_lifecycle",
        (
            "payment.provider_unavailable",
            "payment provider is temporarily unavailable",
        ) => "execute_provider_operation",
        (
            "payment.provider_rejected",
            "payment provider rejected the requested operation",
        ) => "execute_provider_operation",
        (
            "payment.checkout_execution_manual_reconciliation",
            "payment checkout execution requires manual reconciliation",
        ) => "require_manual_reconciliation",
        (
            "payment.provider_not_configured",
            "payment provider is not configured",
        ) => "resolve_provider",
        _ => return error,
    };
    let integrity_failure = matches!(
        local_operation,
        "require_collection_identity" | "require_manual_reconciliation"
    );
    let technical_failure = integrity_failure
        || matches!(
            &error.kind,
            PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
        );
    if technical_failure {
        tracing::error!(
            error = ?error,
            owner = "rustok_payment",
            operation,
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
            cart_id = %facts.cart_id,
            order_id = %facts.order_id,
            customer_id = ?facts.customer_id,
            collection_id = ?facts.collection_id,
            request_amount = %facts.amount,
            currency_code_length = facts.currency_code_length,
            order_plan_hash_length = facts.order_plan_hash_length,
            requested_provider_id_length = ?facts.requested_provider_id_length,
            provider_payment_id_length = ?facts.provider_payment_id_length,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = "checkout_payment_execution_port",
            "payment checkout execution local technical outcome retained delegated context"
        );
    } else {
        tracing::warn!(
            error = ?error,
            owner = "rustok_payment",
            operation,
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
            cart_id = %facts.cart_id,
            order_id = %facts.order_id,
            customer_id = ?facts.customer_id,
            collection_id = ?facts.collection_id,
            request_amount = %facts.amount,
            currency_code_length = facts.currency_code_length,
            order_plan_hash_length = facts.order_plan_hash_length,
            requested_provider_id_length = ?facts.requested_provider_id_length,
            provider_payment_id_length = ?facts.provider_payment_id_length,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = "checkout_payment_execution_port",
            "payment checkout execution local outcome retained delegated context"
        );
    }
    error
}
