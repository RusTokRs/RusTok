use std::sync::Arc;

use rustok_cart::CartCheckoutPort;
use rustok_inventory::InventoryReservationIdentityPort;
use rustok_order::{CheckoutOrderCompensationPort, CheckoutOrderIdentityPort};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::{
    CheckoutPaymentCompensationPort as CanonicalCheckoutPaymentCompensationPort,
    PaymentProviderRegistry,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::entities::checkout_operation;

use super::{
    CheckoutInventoryReservationError, CheckoutInventoryReservationJournal,
    CheckoutInventoryReservationStatus, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, DEFAULT_CHECKOUT_LEASE_SECONDS,
};

mod payment_compensation_boundary {
    use rustok_api::{PortActorKind, PortContext, PortError, PortErrorKind};
    use rustok_payment::CheckoutPaymentCompensationRequest;
    use serde_json::Value;
    use uuid::Uuid;

    const PAYMENT_COMPENSATION_OWNER: &str = "rustok_payment";
    const PAYMENT_COMPENSATION_OPERATION: &str = "compensate_checkout_payment";
    const PAYMENT_COMPENSATION_STAGE: &str = "compensate_payment";
    const PAYMENT_COMPENSATION_ADAPTER_BOUNDARY: &str =
        "commerce_checkout_payment_compensation_adapter";
    const PAYMENT_MANUAL_RECONCILIATION_CODE: &str =
        "payment.checkout_compensation_manual_reconciliation";

    #[derive(Clone, Copy)]
    pub(crate) struct PaymentCompensationRequestFacts {
        checkout_operation_id_non_nil: bool,
        collection_id_shape: &'static str,
        reason_shape: &'static str,
        reason_len: Option<usize>,
        metadata_kind: &'static str,
        metadata_entry_count: Option<usize>,
    }

    impl From<&CheckoutPaymentCompensationRequest> for PaymentCompensationRequestFacts {
        fn from(request: &CheckoutPaymentCompensationRequest) -> Self {
            Self {
                checkout_operation_id_non_nil: !request.checkout_operation_id.is_nil(),
                collection_id_shape: optional_uuid_shape(request.collection_id),
                reason_shape: optional_text_shape(request.reason.as_deref()),
                reason_len: request.reason.as_ref().map(|value| value.chars().count()),
                metadata_kind: json_kind(&request.metadata),
                metadata_entry_count: match &request.metadata {
                    Value::Object(values) => Some(values.len()),
                    Value::Array(values) => Some(values.len()),
                    _ => None,
                },
            }
        }
    }

    #[derive(Clone, Copy)]
    struct PaymentCompensationContextFacts {
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

    impl From<&PortContext> for PaymentCompensationContextFacts {
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

    struct PaymentCompensationDiagnosticError;

    impl std::fmt::Debug for PaymentCompensationDiagnosticError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
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
            Some(value) if value.is_empty() => "empty",
            Some(_) => "present",
        }
    }

    fn optional_uuid_shape(value: Option<Uuid>) -> &'static str {
        match value {
            None => "absent",
            Some(value) if value.is_nil() => "uuid_nil",
            Some(_) => "uuid_non_nil",
        }
    }

    fn json_kind(value: &Value) -> &'static str {
        match value {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    fn public_message(error: &PortError) -> &'static str {
        if error.code == PAYMENT_MANUAL_RECONCILIATION_CODE {
            return "Checkout payment compensation requires manual reconciliation";
        }
        match &error.kind {
            PortErrorKind::Validation => "Checkout payment compensation request is invalid",
            PortErrorKind::NotFound => "Checkout payment compensation resource was not found",
            PortErrorKind::Conflict => {
                "Checkout payment compensation conflicts with the current payment state"
            }
            PortErrorKind::Forbidden => "Checkout payment compensation is not permitted",
            PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                "Checkout payment compensation service is temporarily unavailable"
            }
            PortErrorKind::InvariantViolation => {
                "Checkout payment compensation could not be completed safely"
            }
        }
    }

    pub(crate) fn sanitize_payment_compensation_error(
        context: &PortContext,
        request_facts: PaymentCompensationRequestFacts,
        error: PortError,
    ) -> PortError {
        log_payment_compensation_error(context, request_facts, &error);
        let message = public_message(&error).to_string();
        PortError {
            kind: error.kind,
            code: error.code,
            message,
            retryable: error.retryable,
        }
    }

    fn log_payment_compensation_error(
        context: &PortContext,
        request_facts: PaymentCompensationRequestFacts,
        error: &PortError,
    ) {
        let context_facts = PaymentCompensationContextFacts::from(context);
        let owner_message_present = !error.message.trim().is_empty();
        let owner_message_len = error.message.chars().count();
        let diagnostic_error = PaymentCompensationDiagnosticError;

        match &error.kind {
            PortErrorKind::Unavailable
            | PortErrorKind::Timeout
            | PortErrorKind::InvariantViolation => {
                tracing::error!(
                    error = ?diagnostic_error,
                    owner = PAYMENT_COMPENSATION_OWNER,
                    operation = PAYMENT_COMPENSATION_OPERATION,
                    stage = PAYMENT_COMPENSATION_STAGE,
                    tenant_id_shape = context_facts.tenant_id_shape,
                    actor_kind = context_facts.actor_kind,
                    actor_id_shape = context_facts.actor_id_shape,
                    claim_count = context_facts.claim_count,
                    role_count = context_facts.role_count,
                    channel_shape = context_facts.channel_shape,
                    locale_shape = context_facts.locale_shape,
                    correlation_id_shape = context_facts.correlation_id_shape,
                    causation_id_shape = context_facts.causation_id_shape,
                    traceparent_shape = context_facts.traceparent_shape,
                    idempotency_key_shape = context_facts.idempotency_key_shape,
                    deadline_ms = ?context_facts.deadline_ms,
                    checkout_operation_id_non_nil = request_facts.checkout_operation_id_non_nil,
                    collection_id_shape = request_facts.collection_id_shape,
                    reason_shape = request_facts.reason_shape,
                    reason_len = ?request_facts.reason_len,
                    metadata_kind = request_facts.metadata_kind,
                    metadata_entry_count = ?request_facts.metadata_entry_count,
                    owner_code = %error.code,
                    owner_message_present,
                    owner_message_len,
                    owner_kind = ?error.kind,
                    owner_retryable = error.retryable,
                    boundary = PAYMENT_COMPENSATION_ADAPTER_BOUNDARY,
                    "commerce checkout payment compensation owner call failed"
                );
            }
            _ => {
                tracing::warn!(
                    error = ?diagnostic_error,
                    owner = PAYMENT_COMPENSATION_OWNER,
                    operation = PAYMENT_COMPENSATION_OPERATION,
                    stage = PAYMENT_COMPENSATION_STAGE,
                    tenant_id_shape = context_facts.tenant_id_shape,
                    actor_kind = context_facts.actor_kind,
                    actor_id_shape = context_facts.actor_id_shape,
                    claim_count = context_facts.claim_count,
                    role_count = context_facts.role_count,
                    channel_shape = context_facts.channel_shape,
                    locale_shape = context_facts.locale_shape,
                    correlation_id_shape = context_facts.correlation_id_shape,
                    causation_id_shape = context_facts.causation_id_shape,
                    traceparent_shape = context_facts.traceparent_shape,
                    idempotency_key_shape = context_facts.idempotency_key_shape,
                    deadline_ms = ?context_facts.deadline_ms,
                    checkout_operation_id_non_nil = request_facts.checkout_operation_id_non_nil,
                    collection_id_shape = request_facts.collection_id_shape,
                    reason_shape = request_facts.reason_shape,
                    reason_len = ?request_facts.reason_len,
                    metadata_kind = request_facts.metadata_kind,
                    metadata_entry_count = ?request_facts.metadata_entry_count,
                    owner_code = %error.code,
                    owner_message_present,
                    owner_message_len,
                    owner_kind = ?error.kind,
                    owner_retryable = error.retryable,
                    boundary = PAYMENT_COMPENSATION_ADAPTER_BOUNDARY,
                    "commerce checkout payment compensation owner call was rejected"
                );
            }
        }
    }
}

