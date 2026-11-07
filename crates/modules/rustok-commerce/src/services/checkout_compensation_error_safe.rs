use std::sync::Arc;

use rustok_cart::CartCheckoutPort as CanonicalCartCheckoutPort;
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
    use rustok_cart::{CartCheckoutLifecycleRequest, CartCheckoutSnapshotRequest};
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
        Cart,
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

        pub(crate) fn cart_snapshot(request: &CartCheckoutSnapshotRequest) -> Self {
            Self {
                family: MessageFamily::Cart,
                owner: "rustok_cart",
                operation: "read_cart_checkout_snapshot",
                stage: "read_cart",
                boundary: "commerce_checkout_cart_snapshot_compensation_adapter",
                operation_id_shape: "not_applicable",
                primary_id_shape: uuid_shape(request.cart_id),
                secondary_id_shape: "not_applicable",
                opaque_text_shape: optional_text_shape(request.locale.as_deref()),
                opaque_text_len: request.locale.as_ref().map(|value| value.chars().count()),
                payload_kind: "not_applicable",
                payload_entry_count: None,
            }
        }

        pub(crate) fn cart_release(request: &CartCheckoutLifecycleRequest) -> Self {
            Self {
                family: MessageFamily::Cart,
                owner: "rustok_cart",
                operation: "release_cart_checkout",
                stage: "release_cart",
                boundary: "commerce_checkout_cart_release_compensation_adapter",
                operation_id_shape: "not_applicable",
                primary_id_shape: uuid_shape(request.cart_id),
                secondary_id_shape: "not_applicable",
                opaque_text_shape: "not_applicable",
                opaque_text_len: None,
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
            MessageFamily::Cart => match &error.kind {
                PortErrorKind::Validation => "Checkout cart compensation request is invalid",
                PortErrorKind::NotFound => "Checkout cart compensation resource was not found",
                PortErrorKind::Conflict => {
                    "Checkout cart compensation conflicts with the current cart state"
                }
                PortErrorKind::Forbidden => "Checkout cart compensation is not permitted",
                PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                    "Checkout cart compensation service is temporarily unavailable"
                }
                PortErrorKind::InvariantViolation => {
                    "Checkout cart compensation could not be completed safely"
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

mod rustok_cart_shim {
    use std::sync::Arc;

    use async_trait::async_trait;
    use rustok_api::{PortContext, PortError};

    use super::safe_boundary::{BoundaryFacts, sanitize};

    pub use ::rustok_cart::{
        CartCheckoutLifecycleRequest, CartCheckoutSnapshotRequest, CartResponse, CartStatus,
    };

    #[async_trait]
    pub trait CartCheckoutPort: Send + Sync {
        async fn read_cart_checkout_snapshot(
            &self,
            context: PortContext,
            request: CartCheckoutSnapshotRequest,
        ) -> Result<CartResponse, PortError>;

        async fn release_cart_checkout(
            &self,
            context: PortContext,
            request: CartCheckoutLifecycleRequest,
        ) -> Result<CartResponse, PortError>;
    }

    struct SanitizingPort {
        inner: Arc<dyn ::rustok_cart::CartCheckoutPort>,
    }

    #[async_trait]
    impl CartCheckoutPort for SanitizingPort {
        async fn read_cart_checkout_snapshot(
            &self,
            context: PortContext,
            request: CartCheckoutSnapshotRequest,
        ) -> Result<CartResponse, PortError> {
            let error_context = context.clone();
            let facts = BoundaryFacts::cart_snapshot(&request);
            self.inner
                .read_cart_checkout_snapshot(context, request)
                .await
                .map_err(|error| sanitize(&error_context, facts, error))
        }

        async fn release_cart_checkout(
            &self,
            context: PortContext,
            request: CartCheckoutLifecycleRequest,
        ) -> Result<CartResponse, PortError> {
            let error_context = context.clone();
            let facts = BoundaryFacts::cart_release(&request);
            self.inner
                .release_cart_checkout(context, request)
                .await
                .map_err(|error| sanitize(&error_context, facts, error))
        }
    }

    pub(crate) fn wrap_cart_checkout_port(
        inner: Arc<dyn ::rustok_cart::CartCheckoutPort>,
    ) -> Arc<dyn CartCheckoutPort> {
        Arc::new(SanitizingPort { inner })
    }
}

mod tracing_shim {
    macro_rules! error {
        (error = ?$error:expr, owner = $owner:expr, $($rest:tt)*) => {{
            if $owner != "rustok_payment"
                && $owner != "rustok_order"
                && $owner != "rustok_inventory"
                && $owner != "rustok_cart"
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
                && $owner != "rustok_cart"
            {
                ::tracing::warn!(error = ?$error, owner = $owner, $($rest)*);
            }
        }};
    }

    macro_rules! debug {
        ($($tokens:tt)*) => {{
            ::tracing::debug!($($tokens)*);
        }};
    }

    pub(crate) use debug;
    pub(crate) use error;
    pub(crate) use warn_event;
}

mod legacy {
    use super::rustok_cart_shim as rustok_cart;
    use super::rustok_inventory_shim as rustok_inventory;
    use super::rustok_order_shim as rustok_order;
    use super::rustok_payment_shim as rustok_payment;
    use super::tracing_shim as tracing;

    use std::{sync::Arc, time::Duration};

use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError, PortErrorKind};
use rustok_cart::{
    CartCheckoutLifecycleRequest, CartCheckoutPort, CartCheckoutSnapshotRequest, CartResponse,
    CartStatus,
};
use rustok_inventory::{
    InventoryIdentityReservationReleaseRequest, InventoryReservationIdentityPort,
};
use rustok_order::{
    CheckoutOrderCompensationPort, CheckoutOrderCompensationRequest, CheckoutOrderIdentityPort,
    InProcessCheckoutOrderCompensationPort, OrderStatusKind,
    in_process_checkout_order_compensation_port,
};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::{
    CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,
    InProcessCheckoutPaymentCompensationPort, PaymentCollectionStatusKind, PaymentProviderRegistry,
    in_process_checkout_payment_compensation_port,
};
use sea_orm::DatabaseConnection;
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{checkout_inventory_reservation, checkout_operation};

use super::{
    CheckoutInventoryReservationError, CheckoutInventoryReservationJournal,
    CheckoutInventoryReservationStatus, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, DEFAULT_CHECKOUT_LEASE_SECONDS,
};

const COMPENSATION_PORT_DEADLINE_SECONDS: u64 = 3;
const CHECKOUT_COMPENSATION_OWNER_BOUNDARY: &str = "checkout_compensation_owner_port";
const PAYMENT_COMPENSATION_OWNER: &str = "rustok_payment";
const ORDER_COMPENSATION_OWNER: &str = "rustok_order";
const INVENTORY_COMPENSATION_OWNER: &str = "rustok_inventory";
const CART_COMPENSATION_OWNER: &str = "rustok_cart";
const PAYMENT_COMPENSATION_OPERATION: &str = "compensate_checkout_payment";
const ORDER_COMPENSATION_OPERATION: &str = "compensate_checkout_order";
const INVENTORY_COMPENSATION_OPERATION: &str = "release_inventory_by_identity";
const CART_SNAPSHOT_OPERATION: &str = "read_cart_checkout_snapshot";
const CART_RELEASE_OPERATION: &str = "release_cart_checkout";
const ORDER_MANUAL_RECONCILIATION_CODE: &str = "order.checkout_compensation_manual_reconciliation";
const PAYMENT_MANUAL_RECONCILIATION_CODE: &str =
    "payment.checkout_compensation_manual_reconciliation";

#[derive(Debug, Error)]
pub enum CheckoutCompensationError {
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    ReservationJournal(#[from] CheckoutInventoryReservationError),
    #[error("checkout compensation requires manual reconciliation: {0}")]
    ManualReconciliation(String),
    #[error("checkout compensation conflict: {0}")]
    Conflict(String),
    #[error(
        "checkout compensation boundary `{stage}` failed with `{code}` (retryable={retryable}): {message}"
    )]
    Boundary {
        stage: &'static str,
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("checkout compensation failed: {compensation}; journal update failed: {journal}")]
    CompensationAndJournal {
        compensation: Box<CheckoutCompensationError>,
        journal: CheckoutOperationError,
    },
}

pub type CheckoutCompensationResult<T> = Result<T, CheckoutCompensationError>;

pub struct CheckoutCompensationService {
    owner_db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    operation_journal: CheckoutOperationJournal,
    reservation_journal: CheckoutInventoryReservationJournal,
    reservation_port: Arc<dyn InventoryReservationIdentityPort>,
    cart_port: Arc<dyn CartCheckoutPort>,
    order_compensation_port: Arc<dyn CheckoutOrderCompensationPort>,
    payment_compensation_port: Arc<dyn CheckoutPaymentCompensationPort>,
    lease_seconds: i64,
    port_deadline: Duration,
}

impl CheckoutCompensationService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        reservation_port: Arc<dyn InventoryReservationIdentityPort>,
        cart_port: Arc<dyn CartCheckoutPort>,
    ) -> Self {
        Self {
            owner_db: db.clone(),
            event_bus: event_bus.clone(),
            operation_journal: CheckoutOperationJournal::new(db.clone(), event_bus.clone()),
            reservation_journal: CheckoutInventoryReservationJournal::new(db.clone()),
            reservation_port,
            cart_port,
            order_compensation_port: in_process_checkout_order_compensation_port(
                db.clone(),
                event_bus,
            ),
            payment_compensation_port: in_process_checkout_payment_compensation_port(db),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
            port_deadline: Duration::from_secs(COMPENSATION_PORT_DEADLINE_SECONDS),
        }
    }

    pub fn with_payment_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.payment_compensation_port = Arc::new(
            InProcessCheckoutPaymentCompensationPort::with_provider_registry(
                self.owner_db.clone(),
                payment_provider_registry,
            ),
        );
        self
    }

    pub fn with_order_identity_port(
        mut self,
        order_identity_port: Arc<dyn CheckoutOrderIdentityPort>,
    ) -> Self {
        self.order_compensation_port =
            Arc::new(InProcessCheckoutOrderCompensationPort::with_identity_port(
                self.owner_db.clone(),
                self.event_bus.clone(),
                order_identity_port,
            ));
        self
    }

    pub fn with_order_compensation_port(
        mut self,
        order_compensation_port: Arc<dyn CheckoutOrderCompensationPort>,
    ) -> Self {
        self.order_compensation_port = order_compensation_port;
        self
    }

    pub fn with_payment_compensation_port(
        mut self,
        payment_compensation_port: Arc<dyn CheckoutPaymentCompensationPort>,
    ) -> Self {
        self.payment_compensation_port = payment_compensation_port;
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    pub async fn compensate(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
    ) -> CheckoutCompensationResult<checkout_operation::Model> {
        if tenant_id.is_nil() || operation_id.is_nil() {
            return Err(CheckoutCompensationError::Conflict(
                "tenant_id and operation_id must not be nil UUID".to_string(),
            ));
        }
        let lease_owner = lease_owner.into();
        tracing::debug!(
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            operation_id = %operation_id,
            lease_owner = %lease_owner,
            "CheckoutCompensationService (owner ports)::compensate entry"
        );
        let Some(operation) = self
            .operation_journal
            .claim_compensation(
                tenant_id,
                operation_id,
                lease_owner.as_str(),
                self.lease_seconds,
            )
            .await?
        else {
            let current = self.operation_journal.get(tenant_id, operation_id).await?;
            if current.status == CheckoutOperationStatus::Compensated.as_str() {
                return Ok(current);
            }
            return Err(CheckoutCompensationError::Conflict(format!(
                "checkout operation {} cannot be claimed for compensation; status={}, lease_owner={}",
                current.id,
                current.status,
                current.lease_owner.as_deref().unwrap_or("none")
            )));
        };

        let result = self
            .compensate_claimed(tenant_id, actor_id, &operation)
            .await;
        match result {
            Ok(()) => self
                .operation_journal
                .mark_compensated(tenant_id, operation.id, lease_owner)
                .await
                .map_err(Into::into),
            Err(compensation) => {
                let code = compensation_error_code(&compensation);
                let message = compensation.to_string();
                match self
                    .operation_journal
                    .mark_compensation_retryable(
                        tenant_id,
                        operation.id,
                        lease_owner,
                        code,
                        message,
                    )
                    .await
                {
                    Ok(_) => Err(compensation),
                    Err(journal) => Err(CheckoutCompensationError::CompensationAndJournal {
                        compensation: Box::new(compensation),
                        journal,
                    }),
                }
            }
        }
    }

    async fn compensate_claimed(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        if stage_rank(operation.stage.as_str())?
            >= stage_rank(CheckoutOperationStage::PaymentCaptured.as_str())?
        {
            return Err(CheckoutCompensationError::ManualReconciliation(format!(
                "checkout operation {} reached `{}`; captured funds must be reconciled through refund policy",
                operation.id, operation.stage
            )));
        }

        self.compensate_payment(tenant_id, actor_id, operation)
            .await?;
        self.compensate_order(tenant_id, actor_id, operation)
            .await?;

        // Order cancellation releases adopted reservation rows through the
        // checkout lifecycle trigger. Any still-reserved rows are pre-adoption
        // identities and must be released directly.
        self.release_remaining_reservations(tenant_id, operation)
            .await?;
        self.release_cart(tenant_id, operation).await?;
        Ok(())
    }

    async fn compensate_payment(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        let payment_context = payment_context(tenant_id, actor_id, operation, self.port_deadline);
        let snapshot = self
            .payment_compensation_port
            .compensate_checkout_payment(
                payment_context.clone(),
                CheckoutPaymentCompensationRequest {
                    checkout_operation_id: operation.id,
                    cart_id: operation.cart_id,
                    collection_id: operation.payment_collection_id,
                    reason: Some("checkout_compensation".to_string()),
                    metadata: json!({
                        "checkout": {
                            "operation_id": operation.id,
                            "compensation": true,
                        }
                    }),
                },
            )
            .await
            .map_err(|error| {
                owner_boundary_error(
                    &payment_context,
                    PAYMENT_COMPENSATION_OWNER,
                    PAYMENT_COMPENSATION_OPERATION,
                    "compensate_payment",
                    error,
                )
            })?;
        if let Some(snapshot) = snapshot {
            // The binding is checked only when the journal recorded one: a park
            // between the collection being created and `checkpoint` leaves the
            // column null, and the payment owner resolves that attempt's
            // collection from its own metadata link, so there is no binding here
            // to compare against. The status check is the same in both cases —
            // the compensation is done only once the collection is cancelled.
            if operation.payment_collection_id.is_some()
                && operation.payment_collection_id != Some(snapshot.collection_id)
            {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "payment compensation result does not match checkout operation {}",
                    operation.id
                )));
            }
            if snapshot.status_kind() != PaymentCollectionStatusKind::Cancelled {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "payment collection {} is not cancelled after checkout compensation",
                    snapshot.collection_id
                )));
            }
        } else if operation.payment_collection_id.is_some() {
            return Err(CheckoutCompensationError::ManualReconciliation(format!(
                "checkout operation {} records a payment collection but payment owner returned no compensation result",
                operation.id
            )));
        }
        Ok(())
    }

    async fn compensate_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        let order_context = order_context(tenant_id, actor_id, operation, self.port_deadline);
        let snapshot = self
            .order_compensation_port
            .compensate_checkout_order(
                order_context.clone(),
                CheckoutOrderCompensationRequest {
                    checkout_operation_id: operation.id,
                    cart_id: operation.cart_id,
                    expected_order_id: operation.order_id,
                    reason: Some("checkout_compensation".to_string()),
                },
            )
            .await
            .map_err(|error| {
                owner_boundary_error(
                    &order_context,
                    ORDER_COMPENSATION_OWNER,
                    ORDER_COMPENSATION_OPERATION,
                    "compensate_order",
                    error,
                )
            })?;
        if let Some(snapshot) = snapshot {
            if operation.order_id.is_some() && operation.order_id != Some(snapshot.order_id) {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "order compensation result does not match checkout operation {}",
                    operation.id
                )));
            }
            if snapshot.status_kind() != OrderStatusKind::Cancelled {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "order {} is not cancelled after checkout compensation",
                    snapshot.order_id
                )));
            }
        } else if operation.order_id.is_some() {
            return Err(CheckoutCompensationError::ManualReconciliation(format!(
                "checkout operation {} records an order but order owner returned no compensation result",
                operation.id
            )));
        }
        Ok(())
    }

    async fn release_remaining_reservations(
        &self,
        tenant_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        let reservations = self
            .reservation_journal
            .list_by_operation(tenant_id, operation.id)
            .await?;
        for reservation in reservations {
            match reservation.status.as_str() {
                status if status == CheckoutInventoryReservationStatus::Planned.as_str() => {}
                status if status == CheckoutInventoryReservationStatus::Released.as_str() => {}
                status if status == CheckoutInventoryReservationStatus::Reserved.as_str() => {
                    let inventory_context =
                        inventory_context(tenant_id, operation, &reservation, self.port_deadline);
                    let released = self
                        .reservation_port
                        .release_inventory_by_identity(
                            inventory_context.clone(),
                            InventoryIdentityReservationReleaseRequest {
                                reservation_id: reservation.reservation_id,
                                external_id: reservation.external_id.clone(),
                            },
                        )
                        .await
                        .map_err(|error| {
                            owner_boundary_error(
                                &inventory_context,
                                INVENTORY_COMPENSATION_OWNER,
                                INVENTORY_COMPENSATION_OPERATION,
                                "release_inventory",
                                error,
                            )
                        })?;
                    if released.reservation_id != reservation.reservation_id
                        || released.external_id != reservation.external_id
                        || released.variant_id != reservation.variant_id
                    {
                        return Err(CheckoutCompensationError::Conflict(format!(
                            "inventory release response does not match checkout reservation {}",
                            reservation.reservation_id
                        )));
                    }
                    self.reservation_journal
                        .mark_released(tenant_id, reservation.reservation_id)
                        .await?;
                }
                status if status == CheckoutInventoryReservationStatus::Consumed.as_str() => {
                    return Err(CheckoutCompensationError::ManualReconciliation(format!(
                        "inventory reservation {} is already consumed",
                        reservation.reservation_id
                    )));
                }
                status => {
                    return Err(CheckoutCompensationError::Conflict(format!(
                        "inventory reservation {} has unsupported status `{status}`",
                        reservation.reservation_id
                    )));
                }
            }
        }
        Ok(())
    }

    async fn release_cart(
        &self,
        tenant_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        let cart_read_context =
            cart_context(tenant_id, operation, self.port_deadline, "read", false);
        let current = self
            .cart_port
            .read_cart_checkout_snapshot(
                cart_read_context.clone(),
                CartCheckoutSnapshotRequest {
                    cart_id: operation.cart_id,
                    locale: None,
                },
            )
            .await
            .map_err(|error| {
                owner_boundary_error(
                    &cart_read_context,
                    CART_COMPENSATION_OWNER,
                    CART_SNAPSHOT_OPERATION,
                    "read_cart",
                    error,
                )
            })?;
        match cart_status(&current)? {
            CartStatus::CheckingOut => {
                let cart_release_context =
                    cart_context(tenant_id, operation, self.port_deadline, "release", true);
                let released = self
                    .cart_port
                    .release_cart_checkout(
                        cart_release_context.clone(),
                        CartCheckoutLifecycleRequest {
                            cart_id: operation.cart_id,
                        },
                    )
                    .await
                    .map_err(|error| {
                        owner_boundary_error(
                            &cart_release_context,
                            CART_COMPENSATION_OWNER,
                            CART_RELEASE_OPERATION,
                            "release_cart",
                            error,
                        )
                    })?;
                if cart_status(&released)? != CartStatus::Active {
                    return Err(CheckoutCompensationError::Conflict(format!(
                        "cart {} is not active after checkout release",
                        released.id
                    )));
                }
            }
            CartStatus::Active => {}
            CartStatus::Completed => {
                return Err(CheckoutCompensationError::ManualReconciliation(format!(
                    "cart {} is already completed",
                    current.id
                )));
            }
            CartStatus::Abandoned => {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "cart {} is abandoned and cannot be released",
                    current.id
                )));
            }
        }
        Ok(())
    }
}

