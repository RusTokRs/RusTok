use std::{sync::Arc, time::Duration};

use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError};
use rustok_order::{OrderResponse, OrderStatusKind};
use rustok_payment::{
    AuthorizeCheckoutPaymentCollectionRequest, CaptureCheckoutPaymentCollectionRequest,
    CheckoutPaymentExecutionPort, CheckoutPaymentIdentity, InProcessCheckoutPaymentExecutionPort,
    PaymentCollectionResponse, PaymentProviderRegistry,
    PrepareCheckoutPaymentCollectionRequest, ReadCheckoutPaymentCollectionRequest,
    in_process_checkout_payment_execution_port,
};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::entities::checkout_operation;

use super::{
    CheckoutOperationCheckpoint, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, CheckoutOrderPlanRecord,
    DEFAULT_CHECKOUT_LEASE_SECONDS,
};

const PAYMENT_EXECUTION_PORT_DEADLINE_SECONDS: u64 = 5;

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
    #[error(
        "checkout payment boundary `{stage}` failed with `{code}` (retryable={retryable}): {message}"
    )]
    Boundary {
        stage: &'static str,
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("checkout payment stage conflict: {0}")]
    Conflict(String),
}

pub type CheckoutPaymentStageResult<T> = Result<T, CheckoutPaymentStageError>;

pub struct CheckoutPaymentStageExecutor {
    payment_port: Arc<dyn CheckoutPaymentExecutionPort>,
    operation_journal: CheckoutOperationJournal,
    owner_db: sea_orm::DatabaseConnection,
    lease_seconds: i64,
    port_deadline: Duration,
}

