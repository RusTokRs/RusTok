use std::sync::Arc;

use rustok_fulfillment::CheckoutFulfillmentExecutionPort as CanonicalCheckoutFulfillmentExecutionPort;
use rustok_order::CheckoutOrderPaymentSettlementPort as CanonicalCheckoutOrderPaymentSettlementPort;
use rustok_outbox::TransactionalEventBus;
use uuid::Uuid;

use super::CheckoutPaymentCapturedState;

mod owner_execution_boundary {
    use ::rustok_api::{PortActorKind, PortContext, PortError, PortErrorKind};
    use uuid::Uuid;

    const CHECKOUT_FULFILLMENT_EXECUTION_ADAPTER_BOUNDARY: &str =
        "commerce_checkout_fulfillment_execution_adapter";

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

    fn public_message(owner: &'static str, kind: &PortErrorKind) -> &'static str {
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
        let public_message = public_message(owner, &error.kind);
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
            message: public_message.to_string(),
            retryable: error.retryable,
        }
    }
}

mod rustok_api_shim {
    pub(crate) use super::owner_execution_boundary::BoundaryPortError as PortError;
    pub use ::rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortErrorKind};
}

mod rustok_fulfillment_shim {
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

mod rustok_order_shim {
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

mod tracing_shim {
    macro_rules! error {
        ($($tokens:tt)*) => {{
            ::tracing::error!($($tokens)*);
        }};
    }

    macro_rules! warn_event {
        ($($tokens:tt)*) => {{
            ::tracing::warn!($($tokens)*);
        }};
    }

    pub(crate) use error;
    pub(crate) use warn_event;
}

mod legacy {
    use super::rustok_api_shim as rustok_api;
    use super::rustok_fulfillment_shim as rustok_fulfillment;
    use super::rustok_order_shim as rustok_order;
    use super::tracing_shim as tracing;

    include!("checkout_fulfillment_stages_legacy.rs");
}

pub use legacy::{
    CheckoutFulfillmentCreatedState, CheckoutFulfillmentStageError, CheckoutFulfillmentStageResult,
};

pub struct CheckoutFulfillmentStageExecutor {
    inner: legacy::CheckoutFulfillmentStageExecutor,
}

impl CheckoutFulfillmentStageExecutor {
    pub fn new(db: sea_orm::DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: legacy::CheckoutFulfillmentStageExecutor::new(db, event_bus),
        }
    }

    pub fn with_fulfillment_port(
        mut self,
        fulfillment_port: Arc<dyn CanonicalCheckoutFulfillmentExecutionPort>,
    ) -> Self {
        self.inner = self.inner.with_fulfillment_port(
            rustok_fulfillment_shim::wrap_checkout_fulfillment_execution_port(fulfillment_port),
        );
        self
    }

    pub fn with_order_payment_port(
        mut self,
        order_payment_port: Arc<dyn CanonicalCheckoutOrderPaymentSettlementPort>,
    ) -> Self {
        self.inner = self.inner.with_order_payment_port(
            rustok_order_shim::wrap_checkout_order_payment_settlement_port(order_payment_port),
        );
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.inner = self.inner.with_lease_seconds(lease_seconds);
        self
    }

    pub async fn advance_to_fulfillment_created(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        lease_owner: impl Into<String>,
        state: CheckoutPaymentCapturedState,
    ) -> CheckoutFulfillmentStageResult<CheckoutFulfillmentCreatedState> {
        self.inner
            .advance_to_fulfillment_created(tenant_id, actor_id, lease_owner, state)
            .await
    }

    pub async fn load_fulfillment_created_state(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        state: CheckoutPaymentCapturedState,
    ) -> CheckoutFulfillmentStageResult<CheckoutFulfillmentCreatedState> {
        self.inner
            .load_fulfillment_created_state(tenant_id, actor_id, state)
            .await
    }
}
