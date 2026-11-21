use std::sync::Arc;
use std::time::Duration;

use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext};
use rustok_order::{OrderResponse, OrderStatusKind};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::{
    CheckoutPaymentExecutionPort as CanonicalCheckoutPaymentExecutionPort, PaymentProviderRegistry,
};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::entities::checkout_operation;

use super::{
    CheckoutOperationCheckpoint, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, CheckoutOrderPlanRecord,
    DEFAULT_CHECKOUT_LEASE_SECONDS, checkout_execution_admission_port,
};

use payment_execution_boundary::{BoundaryPortError, public_message};
use payment_execution_port::{
    AuthorizeCheckoutPaymentCollectionRequest, CaptureCheckoutPaymentCollectionRequest,
    CheckoutPaymentExecutionPort, CheckoutPaymentIdentity, InProcessCheckoutPaymentExecutionPort,
    PaymentCollectionResponse, PrepareCheckoutPaymentCollectionRequest,
    ReadCheckoutPaymentCollectionRequest, in_process_checkout_payment_execution_port,
    wrap_checkout_payment_execution_port,
};

const PAYMENT_EXECUTION_PORT_DEADLINE_SECONDS: u64 = 5;

mod payment_execution_boundary {
    use ::rustok_api::{PortActorKind, PortContext, PortError, PortErrorKind};
    use uuid::Uuid;

    const CHECKOUT_PAYMENT_EXECUTION_ADAPTER_BOUNDARY: &str =
        "commerce_checkout_payment_execution_adapter";

    pub(crate) struct BoundaryPortError {
        pub(crate) kind: PortErrorKind,
        pub(crate) code: String,
        pub(crate) message: String,
        pub(crate) retryable: bool,
    }

