use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext};
use rustok_fulfillment::CheckoutFulfillmentExecutionPort as CanonicalCheckoutFulfillmentExecutionPort;
use rustok_order::{
    CheckoutOrderPaymentSettlementPort as CanonicalCheckoutOrderPaymentSettlementPort,
    OrderResponse,
};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::{PaymentCollectionResponse, PaymentCollectionStatusKind};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use super::{
    CheckoutOperationCheckpoint, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, CheckoutOrderPlanRecord,
    CheckoutPaymentCapturedState, DEFAULT_CHECKOUT_LEASE_SECONDS,
};

use fulfillment_execution_port::{
    CheckoutFulfillmentCommand, CheckoutFulfillmentExecutionPort, CheckoutFulfillmentItemCommand,
    EnsureCheckoutFulfillmentsRequest, FulfillmentResponse, ReadCheckoutFulfillmentsRequest,
    in_process_checkout_fulfillment_execution_port, wrap_checkout_fulfillment_execution_port,
};
use order_payment_settlement_port::{
    CheckoutOrderPaymentSettlementPort, OrderLineItemResponse, SettleCheckoutOrderPaymentRequest,
    in_process_checkout_order_payment_settlement_port, wrap_checkout_order_payment_settlement_port,
};
use owner_execution_boundary::{BoundaryPortError, public_message};

const FULFILLMENT_EXECUTION_PORT_DEADLINE_SECONDS: u64 = 5;
const MANUAL_PROVIDER_ID: &str = "manual";

mod owner_execution_boundary {
    use ::rustok_api::{PortActorKind, PortContext, PortError, PortErrorKind};
    use uuid::Uuid;

    const CHECKOUT_FULFILLMENT_EXECUTION_ADAPTER_BOUNDARY: &str =
        "commerce_checkout_fulfillment_execution_adapter";

    pub(crate) struct BoundaryPortError {
        pub(crate) kind: PortErrorKind,
        pub(crate) code: String,
        pub(crate) retryable: bool,
    }

    impl std::fmt::Debug for BoundaryPortError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
        }
    }

    struct CheckoutFulfillmentExecutionDiagnosticError;

    impl std::fmt::Debug for CheckoutFulfillmentExecutionDiagnosticError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
        }
    }

    #[derive(Clone, Copy)]
    struct CheckoutFulfillmentExecutionContextFacts {
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

    impl From<&PortContext> for CheckoutFulfillmentExecutionContextFacts {
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

    pub(crate) fn public_message(owner: &'static str, kind: &PortErrorKind) -> &'static str {
        match owner {
            "rustok_order" => match kind {
                PortErrorKind::Validation => "Checkout order settlement request is invalid",
                PortErrorKind::NotFound => "Checkout order settlement resource was not found",
                PortErrorKind::Conflict => {
                    "Checkout order settlement state conflicts with the requested operation"
                }
                PortErrorKind::Forbidden => "Checkout order settlement operation is not permitted",
                PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                    "Checkout order settlement service is temporarily unavailable"
                }
                PortErrorKind::InvariantViolation => {
                    "Checkout order settlement could not be completed safely"
                }
            },
            _ => match kind {
                PortErrorKind::Validation => "Checkout fulfillment request is invalid",
                PortErrorKind::NotFound => "Checkout fulfillment resource was not found",
                PortErrorKind::Conflict => {
                    "Checkout fulfillment state conflicts with the requested operation"
                }
                PortErrorKind::Forbidden => "Checkout fulfillment operation is not permitted",
                PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                    "Checkout fulfillment service is temporarily unavailable"
                }
                PortErrorKind::InvariantViolation => {
                    "Checkout fulfillment operation could not be completed safely"
                }
            },
        }
    }

    pub(crate) fn sanitize_owner_error(
        context: &PortContext,
        owner: &'static str,
        owner_operation: &'static str,
        stage: &'static str,
        error: PortError,
    ) -> BoundaryPortError {
        let diagnostic_context = CheckoutFulfillmentExecutionContextFacts::from(context);
        let owner_message_shape = text_shape(error.message.as_str());
        let owner_message_len = error.message.chars().count();
        let _public_message = public_message(owner, &error.kind);
        let technical = matches!(
            &error.kind,
            PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
        );
        let diagnostic_error = CheckoutFulfillmentExecutionDiagnosticError;

        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = owner,
                owner_operation = owner_operation,
                stage = stage,
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
                owner_message_shape = owner_message_shape,
                owner_message_len = owner_message_len,
                owner_kind = ?error.kind,
                owner_retryable = error.retryable,
                boundary = CHECKOUT_FULFILLMENT_EXECUTION_ADAPTER_BOUNDARY,
                "commerce checkout fulfillment-stage owner call failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic_error,
                owner = owner,
                owner_operation = owner_operation,
                stage = stage,
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
                owner_message_shape = owner_message_shape,
                owner_message_len = owner_message_len,
                owner_kind = ?error.kind,
                owner_retryable = error.retryable,
                boundary = CHECKOUT_FULFILLMENT_EXECUTION_ADAPTER_BOUNDARY,
                "commerce checkout fulfillment-stage owner call was rejected"
            );
        }

        BoundaryPortError {
            kind: error.kind,
            code: error.code,
            retryable: error.retryable,
        }
    }
}

