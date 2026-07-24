impl InProcessCheckoutPaymentExecutionPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_provider_registry(db, PaymentProviderRegistry::with_manual_provider())
    }

    pub fn with_provider_registry(
        db: DatabaseConnection,
        provider_registry: PaymentProviderRegistry,
    ) -> Self {
        Self {
            payment_service: PaymentService::new(db.clone()),
            operation_journal: PaymentProviderOperationJournal::new(db),
            provider_registry,
        }
    }

    async fn prepare(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        request: PrepareCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        validate_identity(&request.identity)?;
        let metadata = checkout_stage_metadata(request.metadata, &request.identity, "collection");
        let collection = match self
            .payment_service
            .find_reusable_collection_by_cart(tenant_id, request.identity.cart_id)
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?
        {
            Some(existing) => {
                validate_optional_collection_identity(&existing, &request.identity)?;
                self.payment_service
                    .attach_order_to_collection(
                        tenant_id,
                        existing.id,
                        request.identity.order_id,
                        metadata,
                    )
                    .await
                    .map_err(|error| {
                        payment_error_to_port_error(context, owner_operation, error)
                    })?
            }
            None => self
                .payment_service
                .create_collection(
                    tenant_id,
                    CreatePaymentCollectionInput {
                        cart_id: Some(request.identity.cart_id),
                        order_id: Some(request.identity.order_id),
                        customer_id: request.identity.customer_id,
                        currency_code: request.identity.currency_code.clone(),
                        amount: request.identity.amount,
                        metadata,
                    },
                )
                .await
                .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?,
        };
        validate_collection(&collection, tenant_id, &request.identity)?;
        Ok(collection)
    }

    async fn authorize(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        request: AuthorizeCheckoutPaymentCollectionRequest,
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
        let provider_id = request
            .provider_id
            .clone()
            .or_else(|| collection.provider_id.clone())
            .unwrap_or_else(|| MANUAL_PAYMENT_PROVIDER_ID.to_string());
        let idempotency_key = format!("payment_collection:{}:authorize", collection.id);

        match collection.status_kind() {
            PaymentCollectionStatusKind::Authorized | PaymentCollectionStatusKind::Captured => {
                self.commit_existing_provider_operation(
                    context,
                    owner_operation,
                    tenant_id,
                    provider_id.as_str(),
                    idempotency_key.as_str(),
                    "authorize",
                )
                .await?;
                return Ok(collection);
            }
            PaymentCollectionStatusKind::Pending => {}
            PaymentCollectionStatusKind::Cancelled => {
                return Err(PortError::conflict(
                    "payment.checkout_authorize_state_conflict",
                    "cancelled payment collection cannot be authorized",
                ));
            }
            PaymentCollectionStatusKind::Unknown => {
                return Err(manual_reconciliation(
                    context,
                    owner_operation,
                    "payment collection lifecycle is unknown before authorization",
                ));
            }
        }

        let metadata = checkout_stage_metadata(request.metadata, &request.identity, "authorize");
        let local_input = AuthorizePaymentInput {
            provider_id: Some(provider_id.clone()),
            provider_payment_id: request.provider_payment_id.clone(),
            amount: Some(request.identity.amount),
            metadata: metadata.clone(),
        };
        local_input.validate().map_err(|_| {
            PortError::validation(
                "payment.checkout_authorize_request_invalid",
                "checkout payment authorization request is invalid",
            )
        })?;
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
                        "operation": "authorize_payment_collection",
                        "requested_provider_payment_id": request.provider_payment_id,
                    }
                }),
            ),
        };
        let journaled = self
            .execute_journaled_provider_operation(
                context,
                owner_operation,
                "authorize",
                provider_id.as_str(),
                provider_request,
            )
            .await?;
        let provider_result = journaled.result;
        match self
            .payment_service
            .authorize_collection(
                tenant_id,
                collection.id,
                AuthorizePaymentInput {
                    provider_id: Some(provider_result.provider_id),
                    provider_payment_id: provider_result
                        .external_reference
                        .or(local_input.provider_payment_id),
                    amount: Some(provider_result.authorized_amount),
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
                    "authorize",
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
                    "authorize",
                )
                .await;
                tracing::error!(
                    operation_id = %journaled.operation_id,
                    error = ?error,
                    correlation_id = %context.correlation_id,
                    tenant_id = %context.tenant_id,
                    operation = owner_operation,
                    code = "payment.checkout_execution_local_persistence_failed",
                    "payment provider authorization succeeded but local persistence failed"
                );
                Err(manual_reconciliation(
                    context,
                    owner_operation,
                    "payment authorization succeeded externally but local persistence is incomplete",
                ))
            }
        }
    }
}
