#[async_trait]
pub trait CheckoutPaymentExecutionPort: Send + Sync {
    async fn prepare_checkout_collection(
        &self,
        context: PortContext,
        request: PrepareCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;

    async fn authorize_checkout_collection(
        &self,
        context: PortContext,
        request: AuthorizeCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;

    async fn capture_checkout_collection(
        &self,
        context: PortContext,
        request: CaptureCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;

    async fn read_checkout_collection(
        &self,
        context: PortContext,
        request: ReadCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckoutPaymentIdentity {
    pub checkout_operation_id: Uuid,
    pub cart_id: Uuid,
    pub order_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub currency_code: String,
    pub amount: Decimal,
    pub order_plan_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrepareCheckoutPaymentCollectionRequest {
    pub identity: CheckoutPaymentIdentity,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthorizeCheckoutPaymentCollectionRequest {
    pub identity: CheckoutPaymentIdentity,
    pub collection_id: Uuid,
    pub provider_id: Option<String>,
    pub provider_payment_id: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureCheckoutPaymentCollectionRequest {
    pub identity: CheckoutPaymentIdentity,
    pub collection_id: Uuid,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadCheckoutPaymentCollectionRequest {
    pub identity: CheckoutPaymentIdentity,
    pub collection_id: Uuid,
}

pub struct InProcessCheckoutPaymentExecutionPort {
    payment_service: PaymentService,
    operation_journal: PaymentProviderOperationJournal,
    provider_registry: PaymentProviderRegistry,
}