    impl std::fmt::Debug for BoundaryPortError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
        }
    }

    struct CheckoutPaymentExecutionDiagnosticError;

    impl std::fmt::Debug for CheckoutPaymentExecutionDiagnosticError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
        }
    }

    #[derive(Clone, Copy)]
    struct CheckoutPaymentExecutionContextFacts {
        tenant_id_shape: &'static str,
        actor_kind: &'static str,
        actor_id_shape: &'static str,
        claim_count: usize,
        role_count: usize,
        channel_shape: &'static str,
        locale_shape: &'static str,
        correlation_id_shape: &'static str,
        causation_id_shape: &'static str,
        traceparent_shape: &'static str,
        idempotency_key_shape: &'static str,
        deadline_ms: Option<u64>,
    }

    impl From<&PortContext> for CheckoutPaymentExecutionContextFacts {
        fn from(context: &PortContext) -> Self {
            Self {
                tenant_id_shape: identity_text_shape(context.tenant_id.as_str()),
                actor_kind: actor_kind_name(&context.actor.kind),
                actor_id_shape: identity_text_shape(context.actor.id.as_str()),
                claim_count: context.claims.len(),
                role_count: context.roles.len(),
                channel_shape: optional_text_shape(context.channel.as_deref()),
                locale_shape: text_shape(context.locale.as_str()),
                correlation_id_shape: text_shape(context.correlation_id.as_str()),
                causation_id_shape: optional_text_shape(context.causation_id.as_deref()),
                traceparent_shape: optional_text_shape(context.traceparent.as_deref()),
                idempotency_key_shape: optional_text_shape(context.idempotency_key.as_deref()),
                deadline_ms: context.deadline_ms,
            }
        }
    }

    fn actor_kind_name(kind: &PortActorKind) -> &'static str {
        match kind {
            PortActorKind::User => "user",
            PortActorKind::Service => "service",
            PortActorKind::System => "system",
        }
    }

    fn identity_text_shape(value: &str) -> &'static str {
        if value.is_empty() {
            return "empty";
        }
        match Uuid::parse_str(value) {
            Ok(value) if value.is_nil() => "uuid_nil",
            Ok(_) => "uuid_non_nil",
            Err(_) => "opaque",
        }
    }

    fn text_shape(value: &str) -> &'static str {
        if value.is_empty() { "empty" } else { "present" }
    }

    fn optional_text_shape(value: Option<&str>) -> &'static str {
        match value {
            None => "absent",
            Some("") => "empty",
            Some(_) => "present",
        }
    }

    pub(crate) fn public_message(kind: &PortErrorKind) -> &'static str {
        match kind {
            PortErrorKind::Validation => "Checkout payment request is invalid",
            PortErrorKind::NotFound => "Checkout payment resource was not found",
            PortErrorKind::Conflict => {
                "Checkout payment state conflicts with the requested operation"
            }
            PortErrorKind::Forbidden => "Checkout payment operation is not permitted",
            PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                "Checkout payment service is temporarily unavailable"
            }
            PortErrorKind::InvariantViolation => {
                "Checkout payment operation could not be completed safely"
            }
        }
    }

    pub(crate) fn sanitize_owner_error(
        context: &PortContext,
        owner_operation: &'static str,
        error: PortError,
    ) -> BoundaryPortError {
        let diagnostic_context = CheckoutPaymentExecutionContextFacts::from(context);
        let owner_message_shape = text_shape(error.message.as_str());
        let owner_message_len = error.message.chars().count();
        let public_message = public_message(&error.kind);
        let technical = matches!(
            &error.kind,
            PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
        );
        let diagnostic_error = CheckoutPaymentExecutionDiagnosticError;

        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_payment",
                owner_operation,
                tenant_id_shape = diagnostic_context.tenant_id_shape,
                actor_kind = diagnostic_context.actor_kind,
                actor_id_shape = diagnostic_context.actor_id_shape,
                claim_count = diagnostic_context.claim_count,
                role_count = diagnostic_context.role_count,
                channel_shape = diagnostic_context.channel_shape,
                locale_shape = diagnostic_context.locale_shape,
                correlation_id_shape = diagnostic_context.correlation_id_shape,
                causation_id_shape = diagnostic_context.causation_id_shape,
                traceparent_shape = diagnostic_context.traceparent_shape,
                idempotency_key_shape = diagnostic_context.idempotency_key_shape,
                deadline_ms = ?diagnostic_context.deadline_ms,
                owner_code = %error.code,
                owner_message_shape,
                owner_message_len,
                owner_kind = ?error.kind,
                owner_retryable = error.retryable,
                boundary = CHECKOUT_PAYMENT_EXECUTION_ADAPTER_BOUNDARY,
                "commerce checkout payment execution owner call failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic_error,
                owner = "rustok_payment",
                owner_operation,
                tenant_id_shape = diagnostic_context.tenant_id_shape,
                actor_kind = diagnostic_context.actor_kind,
                actor_id_shape = diagnostic_context.actor_id_shape,
                claim_count = diagnostic_context.claim_count,
                role_count = diagnostic_context.role_count,
                channel_shape = diagnostic_context.channel_shape,
                locale_shape = diagnostic_context.locale_shape,
                correlation_id_shape = diagnostic_context.correlation_id_shape,
                causation_id_shape = diagnostic_context.causation_id_shape,
                traceparent_shape = diagnostic_context.traceparent_shape,
                idempotency_key_shape = diagnostic_context.idempotency_key_shape,
                deadline_ms = ?diagnostic_context.deadline_ms,
                owner_code = %error.code,
                owner_message_shape,
                owner_message_len,
                owner_kind = ?error.kind,
                owner_retryable = error.retryable,
                boundary = CHECKOUT_PAYMENT_EXECUTION_ADAPTER_BOUNDARY,
                "commerce checkout payment execution owner call was rejected"
            );
        }

        BoundaryPortError {
            kind: error.kind,
            code: error.code,
            message: public_message.to_string(),
            retryable: error.retryable,
        }
    }
}

mod payment_execution_port {
    use std::sync::Arc;

    use ::rustok_api::PortContext;

    use super::checkout_execution_admission_port;
    use super::payment_execution_boundary::{BoundaryPortError, sanitize_owner_error};

    pub use ::rustok_payment::{
        AuthorizeCheckoutPaymentCollectionRequest, CaptureCheckoutPaymentCollectionRequest,
        CheckoutPaymentIdentity, PaymentCollectionResponse, PaymentProviderRegistry,
        PrepareCheckoutPaymentCollectionRequest, ReadCheckoutPaymentCollectionRequest,
    };

