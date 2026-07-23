use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

use crate::dto::{
    AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput,
    PaymentCollectionResponse,
};
use crate::providers::{
    MANUAL_PAYMENT_PROVIDER_ID, PaymentProviderOperationRequest, PaymentProviderOperationResult,
    PaymentProviderRegistry,
};
use crate::{
    BeginProviderOperation, PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED, PaymentError,
    PaymentProviderOperationJournal, PaymentService,
};

const UNKNOWN_PROVIDER_ID: &str = "payment-provider";

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
        tenant_id: Uuid,
        request: PrepareCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        validate_identity(&request.identity)?;
        let metadata = checkout_stage_metadata(request.metadata, &request.identity, "collection");
        let collection = match self
            .payment_service
            .find_reusable_collection_by_cart(tenant_id, request.identity.cart_id)
            .await
            .map_err(payment_error_to_port_error)?
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
                    .map_err(payment_error_to_port_error)?
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
                .map_err(payment_error_to_port_error)?,
        };
        validate_collection(&collection, tenant_id, &request.identity)?;
        Ok(collection)
    }

    async fn authorize(
        &self,
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
            .map_err(payment_error_to_port_error)?;
        validate_collection(&collection, tenant_id, &request.identity)?;
        let provider_id = request
            .provider_id
            .clone()
            .or_else(|| collection.provider_id.clone())
            .unwrap_or_else(|| MANUAL_PAYMENT_PROVIDER_ID.to_string());
        let idempotency_key = format!("payment_collection:{}:authorize", collection.id);

        match collection.status.as_str() {
            "authorized" | "captured" => {
                self.commit_existing_provider_operation(
                    tenant_id,
                    provider_id.as_str(),
                    idempotency_key.as_str(),
                    "authorize",
                )
                .await?;
                return Ok(collection);
            }
            "pending" => {}
            status => {
                return Err(PortError::conflict(
                    "payment.checkout_authorize_state_conflict",
                    format!("payment collection cannot be authorized from `{status}`"),
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
                self.mark_journal_committed(journaled.operation_id, "authorize")
                    .await?;
                validate_collection(&collection, tenant_id, &request.identity)?;
                Ok(collection)
            }
            Err(error) => {
                self.mark_local_persistence_failed(journaled.operation_id, "authorize")
                    .await;
                tracing::error!(
                    operation_id = %journaled.operation_id,
                    error = ?error,
                    "payment provider authorization succeeded but local persistence failed"
                );
                Err(manual_reconciliation(
                    "payment authorization succeeded externally but local persistence is incomplete",
                ))
            }
        }
    }

    async fn capture(
        &self,
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
            .map_err(payment_error_to_port_error)?;
        validate_collection(&collection, tenant_id, &request.identity)?;
        let provider_id = provider_id_for_collection(&collection);
        let idempotency_key = format!("payment_collection:{}:capture", collection.id);

        match collection.status.as_str() {
            "captured" => {
                self.commit_existing_provider_operation(
                    tenant_id,
                    provider_id.as_str(),
                    idempotency_key.as_str(),
                    "capture",
                )
                .await?;
                return Ok(collection);
            }
            "authorized" => {}
            status => {
                return Err(PortError::conflict(
                    "payment.checkout_capture_state_conflict",
                    format!("payment collection cannot be captured from `{status}`"),
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
            .execute_journaled_provider_operation("capture", provider_id.as_str(), provider_request)
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
                self.mark_journal_committed(journaled.operation_id, "capture")
                    .await?;
                validate_collection(&collection, tenant_id, &request.identity)?;
                Ok(collection)
            }
            Err(error) => {
                self.mark_local_persistence_failed(journaled.operation_id, "capture")
                    .await;
                tracing::error!(
                    operation_id = %journaled.operation_id,
                    error = ?error,
                    "payment provider capture succeeded but local persistence failed"
                );
                Err(manual_reconciliation(
                    "payment capture succeeded externally but local persistence is incomplete",
                ))
            }
        }
    }

    async fn execute_journaled_provider_operation(
        &self,
        operation: &'static str,
        provider_id: &str,
        request: PaymentProviderOperationRequest,
    ) -> Result<JournaledProviderResult, PortError> {
        let request = self
            .enrich_provider_request(operation, provider_id, request)
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
        let request_payload = serde_json::to_value(&request).map_err(|error| {
            tracing::error!(error = ?error, operation, "payment provider request encoding failed");
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
                operation: operation.to_string(),
                provider_id: provider_id.to_string(),
                idempotency_key,
                request_payload,
            })
            .await
            .map_err(payment_error_to_port_error)?;

        if let Some(result) = persisted_provider_result(&journal_operation)? {
            return Ok(JournaledProviderResult {
                operation_id: journal_operation.id,
                result,
            });
        }

        let claimed = self
            .operation_journal
            .claim_execution(journal_operation.id)
            .await
            .map_err(payment_error_to_port_error)?;
        if claimed.is_none() {
            let current = self
                .operation_journal
                .get(journal_operation.id)
                .await
                .map_err(payment_error_to_port_error)?;
            if let Some(result) = persisted_provider_result(&current)? {
                return Ok(JournaledProviderResult {
                    operation_id: current.id,
                    result,
                });
            }
            return Err(manual_reconciliation(
                "payment provider operation is already executing or requires reconciliation",
            ));
        }

        let provider_result = match operation {
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
                    tracing::error!(
                        operation_id = %journal_operation.id,
                        error = ?checkpoint_error,
                        "payment provider failure checkpoint failed"
                    );
                    return Err(manual_reconciliation(
                        "payment provider failure could not be durably checkpointed",
                    ));
                }
                return Err(payment_error_to_port_error(error));
            }
        };
        let result_payload = serde_json::to_value(&provider_result).map_err(|error| {
            tracing::error!(
                operation_id = %journal_operation.id,
                error = ?error,
                "payment provider result encoding failed"
            );
            manual_reconciliation(
                "payment provider succeeded but its normalized result could not be persisted",
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
                tracing::error!(
                    operation_id = %journal_operation.id,
                    error = ?error,
                    "payment provider success checkpoint failed"
                );
                manual_reconciliation(
                    "payment provider succeeded but its durable checkpoint failed",
                )
            })?;
        Ok(JournaledProviderResult {
            operation_id: journal_operation.id,
            result: provider_result,
        })
    }

    async fn enrich_provider_request(
        &self,
        operation: &str,
        provider_id: &str,
        mut request: PaymentProviderOperationRequest,
    ) -> Result<PaymentProviderOperationRequest, PortError> {
        if provider_id == MANUAL_PAYMENT_PROVIDER_ID || operation == "authorize" {
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
            .map_err(payment_error_to_port_error)?
            .ok_or_else(|| {
                manual_reconciliation("payment capture has no durable authorize provider identity")
            })?;
        if !matches!(
            authorize.status.as_str(),
            PROVIDER_OPERATION_COMMITTED
                | PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            return Err(manual_reconciliation(
                "payment capture cannot use an incomplete authorize operation",
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
                    "payment authorize operation has no durable provider payment identity",
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
        tenant_id: Uuid,
        provider_id: &str,
        idempotency_key: &str,
        operation: &'static str,
    ) -> Result<(), PortError> {
        if let Some(existing) = self
            .operation_journal
            .find_by_key(tenant_id, provider_id, idempotency_key)
            .await
            .map_err(payment_error_to_port_error)?
        {
            if matches!(
                existing.status.as_str(),
                PROVIDER_OPERATION_SUCCEEDED | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            ) {
                self.mark_journal_committed(existing.id, operation).await?;
            }
        }
        Ok(())
    }

    async fn mark_journal_committed(
        &self,
        operation_id: Uuid,
        operation: &'static str,
    ) -> Result<(), PortError> {
        if let Err(error) = self.operation_journal.mark_committed(operation_id).await {
            tracing::error!(
                operation_id = %operation_id,
                error = ?error,
                "payment local commit checkpoint failed"
            );
            let _ = self
                .operation_journal
                .mark_reconciliation_required(
                    operation_id,
                    format!("payment.local_{operation}_commit_checkpoint_failed"),
                )
                .await;
            return Err(manual_reconciliation(
                "payment provider result was applied but its commit checkpoint failed",
            ));
        }
        Ok(())
    }

    async fn mark_local_persistence_failed(&self, operation_id: Uuid, operation: &'static str) {
        let _ = self
            .operation_journal
            .mark_reconciliation_required(
                operation_id,
                format!("payment.local_{operation}_persistence_failed"),
            )
            .await;
    }
}

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
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        require_operation_context(&context, request.identity.checkout_operation_id)?;
        self.prepare(tenant_id, request).await
    }

    async fn authorize_checkout_collection(
        &self,
        context: PortContext,
        request: AuthorizeCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        require_operation_context(&context, request.identity.checkout_operation_id)?;
        self.authorize(tenant_id, request).await
    }

    async fn capture_checkout_collection(
        &self,
        context: PortContext,
        request: CaptureCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        require_operation_context(&context, request.identity.checkout_operation_id)?;
        self.capture(tenant_id, request).await
    }

    async fn read_checkout_collection(
        &self,
        context: PortContext,
        request: ReadCheckoutPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        require_operation_context(&context, request.identity.checkout_operation_id)?;
        validate_identity(&request.identity)?;
        let collection = self
            .payment_service
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(payment_error_to_port_error)?;
        validate_collection(&collection, tenant_id, &request.identity)?;
        Ok(collection)
    }
}

struct JournaledProviderResult {
    operation_id: Uuid,
    result: PaymentProviderOperationResult,
}

fn validate_identity(identity: &CheckoutPaymentIdentity) -> Result<(), PortError> {
    if identity.checkout_operation_id.is_nil()
        || identity.cart_id.is_nil()
        || identity.order_id.is_nil()
        || identity.amount <= Decimal::ZERO
    {
        return Err(PortError::validation(
            "payment.checkout_identity_invalid",
            "checkout payment identity contains invalid UUID or amount fields",
        ));
    }
    let currency = identity.currency_code.trim();
    if currency.len() != 3
        || !currency
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return Err(PortError::validation(
            "payment.checkout_currency_invalid",
            "checkout payment currency must be a three-letter alphabetic code",
        ));
    }
    let plan_hash = identity.order_plan_hash.trim();
    if plan_hash.len() != 64 || !plan_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PortError::validation(
            "payment.checkout_plan_hash_invalid",
            "checkout payment order plan hash must be a 64-character hexadecimal value",
        ));
    }
    Ok(())
}

fn validate_optional_collection_identity(
    collection: &PaymentCollectionResponse,
    identity: &CheckoutPaymentIdentity,
) -> Result<(), PortError> {
    let checkout = collection.metadata.get("checkout");
    if let Some(operation_id) = checkout
        .and_then(|value| value.get("operation_id"))
        .and_then(Value::as_str)
    {
        if operation_id != identity.checkout_operation_id.to_string() {
            return Err(PortError::conflict(
                "payment.checkout_collection_operation_conflict",
                "payment collection belongs to another checkout operation",
            ));
        }
    }
    if let Some(plan_hash) = checkout
        .and_then(|value| value.get("order_plan_hash"))
        .and_then(Value::as_str)
    {
        if plan_hash != identity.order_plan_hash {
            return Err(PortError::conflict(
                "payment.checkout_collection_plan_conflict",
                "payment collection belongs to another checkout order plan",
            ));
        }
    }
    Ok(())
}

fn validate_collection(
    collection: &PaymentCollectionResponse,
    tenant_id: Uuid,
    identity: &CheckoutPaymentIdentity,
) -> Result<(), PortError> {
    if collection.tenant_id != tenant_id
        || collection.cart_id != Some(identity.cart_id)
        || collection.order_id != Some(identity.order_id)
        || collection.customer_id != identity.customer_id
        || !collection
            .currency_code
            .eq_ignore_ascii_case(identity.currency_code.as_str())
        || collection.amount != identity.amount
    {
        return Err(PortError::conflict(
            "payment.checkout_collection_identity_conflict",
            "payment collection does not match the checkout identity",
        ));
    }
    validate_optional_collection_identity(collection, identity)?;
    let checkout = collection
        .metadata
        .get("checkout")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            PortError::conflict(
                "payment.checkout_collection_identity_missing",
                "payment collection has no checkout identity",
            )
        })?;
    if checkout.get("operation_id").and_then(Value::as_str)
        != Some(identity.checkout_operation_id.to_string().as_str())
        || checkout.get("order_plan_hash").and_then(Value::as_str)
            != Some(identity.order_plan_hash.as_str())
    {
        return Err(PortError::conflict(
            "payment.checkout_collection_identity_conflict",
            "payment collection has mismatched checkout identity",
        ));
    }
    Ok(())
}