mod rustok_payment_shim {
    use std::sync::Arc;

    use async_trait::async_trait;
    use rustok_api::{PortContext, PortError};

    use super::payment_compensation_boundary::{
        PaymentCompensationRequestFacts, sanitize_payment_compensation_error,
    };

    pub use ::rustok_payment::{
        CheckoutPaymentCompensationRequest, PaymentCollectionStatusKind, PaymentProviderRegistry,
    };

    #[async_trait]
    pub trait CheckoutPaymentCompensationPort: Send + Sync {
        async fn compensate_checkout_payment(
            &self,
            context: PortContext,
            request: CheckoutPaymentCompensationRequest,
        ) -> Result<Option<::rustok_payment::PaymentCollectionStatusSnapshot>, PortError>;
    }

    struct SanitizingCheckoutPaymentCompensationPort {
        inner: Arc<dyn ::rustok_payment::CheckoutPaymentCompensationPort>,
    }

    #[async_trait]
    impl CheckoutPaymentCompensationPort for SanitizingCheckoutPaymentCompensationPort {
        async fn compensate_checkout_payment(
            &self,
            context: PortContext,
            request: CheckoutPaymentCompensationRequest,
        ) -> Result<Option<::rustok_payment::PaymentCollectionStatusSnapshot>, PortError> {
            let error_context = context.clone();
            let request_facts = PaymentCompensationRequestFacts::from(&request);
            self.inner
                .compensate_checkout_payment(context, request)
                .await
                .map_err(|error| {
                    sanitize_payment_compensation_error(&error_context, request_facts, error)
                })
        }
    }