    #[async_trait::async_trait]
    pub trait CheckoutPaymentExecutionPort: Send + Sync {
        async fn prepare_checkout_collection(
            &self,
            context: PortContext,
            request: PrepareCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError>;

        async fn authorize_checkout_collection(
            &self,
            context: PortContext,
            request: AuthorizeCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError>;

        async fn capture_checkout_collection(
            &self,
            context: PortContext,
            request: CaptureCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError>;

        async fn read_checkout_collection(
            &self,
            context: PortContext,
            request: ReadCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError>;
    }

    struct SanitizingCheckoutPaymentExecutionPort {
        inner: Arc<dyn ::rustok_payment::CheckoutPaymentExecutionPort>,
    }

    #[async_trait::async_trait]
    impl CheckoutPaymentExecutionPort for SanitizingCheckoutPaymentExecutionPort {
        async fn prepare_checkout_collection(
            &self,
            context: PortContext,
            request: PrepareCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError> {
            let error_context = context.clone();
            self.inner
                .prepare_checkout_collection(context, request)
                .await
                .map_err(|error| {
                    sanitize_owner_error(&error_context, "prepare_checkout_collection", error)
                })
        }

        async fn authorize_checkout_collection(
            &self,
            context: PortContext,
            request: AuthorizeCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError> {
            let error_context = context.clone();
            self.inner
                .authorize_checkout_collection(context, request)
                .await
                .map_err(|error| {
                    sanitize_owner_error(&error_context, "authorize_checkout_collection", error)
                })
        }

        async fn capture_checkout_collection(
            &self,
            context: PortContext,
            request: CaptureCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError> {
            let error_context = context.clone();
            self.inner
                .capture_checkout_collection(context, request)
                .await
                .map_err(|error| {
                    sanitize_owner_error(&error_context, "capture_checkout_collection", error)
                })
        }

        async fn read_checkout_collection(
            &self,
            context: PortContext,
            request: ReadCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError> {
            let error_context = context.clone();
            self.inner
                .read_checkout_collection(context, request)
                .await
                .map_err(|error| {
                    sanitize_owner_error(&error_context, "read_checkout_collection", error)
                })
        }
    }

    pub struct InProcessCheckoutPaymentExecutionPort {
        inner: Arc<dyn CheckoutPaymentExecutionPort>,
    }

    impl InProcessCheckoutPaymentExecutionPort {
        pub fn with_provider_registry(
            db: sea_orm::DatabaseConnection,
            payment_provider_registry: PaymentProviderRegistry,
        ) -> Self {
            let checkout_admission = checkout_execution_admission_port(db.clone());
            Self {
                inner: wrap_checkout_payment_execution_port(Arc::new(
                    ::rustok_payment::InProcessCheckoutPaymentExecutionPort::with_provider_registry(
                        db,
                        payment_provider_registry,
                        checkout_admission,
                    ),
                )),
            }
        }
    }

    #[async_trait::async_trait]
    impl CheckoutPaymentExecutionPort for InProcessCheckoutPaymentExecutionPort {
        async fn prepare_checkout_collection(
            &self,
            context: PortContext,
            request: PrepareCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError> {
            self.inner
                .prepare_checkout_collection(context, request)
                .await
        }

        async fn authorize_checkout_collection(
            &self,
            context: PortContext,
            request: AuthorizeCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError> {
            self.inner
                .authorize_checkout_collection(context, request)
                .await
        }

        async fn capture_checkout_collection(
            &self,
            context: PortContext,
            request: CaptureCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError> {
            self.inner
                .capture_checkout_collection(context, request)
                .await
        }

        async fn read_checkout_collection(
            &self,
            context: PortContext,
            request: ReadCheckoutPaymentCollectionRequest,
        ) -> Result<PaymentCollectionResponse, BoundaryPortError> {
            self.inner.read_checkout_collection(context, request).await
        }
    }

    pub fn in_process_checkout_payment_execution_port(
        db: sea_orm::DatabaseConnection,
    ) -> Arc<dyn CheckoutPaymentExecutionPort> {
        let checkout_admission = checkout_execution_admission_port(db.clone());
        wrap_checkout_payment_execution_port(
            ::rustok_payment::in_process_checkout_payment_execution_port(db, checkout_admission),
        )
    }

    pub(crate) fn wrap_checkout_payment_execution_port(
        inner: Arc<dyn ::rustok_payment::CheckoutPaymentExecutionPort>,
    ) -> Arc<dyn CheckoutPaymentExecutionPort> {
        Arc::new(SanitizingCheckoutPaymentExecutionPort { inner })
    }
}

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
    pub fn new(db: sea_orm::DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            payment_port: in_process_checkout_payment_execution_port(db.clone()),
            operation_journal: CheckoutOperationJournal::new(db.clone(), event_bus),
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
        payment_port: Arc<dyn CanonicalCheckoutPaymentExecutionPort>,
    ) -> Self {
        self.payment_port = wrap_checkout_payment_execution_port(payment_port);
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
        tracing::debug!(
            tenant_id = %tenant_id,
            operation_id = %operation_id,
            order_id = %order.id,
            lease_owner = %lease_owner,
            "advance_to_payment_captured entry"
        );
        validate_order_plan(tenant_id, operation_id, &order, &plan)?;

        for _ in 0..3 {
            let operation = self.operation_journal.get(tenant_id, operation_id).await?;
            validate_operation(&operation, &order)?;
            let identity = payment_identity(&operation, &order, &plan);

            match operation.stage.as_str() {
                stage if stage == CheckoutOperationStage::PaymentReady.as_str() => {
                    let prepare_context = payment_write_context(
                        tenant_id,
                        operation_id,
                        plan.payload.context.locale.as_str(),
                        self.port_deadline,
                        "prepare",
                        format!("checkout:{operation_id}:payment:collection"),
                    );
                    let collection = self
                        .payment_port
                        .prepare_checkout_collection(
                            prepare_context.clone(),
                            PrepareCheckoutPaymentCollectionRequest {
                                identity: identity.clone(),
                                metadata: plan.payload.checkout_metadata.clone(),
                            },
                        )
                        .await
                        .map_err(|error| payment_boundary_error("prepare", error))?;
                    validate_collection(&collection, tenant_id, &identity)?;

                    let authorize_context = payment_write_context(
                        tenant_id,
                        operation_id,
                        plan.payload.context.locale.as_str(),
                        self.port_deadline,
                        "authorize",
                        format!("payment_collection:{}:authorize", collection.id),
                    );
                    let authorized = self
                        .payment_port
                        .authorize_checkout_collection(
                            authorize_context.clone(),
                            AuthorizeCheckoutPaymentCollectionRequest {
                                identity,
                                collection_id: collection.id,
                                provider_id: collection.provider_id.clone(),
                                provider_payment_id: None,
                                metadata: plan.payload.checkout_metadata.clone(),
                            },
                        )
                        .await
                        .map_err(|error| payment_boundary_error("authorize", error))?;
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
                    let capture_context = payment_write_context(
                        tenant_id,
                        operation_id,
                        plan.payload.context.locale.as_str(),
                        self.port_deadline,
                        "capture",
                        format!("payment_collection:{collection_id}:capture"),
                    );
                    let captured = self
                        .payment_port
                        .capture_checkout_collection(
                            capture_context.clone(),
                            CaptureCheckoutPaymentCollectionRequest {
                                identity,
                                collection_id,
                                metadata: plan.payload.checkout_metadata.clone(),
                            },
                        )
                        .await
                        .map_err(|error| payment_boundary_error("capture", error))?;
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
        tracing::debug!(
            tenant_id = %tenant_id,
            operation_id = %operation_id,
            order_id = %order.id,
            "load_payment_captured_state entry"
        );
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
        let read_context = payment_read_context(
            tenant_id,
            operation_id,
            plan.payload.context.locale.as_str(),
            self.port_deadline,
        );
        let collection = self
            .payment_port
            .read_checkout_collection(
                read_context.clone(),
                ReadCheckoutPaymentCollectionRequest {
                    identity: identity.clone(),
                    collection_id,
                },
            )
            .await
            .map_err(|error| payment_boundary_error("read", error))?;
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
    if operation.status != CheckoutOperationStatus::Executing.as_str()
        && operation.status != CheckoutOperationStatus::Completed.as_str()
    {
        return Err(CheckoutPaymentStageError::Conflict(format!(
            "checkout operation {} must be executing or completed, not `{}`",
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
    if tenant_id.is_nil() || operation_id.is_nil() || order.id.is_nil() {
        return Err(CheckoutPaymentStageError::Conflict(
            "tenant_id, operation_id, and order.id must not be nil UUID".to_string(),
        ));
    }
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

/// Projects a sanitized owner rejection onto the payment stage boundary.
///
/// The owner code and retryability are preserved, and the message is the static
/// kind-based consumer message: `BoundaryPortError` already carries it, and the
/// kind is re-derived here as a second line of defence so a custom port cannot
/// publish owner text through this variant.
fn payment_boundary_error(
    stage: &'static str,
    error: BoundaryPortError,
) -> CheckoutPaymentStageError {
    CheckoutPaymentStageError::Boundary {
        stage,
        code: error.code,
        message: public_message(&error.kind).to_string(),
        retryable: error.retryable,
    }
}