fn checkout_stage_metadata(base: Value, identity: &CheckoutPaymentIdentity, stage: &str) -> Value {
    let mut root = match base {
        Value::Object(root) => root,
        _ => Default::default(),
    };
    let mut checkout = root
        .remove("checkout")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    checkout.insert(
        "operation_id".to_string(),
        Value::String(identity.checkout_operation_id.to_string()),
    );
    checkout.insert(
        "order_id".to_string(),
        Value::String(identity.order_id.to_string()),
    );
    checkout.insert(
        "order_plan_hash".to_string(),
        Value::String(identity.order_plan_hash.clone()),
    );
    checkout.insert(
        "payment_stage".to_string(),
        Value::String(stage.to_string()),
    );
    root.insert("checkout".to_string(), Value::Object(checkout));
    root.insert(
        "commerce_orchestration".to_string(),
        serde_json::json!({"operation": format!("checkout_payment_{stage}")}),
    );
    Value::Object(root)
}

fn provider_id_for_collection(collection: &PaymentCollectionResponse) -> String {
    collection
        .provider_id
        .clone()
        .unwrap_or_else(|| MANUAL_PAYMENT_PROVIDER_ID.to_string())
}

fn persisted_provider_result(
    operation: &crate::entities::provider_operation::Model,
) -> Result<Option<PaymentProviderOperationResult>, PortError> {
    if operation.status == PROVIDER_OPERATION_EXECUTING {
        return Ok(None);
    }
    if !matches!(
        operation.status.as_str(),
        PROVIDER_OPERATION_COMMITTED
            | PROVIDER_OPERATION_SUCCEEDED
            | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
    ) {
        return Ok(None);
    }
    let value = operation.provider_result.clone().ok_or_else(|| {
        manual_reconciliation("payment provider operation has no normalized durable result")
    })?;
    serde_json::from_value(value).map(Some).map_err(|error| {
        tracing::error!(
            operation_id = %operation.id,
            error = ?error,
            "payment provider operation result is malformed"
        );
        manual_reconciliation("payment provider operation result is malformed")
    })
}

