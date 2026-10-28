//! Bounded projection of `rustok_order` failures at the Commerce checkout boundary.
//!
//! Staged checkout reaches the Order owner while it reads, creates, and confirms
//! the order that backs a checkout operation. The owner payload never crosses the
//! boundary: Commerce keeps a stable code, a static consumer message, and the
//! retryability decision, and the owner text survives only as shape and length
//! facts on a redacted tracing event.
//!
//! The projection is deliberately total: every `OrderError` and every
//! order-owned `PortError` maps onto a known kind, so a new owner variant cannot
//! silently start leaking text through a catch-all `Display` conversion.

use rustok_api::{PortActorKind, PortContext, PortError, PortErrorKind};
use rustok_order::OrderError;
use uuid::Uuid;

pub(crate) const CHECKOUT_ORDER_OWNER: &str = "rustok_order";
pub(crate) const CHECKOUT_ORDER_OWNER_BOUNDARY: &str = "commerce_checkout_order_owner_boundary";

/// Redacted tracing token: `Debug` never renders the owner payload.
pub(crate) struct CheckoutOrderDiagnosticError;

impl std::fmt::Debug for CheckoutOrderDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckoutOrderFailureKind {
    Validation,
    NotFound,
    Conflict,
    Forbidden,
    Unavailable,
    Timeout,
    Invariant,
    CorruptReceipt,
    Core,
}

impl CheckoutOrderFailureKind {
    fn of_owner_error(error: &OrderError) -> Self {
        match error {
            OrderError::Validation(_) => Self::Validation,
            OrderError::OrderNotFound(_)
            | OrderError::OrderReturnNotFound(_)
            | OrderError::OrderChangeNotFound(_) => Self::NotFound,
            OrderError::InvalidTransition { .. } | OrderError::IdempotencyConflict => {
                Self::Conflict
            }
            OrderError::CommandReceiptCorrupt => Self::CorruptReceipt,
            OrderError::Database(_) => Self::Unavailable,
            OrderError::Core(_) => Self::Core,
        }
    }

    fn of_port_error(kind: &PortErrorKind) -> Self {
        match kind {
            PortErrorKind::Validation => Self::Validation,
            PortErrorKind::NotFound => Self::NotFound,
            PortErrorKind::Conflict => Self::Conflict,
            PortErrorKind::Forbidden => Self::Forbidden,
            PortErrorKind::Unavailable => Self::Unavailable,
            PortErrorKind::Timeout => Self::Timeout,
            PortErrorKind::InvariantViolation => Self::Invariant,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Forbidden => "forbidden",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::Invariant => "invariant_violation",
            Self::CorruptReceipt => "corrupt_receipt",
            Self::Core => "core",
        }
    }

    pub(crate) fn public_message(self) -> &'static str {
        match self {
            Self::Validation => "Checkout order request is invalid",
            Self::NotFound => "Checkout order resource was not found",
            Self::Conflict => "Checkout order state conflicts with the requested operation",
            Self::Forbidden => "Checkout order operation is not permitted",
            Self::Unavailable | Self::Timeout => {
                "Checkout order service is temporarily unavailable"
            }
            Self::Invariant | Self::CorruptReceipt | Self::Core => {
                "Checkout order operation could not be completed safely"
            }
        }
    }

    /// Stable Commerce code for an owner failure that carries no code of its own.
    pub(crate) fn order_code(self) -> &'static str {
        match self {
            Self::Validation => "checkout.order_request_invalid",
            Self::NotFound => "checkout.order_resource_not_found",
            Self::Conflict => "checkout.order_state_conflict",
            Self::Forbidden => "checkout.order_not_permitted",
            Self::Unavailable => "checkout.order_temporarily_unavailable",
            Self::Timeout => "checkout.order_timeout",
            Self::Invariant => "checkout.order_invariant_violation",
            Self::CorruptReceipt => "checkout.order_receipt_corrupt",
            Self::Core => "checkout.order_operation_failed",
        }
    }

    fn default_retryable(self) -> bool {
        matches!(self, Self::Unavailable | Self::Timeout)
    }

    /// Technical failures are logged at error severity, ordinary rejections at warn.
    fn is_technical(self) -> bool {
        matches!(
            self,
            Self::Unavailable | Self::Timeout | Self::Invariant | Self::CorruptReceipt | Self::Core
        )
    }
}

/// Owner failure projected onto the bounded Commerce vocabulary.
pub(crate) struct CheckoutOrderBoundaryFailure {
    pub(crate) kind: CheckoutOrderFailureKind,
    pub(crate) code: String,
    pub(crate) message: &'static str,
    pub(crate) retryable: bool,
    owner_message_shape: &'static str,
    owner_message_len: usize,
}

impl CheckoutOrderBoundaryFailure {
    /// Projects a typed owner error. The owner `Display` is measured, never stored.
    pub(crate) fn from_owner_error(error: &OrderError) -> Self {
        let kind = CheckoutOrderFailureKind::of_owner_error(error);
        let rendered = error.to_string();
        Self {
            kind,
            code: kind.order_code().to_string(),
            message: kind.public_message(),
            retryable: kind.default_retryable(),
            owner_message_shape: text_shape(rendered.as_str()),
            owner_message_len: rendered.chars().count(),
        }
    }