fn cart_status(cart: &CartResponse) -> CheckoutCompensationResult<CartStatus> {
    cart.lifecycle_status().map_err(|_| {
        CheckoutCompensationError::ManualReconciliation(format!(
            "cart {} has an unknown lifecycle state",
            cart.id
        ))
    })
}

fn inventory_context(
    tenant_id: Uuid,
    operation: &checkout_operation::Model,
    reservation: &checkout_inventory_reservation::Model,
    deadline: Duration,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.checkout-compensation"),
        PLATFORM_FALLBACK_LOCALE,
        format!(
            "checkout:{}:compensation:inventory:{}",
            operation.id, reservation.cart_line_item_id
        ),
    )
    .with_causation_id(operation.id.to_string())
    .with_idempotency_key(reservation.external_id.clone())
    .with_deadline(deadline)
}

fn cart_context(
    tenant_id: Uuid,
    operation: &checkout_operation::Model,
    deadline: Duration,
    action: &str,
    write: bool,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.checkout-compensation"),
        PLATFORM_FALLBACK_LOCALE,
        format!("checkout:{}:compensation:cart:{action}", operation.id),
    )
    .with_causation_id(operation.id.to_string())
    .with_deadline(deadline);
    if write {
        context.with_idempotency_key(format!(
            "checkout:{}:compensation:cart:{action}",
            operation.id
        ))
    } else {
        context
    }
}