mod fulfillment_execution_port {
    use std::sync::Arc;

    use ::rustok_api::PortContext;

    use super::owner_execution_boundary::{BoundaryPortError, sanitize_owner_error};

    pub use ::rustok_fulfillment::{
        CheckoutFulfillmentCommand, CheckoutFulfillmentItemCommand,
        EnsureCheckoutFulfillmentsRequest, FulfillmentResponse, ReadCheckoutFulfillmentsRequest,
    };

    #[async_trait::async_trait]
    pub trait CheckoutFulfillmentExecutionPort: Send + Sync {
        async fn ensure_checkout_fulfillments(
            &self,
            context: PortContext,
            request: EnsureCheckoutFulfillmentsRequest,
        ) -> Result<Vec<FulfillmentResponse>, BoundaryPortError>;

        async fn read_checkout_fulfillments(
            &self,
            context: PortContext,
            request: ReadCheckoutFulfillmentsRequest,
        ) -> Result<Vec<FulfillmentResponse>, BoundaryPortError>;
    }

    struct SanitizingCheckoutFulfillmentExecutionPort {
        inner: Arc<dyn ::rustok_fulfillment::CheckoutFulfillmentExecutionPort>,
    }

    #[async_trait::async_trait]
    impl CheckoutFulfillmentExecutionPort for SanitizingCheckoutFulfillmentExecutionPort {
        async fn ensure_checkout_fulfillments(
            &self,
            context: PortContext,
            request: EnsureCheckoutFulfillmentsRequest,
        ) -> Result<Vec<FulfillmentResponse>, BoundaryPortError> {
            let error_context = context.clone();
            self.inner
                .ensure_checkout_fulfillments(context, request)
                .await
                .map_err(|error| {
                    sanitize_owner_error(
                        &error_context,
                        "rustok_fulfillment",
                        "ensure_checkout_fulfillments",
                        "ensure_fulfillments",
                        error,
                    )
                })
        }

        async fn read_checkout_fulfillments(
            &self,
            context: PortContext,
            request: ReadCheckoutFulfillmentsRequest,
        ) -> Result<Vec<FulfillmentResponse>, BoundaryPortError> {
            let error_context = context.clone();
            self.inner
                .read_checkout_fulfillments(context, request)
                .await
                .map_err(|error| {
                    sanitize_owner_error(
                        &error_context,
                        "rustok_fulfillment",
                        "read_checkout_fulfillments",
                        "read_fulfillments",
                        error,
                    )
                })
        }
    }

    pub fn in_process_checkout_fulfillment_execution_port(
        db: sea_orm::DatabaseConnection,
    ) -> Arc<dyn CheckoutFulfillmentExecutionPort> {
        wrap_checkout_fulfillment_execution_port(
            ::rustok_fulfillment::in_process_checkout_fulfillment_execution_port(db),
        )
    }

    pub(crate) fn wrap_checkout_fulfillment_execution_port(
        inner: Arc<dyn ::rustok_fulfillment::CheckoutFulfillmentExecutionPort>,
    ) -> Arc<dyn CheckoutFulfillmentExecutionPort> {
        Arc::new(SanitizingCheckoutFulfillmentExecutionPort { inner })
    }
}

mod order_payment_settlement_port {
    use std::sync::Arc;

    use ::rustok_api::PortContext;
    use ::rustok_outbox::TransactionalEventBus;

    use super::owner_execution_boundary::{BoundaryPortError, sanitize_owner_error};

