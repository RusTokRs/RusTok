use std::sync::Arc;

use rustok_cart::CartCheckoutPort;
use rustok_inventory::InventoryReservationIdentityPort as CanonicalInventoryReservationIdentityPort;
use rustok_order::{
    CheckoutOrderCompensationPort as CanonicalCheckoutOrderCompensationPort,
    CheckoutOrderIdentityPort,
};
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

mod safe_boundary {
    use rustok_api::{PortActorKind, PortContext, PortError, PortErrorKind};
    use rustok_inventory::InventoryIdentityReservationReleaseRequest;
    use rustok_order::CheckoutOrderCompensationRequest;
    use rustok_payment::CheckoutPaymentCompensationRequest;
    use serde_json::Value;
    use uuid::Uuid;

    const PAYMENT_MANUAL_CODE: &str = "payment.checkout_compensation_manual_reconciliation";
    const ORDER_MANUAL_CODE: &str = "order.checkout_compensation_manual_reconciliation";

    #[derive(Clone, Copy)]
    enum MessageFamily {
        Payment,
        Order,
        Inventory,
    }

    #[derive(Clone, Copy)]
    pub(crate) struct BoundaryFacts {
        family: MessageFamily,
        owner: &'static str,
        operation: &'static str,
        stage: &'static str,
        boundary: &'static str,
        operation_id_shape: &'static str,
        primary_id_shape: &'static str,
        secondary_id_shape: &'static str,
        opaque_text_shape: &'static str,
        opaque_text_len: Option<usize>,
        payload_kind: &'static str,
        payload_entry_count: Option<usize>,
    }

    impl BoundaryFacts {
        pub(crate) fn payment(request: &CheckoutPaymentCompensationRequest) -> Self {
            Self {
                family: MessageFamily::Payment,
                owner: "rustok_payment",
                operation: "compensate_checkout_payment",
                stage: "compensate_payment",
                boundary: "commerce_checkout_payment_compensation_adapter",
                operation_id_shape: uuid_shape(request.checkout_operation_id),
                primary_id_shape: optional_uuid_shape(request.collection_id),
                secondary_id_shape: "not_applicable",
                opaque_text_shape: optional_text_shape(request.reason.as_deref()),
                opaque_text_len: request.reason.as_ref().map(|value| value.chars().count()),
                payload_kind: json_kind(&request.metadata),
                payload_entry_count: match &request.metadata {
                    Value::Object(values) => Some(values.len()),
                    Value::Array(values) => Some(values.len()),
                    _ => None,
                },
            }
        }

        pub(crate) fn order(request: &CheckoutOrderCompensationRequest) -> Self {
            Self {
                family: MessageFamily::Order,
                owner: "rustok_order",
                operation: "compensate_checkout_order",
                stage: "compensate_order",
                boundary: "commerce_checkout_order_compensation_adapter",
                operation_id_shape: uuid_shape(request.checkout_operation_id),
                primary_id_shape: uuid_shape(request.cart_id),
                secondary_id_shape: optional_uuid_shape(request.expected_order_id),
                opaque_text_shape: optional_text_shape(request.reason.as_deref()),
                opaque_text_len: request.reason.as_ref().map(|value| value.chars().count()),
                payload_kind: "not_applicable",
                payload_entry_count: None,
            }
        }

        pub(crate) fn inventory(request: &InventoryIdentityReservationReleaseRequest) -> Self {
            Self {
                family: MessageFamily::Inventory,
                owner: "rustok_inventory",
                operation: "release_inventory_by_identity",
                stage: "release_inventory",
                boundary: "commerce_checkout_inventory_compensation_adapter",
                operation_id_shape: "not_applicable",
                primary_id_shape: uuid_shape(request.reservation_id),
                secondary_id_shape: "not_applicable",
                opaque_text_shape: text_shape(request.external_id.as_str()),
                opaque_text_len: Some(request.external_id.chars().count()),
                payload_kind: "not_applicable",
                payload_entry_count: None,
            }
        }
    }

    #[derive(Clone, Copy)]
    struct ContextFacts {
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