    /// Projects an order-owned port rejection. The owner code and retryability are
    /// preserved; the owner message is replaced by the static consumer message.
    pub(crate) fn from_port_error(error: PortError) -> Self {
        let kind = CheckoutOrderFailureKind::of_port_error(&error.kind);
        let message_shape = text_shape(error.message.as_str());
        let message_len = error.message.chars().count();
        Self {
            kind,
            code: error.code,
            message: kind.public_message(),
            retryable: error.retryable,
            owner_message_shape: message_shape,
            owner_message_len: message_len,
        }
    }

    pub(crate) fn is_technical(&self) -> bool {
        self.kind.is_technical()
    }

    /// Emits bounded facts for an in-process owner call.
    pub(crate) fn observe_owner_error(
        &self,
        owner_operation: &'static str,
        tenant_id: Uuid,
        operation_id: Uuid,
        order_id: Option<Uuid>,
    ) {
        let diagnostic = CheckoutOrderDiagnosticError;
        let tenant_id_shape = uuid_shape(tenant_id);
        let operation_id_shape = uuid_shape(operation_id);
        let order_id_shape = match order_id {
            Some(order_id) => uuid_shape(order_id),
            None => "absent",
        };
        if self.is_technical() {
            tracing::error!(
                error = ?diagnostic,
                owner = CHECKOUT_ORDER_OWNER,
                owner_operation,
                tenant_id_shape,
                operation_id_shape,
                order_id_shape,
                owner_kind = self.kind.label(),
                owner_code = %self.code,
                owner_message_shape = self.owner_message_shape,
                owner_message_len = self.owner_message_len,
                owner_retryable = self.retryable,
                boundary = CHECKOUT_ORDER_OWNER_BOUNDARY,
                "commerce checkout order owner call failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic,
                owner = CHECKOUT_ORDER_OWNER,
                owner_operation,
                tenant_id_shape,
                operation_id_shape,
                order_id_shape,
                owner_kind = self.kind.label(),
                owner_code = %self.code,
                owner_message_shape = self.owner_message_shape,
                owner_message_len = self.owner_message_len,
                owner_retryable = self.retryable,
                boundary = CHECKOUT_ORDER_OWNER_BOUNDARY,
                "commerce checkout order owner call was rejected"
            );
        }
    }

    /// Emits bounded facts for an order-owned port rejection.
    pub(crate) fn observe_port_error(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        stage: &'static str,
    ) {
        let diagnostic = CheckoutOrderDiagnosticError;
        let facts = CheckoutOrderContextFacts::from(context);
        if self.is_technical() {
            tracing::error!(
                error = ?diagnostic,
                owner = CHECKOUT_ORDER_OWNER,
                owner_operation,
                stage,
                tenant_id_shape = facts.tenant_id_shape,
                actor_kind = facts.actor_kind,
                actor_id_shape = facts.actor_id_shape,
                claim_count = facts.claim_count,
                role_count = facts.role_count,
                channel_shape = facts.channel_shape,
                locale_shape = facts.locale_shape,
                correlation_id_shape = facts.correlation_id_shape,
                causation_id_shape = facts.causation_id_shape,
                traceparent_shape = facts.traceparent_shape,
                idempotency_key_shape = facts.idempotency_key_shape,
                deadline_ms = ?facts.deadline_ms,
                owner_kind = self.kind.label(),
                owner_code = %self.code,
                owner_message_shape = self.owner_message_shape,
                owner_message_len = self.owner_message_len,
                owner_retryable = self.retryable,
                boundary = CHECKOUT_ORDER_OWNER_BOUNDARY,
                "commerce checkout order port call failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic,
                owner = CHECKOUT_ORDER_OWNER,
                owner_operation,
                stage,
                tenant_id_shape = facts.tenant_id_shape,
                actor_kind = facts.actor_kind,
                actor_id_shape = facts.actor_id_shape,
                claim_count = facts.claim_count,
                role_count = facts.role_count,
                channel_shape = facts.channel_shape,
                locale_shape = facts.locale_shape,
                correlation_id_shape = facts.correlation_id_shape,
                causation_id_shape = facts.causation_id_shape,
                traceparent_shape = facts.traceparent_shape,
                idempotency_key_shape = facts.idempotency_key_shape,
                deadline_ms = ?facts.deadline_ms,
                owner_kind = self.kind.label(),
                owner_code = %self.code,
                owner_message_shape = self.owner_message_shape,
                owner_message_len = self.owner_message_len,
                owner_retryable = self.retryable,
                boundary = CHECKOUT_ORDER_OWNER_BOUNDARY,
                "commerce checkout order port call was rejected"
            );
        }
    }
}

#[derive(Clone, Copy)]
struct CheckoutOrderContextFacts {
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

impl From<&PortContext> for CheckoutOrderContextFacts {
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

fn uuid_shape(value: Uuid) -> &'static str {
    if value.is_nil() {
        "uuid_nil"
    } else {
        "uuid_non_nil"
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
    if value.is_empty() {
        "empty"
    } else {
        "present"
    }
}

fn optional_text_shape(value: Option<&str>) -> &'static str {
    match value {
        None => "absent",
        Some("") => "empty",
        Some(_) => "present",
    }
}
