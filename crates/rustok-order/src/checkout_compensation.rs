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
const COMPENSATE_OPERATION: &str = "compensate_checkout_order";

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
                        tracing::warn!(
                            owner = ORDER_COMPENSATION_OWNER,
                            correlation_id = %context.correlation_id,
                            tenant_id = %context.tenant_id,
                            channel = ?context.channel,
                            operation = "cancel_checkout_order",
                            code = "order.checkout_compensation_state_conflict",
                            order_id = %current.id,
                            current_state = ?current.status_kind(),
                            from = %from,
                            to = %to,
                            "order lifecycle changed while checkout compensation was being applied"
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
            tracing::warn!(
                owner = ORDER_COMPENSATION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation = COMPENSATE_OPERATION,
                code = "order.checkout_compensation_identity_invalid",
                checkout_operation_id = %request.checkout_operation_id,
                cart_id = %request.cart_id,
                expected_order_id = ?request.expected_order_id,
                "checkout compensation rejected invalid owner identity"
            );
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

fn validate_identity(
    context: &PortContext,
    tenant_id: Uuid,
    request: &CheckoutOrderCompensationRequest,
    identity: &CheckoutOrderIdentitySnapshot,
) -> Result<(), PortError> {
    let valid = identity.tenant_id == tenant_id
        && identity.checkout_operation_id == request.checkout_operation_id
        && identity
            .source_cart_id
            .is_none_or(|cart_id| cart_id == request.cart_id)
        && request
            .expected_order_id
            .is_none_or(|order_id| order_id == identity.order_id);
    if !valid {
        tracing::error!(
            owner = ORDER_COMPENSATION_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            channel = ?context.channel,
            operation = COMPENSATE_OPERATION,
            code = "order.checkout_compensation_identity_conflict",
            request_checkout_operation_id = %request.checkout_operation_id,
            request_cart_id = %request.cart_id,
            request_expected_order_id = ?request.expected_order_id,
            identity_tenant_id = %identity.tenant_id,
            identity_checkout_operation_id = %identity.checkout_operation_id,
            identity_order_id = %identity.order_id,
            identity_source_cart_id = ?identity.source_cart_id,
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
        tracing::warn!(
            owner = ORDER_COMPENSATION_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            channel = ?context.channel,
            operation,
            code = "order.checkout_compensation_causation_invalid",
            expected_checkout_operation_id = %checkout_operation_id,
            actual_causation_id = ?context.causation_id,
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
        tracing::warn!(
            error = ?error,
            owner = ORDER_COMPENSATION_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            channel = ?context.channel,
            operation,
            field = "tenant_id",
            value_length = context.tenant_id.len(),
            code = "order.tenant_id_invalid",
            "order port received invalid request context"
        );
        PortError::validation(
            "order.tenant_id_invalid",
            "order request context is invalid",
        )
    })
}

fn parse_actor_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.actor.id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            owner = ORDER_COMPENSATION_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            channel = ?context.channel,
            operation,
            field = "actor_id",
            value_length = context.actor.id.len(),
            code = "order.actor_id_invalid",
            "order port received invalid request context"
        );
        PortError::validation("order.actor_id_invalid", "order request context is invalid")
    })
}

fn manual_reconciliation(
    context: &PortContext,
    operation: &'static str,
    order_id: Option<Uuid>,
    order_state: OrderStatusKind,
    reason: &'static str,
) -> PortError {
    tracing::error!(
        owner = ORDER_COMPENSATION_OWNER,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        channel = ?context.channel,
        operation,
        code = "order.checkout_compensation_manual_reconciliation",
        order_id = ?order_id,
        order_state = ?order_state,
        reason = %reason,
        "checkout order compensation requires manual reconciliation"
    );
    PortError::conflict(
        "order.checkout_compensation_manual_reconciliation",
        "checkout requires manual reconciliation",
    )
}

fn order_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: OrderError,
) -> PortError {
    match error {
        OrderError::Database(error) => {
            tracing::error!(
                error = ?error,
                owner = ORDER_COMPENSATION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.database_unavailable",
                "order checkout compensation storage failed"
            );
            PortError::unavailable(
                "order.database_unavailable",
                "order storage is temporarily unavailable",
            )
        }
        OrderError::OrderNotFound(order_id) => {
            tracing::warn!(
                owner = ORDER_COMPENSATION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.order_not_found",
                order_id = %order_id,
                "checkout compensation order was not found"
            );
            PortError::not_found("order.order_not_found", "order was not found")
        }
        OrderError::Validation(cause) => {
            tracing::warn!(
                cause = %cause,
                owner = ORDER_COMPENSATION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.checkout_compensation_validation",
                "order owner rejected checkout compensation"
            );
            PortError::validation(
                "order.checkout_compensation_validation",
                "checkout order compensation request is invalid",
            )
        }
        OrderError::InvalidTransition { from, to } => {
            tracing::warn!(
                owner = ORDER_COMPENSATION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.checkout_compensation_state_conflict",
                from = %from,
                to = %to,
                "order lifecycle conflicts with checkout compensation"
            );
            PortError::conflict(
                "order.checkout_compensation_state_conflict",
                "checkout order lifecycle conflicts with compensation",
            )
        }
        OrderError::OrderReturnNotFound(return_id) => {
            tracing::warn!(
                owner = ORDER_COMPENSATION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.related_resource_not_found",
                resource = "order_return",
                resource_id = %return_id,
                "checkout compensation related order resource was not found"
            );
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::OrderChangeNotFound(change_id) => {
            tracing::warn!(
                owner = ORDER_COMPENSATION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.related_resource_not_found",
                resource = "order_change",
                resource_id = %change_id,
                "checkout compensation related order resource was not found"
            );
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::Core(error) => {
            tracing::error!(
                error = ?error,
                owner = ORDER_COMPENSATION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.invariant_violation",
                "order checkout compensation invariant failed"
            );
            PortError::invariant_violation(
                "order.invariant_violation",
                "order compensation failed an internal invariant",
            )
        }
    }
}