    impl From<&PortContext> for ContextFacts {
        fn from(context: &PortContext) -> Self {
            Self {
                tenant_id_shape: identity_shape(context.tenant_id.as_str()),
                actor_kind: match &context.actor.kind {
                    PortActorKind::User => "user",
                    PortActorKind::Service => "service",
                    PortActorKind::System => "system",
                },
                actor_id_shape: identity_shape(context.actor.id.as_str()),
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

    struct DiagnosticError;

    impl std::fmt::Debug for DiagnosticError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
        }
    }

    fn identity_shape(value: &str) -> &'static str {
        if value.is_empty() {
            return "empty";
        }
        match Uuid::parse_str(value) {
            Ok(value) if value.is_nil() => "uuid_nil",
            Ok(_) => "uuid_non_nil",
            Err(_) => "opaque",
        }
    }

    fn uuid_shape(value: Uuid) -> &'static str {
        if value.is_nil() {
            "uuid_nil"
        } else {
            "uuid_non_nil"
        }
    }

    fn optional_uuid_shape(value: Option<Uuid>) -> &'static str {
        match value {
            None => "absent",
            Some(value) => uuid_shape(value),
        }
    }

    fn text_shape(value: &str) -> &'static str {
        if value.is_empty() { "empty" } else { "present" }
    }

    fn optional_text_shape(value: Option<&str>) -> &'static str {
        match value {
            None => "absent",
            Some(value) => text_shape(value),
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

    fn public_message(family: MessageFamily, error: &PortError) -> &'static str {
        match family {
            MessageFamily::Payment => {
                if error.code == PAYMENT_MANUAL_CODE {
                    return "Checkout payment compensation requires manual reconciliation";
                }
                match &error.kind {
                    PortErrorKind::Validation => "Checkout payment compensation request is invalid",
                    PortErrorKind::NotFound => {
                        "Checkout payment compensation resource was not found"
                    }
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
            MessageFamily::Order => {
                if error.code == ORDER_MANUAL_CODE {
                    return "Checkout order compensation requires manual reconciliation";
                }
                match &error.kind {
                    PortErrorKind::Validation => "Checkout order compensation request is invalid",
                    PortErrorKind::NotFound => "Checkout order compensation resource was not found",
                    PortErrorKind::Conflict => {
                        "Checkout order compensation conflicts with the current order state"
                    }
                    PortErrorKind::Forbidden => "Checkout order compensation is not permitted",
                    PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                        "Checkout order compensation service is temporarily unavailable"
                    }
                    PortErrorKind::InvariantViolation => {
                        "Checkout order compensation could not be completed safely"
                    }
                }
            }
            MessageFamily::Inventory => match &error.kind {
                PortErrorKind::Validation => "Checkout inventory compensation request is invalid",
                PortErrorKind::NotFound => "Checkout inventory compensation resource was not found",
                PortErrorKind::Conflict => {
                    "Checkout inventory compensation conflicts with the current inventory state"
                }
                PortErrorKind::Forbidden => "Checkout inventory compensation is not permitted",
                PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                    "Checkout inventory compensation service is temporarily unavailable"
                }
                PortErrorKind::InvariantViolation => {
                    "Checkout inventory compensation could not be completed safely"
                }
            },
        }
    }

    pub(crate) fn sanitize(
        context: &PortContext,
        facts: BoundaryFacts,
        error: PortError,
    ) -> PortError {
        log(context, facts, &error);
        let message = public_message(facts.family, &error);
        PortError {
            kind: error.kind,
            code: error.code,
            message: message.to_string(),
            retryable: error.retryable,
        }
    }

    fn log(context: &PortContext, facts: BoundaryFacts, error: &PortError) {
        let context_facts = ContextFacts::from(context);
        let diagnostic_error = DiagnosticError;
        let owner_message_present = !error.message.trim().is_empty();
        let owner_message_len = error.message.chars().count();
        let technical = matches!(
            &error.kind,
            PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
        );

        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = facts.owner,
                operation = facts.operation,
                stage = facts.stage,
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
                operation_id_shape = facts.operation_id_shape,
                primary_id_shape = facts.primary_id_shape,
                secondary_id_shape = facts.secondary_id_shape,
                opaque_text_shape = facts.opaque_text_shape,
                opaque_text_len = ?facts.opaque_text_len,
                payload_kind = facts.payload_kind,
                payload_entry_count = ?facts.payload_entry_count,
                owner_code = %error.code,
                owner_message_present,
                owner_message_len,
                owner_kind = ?error.kind,
                owner_retryable = error.retryable,
                boundary = facts.boundary,
                "commerce checkout compensation owner call failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic_error,
                owner = facts.owner,
                operation = facts.operation,
                stage = facts.stage,
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
                operation_id_shape = facts.operation_id_shape,
                primary_id_shape = facts.primary_id_shape,
                secondary_id_shape = facts.secondary_id_shape,
                opaque_text_shape = facts.opaque_text_shape,
                opaque_text_len = ?facts.opaque_text_len,
                payload_kind = facts.payload_kind,
                payload_entry_count = ?facts.payload_entry_count,
                owner_code = %error.code,
                owner_message_present,
                owner_message_len,
                owner_kind = ?error.kind,
                owner_retryable = error.retryable,
                boundary = facts.boundary,
                "commerce checkout compensation owner call was rejected"
            );
        }
    }
}