    pub use ::rustok_order::{
        OrderLineItemResponse, OrderResponse, SettleCheckoutOrderPaymentRequest,
    };

    #[async_trait::async_trait]
    pub trait CheckoutOrderPaymentSettlementPort: Send + Sync {
        async fn settle_checkout_payment(
            &self,
            context: PortContext,
            request: SettleCheckoutOrderPaymentRequest,
        ) -> Result<OrderResponse, BoundaryPortError>;
    }

    struct SanitizingCheckoutOrderPaymentSettlementPort {
        inner: Arc<dyn ::rustok_order::CheckoutOrderPaymentSettlementPort>,
    }

    #[async_trait::async_trait]
    impl CheckoutOrderPaymentSettlementPort for SanitizingCheckoutOrderPaymentSettlementPort {
        async fn settle_checkout_payment(
            &self,
            context: PortContext,
            request: SettleCheckoutOrderPaymentRequest,
        ) -> Result<OrderResponse, BoundaryPortError> {
            let error_context = context.clone();
            self.inner
                .settle_checkout_payment(context, request)
                .await
                .map_err(|error| {
                    sanitize_owner_error(
                        &error_context,
                        "rustok_order",
                        "settle_checkout_payment",
                        "settle_order_payment",
                        error,
                    )
                })
        }
    }

    pub fn in_process_checkout_order_payment_settlement_port(
        db: sea_orm::DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> Arc<dyn CheckoutOrderPaymentSettlementPort> {
        wrap_checkout_order_payment_settlement_port(
            ::rustok_order::in_process_checkout_order_payment_settlement_port(db, event_bus),
        )
    }

    pub(crate) fn wrap_checkout_order_payment_settlement_port(
        inner: Arc<dyn ::rustok_order::CheckoutOrderPaymentSettlementPort>,
    ) -> Arc<dyn CheckoutOrderPaymentSettlementPort> {
        Arc::new(SanitizingCheckoutOrderPaymentSettlementPort { inner })
    }
}

#[derive(Clone, Debug)]
pub struct CheckoutFulfillmentCreatedState {
    pub operation_id: Uuid,
    pub order: OrderResponse,
    pub plan: CheckoutOrderPlanRecord,
    pub payment_collection: PaymentCollectionResponse,
    pub fulfillments: Vec<FulfillmentResponse>,
}

#[derive(Debug, Error)]
pub enum CheckoutFulfillmentStageError {
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(
        "checkout fulfillment boundary `{stage}` failed with `{code}` (retryable={retryable}): {message}"
    )]
    Boundary {
        stage: &'static str,
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("checkout fulfillment stage conflict: {0}")]
    Conflict(String),
}

pub type CheckoutFulfillmentStageResult<T> = Result<T, CheckoutFulfillmentStageError>;

pub struct CheckoutFulfillmentStageExecutor {
    fulfillment_port: Arc<dyn CheckoutFulfillmentExecutionPort>,
    order_payment_port: Arc<dyn CheckoutOrderPaymentSettlementPort>,
    operation_journal: CheckoutOperationJournal,
    lease_seconds: i64,
    port_deadline: Duration,
}

