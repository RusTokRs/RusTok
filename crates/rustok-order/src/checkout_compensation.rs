use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AdoptLegacyCheckoutOrderIdentityRequest, CheckoutOrderIdentityPort,
    CheckoutOrderIdentitySnapshot, InProcessCheckoutOrderIdentityPort, OrderError, OrderResponse,
    OrderService, OrderStatusKind, ReadCheckoutOrderIdentityByOperationRequest,
};

const ORDER_COMPENSATION_OWNER: &str = "rustok_order.checkout_compensation";
const ORDER_COMPENSATION_BOUNDARY: &str = "checkout_order_compensation_port";
const COMPENSATE_OPERATION: &str = "compensate_checkout_order";

struct OrderCompensationContextFacts {
    tenant_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
    channel_present: bool,
    channel_length: Option<usize>,
    locale_length: usize,
    causation_id_present: bool,
    causation_id_length: Option<usize>,
    traceparent_present: bool,
    traceparent_length: Option<usize>,
    idempotency_key_present: bool,
    idempotency_key_length: Option<usize>,
    deadline_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
struct OrderCompensationOrderErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

#[async_trait]
pub trait CheckoutOrderCompensationPort: Send + Sync {
    async fn compensate_checkout_order(
        &self,
        context: PortContext,
        request: CheckoutOrderCompensationRequest,
    ) -> Result<Option<CheckoutOrderCompensationSnapshot>, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckoutOrderCompensationRequest {
    pub checkout_operation_id: Uuid,
    pub cart_id: Uuid,
    pub expected_order_id: Option<Uuid>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckoutOrderCompensationSnapshot {
    pub order_id: Uuid,
    pub status: String,
}

impl CheckoutOrderCompensationSnapshot {
    pub fn status_kind(&self) -> OrderStatusKind {
        OrderStatusKind::from_raw(self.status.as_str())
    }
}

pub struct InProcessCheckoutOrderCompensationPort {
    order_service: OrderService,
    identity_port: Arc<dyn CheckoutOrderIdentityPort>,
}

impl InProcessCheckoutOrderCompensationPort {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self::with_identity_port(
            db.clone(),
            event_bus,
            Arc::new(InProcessCheckoutOrderIdentityPort::new(db)),
        )
    }

    pub fn with_identity_port(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        identity_port: Arc<dyn CheckoutOrderIdentityPort>,
    ) -> Self {
        Self {
            order_service: OrderService::new(db, event_bus),
            identity_port,
        }
    }

    async fn resolve_identity(
        &self,
        context: &PortContext,
        request: &CheckoutOrderCompensationRequest,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError> {
        let mut identity = self
            .identity_port
            .read_by_operation(
                context.clone(),
                ReadCheckoutOrderIdentityByOperationRequest {
                    checkout_operation_id: request.checkout_operation_id,
                },
            )
            .await?;
        if identity.is_none() {
            identity = self
                .identity_port
                .adopt_legacy(
                    context.clone(),
                    AdoptLegacyCheckoutOrderIdentityRequest {
                        checkout_operation_id: request.checkout_operation_id,
                        cart_id: request.cart_id,
                    },
                )
                .await?;
        }
        Ok(identity)
    }

    async fn cancel_or_adopt_cancelled(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        actor_id: Uuid,
        order: OrderResponse,
        reason: Option<String>,
    ) -> Result<OrderResponse, PortError> {
        match order.status_kind() {
            OrderStatusKind::Pending | OrderStatusKind::Confirmed => match self
                .order_service
                .cancel_order(tenant_id, actor_id, order.id, reason)
                .await
            {
                Ok(cancelled) => Ok(cancelled),
                Err(OrderError::InvalidTransition { from, to }) => {
                    let current = self
                        .order_service
                        .get_order(tenant_id, order.id)
                        .await
                        .map_err(|error| {
                            order_error_to_port_error(
                                context,
                                "read_order_after_compensation_transition",
                                error,
                            )
                        })?;
                    if current.status_kind() == OrderStatusKind::Cancelled {
                        Ok(current)
                    } else {
                        log_compensation_transition_conflict(
                            context,
                            "cancel_checkout_order",
                            current.id,
                            current.status_kind(),
                            from.as_str(),
                            to.as_str(),
                        );
                        Err(PortError::conflict(
                            "order.checkout_compensation_state_conflict",
                            "checkout order changed while compensation was being applied",
                        ))
                    }
                }
                Err(error) => Err(order_error_to_port_error(
                    context,
                    "cancel_checkout_order",
                    error,
                )),
            },
            OrderStatusKind::Cancelled => Ok(order),
            state @ (OrderStatusKind::Paid
            | OrderStatusKind::Shipped
            | OrderStatusKind::Delivered) => Err(manual_reconciliation(
                context,
                "cancel_checkout_order",
                Some(order.id),
                state,
                "checkout order has financial or fulfillment effects and cannot be cancelled automatically",
            )),
            OrderStatusKind::Unknown => Err(manual_reconciliation(
                context,
                "cancel_checkout_order",
                Some(order.id),
                OrderStatusKind::Unknown,
                "checkout order lifecycle is unknown",
            )),
        }
    }
}

pub fn in_process_checkout_order_compensation_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CheckoutOrderCompensationPort> {
    Arc::new(InProcessCheckoutOrderCompensationPort::new(db, event_bus))
}

#[async_trait]
impl CheckoutOrderCompensationPort for InProcessCheckoutOrderCompensationPort {
    async fn compensate_checkout_order(
        &self,
        context: PortContext,
        request: CheckoutOrderCompensationRequest,
    ) -> Result<Option<CheckoutOrderCompensationSnapshot>, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context, COMPENSATE_OPERATION)?;
        let actor_id = parse_actor_id(&context, COMPENSATE_OPERATION)?;
        require_operation_context(
            &context,
            COMPENSATE_OPERATION,
            request.checkout_operation_id,
        )?;
        if request.checkout_operation_id.is_nil() || request.cart_id.is_nil() {
            log_invalid_compensation_request(&context, &request);
            return Err(PortError::validation(
                "order.checkout_compensation_identity_invalid",
                "checkout compensation request is invalid",
            ));
        }

        let Some(identity) = self.resolve_identity(&context, &request).await? else {
            return if request.expected_order_id.is_none() {
                Ok(None)
            } else {
                Err(manual_reconciliation(
                    &context,
                    COMPENSATE_OPERATION,
                    request.expected_order_id,
                    OrderStatusKind::Unknown,
                    "checkout operation records an order but the order owner has no durable checkout identity",
                ))
            };
        };
        validate_identity(&context, tenant_id, &request, &identity)?;

        let order = self
            .order_service
            .get_order(tenant_id, identity.order_id)
            .await
            .map_err(|error| {
                order_error_to_port_error(&context, "read_checkout_order_for_compensation", error)
            })?;
        let order = self
            .cancel_or_adopt_cancelled(&context, tenant_id, actor_id, order, request.reason)
            .await?;
        Ok(Some(CheckoutOrderCompensationSnapshot {
            order_id: order.id,
            status: order.status,
        }))
    }
}

fn order_compensation_context_facts(context: &PortContext) -> OrderCompensationContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    OrderCompensationContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
        channel_present: context.channel.is_some(),
        channel_length: context.channel.as_ref().map(|value| value.chars().count()),
        locale_length: context.locale.chars().count(),
        causation_id_present: context.causation_id.is_some(),
        causation_id_length: context
            .causation_id
            .as_ref()
            .map(|value| value.chars().count()),
        traceparent_present: context.traceparent.is_some(),
        traceparent_length: context
            .traceparent
            .as_ref()
            .map(|value| value.chars().count()),
        idempotency_key_present: context.idempotency_key.is_some(),
        idempotency_key_length: context
            .idempotency_key
            .as_ref()
            .map(|value| value.chars().count()),
        deadline_ms: context.deadline_ms,
    }
}

