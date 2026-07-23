use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AdoptLegacyCheckoutOrderIdentityRequest, CheckoutOrderIdentityPort,
    CheckoutOrderIdentitySnapshot, InProcessCheckoutOrderIdentityPort, OrderError, OrderResponse,
    OrderService, OrderStatusKind, ReadCheckoutOrderIdentityByOperationRequest,
};

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
                Err(OrderError::InvalidTransition { .. }) => {
                    let current = self
                        .order_service
                        .get_order(tenant_id, order.id)
                        .await
                        .map_err(order_error_to_port_error)?;
                    if current.status_kind() == OrderStatusKind::Cancelled {
                        Ok(current)
                    } else {
                        Err(PortError::conflict(
                            "order.checkout_compensation_state_conflict",
                            "checkout order changed while compensation was being applied",
                        ))
                    }
                }
                Err(error) => Err(order_error_to_port_error(error)),
            },
            OrderStatusKind::Cancelled => Ok(order),
            OrderStatusKind::Paid | OrderStatusKind::Shipped | OrderStatusKind::Delivered => {
                Err(manual_reconciliation(
                    "checkout order has financial or fulfillment effects and cannot be cancelled automatically",
                ))
            }
            OrderStatusKind::Unknown => Err(manual_reconciliation(
                "checkout order lifecycle is unknown and requires manual reconciliation",
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
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        require_operation_context(&context, request.checkout_operation_id)?;
        if request.checkout_operation_id.is_nil() || request.cart_id.is_nil() {
            return Err(PortError::validation(
                "order.checkout_compensation_identity_invalid",
                "checkout operation and cart identity must be non-nil UUIDs",
            ));
        }

        let Some(identity) = self.resolve_identity(&context, &request).await? else {
            return if request.expected_order_id.is_none() {
                Ok(None)
            } else {
                Err(manual_reconciliation(
                    "checkout operation records an order but the order owner has no durable checkout identity",
                ))
            };
        };
        validate_identity(tenant_id, &request, &identity)?;

        let order = self
            .order_service
            .get_order(tenant_id, identity.order_id)
            .await
            .map_err(order_error_to_port_error)?;
        let order = self
            .cancel_or_adopt_cancelled(tenant_id, actor_id, order, request.reason)
            .await?;
        Ok(Some(CheckoutOrderCompensationSnapshot {
            order_id: order.id,
            status: order.status,
        }))
    }
}

fn validate_identity(
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
        return Err(PortError::conflict(
            "order.checkout_compensation_identity_conflict",
            "checkout order identity conflicts with the compensation request",
        ));
    }
    Ok(())
}

fn require_operation_context(
    context: &PortContext,
    checkout_operation_id: Uuid,
) -> Result<(), PortError> {
    let context_operation = context
        .causation_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    if context_operation != Some(checkout_operation_id) {
        return Err(PortError::validation(
            "order.checkout_compensation_causation_invalid",
            "checkout compensation causation_id must match the checkout operation",
        ));
    }
    Ok(())
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "order.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for order ports",
        )
    })
}

fn parse_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.actor.id).map_err(|_| {
        PortError::validation(
            "order.actor_id_invalid",
            "PortContext.actor.id must be a UUID for order write ports",
        )
    })
}

fn manual_reconciliation(message: impl Into<String>) -> PortError {
    PortError::new(
        PortErrorKind::Conflict,
        "order.checkout_compensation_manual_reconciliation",
        message,
        false,
    )
}

fn order_error_to_port_error(error: OrderError) -> PortError {
    match error {
        OrderError::Database(error) => {
            tracing::error!(error = ?error, "order checkout compensation storage failed");
            PortError::unavailable(
                "order.database_unavailable",
                "order storage is temporarily unavailable",
            )
        }
        OrderError::OrderNotFound(_) => {
            PortError::not_found("order.order_not_found", "order was not found")
        }
        OrderError::Validation(_) => PortError::validation(
            "order.checkout_compensation_validation",
            "checkout order compensation request is invalid",
        ),
        OrderError::InvalidTransition { .. } => PortError::conflict(
            "order.checkout_compensation_state_conflict",
            "checkout order lifecycle conflicts with compensation",
        ),
        OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_) => PortError::new(
            PortErrorKind::NotFound,
            "order.related_resource_not_found",
            "related order resource was not found",
            false,
        ),
        OrderError::Core(error) => {
            tracing::error!(error = ?error, "order checkout compensation invariant failed");
            PortError::new(
                PortErrorKind::InvariantViolation,
                "order.invariant_violation",
                "order compensation failed an internal invariant",
                false,
            )
        }
    }
}