impl CheckoutPaymentStageExecutor {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            payment_port: in_process_checkout_payment_execution_port(db.clone()),
            operation_journal: CheckoutOperationJournal::new(db.clone()),
            owner_db: db,
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
            port_deadline: Duration::from_secs(PAYMENT_EXECUTION_PORT_DEADLINE_SECONDS),
        }
    }

    pub fn with_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.payment_port = Arc::new(
            InProcessCheckoutPaymentExecutionPort::with_provider_registry(
                self.owner_db.clone(),
                payment_provider_registry,
            ),
        );
        self
    }

    pub fn with_payment_port(
        mut self,
        payment_port: Arc<dyn CheckoutPaymentExecutionPort>,
    ) -> Self {
        self.payment_port = payment_port;
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    /// Advances a claimed checkout operation through payment owner
    /// prepare/authorize/capture commands. Provider journal and payment lifecycle
    /// policy remain inside `rustok-payment`; commerce owns only stage checkpoints.
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
            validate_operation(&operation, &order)?;
            let identity = payment_identity(&operation, &order, &plan);

            match operation.stage.as_str() {
                stage if stage == CheckoutOperationStage::PaymentReady.as_str() => {
                    let collection = self
                        .payment_port
                        .prepare_checkout_collection(
                            payment_write_context(
                                tenant_id,
                                operation_id,
                                plan.payload.context.locale.as_str(),
                                self.port_deadline,
                                "prepare",
                                format!("checkout:{operation_id}:payment:collection"),
                            ),
                            PrepareCheckoutPaymentCollectionRequest {
                                identity: identity.clone(),
                                metadata: plan.payload.checkout_metadata.clone(),
                            },
                        )
                        .await
                        .map_err(|error| boundary_error("prepare", error))?;
                    validate_collection(&collection, tenant_id, &identity)?;

                    let authorized = self
                        .payment_port
                        .authorize_checkout_collection(
                            payment_write_context(
                                tenant_id,
                                operation_id,
                                plan.payload.context.locale.as_str(),
                                self.port_deadline,
                                "authorize",
                                format!("payment_collection:{}:authorize", collection.id),
                            ),
                            AuthorizeCheckoutPaymentCollectionRequest {
                                identity,
                                collection_id: collection.id,
                                provider_id: collection.provider_id.clone(),
                                provider_payment_id: None,
                                metadata: plan.payload.checkout_metadata.clone(),
                            },
                        )
                        .await
                        .map_err(|error| boundary_error("authorize", error))?;
                    validate_collection(
                        &authorized,
                        tenant_id,
                        &payment_identity(&operation, &order, &plan),
                    )?;
                    if !authorized.status_kind().is_authorized_or_captured() {
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
                    let captured = self
                        .payment_port
                        .capture_checkout_collection(
                            payment_write_context(
                                tenant_id,
                                operation_id,
                                plan.payload.context.locale.as_str(),
                                self.port_deadline,
                                "capture",
                                format!("payment_collection:{collection_id}:capture"),
                            ),
                            CaptureCheckoutPaymentCollectionRequest {
                                identity,
                                collection_id,
                                metadata: plan.payload.checkout_metadata.clone(),
                            },
                        )
                        .await
                        .map_err(|error| boundary_error("capture", error))?;
                    validate_collection(
                        &captured,
                        tenant_id,
                        &payment_identity(&operation, &order, &plan),
                    )?;
                    if !captured.status_kind().is_captured()
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
                    return self
                        .load_payment_captured_state(tenant_id, operation_id, order, plan)
                        .await;
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

    pub async fn load_payment_captured_state(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        order: OrderResponse,
        plan: CheckoutOrderPlanRecord,
    ) -> CheckoutPaymentStageResult<CheckoutPaymentCapturedState> {
        validate_order_plan(tenant_id, operation_id, &order, &plan)?;
        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        validate_operation(&operation, &order)?;
        if !matches!(
            operation.stage.as_str(),
            "payment_captured" | "fulfillment_created" | "cart_completed" | "completed"
        ) {
            return Err(CheckoutPaymentStageError::Conflict(format!(
                "checkout operation {} has not reached payment_captured, stage={}",
                operation.id, operation.stage
            )));
        }
        let collection_id = operation.payment_collection_id.ok_or_else(|| {
            CheckoutPaymentStageError::Conflict(format!(
                "checkout operation {} has no captured payment collection",
                operation.id
            ))
        })?;
        let identity = payment_identity(&operation, &order, &plan);
        let collection = self
            .payment_port
            .read_checkout_collection(
                payment_read_context(
                    tenant_id,
                    operation_id,
                    plan.payload.context.locale.as_str(),
                    self.port_deadline,
                ),
                ReadCheckoutPaymentCollectionRequest {
                    identity: identity.clone(),
                    collection_id,
                },
            )
            .await
            .map_err(|error| boundary_error("read", error))?;
        validate_collection(&collection, tenant_id, &identity)?;
        if !collection.status_kind().is_captured()
            || collection.captured_amount != order.total_amount
        {
            return Err(CheckoutPaymentStageError::Conflict(format!(
                "checkout operation {} is payment_captured but collection {} is `{}`",
                operation.id, collection.id, collection.status
            )));
        }
        Ok(CheckoutPaymentCapturedState {
            operation_id,
            order,
            plan,
            payment_collection: collection,
        })
    }
}

fn validate_operation(
    operation: &checkout_operation::Model,
    order: &OrderResponse,
) -> CheckoutPaymentStageResult<()> {
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
    Ok(())
}

fn payment_identity(
    operation: &checkout_operation::Model,
    order: &OrderResponse,
    plan: &CheckoutOrderPlanRecord,
) -> CheckoutPaymentIdentity {
    CheckoutPaymentIdentity {
        checkout_operation_id: operation.id,
        cart_id: operation.cart_id,
        order_id: order.id,
        customer_id: order.customer_id,
        currency_code: order.currency_code.clone(),
        amount: order.total_amount,
        order_plan_hash: plan.plan_hash.clone(),
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
        order.status_kind(),
        OrderStatusKind::Confirmed
            | OrderStatusKind::Paid
            | OrderStatusKind::Shipped
            | OrderStatusKind::Delivered
    ) {
        return Err(CheckoutPaymentStageError::Conflict(format!(
            "order {} is `{}` before payment stages",
            order.id, order.status
        )));
    }
    Ok(())
}

fn validate_collection(
    collection: &PaymentCollectionResponse,
    tenant_id: Uuid,
    identity: &CheckoutPaymentIdentity,
) -> CheckoutPaymentStageResult<()> {
    if collection.tenant_id != tenant_id
        || collection.cart_id != Some(identity.cart_id)
        || collection.order_id != Some(identity.order_id)
        || collection.customer_id != identity.customer_id
        || !collection
            .currency_code
            .eq_ignore_ascii_case(identity.currency_code.as_str())
        || collection.amount != identity.amount
    {
        return Err(CheckoutPaymentStageError::Conflict(format!(
            "payment collection {} does not match checkout order {}",
            collection.id, identity.order_id
        )));
    }
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
    let operation_id = identity.checkout_operation_id.to_string();
    if checkout.get("operation_id").and_then(Value::as_str) != Some(operation_id.as_str())
        || checkout.get("order_plan_hash").and_then(Value::as_str)
            != Some(identity.order_plan_hash.as_str())
    {
        return Err(CheckoutPaymentStageError::Conflict(format!(
            "payment collection {} has a mismatched checkout identity",
            collection.id
        )));
    }
    Ok(())
}

fn payment_write_context(
    tenant_id: Uuid,
    operation_id: Uuid,
    locale: &str,
    deadline: Duration,
    action: &str,
    idempotency_key: String,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.checkout-payment-stage"),
        normalize_locale(locale),
        format!("checkout:{operation_id}:payment:{action}"),
    )
    .with_causation_id(operation_id.to_string())
    .with_idempotency_key(idempotency_key)
    .with_deadline(deadline)
}

fn payment_read_context(
    tenant_id: Uuid,
    operation_id: Uuid,
    locale: &str,
    deadline: Duration,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.checkout-payment-stage"),
        normalize_locale(locale),
        format!("checkout:{operation_id}:payment:read"),
    )
    .with_causation_id(operation_id.to_string())
    .with_deadline(deadline)
}

fn normalize_locale(locale: &str) -> String {
    let locale = locale.trim();
    if locale.is_empty() {
        PLATFORM_FALLBACK_LOCALE.to_string()
    } else {
        locale.to_string()
    }
}

fn boundary_error(stage: &'static str, error: PortError) -> CheckoutPaymentStageError {
    CheckoutPaymentStageError::Boundary {
        stage,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}