fn order_status_kind_label(status: OrderStatusKind) -> &'static str {
    match status {
        OrderStatusKind::Pending => "pending",
        OrderStatusKind::Confirmed => "confirmed",
        OrderStatusKind::Paid => "paid",
        OrderStatusKind::Shipped => "shipped",
        OrderStatusKind::Delivered => "delivered",
        OrderStatusKind::Cancelled => "cancelled",
        OrderStatusKind::Unknown => "unknown",
    }
}

fn order_compensation_order_error_facts(error: &OrderError) -> OrderCompensationOrderErrorFacts {
    let (
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    ) = match error {
        OrderError::Database(_) => ("database", 0, 0, 0, 0, true),
        OrderError::OrderNotFound(id) => (
            "order_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        OrderError::Validation(cause) => ("validation", 1, cause.chars().count(), 0, 0, false),
        OrderError::InvalidTransition { from, to } => (
            "invalid_transition",
            2,
            from.as_str().chars().count() + to.as_str().chars().count(),
            0,
            0,
            false,
        ),
        OrderError::OrderReturnNotFound(id) => (
            "order_return_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        OrderError::OrderChangeNotFound(id) => (
            "order_change_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        OrderError::Core(_) => ("core", 0, 0, 0, 0, true),
    };
    OrderCompensationOrderErrorFacts {
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    }
}

fn log_invalid_compensation_request(
    context: &PortContext,
    request: &CheckoutOrderCompensationRequest,
) {
    let context_facts = order_compensation_context_facts(context);
    tracing::warn!(
        owner = ORDER_COMPENSATION_OWNER,
        operation = COMPENSATE_OPERATION,
        local_operation = "validate_request",
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        checkout_operation_id_non_nil = !request.checkout_operation_id.is_nil(),
        cart_id_non_nil = !request.cart_id.is_nil(),
        expected_order_id_present = request.expected_order_id.is_some(),
        expected_order_id_non_nil = ?request.expected_order_id.map(|value| !value.is_nil()),
        reason_present = request.reason.is_some(),
        reason_length = ?request.reason.as_ref().map(|value| value.chars().count()),
        code = "order.checkout_compensation_identity_invalid",
        boundary = ORDER_COMPENSATION_BOUNDARY,
        "checkout compensation rejected invalid owner identity safely"
    );
}

fn log_compensation_transition_conflict(
    context: &PortContext,
    operation: &'static str,
    order_id: Uuid,
    current_state: OrderStatusKind,
    from: &str,
    to: &str,
) {
    let context_facts = order_compensation_context_facts(context);
    let current_state = order_status_kind_label(current_state);
    let transition_from_present = !from.trim().is_empty();
    let transition_from_length = from.chars().count();
    let transition_to_present = !to.trim().is_empty();
    let transition_to_length = to.chars().count();
    tracing::warn!(
        owner = ORDER_COMPENSATION_OWNER,
        operation,
        local_operation = "apply_compensation_state",
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        order_id_non_nil = !order_id.is_nil(),
        current_state,
        transition_from_present,
        transition_from_length,
        transition_to_present,
        transition_to_length,
        code = "order.checkout_compensation_state_conflict",
        boundary = ORDER_COMPENSATION_BOUNDARY,
        "order lifecycle changed while checkout compensation was being applied"
    );
}

fn validate_identity(
    context: &PortContext,
    tenant_id: Uuid,
    request: &CheckoutOrderCompensationRequest,
    identity: &CheckoutOrderIdentitySnapshot,
) -> Result<(), PortError> {
    let tenant_matches = identity.tenant_id == tenant_id;
    let checkout_operation_matches =
        identity.checkout_operation_id == request.checkout_operation_id;
    let source_cart_matches = identity
        .source_cart_id
        .is_none_or(|cart_id| cart_id == request.cart_id);
    let expected_order_matches = request
        .expected_order_id
        .is_none_or(|order_id| order_id == identity.order_id);
    let valid = tenant_matches
        && checkout_operation_matches
        && source_cart_matches
        && expected_order_matches;
    if !valid {
        let context_facts = order_compensation_context_facts(context);
        tracing::error!(
            owner = ORDER_COMPENSATION_OWNER,
            operation = COMPENSATE_OPERATION,
            local_operation = "validate_durable_checkout_identity",
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            tenant_matches,
            checkout_operation_matches,
            source_cart_matches,
            expected_order_matches,
            request_checkout_operation_id_non_nil = !request.checkout_operation_id.is_nil(),
            request_cart_id_non_nil = !request.cart_id.is_nil(),
            request_expected_order_id_present = request.expected_order_id.is_some(),
            request_expected_order_id_non_nil = ?request.expected_order_id.map(|value| !value.is_nil()),
            identity_checkout_operation_id_non_nil = !identity.checkout_operation_id.is_nil(),
            identity_order_id_non_nil = !identity.order_id.is_nil(),
            identity_source_cart_id_present = identity.source_cart_id.is_some(),
            identity_source_cart_id_non_nil = ?identity.source_cart_id.map(|value| !value.is_nil()),
            code = "order.checkout_compensation_identity_conflict",
            boundary = ORDER_COMPENSATION_BOUNDARY,
            "checkout order identity conflicts with compensation"
        );
        return Err(PortError::conflict(
            "order.checkout_compensation_identity_conflict",
            "checkout order identity conflicts with the compensation request",
        ));
    }
    Ok(())
}

fn require_operation_context(
    context: &PortContext,
    operation: &'static str,
    checkout_operation_id: Uuid,
) -> Result<(), PortError> {
    let context_operation = context
        .causation_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    if context_operation != Some(checkout_operation_id) {
        let context_facts = order_compensation_context_facts(context);
        tracing::warn!(
            owner = ORDER_COMPENSATION_OWNER,
            operation,
            local_operation = "validate_causation_context",
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            checkout_operation_id_non_nil = !checkout_operation_id.is_nil(),
            causation_matches = false,
            code = "order.checkout_compensation_causation_invalid",
            boundary = ORDER_COMPENSATION_BOUNDARY,
            "checkout compensation received invalid causation identity"
        );
        return Err(PortError::validation(
            "order.checkout_compensation_causation_invalid",
            "checkout operation context is invalid",
        ));
    }
    Ok(())
}

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        log_context_parse_rejection(
            context,
            operation,
            "tenant_id",
            context.tenant_id.chars().count(),
            "order.tenant_id_invalid",
        );
        PortError::validation(
            "order.tenant_id_invalid",
            "order request context is invalid",
        )
    })
}

