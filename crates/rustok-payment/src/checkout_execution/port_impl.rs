pub fn in_process_checkout_payment_execution_port(
    db: DatabaseConnection,
) -> Arc<dyn CheckoutPaymentExecutionPort> {
    Arc::new(InProcessCheckoutPaymentExecutionPort::new(db))
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
        let diagnostic_facts =
            checkout_payment_execution_diagnostic_facts(&request.identity, None, None, None);
        let result = self
            .prepare(&context, owner_operation, tenant_id, request)
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
        let result = self
            .capture(&context, owner_operation, tenant_id, request)
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
        let result = self
            .read(&context, owner_operation, tenant_id, request)
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

fn map_checkout_payment_execution_local_port_error(
    context: &PortContext,
    operation: &'static str,
    facts: &CheckoutPaymentExecutionDiagnosticFacts,
    error: PortError,
) -> PortError {
    let Some(local_operation) =
        checkout_payment_execution_local_operation(operation, error.code.as_str())
    else {
        return error;
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
    let context_facts = checkout_payment_execution_context_facts(context);
    let error_facts = checkout_payment_execution_port_error_facts(&error);
    if technical_failure {
        tracing::error!(
            owner = "rustok_payment",
            operation,
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
            order_id_non_nil = facts.order_id_non_nil,
            customer_id_present = facts.customer_id_present,
            customer_id_non_nil = ?facts.customer_id_non_nil,
            collection_id_present = facts.collection_id_present,
            collection_id_non_nil = ?facts.collection_id_non_nil,
            amount_text_length = facts.amount_text_length,
            currency_code_length = facts.currency_code_length,
            order_plan_hash_length = facts.order_plan_hash_length,
            requested_provider_id_present = facts.requested_provider_id_present,
            requested_provider_id_length = ?facts.requested_provider_id_length,
            provider_payment_id_present = facts.provider_payment_id_present,
            provider_payment_id_length = ?facts.provider_payment_id_length,
            internal_code = %error.code,
            error_message_present = error_facts.message_present,
            error_message_length = error_facts.message_length,
            error_kind = error_facts.error_kind,
            retryable = error.retryable,
            boundary = PAYMENT_EXECUTION_BOUNDARY,
            "payment checkout execution local technical outcome retained safe context"
        );
    } else {
        tracing::warn!(
            owner = "rustok_payment",
            operation,
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
            order_id_non_nil = facts.order_id_non_nil,
            customer_id_present = facts.customer_id_present,
            customer_id_non_nil = ?facts.customer_id_non_nil,
            collection_id_present = facts.collection_id_present,
            collection_id_non_nil = ?facts.collection_id_non_nil,
            amount_text_length = facts.amount_text_length,
            currency_code_length = facts.currency_code_length,
            order_plan_hash_length = facts.order_plan_hash_length,
            requested_provider_id_present = facts.requested_provider_id_present,
            requested_provider_id_length = ?facts.requested_provider_id_length,
            provider_payment_id_present = facts.provider_payment_id_present,
            provider_payment_id_length = ?facts.provider_payment_id_length,
            internal_code = %error.code,
            error_message_present = error_facts.message_present,
            error_message_length = error_facts.message_length,
            error_kind = error_facts.error_kind,
            retryable = error.retryable,
            boundary = PAYMENT_EXECUTION_BOUNDARY,
            "payment checkout execution local outcome retained safe context"
        );
    }
    error
}
