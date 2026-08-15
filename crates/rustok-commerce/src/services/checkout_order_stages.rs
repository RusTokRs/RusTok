use std::sync::Arc;

use rustok_cart::PreparedCartCheckoutSnapshot;
use rustok_inventory::InventoryReservationIdentityPort;
use rustok_order::CheckoutCompletionPort as CanonicalCheckoutCompletionPort;
use rustok_outbox::TransactionalEventBus;
use uuid::Uuid;

use super::{CheckoutOrderPlanJournal, CheckoutOrderPlanPayload};

mod order_stage_boundary {
    use ::rustok_api::{PortActorKind, PortContext, PortError, PortErrorKind};
    use uuid::Uuid;

    const CHECKOUT_ORDER_STAGE_ADAPTER_BOUNDARY: &str = "commerce_checkout_order_stage_adapter";

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

    struct CheckoutOrderStageDiagnosticError;

    impl std::fmt::Debug for CheckoutOrderStageDiagnosticError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
        }
    }

    #[derive(Clone, Copy)]
    struct CheckoutOrderStageContextFacts {
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

    impl From<&PortContext> for CheckoutOrderStageContextFacts {
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

    fn public_message(kind: &PortErrorKind) -> &'static str {
        match kind {
            PortErrorKind::Validation => "Checkout order request is invalid",
            PortErrorKind::NotFound => "Checkout order resource was not found",
            PortErrorKind::Conflict => {
                "Checkout order state conflicts with the requested operation"
            }
            PortErrorKind::Forbidden => "Checkout order operation is not permitted",
            PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                "Checkout order service is temporarily unavailable"
            }
            PortErrorKind::InvariantViolation => {
                "Checkout order operation could not be completed safely"
            }
        }
    }

    pub(crate) fn sanitize_owner_error(
        context: &PortContext,
        owner_operation: &'static str,
        error: PortError,
    ) -> BoundaryPortError {
        let diagnostic_context = CheckoutOrderStageContextFacts::from(context);
        let owner_message_shape = text_shape(error.message.as_str());
        let owner_message_len = error.message.chars().count();
        let public_message = public_message(&error.kind);
        let technical = matches!(
            &error.kind,
            PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
        );
        let diagnostic_error = CheckoutOrderStageDiagnosticError;

        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_order",
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
                boundary = CHECKOUT_ORDER_STAGE_ADAPTER_BOUNDARY,
                "commerce checkout order owner call failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic_error,
                owner = "rustok_order",
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
                boundary = CHECKOUT_ORDER_STAGE_ADAPTER_BOUNDARY,
                "commerce checkout order owner call was rejected"
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
    pub(crate) use super::order_stage_boundary::BoundaryPortError as PortError;
    pub use ::rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortErrorKind};
}

mod rustok_order_shim {
    use std::sync::Arc;

    use ::rustok_api::PortContext;

    use super::order_stage_boundary::{BoundaryPortError, sanitize_owner_error};

    pub use ::rustok_order::{
        CheckoutCompletionSnapshot, CompleteCheckoutPortRequest, CreateOrderInput, OrderResponse,
        OrderStatusKind, ReadCheckoutOrderProjectionRequest, RecoverExistingCheckoutOrderRequest,
    };

    #[async_trait::async_trait]
    pub trait CheckoutCompletionPort: Send + Sync {
        async fn complete_checkout(
            &self,
            context: PortContext,
            request: CompleteCheckoutPortRequest,
        ) -> Result<CheckoutCompletionSnapshot, BoundaryPortError>;
    }

    struct SanitizingCheckoutCompletionPort {
        inner: Arc<dyn ::rustok_order::CheckoutCompletionPort>,
    }

    #[async_trait::async_trait]
    impl CheckoutCompletionPort for SanitizingCheckoutCompletionPort {
        async fn complete_checkout(
            &self,
            context: PortContext,
            request: CompleteCheckoutPortRequest,
        ) -> Result<CheckoutCompletionSnapshot, BoundaryPortError> {
            let error_context = context.clone();
            self.inner
                .complete_checkout(context, request)
                .await
                .map_err(|error| sanitize_owner_error(&error_context, "complete_checkout", error))
        }
    }