fn parse_actor_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.actor.id).map_err(|_| {
        log_context_parse_rejection(
            context,
            operation,
            "actor_id",
            context.actor.id.chars().count(),
            "order.actor_id_invalid",
        );
        PortError::validation("order.actor_id_invalid", "order request context is invalid")
    })
}

fn log_context_parse_rejection(
    context: &PortContext,
    operation: &'static str,
    field: &'static str,
    value_length: usize,
    code: &'static str,
) {
    let context_facts = order_compensation_context_facts(context);
    tracing::warn!(
        parse_failed = true,
        owner = ORDER_COMPENSATION_OWNER,
        operation,
        local_operation = "validate_owner_context",
        field,
        value_length,
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        code,
        boundary = ORDER_COMPENSATION_BOUNDARY,
        "order port received invalid request context"
    );
}

fn manual_reconciliation(
    context: &PortContext,
    operation: &'static str,
    order_id: Option<Uuid>,
    order_state: OrderStatusKind,
    reason: &'static str,
) -> PortError {
    let context_facts = order_compensation_context_facts(context);
    let order_state = order_status_kind_label(order_state);
    let reconciliation_reason_present = !reason.trim().is_empty();
    let reconciliation_reason_length = reason.chars().count();
    tracing::error!(
        owner = ORDER_COMPENSATION_OWNER,
        operation,
        local_operation = "require_manual_reconciliation",
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        order_id_present = order_id.is_some(),
        order_id_non_nil = ?order_id.map(|value| !value.is_nil()),
        order_state,
        reconciliation_reason_present,
        reconciliation_reason_length,
        code = "order.checkout_compensation_manual_reconciliation",
        boundary = ORDER_COMPENSATION_BOUNDARY,
        "checkout order compensation requires manual reconciliation"
    );
    PortError::conflict(
        "order.checkout_compensation_manual_reconciliation",
        "checkout requires manual reconciliation",
    )
}

