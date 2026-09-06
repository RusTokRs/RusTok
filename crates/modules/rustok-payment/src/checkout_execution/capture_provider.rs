impl InProcessCheckoutPaymentExecutionPort {
    async fn capture(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        request: CaptureCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        validate_identity(&request.identity)?;
        if request.collection_id.is_nil() {
            return Err(PortError::validation(
                "payment.checkout_collection_id_invalid",
                "checkout payment collection identity must be a non-nil UUID",
            ));
        }
        let collection = self
            .payment_service
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?;
        validate_collection(&collection, tenant_id, &request.identity)?;
        let provider_id = provider_id_for_collection(&collection);
        let idempotency_key = format!("payment_collection:{}:capture", collection.id);

        match collection.status_kind() {
            PaymentCollectionStatusKind::Captured => {
                self.commit_existing_provider_operation(
                    context,
                    owner_operation,
                    tenant_id,
                    provider_id.as_str(),
                    idempotency_key.as_str(),
                    "capture",
                )
                .await?;
                return Ok(collection);
            }
            PaymentCollectionStatusKind::Authorized => {}
            PaymentCollectionStatusKind::Pending | PaymentCollectionStatusKind::Cancelled => {
                return Err(PortError::conflict(
                    "payment.checkout_capture_state_conflict",
                    "payment collection lifecycle does not allow capture",
                ));
            }
            PaymentCollectionStatusKind::Unknown => {
                return Err(manual_reconciliation(
                    context,
                    owner_operation,
                    CheckoutPaymentExecutionReconciliationReason::UnknownCollectionLifecycleBeforeCapture,
                ));
            }
        }

        let metadata = checkout_stage_metadata(request.metadata, &request.identity, "capture");
        let provider_request = PaymentProviderOperationRequest {
            tenant_id,
            collection_id: collection.id,
            amount: request.identity.amount,
            currency_code: collection.currency_code.clone(),
            idempotency_key: Some(idempotency_key),
            metadata: merge_metadata(
                metadata.clone(),
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "capture_payment_collection"
                    }
                }),
            ),
        };
        let journaled = self
            .execute_journaled_provider_operation(
                context,
                owner_operation,
                "capture",
                provider_id.as_str(),
                provider_request,
            )
            .await?;
        let provider_result = journaled.result;
        match self
            .payment_service
            .capture_collection(
                tenant_id,
                collection.id,
                CapturePaymentInput {
                    amount: Some(provider_result.captured_amount),
                    metadata: merge_metadata(metadata, provider_result.metadata),
                },
            )
            .await
        {
            Ok(collection) => {
                self.mark_journal_committed(
                    context,
                    owner_operation,
                    journaled.operation_id,
                    "capture",
                )
                .await?;
                validate_collection(&collection, tenant_id, &request.identity)?;
                Ok(collection)
            }
            Err(error) => {
                self.mark_local_persistence_failed(
                    context,
                    owner_operation,
                    journaled.operation_id,
                    "capture",
                )
                .await;
                let context_facts = checkout_payment_execution_context_facts(context);
                let error_facts = checkout_payment_execution_payment_error_facts(&error);
                tracing::error!(
                    operation_id_non_nil = !journaled.operation_id.is_nil(),
                    provider_operation = "capture",
                    local_persistence_error_variant = error_facts.error_variant,
                    local_persistence_error_text_field_count = error_facts.text_field_count,
                    local_persistence_error_text_total_length = error_facts.text_total_length,
                    local_persistence_error_uuid_field_count = error_facts.uuid_field_count,
                    local_persistence_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
                    local_persistence_error_opaque_payload_present = error_facts.opaque_payload_present,
                    correlation_id = %context.correlation_id,
                    tenant_id_length = context_facts.tenant_id_length,
                    actor_kind = context_facts.actor_kind,
                    channel_present = context_facts.channel_present,
                    locale_length = context_facts.locale_length,
                    causation_id_present = context_facts.causation_id_present,
                    idempotency_key_present = context_facts.idempotency_key_present,
                    operation = owner_operation,
                    code = "payment.checkout_execution_local_persistence_failed",
                    boundary = PAYMENT_EXECUTION_BOUNDARY,
                    "payment provider capture succeeded but local persistence failed"
                );
                Err(manual_reconciliation(
                    context,
                    owner_operation,
                    CheckoutPaymentExecutionReconciliationReason::CaptureLocalPersistenceIncomplete,
                ))
            }
        }
    }

    async fn execute_journaled_provider_operation(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        provider_operation: &'static str,
        provider_id: &str,
        request: PaymentProviderOperationRequest,
    ) -> Result<JournaledProviderResult, PortError> {
        let request = self
            .enrich_provider_request(
                context,
                owner_operation,
                provider_operation,
                provider_id,
                request,
            )
            .await?;
        let idempotency_key = request
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                PortError::validation(
                    "payment.provider_idempotency_key_required",
                    "payment provider operation requires an idempotency key",
                )
            })?
            .to_string();
        let request_payload = serde_json::to_value(&request).map_err(|_| {
            let context_facts = checkout_payment_execution_context_facts(context);
            tracing::error!(
                request_encoding_failed = true,
                provider_operation,
                provider_id_length = provider_id.chars().count(),
                correlation_id = %context.correlation_id,
                tenant_id_length = context_facts.tenant_id_length,
                actor_kind = context_facts.actor_kind,
                channel_present = context_facts.channel_present,
                locale_length = context_facts.locale_length,
                causation_id_present = context_facts.causation_id_present,
                idempotency_key_present = context_facts.idempotency_key_present,
                operation = owner_operation,
                code = "payment.provider_request_encoding_failed",
                boundary = PAYMENT_EXECUTION_BOUNDARY,
                "payment provider request encoding failed"
            );
            PortError::invariant_violation(
                "payment.provider_request_encoding_failed",
                "payment provider request could not be encoded",
            )
        })?;
        let journal_operation = self
            .operation_journal
            .begin(BeginProviderOperation {
                tenant_id: request.tenant_id,
                payment_collection_id: request.collection_id,
                refund_id: None,
                operation: provider_operation.to_string(),
                provider_id: provider_id.to_string(),
                idempotency_key,
                request_payload,
            })
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?;

        if let Some(result) =
            persisted_provider_result(context, owner_operation, &journal_operation)?
        {
            return Ok(JournaledProviderResult {
                operation_id: journal_operation.id,
                result,
            });
        }

        let claimed = self
            .operation_journal
            .claim_execution(journal_operation.id)
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?;
        if claimed.is_none() {
            let current = self
                .operation_journal
                .get(journal_operation.id)
                .await
                .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?;
            if let Some(result) = persisted_provider_result(context, owner_operation, &current)? {
                return Ok(JournaledProviderResult {
                    operation_id: current.id,
                    result,
                });
            }
            return Err(manual_reconciliation(
                context,
                owner_operation,
                CheckoutPaymentExecutionReconciliationReason::ProviderOperationInProgressOrReconciliationRequired,
            ));
        }

        let provider_result = match provider_operation {
            "authorize" => {
                self.provider_registry
                    .execute_authorize(provider_id, request)
                    .await
            }
            "capture" => {
                self.provider_registry
                    .execute_capture(provider_id, request)
                    .await
            }
            _ => {
                return Err(PortError::validation(
                    "payment.provider_operation_invalid",
                    "unsupported checkout payment provider operation",
                ));
            }
        };
        let provider_result = match provider_result {
            Ok(result) => result,
            Err(error) => {
                let code = stable_payment_error_code(&error);
                let checkpoint = if error.requires_provider_reconciliation() {
                    self.operation_journal
                        .mark_reconciliation_required(journal_operation.id, code)
                        .await
                } else {
                    self.operation_journal
                        .mark_provider_error(journal_operation.id, code)
                        .await
                };
                if let Err(checkpoint_error) = checkpoint {
                    let context_facts = checkout_payment_execution_context_facts(context);
                    let error_facts =
                        checkout_payment_execution_payment_error_facts(&checkpoint_error);
                    tracing::error!(
                        operation_id_non_nil = !journal_operation.id.is_nil(),
                        provider_operation,
                        provider_id_length = provider_id.chars().count(),
                        provider_failure_checkpoint_error_variant = error_facts.error_variant,
                        provider_failure_checkpoint_error_text_field_count = error_facts.text_field_count,
                        provider_failure_checkpoint_error_text_total_length = error_facts.text_total_length,
                        provider_failure_checkpoint_error_uuid_field_count = error_facts.uuid_field_count,
                        provider_failure_checkpoint_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
                        provider_failure_checkpoint_error_opaque_payload_present = error_facts.opaque_payload_present,
                        correlation_id = %context.correlation_id,
                        tenant_id_length = context_facts.tenant_id_length,
                        actor_kind = context_facts.actor_kind,
                        channel_present = context_facts.channel_present,
                        locale_length = context_facts.locale_length,
                        causation_id_present = context_facts.causation_id_present,
                        idempotency_key_present = context_facts.idempotency_key_present,
                        operation = owner_operation,
                        code = "payment.checkout_execution_provider_failure_checkpoint_failed",
                        boundary = PAYMENT_EXECUTION_BOUNDARY,
                        "payment provider failure checkpoint failed"
                    );
                    return Err(manual_reconciliation(
                        context,
                        owner_operation,
                        CheckoutPaymentExecutionReconciliationReason::ProviderFailureCheckpointFailed,
                    ));
                }
                return Err(payment_error_to_port_error(context, owner_operation, error));
            }
        };
        let result_payload = serde_json::to_value(&provider_result).map_err(|_| {
            let context_facts = checkout_payment_execution_context_facts(context);
            tracing::error!(
                operation_id_non_nil = !journal_operation.id.is_nil(),
                provider_operation,
                provider_id_length = provider_id.chars().count(),
                result_encoding_failed = true,
                correlation_id = %context.correlation_id,
                tenant_id_length = context_facts.tenant_id_length,
                actor_kind = context_facts.actor_kind,
                channel_present = context_facts.channel_present,
                locale_length = context_facts.locale_length,
                causation_id_present = context_facts.causation_id_present,
                idempotency_key_present = context_facts.idempotency_key_present,
                operation = owner_operation,
                code = "payment.provider_result_encoding_failed",
                boundary = PAYMENT_EXECUTION_BOUNDARY,
                "payment provider result encoding failed"
            );
            manual_reconciliation(
                context,
                owner_operation,
                CheckoutPaymentExecutionReconciliationReason::ProviderResultEncodingFailed,
            )
        })?;
        self.operation_journal
            .mark_provider_succeeded(
                journal_operation.id,
                provider_result.external_reference.clone(),
                result_payload,
            )
            .await
            .map_err(|error| {
                let context_facts = checkout_payment_execution_context_facts(context);
                let error_facts = checkout_payment_execution_payment_error_facts(&error);
                tracing::error!(
                    operation_id_non_nil = !journal_operation.id.is_nil(),
                    provider_operation,
                    provider_id_length = provider_id.chars().count(),
                    provider_success_checkpoint_error_variant = error_facts.error_variant,
                    provider_success_checkpoint_error_text_field_count = error_facts.text_field_count,
                    provider_success_checkpoint_error_text_total_length = error_facts.text_total_length,
                    provider_success_checkpoint_error_uuid_field_count = error_facts.uuid_field_count,
                    provider_success_checkpoint_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
                    provider_success_checkpoint_error_opaque_payload_present = error_facts.opaque_payload_present,
                    correlation_id = %context.correlation_id,
                    tenant_id_length = context_facts.tenant_id_length,
                    actor_kind = context_facts.actor_kind,
                    channel_present = context_facts.channel_present,
                    locale_length = context_facts.locale_length,
                    causation_id_present = context_facts.causation_id_present,
                    idempotency_key_present = context_facts.idempotency_key_present,
                    operation = owner_operation,
                    code = "payment.checkout_execution_provider_checkpoint_failed",
                    boundary = PAYMENT_EXECUTION_BOUNDARY,
                    "payment provider success checkpoint failed"
                );
                manual_reconciliation(
                    context,
                    owner_operation,
                    CheckoutPaymentExecutionReconciliationReason::ProviderSuccessCheckpointFailed,
                )
            })?;
        Ok(JournaledProviderResult {
            operation_id: journal_operation.id,
            result: provider_result,
        })
    }
}