    pub struct CheckoutOrderRecoveryAdapter {
        inner: ::rustok_order::CheckoutOrderRecoveryAdapter,
    }

    impl CheckoutOrderRecoveryAdapter {
        pub async fn recover_existing_checkout(
            &self,
            context: PortContext,
            request: RecoverExistingCheckoutOrderRequest,
        ) -> Result<Option<OrderResponse>, BoundaryPortError> {
            let error_context = context.clone();
            self.inner
                .recover_existing_checkout(context, request)
                .await
                .map_err(|error| {
                    sanitize_owner_error(&error_context, "recover_existing_checkout", error)
                })
        }

        pub async fn read_checkout_order(
            &self,
            context: PortContext,
            request: ReadCheckoutOrderProjectionRequest,
        ) -> Result<OrderResponse, BoundaryPortError> {
            let error_context = context.clone();
            self.inner
                .read_checkout_order(context, request)
                .await
                .map_err(|error| sanitize_owner_error(&error_context, "read_checkout_order", error))
        }
    }

    pub fn in_process_checkout_completion_port(
        db: sea_orm::DatabaseConnection,
        event_bus: rustok_outbox::TransactionalEventBus,
    ) -> Arc<dyn CheckoutCompletionPort> {
        wrap_checkout_completion_port(::rustok_order::in_process_checkout_completion_port(
            db, event_bus,
        ))
    }

    pub fn in_process_checkout_order_recovery_adapter(
        db: sea_orm::DatabaseConnection,
        event_bus: rustok_outbox::TransactionalEventBus,
    ) -> CheckoutOrderRecoveryAdapter {
        CheckoutOrderRecoveryAdapter {
            inner: ::rustok_order::in_process_checkout_order_recovery_adapter(db, event_bus),
        }
    }

    pub(crate) fn wrap_checkout_completion_port(
        inner: Arc<dyn ::rustok_order::CheckoutCompletionPort>,
    ) -> Arc<dyn CheckoutCompletionPort> {
        Arc::new(SanitizingCheckoutCompletionPort { inner })
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
    use super::rustok_order_shim as rustok_order;
    use super::tracing_shim as tracing;

    include!("checkout_order_stages_legacy.rs");
}

pub use legacy::{CheckoutOrderStageError, CheckoutOrderStageResult, CheckoutPaymentReadyState};

pub struct CheckoutOrderStageExecutor {
    inner: legacy::CheckoutOrderStageExecutor,
}

impl CheckoutOrderStageExecutor {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        event_bus: TransactionalEventBus,
        inventory_port: Arc<dyn InventoryReservationIdentityPort>,
    ) -> Self {
        Self {
            inner: legacy::CheckoutOrderStageExecutor::new(db, event_bus, inventory_port),
        }
    }

    pub fn with_completion_port(
        mut self,
        completion_port: Arc<dyn CanonicalCheckoutCompletionPort>,
    ) -> Self {
        self.inner =
            self.inner
                .with_completion_port(rustok_order_shim::wrap_checkout_completion_port(
                    completion_port,
                ));
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.inner = self.inner.with_lease_seconds(lease_seconds);
        self
    }

    pub async fn advance_to_payment_ready(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        snapshot: &PreparedCartCheckoutSnapshot,
        initial_plan: Option<CheckoutOrderPlanPayload>,
    ) -> CheckoutOrderStageResult<CheckoutPaymentReadyState> {
        self.inner
            .advance_to_payment_ready(
                tenant_id,
                actor_id,
                operation_id,
                lease_owner,
                snapshot,
                initial_plan,
            )
            .await
    }

    pub async fn load_payment_ready_state(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> CheckoutOrderStageResult<CheckoutPaymentReadyState> {
        self.inner
            .load_payment_ready_state(tenant_id, operation_id)
            .await
    }

    pub fn plan_journal(&self) -> &CheckoutOrderPlanJournal {
        self.inner.plan_journal()
    }
}