fn log_order_owner_warning(
    context: &PortContext,
    operation: &'static str,
    local_operation: &'static str,
    code: &'static str,
    resource: Option<&'static str>,
    error_facts: OrderCompensationOrderErrorFacts,
) {
    let context_facts = order_compensation_context_facts(context);
    tracing::warn!(
        owner = ORDER_COMPENSATION_OWNER,
        operation,
        local_operation,
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        resource = ?resource,
        order_error_variant = error_facts.error_variant,
        order_error_text_field_count = error_facts.text_field_count,
        order_error_text_total_length = error_facts.text_total_length,
        order_error_uuid_field_count = error_facts.uuid_field_count,
        order_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
        order_error_opaque_payload_present = error_facts.opaque_payload_present,
        code,
        boundary = ORDER_COMPENSATION_BOUNDARY,
        "order checkout compensation owner outcome retained bounded diagnostics"
    );
}

fn log_order_owner_error(
    context: &PortContext,
    operation: &'static str,
    local_operation: &'static str,
    code: &'static str,
    error_facts: OrderCompensationOrderErrorFacts,
) {
    let context_facts = order_compensation_context_facts(context);
    tracing::error!(
        owner = ORDER_COMPENSATION_OWNER,
        operation,
        local_operation,
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        order_error_variant = error_facts.error_variant,
        order_error_text_field_count = error_facts.text_field_count,
        order_error_text_total_length = error_facts.text_total_length,
        order_error_uuid_field_count = error_facts.uuid_field_count,
        order_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
        order_error_opaque_payload_present = error_facts.opaque_payload_present,
        code,
        boundary = ORDER_COMPENSATION_BOUNDARY,
        "order checkout compensation owner technical outcome retained bounded diagnostics"
    );
}

