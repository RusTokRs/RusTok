use std::sync::Arc;

use rustok_order::OrderResponse;
use rustok_payment::{
    CheckoutPaymentExecutionPort as CanonicalCheckoutPaymentExecutionPort, PaymentProviderRegistry,
};
use uuid::Uuid;

use super::{
    CheckoutOperationCheckpoint, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, CheckoutOrderPlanRecord,
    DEFAULT_CHECKOUT_LEASE_SECONDS,
};

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

    fn public_message(kind: &PortErrorKind) -> &'static str {
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

mod rustok_api_shim {
    pub(crate) use super::payment_execution_boundary::BoundaryPortError as PortError;
    pub use ::rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortErrorKind};
}

mod rustok_payment_shim {
    use std::sync::Arc;

    use ::rustok_api::PortContext;

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
            Self {
                inner: wrap_checkout_payment_execution_port(Arc::new(
                    ::rustok_payment::InProcessCheckoutPaymentExecutionPort::with_provider_registry(
                        db,
                        payment_provider_registry,
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
        wrap_checkout_payment_execution_port(
            ::rustok_payment::in_process_checkout_payment_execution_port(db),
        )
    }

    pub(crate) fn wrap_checkout_payment_execution_port(
        inner: Arc<dyn ::rustok_payment::CheckoutPaymentExecutionPort>,
    ) -> Arc<dyn CheckoutPaymentExecutionPort> {
        Arc::new(SanitizingCheckoutPaymentExecutionPort { inner })
    }
}

mod tracing_shim {
    macro_rules! error {
        ($($tokens:tt)*) => {{
            let _ = stringify!($($tokens)*);
        }};
    }

    macro_rules! warn_event {
        ($($tokens:tt)*) => {{
            let _ = stringify!($($tokens)*);
        }};
    }

    pub(crate) use error;
    pub(crate) use warn_event;
}

mod legacy {
    use super::rustok_api_shim as rustok_api;
    use super::rustok_payment_shim as rustok_payment;
    use super::tracing_shim as tracing;

    include!("checkout_payment_stages_legacy.rs");
}

pub use legacy::{
    CheckoutPaymentCapturedState, CheckoutPaymentStageError, CheckoutPaymentStageResult,
};

pub struct CheckoutPaymentStageExecutor {
    inner: legacy::CheckoutPaymentStageExecutor,
}

impl CheckoutPaymentStageExecutor {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            inner: legacy::CheckoutPaymentStageExecutor::new(db),
        }
    }

    pub fn with_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.inner = self.inner.with_provider_registry(payment_provider_registry);
        self
    }

    pub fn with_payment_port(
        mut self,
        payment_port: Arc<dyn CanonicalCheckoutPaymentExecutionPort>,
    ) -> Self {
        self.inner = self.inner.with_payment_port(
            rustok_payment_shim::wrap_checkout_payment_execution_port(payment_port),
        );
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.inner = self.inner.with_lease_seconds(lease_seconds);
        self
    }

    pub async fn advance_to_payment_captured(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        order: OrderResponse,
        plan: CheckoutOrderPlanRecord,
    ) -> CheckoutPaymentStageResult<CheckoutPaymentCapturedState> {
        self.inner
            .advance_to_payment_captured(tenant_id, operation_id, lease_owner, order, plan)
            .await
    }

    pub async fn load_payment_captured_state(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        order: OrderResponse,
        plan: CheckoutOrderPlanRecord,
    ) -> CheckoutPaymentStageResult<CheckoutPaymentCapturedState> {
        self.inner
            .load_payment_captured_state(tenant_id, operation_id, order, plan)
            .await
    }
}
