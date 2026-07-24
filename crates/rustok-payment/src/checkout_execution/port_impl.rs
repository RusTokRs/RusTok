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
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        require_operation_context(
            &context,
            owner_operation,
            request.identity.checkout_operation_id,
        )?;
        self.prepare(&context, owner_operation, tenant_id, request)
            .await
    }

    async fn authorize_checkout_collection(
        &self,
        context: PortContext,
        request: AuthorizeCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        let owner_operation = AUTHORIZE_CHECKOUT_COLLECTION_OPERATION;
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        require_operation_context(
            &context,
            owner_operation,
            request.identity.checkout_operation_id,
        )?;
        self.authorize(&context, owner_operation, tenant_id, request)
            .await
    }

    async fn capture_checkout_collection(
        &self,
        context: PortContext,
        request: CaptureCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        let owner_operation = CAPTURE_CHECKOUT_COLLECTION_OPERATION;
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        require_operation_context(
            &context,
            owner_operation,
            request.identity.checkout_operation_id,
        )?;
        self.capture(&context, owner_operation, tenant_id, request)
            .await
    }

    async fn read_checkout_collection(
        &self,
        context: PortContext,
        request: ReadCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        let owner_operation = READ_CHECKOUT_COLLECTION_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        require_operation_context(
            &context,
            owner_operation,
            request.identity.checkout_operation_id,
        )?;
        validate_identity(&request.identity)?;
        let collection = self
            .payment_service
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(|error| payment_error_to_port_error(&context, owner_operation, error))?;
        validate_collection(&collection, tenant_id, &request.identity)?;
        Ok(collection)
    }
}
