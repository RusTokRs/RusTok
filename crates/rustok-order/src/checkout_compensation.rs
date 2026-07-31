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
        current_state = ?current_state,
        from,
        to,
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
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        log_context_parse_rejection(
            context,
            operation,
            "tenant_id",
            context.tenant_id.chars().count(),
            "order.tenant_id_invalid",
            &error,
        );
        PortError::validation(
            "order.tenant_id_invalid",
            "order request context is invalid",
        )
    })
}

fn parse_actor_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.actor.id).map_err(|error| {
        log_context_parse_rejection(
            context,
            operation,
            "actor_id",
            context.actor.id.chars().count(),
            "order.actor_id_invalid",
            &error,
        );
        PortError::validation("order.actor_id_invalid", "order request context is invalid")
    })
}

fn log_context_parse_rejection<E: std::fmt::Debug>(
    context: &PortContext,
    operation: &'static str,
    field: &'static str,
    value_length: usize,
    code: &'static str,
    error: &E,
) {
    let context_facts = order_compensation_context_facts(context);
    tracing::warn!(
        error = ?error,
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
        order_state = ?order_state,
        internal_reason = reason,
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
    resource_id: Option<Uuid>,
    internal_cause: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
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
        resource_id_present = resource_id.is_some(),
        resource_id_non_nil = ?resource_id.map(|value| !value.is_nil()),
        internal_cause = ?internal_cause,
        from = ?from,
        to = ?to,
        code,
        boundary = ORDER_COMPENSATION_BOUNDARY,
        "order checkout compensation owner outcome retained safe context"
    );
}

fn log_order_owner_error<E: std::fmt::Debug>(
    context: &PortContext,
    operation: &'static str,
    local_operation: &'static str,
    code: &'static str,
    error: &E,
) {
    let context_facts = order_compensation_context_facts(context);
    tracing::error!(
        error = ?error,
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
        code,
        boundary = ORDER_COMPENSATION_BOUNDARY,
        "order checkout compensation owner technical outcome retained safe context"
    );
}

fn order_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: OrderError,
) -> PortError {
    match error {
        OrderError::Database(error) => {
            log_order_owner_error(
                context,
                operation,
                "owner_storage",
                "order.database_unavailable",
                &error,
            );
            PortError::unavailable(
                "order.database_unavailable",
                "order storage is temporarily unavailable",
            )
        }
        OrderError::OrderNotFound(order_id) => {
            log_order_owner_warning(
                context,
                operation,
                "load_order",
                "order.order_not_found",
                Some("order"),
                Some(order_id),
                None,
                None,
                None,
            );
            PortError::not_found("order.order_not_found", "order was not found")
        }
        OrderError::Validation(cause) => {
            log_order_owner_warning(
                context,
                operation,
                "validate_owner_request",
                "order.checkout_compensation_validation",
                None,
                None,
                Some(cause.as_str()),
                None,
                None,
            );
            PortError::validation(
                "order.checkout_compensation_validation",
                "checkout order compensation request is invalid",
            )
        }
        OrderError::InvalidTransition { from, to } => {
            log_order_owner_warning(
                context,
                operation,
                "apply_compensation_state",
                "order.checkout_compensation_state_conflict",
                None,
                None,
                None,
                Some(from.as_str()),
                Some(to.as_str()),
            );
            PortError::conflict(
                "order.checkout_compensation_state_conflict",
                "checkout order lifecycle conflicts with compensation",
            )
        }
        OrderError::OrderReturnNotFound(return_id) => {
            log_order_owner_warning(
                context,
                operation,
                "load_related_order_resource",
                "order.related_resource_not_found",
                Some("order_return"),
                Some(return_id),
                None,
                None,
                None,
            );
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::OrderChangeNotFound(change_id) => {
            log_order_owner_warning(
                context,
                operation,
                "load_related_order_resource",
                "order.related_resource_not_found",
                Some("order_change"),
                Some(change_id),
                None,
                None,
                None,
            );
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::Core(error) => {
            log_order_owner_error(
                context,
                operation,
                "owner_invariant",
                "order.invariant_violation",
                &error,
            );
            PortError::invariant_violation(
                "order.invariant_violation",
                "order compensation failed an internal invariant",
            )
        }
    }
}