impl CheckoutFulfillmentStageExecutor {
    pub fn new(db: sea_orm::DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            fulfillment_port: in_process_checkout_fulfillment_execution_port(db.clone()),
            order_payment_port: in_process_checkout_order_payment_settlement_port(
                db.clone(),
                event_bus.clone(),
            ),
            operation_journal: CheckoutOperationJournal::new(db, event_bus),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
            port_deadline: Duration::from_secs(FULFILLMENT_EXECUTION_PORT_DEADLINE_SECONDS),
        }
    }

    pub fn with_fulfillment_port(
        mut self,
        fulfillment_port: Arc<dyn CanonicalCheckoutFulfillmentExecutionPort>,
    ) -> Self {
        self.fulfillment_port = wrap_checkout_fulfillment_execution_port(fulfillment_port);
        self
    }

    pub fn with_order_payment_port(
        mut self,
        order_payment_port: Arc<dyn CanonicalCheckoutOrderPaymentSettlementPort>,
    ) -> Self {
        self.order_payment_port = wrap_checkout_order_payment_settlement_port(order_payment_port);
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    /// Creates or adopts the immutable fulfillment set through the fulfillment
    /// owner, settles the captured payment identity through the order owner, and
    /// checkpoints `payment_captured -> fulfillment_created` in commerce.
    pub async fn advance_to_fulfillment_created(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        lease_owner: impl Into<String>,
        state: CheckoutPaymentCapturedState,
    ) -> CheckoutFulfillmentStageResult<CheckoutFulfillmentCreatedState> {
        let lease_owner = lease_owner.into();
        validate_captured_state(tenant_id, &state)?;

        for _ in 0..2 {
            let operation = self
                .operation_journal
                .get(tenant_id, state.operation_id)
                .await?;
            validate_operation(&operation, &state)?;

            match operation.stage.as_str() {
                stage if stage == CheckoutOperationStage::PaymentCaptured.as_str() => {
                    let fulfillments = self.ensure_fulfillments(tenant_id, &state).await?;
                    let paid_order = self.settle_paid_order(tenant_id, actor_id, &state).await?;
                    self.operation_journal
                        .checkpoint(CheckoutOperationCheckpoint {
                            tenant_id,
                            operation_id: operation.id,
                            lease_owner: lease_owner.clone(),
                            expected_stage: CheckoutOperationStage::PaymentCaptured,
                            next_stage: CheckoutOperationStage::FulfillmentCreated,
                            snapshot_hash: None,
                            order_id: Some(paid_order.id),
                            payment_collection_id: Some(state.payment_collection.id),
                            lease_seconds: self.lease_seconds,
                        })
                        .await?;
                    let _ = fulfillments;
                }
                stage if stage == CheckoutOperationStage::FulfillmentCreated.as_str() => {
                    return self
                        .load_fulfillment_created_state(tenant_id, actor_id, state)
                        .await;
                }
                stage => {
                    return Err(CheckoutFulfillmentStageError::Conflict(format!(
                        "checkout operation {} cannot enter fulfillment stages from `{stage}`",
                        operation.id
                    )));
                }
            }
        }

        Err(CheckoutFulfillmentStageError::Conflict(format!(
            "checkout operation {} did not reach fulfillment_created within the bounded stage loop",
            state.operation_id
        )))
    }

    pub async fn load_fulfillment_created_state(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        state: CheckoutPaymentCapturedState,
    ) -> CheckoutFulfillmentStageResult<CheckoutFulfillmentCreatedState> {
        validate_captured_state(tenant_id, &state)?;
        let operation = self
            .operation_journal
            .get(tenant_id, state.operation_id)
            .await?;
        validate_operation(&operation, &state)?;
        if !matches!(
            operation.stage.as_str(),
            "fulfillment_created" | "cart_completed" | "completed"
        ) {
            return Err(CheckoutFulfillmentStageError::Conflict(format!(
                "checkout operation {} has not reached fulfillment_created, stage={}",
                operation.id, operation.stage
            )));
        }
        let fulfillments = self.read_fulfillments(tenant_id, &state).await?;
        let paid_order = self.settle_paid_order(tenant_id, actor_id, &state).await?;
        Ok(CheckoutFulfillmentCreatedState {
            operation_id: state.operation_id,
            order: paid_order,
            plan: state.plan,
            payment_collection: state.payment_collection,
            fulfillments,
        })
    }

    async fn ensure_fulfillments(
        &self,
        tenant_id: Uuid,
        state: &CheckoutPaymentCapturedState,
    ) -> CheckoutFulfillmentStageResult<Vec<FulfillmentResponse>> {
        let plans = fulfillment_commands(&state.order, &state.plan)?;
        if !state.plan.payload.create_fulfillment && !plans.is_empty() {
            return Err(CheckoutFulfillmentStageError::Conflict(
                "immutable order plan contains disabled fulfillment work".to_string(),
            ));
        }
        if plans.is_empty() {
            return Ok(Vec::new());
        }
        let fulfillment_context = fulfillment_write_context(
            tenant_id,
            state.operation_id,
            state.plan.payload.context.locale.as_str(),
            self.port_deadline,
        );
        self.fulfillment_port
            .ensure_checkout_fulfillments(
                fulfillment_context.clone(),
                EnsureCheckoutFulfillmentsRequest {
                    checkout_operation_id: state.operation_id,
                    order_id: state.order.id,
                    customer_id: state.order.customer_id,
                    order_plan_hash: state.plan.plan_hash.clone(),
                    plans,
                },
            )
            .await
            .map_err(|error| {
                fulfillment_stage_boundary_error("rustok_fulfillment", "ensure_fulfillments", error)
            })
    }

    async fn read_fulfillments(
        &self,
        tenant_id: Uuid,
        state: &CheckoutPaymentCapturedState,
    ) -> CheckoutFulfillmentStageResult<Vec<FulfillmentResponse>> {
        let plans = fulfillment_commands(&state.order, &state.plan)?;
        let fulfillment_context = fulfillment_read_context(
            tenant_id,
            state.operation_id,
            state.plan.payload.context.locale.as_str(),
            self.port_deadline,
        );
        self.fulfillment_port
            .read_checkout_fulfillments(
                fulfillment_context.clone(),
                ReadCheckoutFulfillmentsRequest {
                    checkout_operation_id: state.operation_id,
                    order_id: state.order.id,
                    customer_id: state.order.customer_id,
                    order_plan_hash: state.plan.plan_hash.clone(),
                    expected_plans: plans,
                },
            )
            .await
            .map_err(|error| {
                fulfillment_stage_boundary_error("rustok_fulfillment", "read_fulfillments", error)
            })
    }

    async fn settle_paid_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        state: &CheckoutPaymentCapturedState,
    ) -> CheckoutFulfillmentStageResult<OrderResponse> {
        let payment_reference = payment_reference(&state.payment_collection, state.order.id);
        let payment_method = state
            .payment_collection
            .provider_id
            .clone()
            .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());
        let order_context = order_payment_context(
            tenant_id,
            actor_id,
            state.operation_id,
            state.plan.payload.context.locale.as_str(),
            self.port_deadline,
        );
        self.order_payment_port
            .settle_checkout_payment(
                order_context.clone(),
                SettleCheckoutOrderPaymentRequest {
                    checkout_operation_id: state.operation_id,
                    cart_id: state.payment_collection.cart_id.ok_or_else(|| {
                        CheckoutFulfillmentStageError::Conflict(
                            "captured payment collection has no checkout cart identity".to_string(),
                        )
                    })?,
                    order_id: state.order.id,
                    payment_collection_id: state.payment_collection.id,
                    payment_reference,
                    payment_method,
                    locale: Some(state.plan.payload.context.locale.clone()),
                    fallback_locale: Some(state.plan.payload.context.default_locale.clone()),
                },
            )
            .await
            .map_err(|error| {
                fulfillment_stage_boundary_error("rustok_order", "settle_order_payment", error)
            })
    }
}

