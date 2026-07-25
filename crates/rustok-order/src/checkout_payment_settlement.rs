use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AdoptLegacyCheckoutOrderIdentityRequest, CheckoutOrderIdentityPort,
    InProcessCheckoutOrderIdentityPort, OrderError, OrderResponse, OrderService, OrderStatusKind,
    ReadCheckoutOrderIdentityByOperationRequest,
};

const ORDER_PAYMENT_SETTLEMENT_OWNER: &str = "rustok_order.checkout_payment_settlement";
const SETTLE_PAYMENT_OPERATION: &str = "settle_checkout_payment";

#[async_trait]
pub trait CheckoutOrderPaymentSettlementPort: Send + Sync {
    async fn settle_checkout_payment(
        &self,
        context: PortContext,
        request: SettleCheckoutOrderPaymentRequest,
    ) -> Result<OrderResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettleCheckoutOrderPaymentRequest {
    pub checkout_operation_id: Uuid,
    pub cart_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub payment_reference: String,
    pub payment_method: String,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

pub struct InProcessCheckoutOrderPaymentSettlementPort {
    service: OrderService,
    identity_port: Arc<dyn CheckoutOrderIdentityPort>,
}

impl InProcessCheckoutOrderPaymentSettlementPort {
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
            service: OrderService::new(db, event_bus),
            identity_port,
        }
    }

    async fn load_order(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        request: &SettleCheckoutOrderPaymentRequest,
    ) -> Result<OrderResponse, PortError> {
        match request.locale.as_deref() {
            Some(locale) => {
                self.service
                    .get_order_with_locale_fallback(
                        tenant_id,
                        request.order_id,
                        locale,
                        request.fallback_locale.as_deref(),
                    )
                    .await
            }
            None => self.service.get_order(tenant_id, request.order_id).await,
        }
        .map_err(|error| order_error_to_port_error(context, "load_checkout_order", error))
    }
}

pub fn in_process_checkout_order_payment_settlement_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CheckoutOrderPaymentSettlementPort> {
    Arc::new(InProcessCheckoutOrderPaymentSettlementPort::new(
        db, event_bus,
    ))
}

#[async_trait]
impl CheckoutOrderPaymentSettlementPort for InProcessCheckoutOrderPaymentSettlementPort {
    async fn settle_checkout_payment(
        &self,
        context: PortContext,
        request: SettleCheckoutOrderPaymentRequest,
    ) -> Result<OrderResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context, SETTLE_PAYMENT_OPERATION)?;
        let actor_id = parse_actor_id(&context, SETTLE_PAYMENT_OPERATION)?;
        require_operation_context(
            &context,
            SETTLE_PAYMENT_OPERATION,
            request.checkout_operation_id,
        )?;
        validate_request(&context, &request)?;

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
        let identity = identity.ok_or_else(|| {
            tracing::error!(
                owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation = SETTLE_PAYMENT_OPERATION,
                code = "order.checkout_payment_identity_missing",
                checkout_operation_id = %request.checkout_operation_id,
                cart_id = %request.cart_id,
                order_id = %request.order_id,
                payment_collection_id = %request.payment_collection_id,
                "checkout payment settlement has no durable order identity"
            );
            PortError::conflict(
                "order.checkout_payment_identity_missing",
                "checkout requires manual reconciliation",
            )
        })?;
        if identity.tenant_id != tenant_id
            || identity.checkout_operation_id != request.checkout_operation_id
            || identity.order_id != request.order_id
            || identity
                .source_cart_id
                .is_some_and(|cart_id| cart_id != request.cart_id)
            || identity
                .payment_collection_id
                .is_some_and(|collection_id| collection_id != request.payment_collection_id)
        {
            tracing::error!(
                owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation = SETTLE_PAYMENT_OPERATION,
                code = "order.checkout_payment_identity_conflict",
                checkout_operation_id = %request.checkout_operation_id,
                request_cart_id = %request.cart_id,
                request_order_id = %request.order_id,
                request_payment_collection_id = %request.payment_collection_id,
                identity_tenant_id = %identity.tenant_id,
                identity_checkout_operation_id = %identity.checkout_operation_id,
                identity_order_id = %identity.order_id,
                identity_source_cart_id = ?identity.source_cart_id,
                identity_payment_collection_id = ?identity.payment_collection_id,
                "checkout order identity conflicts with payment settlement"
            );
            return Err(PortError::conflict(
                "order.checkout_payment_identity_conflict",
                "checkout order identity conflicts with the payment settlement request",
            ));
        }

        let current = self.load_order(&context, tenant_id, &request).await?;
        let settled = match current.status_kind() {
            OrderStatusKind::Confirmed => self
                .service
                .mark_paid(
                    tenant_id,
                    actor_id,
                    current.id,
                    request.payment_reference.clone(),
                    request.payment_method.clone(),
                )
                .await
                .map_err(|error| {
                    order_error_to_port_error(&context, "mark_checkout_order_paid", error)
                })?,
            OrderStatusKind::Paid | OrderStatusKind::Shipped | OrderStatusKind::Delivered => {
                current
            }
            state @ (OrderStatusKind::Pending
            | OrderStatusKind::Cancelled
            | OrderStatusKind::Unknown) => {
                tracing::warn!(
                    owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                    correlation_id = %context.correlation_id,
                    tenant_id = %context.tenant_id,
                    channel = ?context.channel,
                    operation = SETTLE_PAYMENT_OPERATION,
                    code = "order.checkout_payment_state_conflict",
                    order_id = %current.id,
                    order_state = ?state,
                    "checkout order lifecycle does not allow payment settlement"
                );
                return Err(PortError::conflict(
                    "order.checkout_payment_state_conflict",
                    "checkout order lifecycle does not allow payment settlement",
                ));
            }
        };
        if settled.payment_id.as_deref() != Some(request.payment_reference.as_str())
            || settled.payment_method.as_deref() != Some(request.payment_method.as_str())
        {
            tracing::error!(
                owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation = SETTLE_PAYMENT_OPERATION,
                code = "order.checkout_payment_reference_conflict",
                order_id = %settled.id,
                requested_payment_reference = %request.payment_reference,
                requested_payment_method = %request.payment_method,
                settled_payment_reference = ?settled.payment_id,
                settled_payment_method = ?settled.payment_method,
                "checkout order is settled by another payment identity"
            );
            return Err(PortError::conflict(
                "order.checkout_payment_reference_conflict",
                "checkout order is settled by another payment identity",
            ));
        }
        Ok(settled)
    }
}