mod rustok_payment_shim {
    use std::sync::Arc;

    use async_trait::async_trait;
    use rustok_api::{PortContext, PortError};

    use super::safe_boundary::{BoundaryFacts, sanitize};

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

    struct SanitizingPort {
        inner: Arc<dyn ::rustok_payment::CheckoutPaymentCompensationPort>,
    }

    #[async_trait]
    impl CheckoutPaymentCompensationPort for SanitizingPort {
        async fn compensate_checkout_payment(
            &self,
            context: PortContext,
            request: CheckoutPaymentCompensationRequest,
        ) -> Result<Option<::rustok_payment::PaymentCollectionStatusSnapshot>, PortError> {
            let error_context = context.clone();
            let facts = BoundaryFacts::payment(&request);
            self.inner
                .compensate_checkout_payment(context, request)
                .await
                .map_err(|error| sanitize(&error_context, facts, error))
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
        Arc::new(SanitizingPort { inner })
    }
}

mod rustok_order_shim {
    use std::sync::Arc;

    use async_trait::async_trait;
    use rustok_api::{PortContext, PortError};

    use super::safe_boundary::{BoundaryFacts, sanitize};

    pub use ::rustok_order::{
        CheckoutOrderCompensationRequest, CheckoutOrderCompensationSnapshot,
        CheckoutOrderIdentityPort, OrderStatusKind,
    };

    #[async_trait]
    pub trait CheckoutOrderCompensationPort: Send + Sync {
        async fn compensate_checkout_order(
            &self,
            context: PortContext,
            request: CheckoutOrderCompensationRequest,
        ) -> Result<Option<CheckoutOrderCompensationSnapshot>, PortError>;
    }

    struct SanitizingPort {
        inner: Arc<dyn ::rustok_order::CheckoutOrderCompensationPort>,
    }

    #[async_trait]
    impl CheckoutOrderCompensationPort for SanitizingPort {
        async fn compensate_checkout_order(
            &self,
            context: PortContext,
            request: CheckoutOrderCompensationRequest,
        ) -> Result<Option<CheckoutOrderCompensationSnapshot>, PortError> {
            let error_context = context.clone();
            let facts = BoundaryFacts::order(&request);
            self.inner
                .compensate_checkout_order(context, request)
                .await
                .map_err(|error| sanitize(&error_context, facts, error))
        }
    }

    pub struct InProcessCheckoutOrderCompensationPort {
        inner: Arc<dyn CheckoutOrderCompensationPort>,
    }

    impl InProcessCheckoutOrderCompensationPort {
        pub fn with_identity_port(
            db: sea_orm::DatabaseConnection,
            event_bus: rustok_outbox::TransactionalEventBus,
            identity_port: Arc<dyn CheckoutOrderIdentityPort>,
        ) -> Self {
            Self {
                inner: wrap_checkout_order_compensation_port(Arc::new(
                    ::rustok_order::InProcessCheckoutOrderCompensationPort::with_identity_port(
                        db,
                        event_bus,
                        identity_port,
                    ),
                )),
            }
        }
    }

    #[async_trait]
    impl CheckoutOrderCompensationPort for InProcessCheckoutOrderCompensationPort {
        async fn compensate_checkout_order(
            &self,
            context: PortContext,
            request: CheckoutOrderCompensationRequest,
        ) -> Result<Option<CheckoutOrderCompensationSnapshot>, PortError> {
            self.inner.compensate_checkout_order(context, request).await
        }
    }