fn insert_metadata_string(metadata: &mut Value, key: &str, value: String) -> Result<(), PortError> {
    if metadata.is_null() {
        *metadata = serde_json::json!({});
    }
    let object = metadata.as_object_mut().ok_or_else(|| {
        PortError::validation(
            "payment.provider_metadata_invalid",
            "payment provider metadata must be a JSON object",
        )
    })?;
    if let Some(existing) = object.get(key).and_then(Value::as_str) {
        if existing != value {
            return Err(PortError::conflict(
                "payment.provider_identity_conflict",
                "payment provider identity conflicts with the durable authorize operation",
            ));
        }
        return Ok(());
    }
    object.insert(key.to_string(), Value::String(value));
    Ok(())
}

fn metadata_string<'a>(metadata: &'a Value, key: &str) -> Option<&'a str> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn merge_metadata(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

fn require_operation_context(
    context: &PortContext,
    checkout_operation_id: Uuid,
) -> Result<(), PortError> {
    let context_operation = context
        .causation_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    if context_operation != Some(checkout_operation_id) {
        return Err(PortError::validation(
            "payment.checkout_operation_id_invalid",
            "checkout payment causation_id must match the checkout operation",
        ));
    }
    Ok(())
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "payment.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for payment ports",
        )
    })
}