fn order_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    operation: &checkout_operation::Model,
    deadline: Duration,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        PLATFORM_FALLBACK_LOCALE,
        format!("checkout:{}:compensation:order", operation.id),
    )
    .with_causation_id(operation.id.to_string())
    .with_idempotency_key(format!("checkout:{}:compensation:order", operation.id))
    .with_deadline(deadline)
}

fn payment_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    operation: &checkout_operation::Model,
    deadline: Duration,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        PLATFORM_FALLBACK_LOCALE,
        format!("checkout:{}:compensation:payment", operation.id),
    )
    .with_causation_id(operation.id.to_string())
    .with_idempotency_key(format!("checkout:{}:compensation:payment", operation.id))
    .with_deadline(deadline)
}

fn owner_boundary_error(
    context: &PortContext,
    owner: &'static str,
    operation: &'static str,
    stage: &'static str,
    error: PortError,
) -> CheckoutCompensationError {
    log_owner_boundary_error(context, owner, operation, stage, &error);
    if matches!(
        error.code.as_str(),
        ORDER_MANUAL_RECONCILIATION_CODE | PAYMENT_MANUAL_RECONCILIATION_CODE
    ) {
        CheckoutCompensationError::ManualReconciliation(error.message)
    } else {
        boundary_error(stage, error)
    }
}