fn validate_operation(
    operation: &crate::entities::checkout_operation::Model,
    state: &CheckoutPaymentCapturedState,
) -> CheckoutFulfillmentStageResult<()> {
    if operation.status != CheckoutOperationStatus::Executing.as_str()
        && operation.status != CheckoutOperationStatus::Completed.as_str()
    {
        return Err(CheckoutFulfillmentStageError::Conflict(format!(
            "checkout operation {} must be executing or completed, not `{}`",
            operation.id, operation.status
        )));
    }
    if operation.order_id != Some(state.order.id)
        || operation.payment_collection_id != Some(state.payment_collection.id)
    {
        return Err(CheckoutFulfillmentStageError::Conflict(format!(
            "checkout operation {} does not match the captured order and collection",
            operation.id
        )));
    }
    Ok(())
}

fn fulfillment_commands(
    order: &OrderResponse,
    plan: &CheckoutOrderPlanRecord,
) -> CheckoutFulfillmentStageResult<Vec<CheckoutFulfillmentCommand>> {
    if !plan.payload.create_fulfillment {
        if !plan.payload.fulfillment_plans.is_empty() {
            return Err(CheckoutFulfillmentStageError::Conflict(
                "fulfillment plans require create_fulfillment=true".to_string(),
            ));
        }
        return Ok(Vec::new());
    }
    let order_lines = order_lines_by_cart_identity(order)?;
    plan.payload
        .fulfillment_plans
        .iter()
        .enumerate()
        .map(|(index, fulfillment_plan)| {
            let index = u32::try_from(index).map_err(|_| {
                CheckoutFulfillmentStageError::Conflict(
                    "fulfillment plan index exceeds the owner contract".to_string(),
                )
            })?;
            let mut items = Vec::with_capacity(fulfillment_plan.items.len());
            for item in &fulfillment_plan.items {
                let order_line = order_lines.get(&item.cart_line_item_id).ok_or_else(|| {
                    CheckoutFulfillmentStageError::Conflict(format!(
                        "fulfillment plan {index} references missing cart line {}",
                        item.cart_line_item_id
                    ))
                })?;
                if item.quantity != order_line.quantity {
                    return Err(CheckoutFulfillmentStageError::Conflict(format!(
                        "fulfillment plan {index} does not exactly cover order line {}",
                        order_line.id
                    )));
                }
                items.push(CheckoutFulfillmentItemCommand {
                    order_line_item_id: order_line.id,
                    cart_line_item_id: item.cart_line_item_id,
                    quantity: item.quantity,
                    metadata: item.metadata.clone(),
                });
            }
            Ok(CheckoutFulfillmentCommand {
                index,
                shipping_option_id: fulfillment_plan.shipping_option_id,
                carrier: fulfillment_plan.carrier.clone(),
                tracking_number: fulfillment_plan.tracking_number.clone(),
                items,
                metadata: fulfillment_plan.metadata.clone(),
            })
        })
        .collect()
}