fn manual_reconciliation(message: impl Into<String>) -> PortError {
    PortError::new(
        PortErrorKind::Conflict,
        "payment.checkout_execution_manual_reconciliation",
        message,
        false,
    )
}

fn stable_payment_error_code(error: &PaymentError) -> &'static str {
    match error {
        PaymentError::Database(_) => "payment.database_unavailable",
        PaymentError::Validation(_) => "payment.validation",
        PaymentError::PaymentCollectionNotFound(_) => "payment.collection_not_found",
        PaymentError::PaymentNotFound(_) => "payment.payment_not_found",
        PaymentError::RefundNotFound(_) => "payment.refund_not_found",
        PaymentError::InvalidTransition { .. } => "payment.invalid_transition",
        PaymentError::ProviderUnavailable { .. } => "payment.provider_unavailable",
        PaymentError::ProviderRejected { .. } => "payment.provider_rejected",
        PaymentError::ProviderInvalidResponse { .. } => "payment.provider_invalid_response",
        PaymentError::ProviderOutcomeUnknown { .. } => "payment.provider_outcome_unknown",
        PaymentError::ProviderConfiguration { .. } => "payment.provider_not_configured",
    }
}

fn payment_error_to_port_error(error: PaymentError) -> PortError {
    match error {
        PaymentError::Database(error) => {
            tracing::error!(error = ?error, "checkout payment storage failed");
            PortError::unavailable(
                "payment.database_unavailable",
                "payment storage is temporarily unavailable",
            )
        }
        PaymentError::Validation(_) => PortError::validation(
            "payment.checkout_execution_validation",
            "checkout payment request is invalid",
        ),
        PaymentError::PaymentCollectionNotFound(_) => PortError::not_found(
            "payment.collection_not_found",
            "payment collection was not found",
        ),
        PaymentError::PaymentNotFound(_) => {
            PortError::not_found("payment.payment_not_found", "payment was not found")
        }
        PaymentError::RefundNotFound(_) => {
            PortError::not_found("payment.refund_not_found", "refund was not found")
        }
        PaymentError::InvalidTransition { .. } => PortError::conflict(
            "payment.checkout_execution_state_conflict",
            "payment lifecycle conflicts with checkout execution",
        ),
        PaymentError::ProviderUnavailable { .. } => PortError::unavailable(
            "payment.provider_unavailable",
            "payment provider is temporarily unavailable",
        ),
        PaymentError::ProviderRejected { .. } => PortError::conflict(
            "payment.provider_rejected",
            "payment provider rejected the operation",
        ),
        PaymentError::ProviderInvalidResponse { .. } => {
            manual_reconciliation("payment provider returned an invalid successful response")
        }
        PaymentError::ProviderOutcomeUnknown { .. } => {
            manual_reconciliation("payment provider operation outcome is unknown")
        }
        PaymentError::ProviderConfiguration { .. } => PortError::invariant_violation(
            "payment.provider_not_configured",
            "payment provider is not configured",
        ),
    }
}
