use super::*;

impl InProcessCheckoutPaymentExecutionPort {
    pub(super) async fn enrich_provider_request(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        provider_operation: &str,
        provider_id: &str,
        mut request: PaymentProviderOperationRequest,
    ) -> Result<PaymentProviderOperationRequest, PortError> {
        if provider_id == MANUAL_PAYMENT_PROVIDER_ID || provider_operation == "authorize" {
            return Ok(request);
        }
        if metadata_string(&request.metadata, "provider_payment_id").is_some() {
            return Ok(request);
        }
        let authorize_key = format!("payment_collection:{}:authorize", request.collection_id);
        let authorize = self
            .operation_journal
            .find_by_key(request.tenant_id, provider_id, authorize_key.as_str())
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?
            .ok_or_else(|| {
                manual_reconciliation(
                    context,
                    owner_operation,
                    CheckoutPaymentExecutionReconciliationReason::MissingDurableAuthorizeProviderIdentity,
                )
            })?;
        if !matches!(
            authorize.status.as_str(),
            PROVIDER_OPERATION_COMMITTED
                | PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            return Err(manual_reconciliation(
                context,
                owner_operation,
                CheckoutPaymentExecutionReconciliationReason::IncompleteAuthorizeOperation,
            ));
        }
        let provider_payment_id = authorize
            .provider_reference
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                authorize
                    .provider_result
                    .as_ref()
                    .and_then(|result| result.get("external_reference"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .ok_or_else(|| {
                manual_reconciliation(
                    context,
                    owner_operation,
                    CheckoutPaymentExecutionReconciliationReason::MissingDurableProviderPaymentIdentity,
                )
            })?;
        insert_metadata_string(
            &mut request.metadata,
            "provider_payment_id",
            provider_payment_id,
        )?;
        Ok(request)
    }

    pub(super) async fn commit_existing_provider_operation(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        provider_id: &str,
        idempotency_key: &str,
        provider_operation: &'static str,
    ) -> Result<(), PortError> {
        if let Some(existing) = self
            .operation_journal
            .find_by_key(tenant_id, provider_id, idempotency_key)
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?
            && matches!(
                existing.status.as_str(),
                PROVIDER_OPERATION_SUCCEEDED | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            )
        {
            self.mark_journal_committed(
                context,
                owner_operation,
                tenant_id,
                existing.id,
                provider_operation,
            )
                .await?;
        }
        Ok(())
    }

    pub(super) async fn mark_journal_committed(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        operation_id: Uuid,
        provider_operation: &'static str,
    ) -> Result<(), PortError> {
        if let Err(error) = self.operation_journal.mark_committed(tenant_id, operation_id).await {
            let context_facts = checkout_payment_execution_context_facts(context);
            let error_facts = checkout_payment_execution_payment_error_facts(&error);
            tracing::error!(
                operation_id_non_nil = !operation_id.is_nil(),
                provider_operation,
                commit_checkpoint_error_variant = error_facts.error_variant,
                commit_checkpoint_error_text_field_count = error_facts.text_field_count,
                commit_checkpoint_error_text_total_length = error_facts.text_total_length,
                commit_checkpoint_error_uuid_field_count = error_facts.uuid_field_count,
                commit_checkpoint_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
                commit_checkpoint_error_opaque_payload_present = error_facts.opaque_payload_present,
                correlation_id = %context.correlation_id,
                tenant_id_length = context_facts.tenant_id_length,
                actor_kind = context_facts.actor_kind,
                channel_present = context_facts.channel_present,
                locale_length = context_facts.locale_length,
                causation_id_present = context_facts.causation_id_present,
                idempotency_key_present = context_facts.idempotency_key_present,
                operation = owner_operation,
                code = "payment.checkout_execution_commit_checkpoint_failed",
                boundary = PAYMENT_EXECUTION_BOUNDARY,
                "payment local commit checkpoint failed"
            );
            if let Err(mark_error) = self
                .operation_journal
                .mark_reconciliation_required(
                    tenant_id,
                    operation_id,
                    format!("payment.local_{provider_operation}_commit_checkpoint_failed"),
                )
                .await
            {
                // The journal write that records the manual-reconciliation state
                // can itself fail. That failure must be visible, but this path
                // logs bounded facts only: no raw operation id, no error text, and
                // no second PaymentError extraction (the checkpoint encoding
                // contract fixes that count).
                tracing::error!(
                    operation_id_non_nil = !operation_id.is_nil(),
                    provider_operation,
                    reconciliation_mark_failed = true,
                    reconciliation_mark_error_is_validation = matches!(
                        mark_error,
                        PaymentError::Validation(_)
                    ),
                    correlation_id = %context.correlation_id,
                    operation = owner_operation,
                    code = "payment.checkout_execution_reconciliation_mark_failed",
                    boundary = PAYMENT_EXECUTION_BOUNDARY,
                    "payment provider operation could not be marked as reconciliation required"
                );
            }
            return Err(manual_reconciliation(
                context,
                owner_operation,
                CheckoutPaymentExecutionReconciliationReason::CommitCheckpointFailed,
            ));
        }
        Ok(())
    }

    pub(super) async fn mark_local_persistence_failed(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        operation_id: Uuid,
        provider_operation: &'static str,
    ) {
        if let Err(error) = self
            .operation_journal
            .mark_reconciliation_required(
                tenant_id,
                operation_id,
                format!("payment.local_{provider_operation}_persistence_failed"),
            )
            .await
        {
            let context_facts = checkout_payment_execution_context_facts(context);
            let error_facts = checkout_payment_execution_payment_error_facts(&error);
            tracing::error!(
                operation_id_non_nil = !operation_id.is_nil(),
                provider_operation,
                reconciliation_checkpoint_error_variant = error_facts.error_variant,
                reconciliation_checkpoint_error_text_field_count = error_facts.text_field_count,
                reconciliation_checkpoint_error_text_total_length = error_facts.text_total_length,
                reconciliation_checkpoint_error_uuid_field_count = error_facts.uuid_field_count,
                reconciliation_checkpoint_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
                reconciliation_checkpoint_error_opaque_payload_present = error_facts.opaque_payload_present,
                correlation_id = %context.correlation_id,
                tenant_id_length = context_facts.tenant_id_length,
                actor_kind = context_facts.actor_kind,
                channel_present = context_facts.channel_present,
                locale_length = context_facts.locale_length,
                causation_id_present = context_facts.causation_id_present,
                idempotency_key_present = context_facts.idempotency_key_present,
                operation = owner_operation,
                code = "payment.checkout_execution_reconciliation_checkpoint_failed",
                boundary = PAYMENT_EXECUTION_BOUNDARY,
                "payment local persistence failure checkpoint failed"
            );
        }
    }
}