    pub fn in_process_checkout_order_compensation_port(
        db: sea_orm::DatabaseConnection,
        event_bus: rustok_outbox::TransactionalEventBus,
    ) -> Arc<dyn CheckoutOrderCompensationPort> {
        wrap_checkout_order_compensation_port(
            ::rustok_order::in_process_checkout_order_compensation_port(db, event_bus),
        )
    }

    pub(crate) fn wrap_checkout_order_compensation_port(
        inner: Arc<dyn ::rustok_order::CheckoutOrderCompensationPort>,
    ) -> Arc<dyn CheckoutOrderCompensationPort> {
        Arc::new(SanitizingPort { inner })
    }
}

mod rustok_inventory_shim {
    use std::sync::Arc;

    use async_trait::async_trait;
    use rustok_api::{PortContext, PortError};

    use super::safe_boundary::{BoundaryFacts, sanitize};

    pub use ::rustok_inventory::{
        InventoryIdentityReservationReleaseRequest, InventoryIdentityReservationReleaseSnapshot,
    };

    #[async_trait]
    pub trait InventoryReservationIdentityPort: Send + Sync {
        async fn release_inventory_by_identity(
            &self,
            context: PortContext,
            request: InventoryIdentityReservationReleaseRequest,
        ) -> Result<InventoryIdentityReservationReleaseSnapshot, PortError>;
    }

    struct SanitizingPort {
        inner: Arc<dyn ::rustok_inventory::InventoryReservationIdentityPort>,
    }

    #[async_trait]
    impl InventoryReservationIdentityPort for SanitizingPort {
        async fn release_inventory_by_identity(
            &self,
            context: PortContext,
            request: InventoryIdentityReservationReleaseRequest,
        ) -> Result<InventoryIdentityReservationReleaseSnapshot, PortError> {
            let error_context = context.clone();
            let facts = BoundaryFacts::inventory(&request);
            self.inner
                .release_inventory_by_identity(context, request)
                .await
                .map_err(|error| sanitize(&error_context, facts, error))
        }
    }

    pub(crate) fn wrap_inventory_reservation_identity_port(
        inner: Arc<dyn ::rustok_inventory::InventoryReservationIdentityPort>,
    ) -> Arc<dyn InventoryReservationIdentityPort> {
        Arc::new(SanitizingPort { inner })
    }
}

mod tracing_shim {
    macro_rules! error {
        (error = ?$error:expr, owner = $owner:expr, $($rest:tt)*) => {{
            if $owner != "rustok_payment"
                && $owner != "rustok_order"
                && $owner != "rustok_inventory"
            {
                ::tracing::error!(error = ?$error, owner = $owner, $($rest)*);
            }
        }};
    }

    macro_rules! warn_event {
        (error = ?$error:expr, owner = $owner:expr, $($rest:tt)*) => {{
            if $owner != "rustok_payment"
                && $owner != "rustok_order"
                && $owner != "rustok_inventory"
            {
                ::tracing::warn!(error = ?$error, owner = $owner, $($rest)*);
            }
        }};
    }

    pub(crate) use error;
    pub(crate) use warn_event;
}

mod legacy {
    use super::rustok_inventory_shim as rustok_inventory;
    use super::rustok_order_shim as rustok_order;
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
        reservation_port: Arc<dyn CanonicalInventoryReservationIdentityPort>,
        cart_port: Arc<dyn CartCheckoutPort>,
    ) -> Self {
        Self {
            inner: legacy::CheckoutCompensationService::new(
                db,
                event_bus,
                rustok_inventory_shim::wrap_inventory_reservation_identity_port(reservation_port),
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
        order_compensation_port: Arc<dyn CanonicalCheckoutOrderCompensationPort>,
    ) -> Self {
        self.inner = self.inner.with_order_compensation_port(
            rustok_order_shim::wrap_checkout_order_compensation_port(order_compensation_port),
        );
        self
    }

    pub fn with_payment_compensation_port(
        mut self,
        payment_compensation_port: Arc<dyn CanonicalCheckoutPaymentCompensationPort>,
    ) -> Self {
        self.inner = self.inner.with_payment_compensation_port(
            rustok_payment_shim::wrap_checkout_payment_compensation_port(payment_compensation_port),
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