fn log_owner_boundary_error(
    context: &PortContext,
    owner: &'static str,
    operation: &'static str,
    stage: &'static str,
    error: &PortError,
) {
    match &error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation => {
            tracing::error!(
                error = ?error,
                owner = owner,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                operation = operation,
                stage = stage,
                code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = CHECKOUT_COMPENSATION_OWNER_BOUNDARY,
                "checkout compensation owner call failed"
            );
        }
        _ => {
            tracing::warn_event!(
                error = ?error,
                owner = owner,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                operation = operation,
                stage = stage,
                code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = CHECKOUT_COMPENSATION_OWNER_BOUNDARY,
                "checkout compensation owner call was rejected"
            );
        }
    }
}

fn boundary_error(stage: &'static str, error: PortError) -> CheckoutCompensationError {
    CheckoutCompensationError::Boundary {
        stage,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

fn compensation_error_code(error: &CheckoutCompensationError) -> &'static str {
    match error {
        CheckoutCompensationError::ManualReconciliation(_) => {
            "checkout.compensation_manual_reconciliation"
        }
        CheckoutCompensationError::Boundary { .. } => "checkout.compensation_boundary_failed",
        CheckoutCompensationError::ReservationJournal(_) => {
            "checkout.compensation_inventory_failed"
        }
        CheckoutCompensationError::Operation(_)
        | CheckoutCompensationError::Conflict(_)
        | CheckoutCompensationError::CompensationAndJournal { .. } => {
            "checkout.compensation_failed"
        }
    }
}

fn stage_rank(stage: &str) -> CheckoutCompensationResult<u8> {
    match stage {
        "created" => Ok(0),
        "cart_locked" => Ok(1),
        "inventory_reserved" => Ok(2),
        "order_created" => Ok(3),
        "payment_ready" => Ok(4),
        "payment_authorized" => Ok(5),
        "payment_captured" => Ok(6),
        "fulfillment_created" => Ok(7),
        "cart_completed" => Ok(8),
        "completed" => Ok(9),
        other => Err(CheckoutCompensationError::Conflict(format!(
            "unsupported checkout stage `{other}`"
        ))),
    }
}

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
        cart_port: Arc<dyn CanonicalCartCheckoutPort>,
    ) -> Self {
        Self {
            inner: legacy::CheckoutCompensationService::new(
                db,
                event_bus,
                rustok_inventory_shim::wrap_inventory_reservation_identity_port(reservation_port),
                rustok_cart_shim::wrap_cart_checkout_port(cart_port),
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
        let lease_owner = lease_owner.into();
        tracing::debug!(
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            operation_id = %operation_id,
            lease_owner = %lease_owner,
            "ErrorSafeCheckoutCompensationService::compensate entry"
        );
        self.inner
            .compensate(tenant_id, actor_id, operation_id, lease_owner)
            .await
    }
}
