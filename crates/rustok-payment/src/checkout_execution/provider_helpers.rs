impl InProcessCheckoutPaymentExecutionPort {
    async fn enrich_provider_request(
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

    async fn commit_existing_provider_operation(
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
            self.mark_journal_committed(context, owner_operation, existing.id, provider_operation)
                .await?;
        }
        Ok(())
    }

    async fn mark_journal_committed(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        operation_id: Uuid,
        provider_operation: &'static str,
    ) -> Result<(), PortError> {
        if let Err(error) = self.operation_journal.mark_committed(operation_id).await {
            let context_facts = checkout_payment_execution_context_facts(context);
            tracing::error!(
                operation_id_non_nil = !operation_id.is_nil(),
                provider_operation,
                error = ?error,
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
            let _ = self
                .operation_journal
                .mark_reconciliation_required(
                    operation_id,
                    format!("payment.local_{provider_operation}_commit_checkpoint_failed"),
                )
                .await;
            return Err(manual_reconciliation(
                context,
                owner_operation,
                CheckoutPaymentExecutionReconciliationReason::CommitCheckpointFailed,
            ));
        }
        Ok(())
    }

    async fn mark_local_persistence_failed(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        operation_id: Uuid,
        provider_operation: &'static str,
    ) {
        if let Err(error) = self
            .operation_journal
            .mark_reconciliation_required(
                operation_id,
                format!("payment.local_{provider_operation}_persistence_failed"),
            )
            .await
        {
            let context_facts = checkout_payment_execution_context_facts(context);
            tracing::error!(
                operation_id_non_nil = !operation_id.is_nil(),
                provider_operation,
                error = ?error,
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