fn validate_request(
    context: &PortContext,
    request: &SettleCheckoutOrderPaymentRequest,
) -> Result<(), PortError> {
    if request.checkout_operation_id.is_nil()
        || request.cart_id.is_nil()
        || request.order_id.is_nil()
        || request.payment_collection_id.is_nil()
        || request.payment_reference.trim().is_empty()
        || request.payment_method.trim().is_empty()
    {
        tracing::warn!(
            owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            channel = ?context.channel,
            operation = SETTLE_PAYMENT_OPERATION,
            code = "order.checkout_payment_request_invalid",
            checkout_operation_id = %request.checkout_operation_id,
            cart_id = %request.cart_id,
            order_id = %request.order_id,
            payment_collection_id = %request.payment_collection_id,
            payment_reference_present = !request.payment_reference.trim().is_empty(),
            payment_method_present = !request.payment_method.trim().is_empty(),
            "checkout payment settlement rejected invalid owner identities"
        );
        return Err(PortError::validation(
            "order.checkout_payment_request_invalid",
            "checkout payment settlement request is invalid",
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
            owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            channel = ?context.channel,
            operation,
            code = "order.checkout_payment_causation_invalid",
            expected_checkout_operation_id = %checkout_operation_id,
            actual_causation_id = ?context.causation_id,
            "checkout payment settlement received invalid causation identity"
        );
        return Err(PortError::validation(
            "order.checkout_payment_causation_invalid",
            "checkout operation context is invalid",
        ));
    }
    Ok(())
}

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
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
            owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
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

fn order_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: OrderError,
) -> PortError {
    match error {
        OrderError::Database(error) => {
            tracing::error!(
                error = ?error,
                owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.database_unavailable",
                "checkout order payment settlement storage failed"
            );
            PortError::unavailable(
                "order.database_unavailable",
                "order storage is temporarily unavailable",
            )
        }
        OrderError::OrderNotFound(order_id) => {
            tracing::warn!(
                owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.order_not_found",
                order_id = %order_id,
                "checkout payment settlement order was not found"
            );
            PortError::not_found("order.order_not_found", "order was not found")
        }
        OrderError::Validation(cause) => {
            tracing::warn!(
                cause = %cause,
                owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.checkout_payment_validation",
                "order owner rejected checkout payment settlement"
            );
            PortError::validation(
                "order.checkout_payment_validation",
                "checkout order payment settlement request is invalid",
            )
        }
        OrderError::InvalidTransition { from, to } => {
            tracing::warn!(
                owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.checkout_payment_state_conflict",
                from = %from,
                to = %to,
                "order lifecycle conflicts with checkout payment settlement"
            );
            PortError::conflict(
                "order.checkout_payment_state_conflict",
                "order lifecycle conflicts with payment settlement",
            )
        }
        OrderError::OrderReturnNotFound(return_id) => {
            tracing::warn!(
                owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.related_resource_not_found",
                resource = "order_return",
                resource_id = %return_id,
                "checkout payment settlement related order resource was not found"
            );
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::OrderChangeNotFound(change_id) => {
            tracing::warn!(
                owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.related_resource_not_found",
                resource = "order_change",
                resource_id = %change_id,
                "checkout payment settlement related order resource was not found"
            );
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::Core(error) => {
            tracing::error!(
                error = ?error,
                owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation,
                code = "order.invariant_violation",
                "checkout order payment settlement invariant failed"
            );
            PortError::invariant_violation(
                "order.invariant_violation",
                "order payment settlement failed an internal invariant",
            )
        }
    }
}