fn order_lines_by_cart_identity(
    order: &OrderResponse,
) -> CheckoutFulfillmentStageResult<HashMap<Uuid, &OrderLineItemResponse>> {
    let mut result = HashMap::new();
    for line in &order.line_items {
        let Some(cart_line_item_id) = line
            .metadata
            .get("checkout")
            .and_then(|checkout| checkout.get("cart_line_item_id"))
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            continue;
        };
        if result.insert(cart_line_item_id, line).is_some() {
            return Err(CheckoutFulfillmentStageError::Conflict(format!(
                "order {} contains duplicate cart-line provenance {}",
                order.id, cart_line_item_id
            )));
        }
    }
    Ok(result)
}

fn validate_captured_state(
    tenant_id: Uuid,
    state: &CheckoutPaymentCapturedState,
) -> CheckoutFulfillmentStageResult<()> {
    if state.order.tenant_id != tenant_id
        || state.plan.tenant_id != tenant_id
        || state.payment_collection.tenant_id != tenant_id
        || state.plan.checkout_operation_id != state.operation_id
        || state.payment_collection.order_id != Some(state.order.id)
        || state.payment_collection.status_kind() != PaymentCollectionStatusKind::Captured
        || state.payment_collection.captured_amount != state.order.total_amount
    {
        return Err(CheckoutFulfillmentStageError::Conflict(
            "captured payment state does not describe one checkout identity".to_string(),
        ));
    }
    Ok(())
}

fn payment_reference(collection: &PaymentCollectionResponse, order_id: Uuid) -> String {
    collection
        .payments
        .last()
        .map(|payment| payment.provider_payment_id.clone())
        .unwrap_or_else(|| format!("manual_{order_id}"))
}

fn fulfillment_write_context(
    tenant_id: Uuid,
    operation_id: Uuid,
    locale: &str,
    deadline: Duration,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.checkout-fulfillment-stage"),
        normalize_locale(locale),
        format!("checkout:{operation_id}:fulfillment:ensure"),
    )
    .with_causation_id(operation_id.to_string())
    .with_idempotency_key(format!("checkout:{operation_id}:fulfillment-set"))
    .with_deadline(deadline)
}

fn fulfillment_read_context(
    tenant_id: Uuid,
    operation_id: Uuid,
    locale: &str,
    deadline: Duration,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.checkout-fulfillment-stage"),
        normalize_locale(locale),
        format!("checkout:{operation_id}:fulfillment:read"),
    )
    .with_causation_id(operation_id.to_string())
    .with_deadline(deadline)
}

fn order_payment_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    operation_id: Uuid,
    locale: &str,
    deadline: Duration,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        normalize_locale(locale),
        format!("checkout:{operation_id}:order:payment-settlement"),
    )
    .with_causation_id(operation_id.to_string())
    .with_idempotency_key(format!("checkout:{operation_id}:order:payment-settlement"))
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

/// Projects a sanitized owner rejection onto the fulfillment stage boundary.
///
/// The owner code and retryability are preserved, and the message is the static
/// kind-based consumer message: `BoundaryPortError` already carries it, and the
/// kind is re-derived here as a second line of defence so a custom port cannot
/// publish owner text through this variant.
fn fulfillment_stage_boundary_error(
    owner: &'static str,
    stage: &'static str,
    error: BoundaryPortError,
) -> CheckoutFulfillmentStageError {
    CheckoutFulfillmentStageError::Boundary {
        stage,
        code: error.code,
        message: public_message(owner, &error.kind).to_string(),
        retryable: error.retryable,
    }
}