    pub struct InProcessCheckoutPaymentCompensationPort {
        inner: Arc<dyn CheckoutPaymentCompensationPort>,
    }

    impl InProcessCheckoutPaymentCompensationPort {
        pub fn with_provider_registry(
            db: sea_orm::DatabaseConnection,
            payment_provider_registry: PaymentProviderRegistry,
        ) -> Self {
            Self {
                inner: wrap_checkout_payment_compensation_port(Arc::new(
                    ::rustok_payment::InProcessCheckoutPaymentCompensationPort::with_provider_registry(
                        db,
                        payment_provider_registry,
                    ),
                )),
            }
        }
    }

    #[async_trait]
    impl CheckoutPaymentCompensationPort for InProcessCheckoutPaymentCompensationPort {
        async fn compensate_checkout_payment(
            &self,
            context: PortContext,
            request: CheckoutPaymentCompensationRequest,
        ) -> Result<Option<::rustok_payment::PaymentCollectionStatusSnapshot>, PortError> {
            self.inner
                .compensate_checkout_payment(context, request)
                .await
        }
    }

    pub fn in_process_checkout_payment_compensation_port(
        db: sea_orm::DatabaseConnection,
    ) -> Arc<dyn CheckoutPaymentCompensationPort> {
        wrap_checkout_payment_compensation_port(
            ::rustok_payment::in_process_checkout_payment_compensation_port(db),
        )
    }

    pub(crate) fn wrap_checkout_payment_compensation_port(
        inner: Arc<dyn ::rustok_payment::CheckoutPaymentCompensationPort>,
    ) -> Arc<dyn CheckoutPaymentCompensationPort> {
        Arc::new(SanitizingCheckoutPaymentCompensationPort { inner })
    }
}

mod tracing_shim {
    macro_rules! error {
        (error = ?$error:expr, owner = $owner:expr, $($rest:tt)*) => {{
            if $owner != "rustok_payment" {
                ::tracing::error!(error = ?$error, owner = $owner, $($rest)*);
            }
        }};
    }

    macro_rules! warn {
        (error = ?$error:expr, owner = $owner:expr, $($rest:tt)*) => {{
            if $owner != "rustok_payment" {
                ::tracing::warn!(error = ?$error, owner = $owner, $($rest)*);
            }
        }};
    }

    pub(crate) use error;
    pub(crate) use warn;
}

mod legacy {
    use super::rustok_payment_shim as rustok_payment;
    use super::tracing_shim as tracing;

    include!("checkout_compensation_owner_ports.rs");
}

pub use legacy::{CheckoutCompensationError, CheckoutCompensationResult};

pub struct CheckoutCompensationService {
    inner: legacy::CheckoutCompensationService,
}

impl CheckoutCompensationService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        reservation_port: Arc<dyn InventoryReservationIdentityPort>,
        cart_port: Arc<dyn CartCheckoutPort>,
    ) -> Self {
        Self {
            inner: legacy::CheckoutCompensationService::new(
                db,
                event_bus,
                reservation_port,
                cart_port,
            ),
        }
    }

    pub fn with_payment_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.inner = self
            .inner
            .with_payment_provider_registry(payment_provider_registry);
        self
    }

    pub fn with_order_identity_port(
        mut self,
        order_identity_port: Arc<dyn CheckoutOrderIdentityPort>,
    ) -> Self {
        self.inner = self.inner.with_order_identity_port(order_identity_port);
        self
    }

    pub fn with_order_compensation_port(
        mut self,
        order_compensation_port: Arc<dyn CheckoutOrderCompensationPort>,
    ) -> Self {
        self.inner = self
            .inner
            .with_order_compensation_port(order_compensation_port);
        self
    }

    pub fn with_payment_compensation_port(
        mut self,
        payment_compensation_port: Arc<dyn CanonicalCheckoutPaymentCompensationPort>,
    ) -> Self {
        self.inner = self.inner.with_payment_compensation_port(
            rustok_payment_shim::wrap_checkout_payment_compensation_port(
                payment_compensation_port,
            ),
        );
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.inner = self.inner.with_lease_seconds(lease_seconds);
        self
    }

    pub async fn compensate(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
    ) -> CheckoutCompensationResult<checkout_operation::Model> {
        self.inner
            .compensate(tenant_id, actor_id, operation_id, lease_owner)
            .await
    }
}