fn order_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: OrderError,
) -> PortError {
    let error_facts = order_compensation_order_error_facts(&error);
    match error {
        OrderError::Database(_) => {
            log_order_owner_error(
                context,
                operation,
                "owner_storage",
                "order.database_unavailable",
                error_facts,
            );
            PortError::unavailable(
                "order.database_unavailable",
                "order storage is temporarily unavailable",
            )
        }
        OrderError::OrderNotFound(_) => {
            log_order_owner_warning(
                context,
                operation,
                "load_order",
                "order.order_not_found",
                Some("order"),
                error_facts,
            );
            PortError::not_found("order.order_not_found", "order was not found")
        }
        OrderError::Validation(_) => {
            log_order_owner_warning(
                context,
                operation,
                "validate_owner_request",
                "order.checkout_compensation_validation",
                None,
                error_facts,
            );
            PortError::validation(
                "order.checkout_compensation_validation",
                "checkout order compensation request is invalid",
            )
        }
        OrderError::InvalidTransition { .. } => {
            log_order_owner_warning(
                context,
                operation,
                "apply_compensation_state",
                "order.checkout_compensation_state_conflict",
                None,
                error_facts,
            );
            PortError::conflict(
                "order.checkout_compensation_state_conflict",
                "checkout order lifecycle conflicts with compensation",
            )
        }
        OrderError::OrderReturnNotFound(_) => {
            log_order_owner_warning(
                context,
                operation,
                "load_related_order_resource",
                "order.related_resource_not_found",
                Some("order_return"),
                error_facts,
            );
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::OrderChangeNotFound(_) => {
            log_order_owner_warning(
                context,
                operation,
                "load_related_order_resource",
                "order.related_resource_not_found",
                Some("order_change"),
                error_facts,
            );
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::Core(_) => {
            log_order_owner_error(
                context,
                operation,
                "owner_invariant",
                "order.invariant_violation",
                error_facts,
            );
            PortError::invariant_violation(
                "order.invariant_violation",
                "order compensation failed an internal invariant",
            )
        }
    }
}
