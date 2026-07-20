use rustok_order::OrderResponse;
use rustok_payment::PaymentService;
use rustok_payment::dto::{
    AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput,
    PaymentCollectionResponse,
};
use rustok_payment::error::PaymentError;
use rustok_payment::providers::PaymentProviderRegistry;
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use super::{
    CheckoutOperationCheckpoint, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, CheckoutOrderPlanRecord,
    DEFAULT_CHECKOUT_LEASE_SECONDS, PaymentOrchestrationError, PaymentOrchestrationService,
};

#[derive(Clone, Debug)]
pub struct CheckoutPaymentCapturedState {
    pub operation_id: Uuid,
    pub order: OrderResponse,
    pub plan: CheckoutOrderPlanRecord,
    pub payment_collection: PaymentCollectionResponse,
}

#[derive(Debug, Error)]
pub enum CheckoutPaymentStageError {
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    Payment(#[from] PaymentError),
    #[error(transparent)]
    Orchestration(#[from] PaymentOrchestrationError),
    #[error("checkout payment stage conflict: {0}")]
    Conflict(String),
}

pub type CheckoutPaymentStageResult<T> = Result<T, CheckoutPaymentStageError>;

pub struct CheckoutPaymentStageExecutor {
    payment_service: PaymentService,
    payment_orchestration: PaymentOrchestrationService,
    operation_journal: CheckoutOperationJournal,
    lease_seconds: i64,
}

impl CheckoutPaymentStageExecutor {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            payment_service: PaymentService::new(db.clone()),
            payment_orchestration: PaymentOrchestrationService::new(db.clone()),
            operation_journal: CheckoutOperationJournal::new(db),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.payment_orchestration = self
            .payment_orchestration
            .with_provider_registry(payment_provider_registry);
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    /// Advances a claimed checkout operation through journaled authorization
    /// and capture. Provider side effects are owned by
    /// `PaymentOrchestrationService`; this executor only binds them to durable
    /// checkout checkpoints.
    pub async fn advance_to_payment_captured(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        order: OrderResponse,
        plan: CheckoutOrderPlanRecord,
    ) -> CheckoutPaymentStageResult<CheckoutPaymentCapturedState> {
        let lease_owner = lease_owner.into();
        validate_order_plan(tenant_id, operation_id, &order, &plan)?;

        for _ in 0..3 {
            let operation = self.operation_journal.get(tenant_id, operation_id).await?;
            if operation.status != CheckoutOperationStatus::Executing.as_str() {
                return Err(CheckoutPaymentStageError::Conflict(format!(
                    "checkout operation {} must be executing, not `{}`",
                    operation.id, operation.status
                )));
            }
            if operation.order_id != Some(order.id) {
                return Err(CheckoutPaymentStageError::Conflict(format!(
                    "checkout operation {} is not bound to order {}",
                    operation.id, order.id
                )));
            }

            match operation.stage.as_str() {
                stage if stage == CheckoutOperationStage::PaymentReady.as_str() => {
                    let collection = self
                        .prepare_collection(tenant_id, &operation, &order, &plan)
                        .await?;
                    let authorized = self
                        .payment_orchestration
                        .authorize_collection(
                            tenant_id,
                            collection.id,
                            AuthorizePaymentInput {
                                provider_id: collection.provider_id.clone(),
                                provider_payment_id: None,
                                amount: Some(order.total_amount),
                                metadata: payment_stage_metadata(
                                    plan.payload.checkout_metadata.clone(),
                                    operation_id,
                                    order.id,
                                    plan.plan_hash.as_str(),
                                    "authorize",
                                ),
                            },
                        )
                        .await?;
                    validate_collection(
                        &authorized,
                        tenant_id,
                        operation.cart_id,
                        &order,
                        operation_id,
                        plan.plan_hash.as_str(),
                    )?;
                    if !matches!(authorized.status.as_str(), "authorized" | "captured") {
                        return Err(CheckoutPaymentStageError::Conflict(format!(
                            "payment collection {} is `{}` after authorization",
                            authorized.id, authorized.status
                        )));
                    }
                    self.operation_journal
                        .checkpoint(CheckoutOperationCheckpoint {
                            tenant_id,
                            operation_id,
                            lease_owner: lease_owner.clone(),
                            expected_stage: CheckoutOperationStage::PaymentReady,
                            next_stage: CheckoutOperationStage::PaymentAuthorized,
                            snapshot_hash: None,
                            order_id: Some(order.id),
                            payment_collection_id: Some(authorized.id),
                            lease_seconds: self.lease_seconds,
                        })
                        .await?;
                }
                stage if stage == CheckoutOperationStage::PaymentAuthorized.as_str() => {
                    let collection_id = operation.payment_collection_id.ok_or_else(|| {
                        CheckoutPaymentStageError::Conflict(format!(
                            "checkout operation {} has no payment collection at payment_authorized",
                            operation.id
                        ))
                    })?;
                    let collection = self
                        .payment_service
                        .get_collection(tenant_id, collection_id)
                        .await?;
                    validate_collection(
                        &collection,
                        tenant_id,
                        operation.cart_id,
                        &order,
                        operation_id,
                        plan.plan_hash.as_str(),
                    )?;
                    let captured = self
                        .payment_orchestration
                        .capture_collection(
                            tenant_id,
                            collection_id,
                            CapturePaymentInput {
                                amount: Some(order.total_amount),
                                metadata: payment_stage_metadata(
                                    plan.payload.checkout_metadata.clone(),
                                    operation_id,
                                    order.id,
                                    plan.plan_hash.as_str(),
                                    "capture",
                                ),
                            },
                        )
                        .await?;
                    validate_collection(
                        &captured,
                        tenant_id,
                        operation.cart_id,
                        &order,
                        operation_id,
                        plan.plan_hash.as_str(),
                    )?;
                    if captured.status != "captured"
                        || captured.captured_amount != order.total_amount
                    {
                        return Err(CheckoutPaymentStageError::Conflict(format!(
                            "payment collection {} did not capture the full order amount",
                            captured.id
                        )));
                    }
                    self.operation_journal
                        .checkpoint(CheckoutOperationCheckpoint {
                            tenant_id,
                            operation_id,
                            lease_owner: lease_owner.clone(),
                            expected_stage: CheckoutOperationStage::PaymentAuthorized,
                            next_stage: CheckoutOperationStage::PaymentCaptured,
                            snapshot_hash: None,
                            order_id: Some(order.id),
                            payment_collection_id: Some(captured.id),
                            lease_seconds: self.lease_seconds,
                        })
                        .await?;
                }
                stage if stage == CheckoutOperationStage::PaymentCaptured.as_str() => {
                    let collection_id = operation.payment_collection_id.ok_or_else(|| {
                        CheckoutPaymentStageError::Conflict(format!(
                            "checkout operation {} has no captured payment collection",
                            operation.id
                        ))
                    })?;
                    let collection = self
                        .payment_service
                        .get_collection(tenant_id, collection_id)
                        .await?;
                    validate_collection(
                        &collection,
                        tenant_id,
                        operation.cart_id,
                        &order,
                        operation_id,
                        plan.plan_hash.as_str(),
                    )?;
                    if collection.status != "captured"
                        || collection.captured_amount != order.total_amount
                    {
                        return Err(CheckoutPaymentStageError::Conflict(format!(
                            "checkout operation {} is payment_captured but collection {} is `{}`",
                            operation.id, collection.id, collection.status
                        )));
                    }
                    return Ok(CheckoutPaymentCapturedState {
                        operation_id,
                        order,
                        plan,
                        payment_collection: collection,
                    });
                }
                stage => {
                    return Err(CheckoutPaymentStageError::Conflict(format!(
                        "checkout operation {} cannot enter payment stages from `{stage}`",
                        operation.id
                    )));
                }
            }
        }

        Err(CheckoutPaymentStageError::Conflict(format!(
            "checkout operation {operation_id} did not reach payment_captured within the bounded stage loop"
        )))
    }

    async fn prepare_collection(
        &self,
        tenant_id: Uuid,
        operation: &crate::entities::checkout_operation::Model,
        order: &OrderResponse,
        plan: &CheckoutOrderPlanRecord,
    ) -> CheckoutPaymentStageResult<PaymentCollectionResponse> {
        let metadata = payment_stage_metadata(
            plan.payload.checkout_metadata.clone(),
            operation.id,
            order.id,
            plan.plan_hash.as_str(),
            "collection",
        );
        let collection = match self
            .payment_service
            .find_reusable_collection_by_cart(tenant_id, operation.cart_id)
            .await?
        {
            Some(existing) => {
                validate_optional_collection_identity(
                    &existing,
                    operation.id,
                    plan.plan_hash.as_str(),
                )?;
                self.payment_service
                    .attach_order_to_collection(tenant_id, existing.id, order.id, metadata)
                    .await?
            }
            None => {
                self.payment_service
                    .create_collection(
                        tenant_id,
                        CreatePaymentCollectionInput {
                            cart_id: Some(operation.cart_id),
                            order_id: Some(order.id),
                            customer_id: order.customer_id,
                            currency_code: order.currency_code.clone(),
                            amount: order.total_amount,
                            metadata,
                        },
                    )
                    .await?
            }
        };
        validate_collection(
            &collection,
            tenant_id,
            operation.cart_id,
            order,
            operation.id,
            plan.plan_hash.as_str(),
        )?;
        Ok(collection)
    }
}

fn validate_order_plan(
    tenant_id: Uuid,
    operation_id: Uuid,
    order: &OrderResponse,
    plan: &CheckoutOrderPlanRecord,
) -> CheckoutPaymentStageResult<()> {
    if order.tenant_id != tenant_id
        || plan.tenant_id != tenant_id
        || plan.checkout_operation_id != operation_id
    {
        return Err(CheckoutPaymentStageError::Conflict(
            "order or plan belongs to another checkout identity".to_string(),
        ));
    }
    let source_operation = order
        .metadata
        .get("checkout")
        .and_then(|checkout| checkout.get("operation_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    if source_operation != Some(operation_id) {
        return Err(CheckoutPaymentStageError::Conflict(format!(
            "order {} is not attributed to checkout operation {}",
            order.id, operation_id
        )));
    }
    if !matches!(
        order.status.as_str(),
        "confirmed" | "paid" | "shipped" | "delivered"
    ) {
        return Err(CheckoutPaymentStageError::Conflict(format!(
            "order {} is `{}` before payment stages",
            order.id, order.status
        )));
    }
    Ok(())
}

fn validate_optional_collection_identity(
    collection: &PaymentCollectionResponse,
    operation_id: Uuid,
    plan_hash: &str,
) -> CheckoutPaymentStageResult<()> {
    let checkout = collection.metadata.get("checkout");
    let operation_id = operation_id.to_string();
    if let Some(existing_operation) = checkout
        .and_then(|value| value.get("operation_id"))
        .and_then(Value::as_str)
    {
        if existing_operation != operation_id {
            return Err(CheckoutPaymentStageError::Conflict(format!(
                "payment collection {} belongs to another checkout operation",
                collection.id
            )));
        }
    }
    if let Some(existing_plan_hash) = checkout
        .and_then(|value| value.get("order_plan_hash"))
        .and_then(Value::as_str)
    {
        if existing_plan_hash != plan_hash {
            return Err(CheckoutPaymentStageError::Conflict(format!(
                "payment collection {} belongs to another order plan",
                collection.id
            )));
        }
    }
    Ok(())
}

fn validate_collection(
    collection: &PaymentCollectionResponse,
    tenant_id: Uuid,
    cart_id: Uuid,
    order: &OrderResponse,
    operation_id: Uuid,
    plan_hash: &str,
) -> CheckoutPaymentStageResult<()> {
    if collection.tenant_id != tenant_id
        || collection.cart_id != Some(cart_id)
        || collection.order_id != Some(order.id)
        || collection.customer_id != order.customer_id
        || !collection
            .currency_code
            .eq_ignore_ascii_case(order.currency_code.as_str())
        || collection.amount != order.total_amount
    {
        return Err(CheckoutPaymentStageError::Conflict(format!(
            "payment collection {} does not match checkout order {}",
            collection.id, order.id
        )));
    }
    validate_optional_collection_identity(collection, operation_id, plan_hash)?;
    let checkout = collection
        .metadata
        .get("checkout")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            CheckoutPaymentStageError::Conflict(format!(
                "payment collection {} has no checkout identity metadata",
                collection.id
            ))
        })?;
    let operation_id = operation_id.to_string();
    if checkout.get("operation_id").and_then(Value::as_str) != Some(operation_id.as_str())
        || checkout.get("order_plan_hash").and_then(Value::as_str) != Some(plan_hash)
    {
        return Err(CheckoutPaymentStageError::Conflict(format!(
            "payment collection {} has a mismatched checkout identity",
            collection.id
        )));
    }
    Ok(())
}

fn payment_stage_metadata(
    base: Value,
    operation_id: Uuid,
    order_id: Uuid,
    plan_hash: &str,
    stage: &str,
) -> Value {
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
        Value::String(operation_id.to_string()),
    );
    checkout.insert("order_id".to_string(), Value::String(order_id.to_string()));
    checkout.insert(
        "order_plan_hash".to_string(),
        Value::String(plan_hash.to_string()),
    );
    checkout.insert(
        "payment_stage".to_string(),
        Value::String(stage.to_string()),
    );
    root.insert("checkout".to_string(), Value::Object(checkout));
    root.insert(
        "commerce_orchestration".to_string(),
        json!({"operation": format!("checkout_payment_{stage}")}),
    );
    Value::Object(root)
}
