use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AdoptLegacyCheckoutOrderIdentityRequest, CheckoutOrderIdentityPort,
    InProcessCheckoutOrderIdentityPort, OrderError, OrderResponse, OrderService,
    ReadCheckoutOrderIdentityByOperationRequest,
};

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
        tenant_id: Uuid,
        request: &SettleCheckoutOrderPaymentRequest,
    ) -> Result<OrderResponse, PortError> {
        match request.locale.as_deref() {
            Some(locale) => self
                .service
                .get_order_with_locale_fallback(
                    tenant_id,
                    request.order_id,
                    locale,
                    request.fallback_locale.as_deref(),
                )
                .await,
            None => self.service.get_order(tenant_id, request.order_id).await,
        }
        .map_err(order_error_to_port_error)
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
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        require_operation_context(&context, request.checkout_operation_id)?;
        validate_request(&request)?;

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
                    context,
                    AdoptLegacyCheckoutOrderIdentityRequest {
                        checkout_operation_id: request.checkout_operation_id,
                        cart_id: request.cart_id,
                    },
                )
                .await?;
        }
        let identity = identity.ok_or_else(|| {
            PortError::conflict(
                "order.checkout_payment_identity_missing",
                "checkout order has no durable owner identity",
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
            return Err(PortError::conflict(
                "order.checkout_payment_identity_conflict",
                "checkout order identity conflicts with the payment settlement request",
            ));
        }

        let current = self.load_order(tenant_id, &request).await?;
        let settled = match current.status.as_str() {
            "confirmed" => self
                .service
                .mark_paid(
                    tenant_id,
                    actor_id,
                    current.id,
                    request.payment_reference.clone(),
                    request.payment_method.clone(),
                )
                .await
                .map_err(order_error_to_port_error)?,
            "paid" | "shipped" | "delivered" => current,
            status => {
                return Err(PortError::conflict(
                    "order.checkout_payment_state_conflict",
                    format!("checkout order cannot settle payment from `{status}`"),
                ));
            }
        };
        if settled.payment_id.as_deref() != Some(request.payment_reference.as_str())
            || settled.payment_method.as_deref() != Some(request.payment_method.as_str())
        {
            return Err(PortError::conflict(
                "order.checkout_payment_reference_conflict",
                "checkout order is settled by another payment identity",
            ));
        }
        Ok(settled)
    }
}

fn validate_request(request: &SettleCheckoutOrderPaymentRequest) -> Result<(), PortError> {
    if request.checkout_operation_id.is_nil()
        || request.cart_id.is_nil()
        || request.order_id.is_nil()
        || request.payment_collection_id.is_nil()
        || request.payment_reference.trim().is_empty()
        || request.payment_method.trim().is_empty()
    {
        return Err(PortError::validation(
            "order.checkout_payment_request_invalid",
            "checkout payment settlement requires non-empty owner identities",
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
            "order.checkout_payment_causation_invalid",
            "checkout payment causation_id must match the checkout operation",
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

fn order_error_to_port_error(error: OrderError) -> PortError {
    match error {
        OrderError::Database(error) => {
            tracing::error!(error = ?error, "checkout order payment settlement storage failed");
            PortError::unavailable(
                "order.database_unavailable",
                "order storage is temporarily unavailable",
            )
        }
        OrderError::OrderNotFound(_) => {
            PortError::not_found("order.order_not_found", "order was not found")
        }
        OrderError::Validation(_) => PortError::validation(
            "order.checkout_payment_validation",
            "checkout order payment settlement request is invalid",
        ),
        OrderError::InvalidTransition { .. } => PortError::conflict(
            "order.checkout_payment_state_conflict",
            "order lifecycle conflicts with payment settlement",
        ),
        OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_) => PortError::new(
            PortErrorKind::NotFound,
            "order.related_resource_not_found",
            "related order resource was not found",
            false,
        ),
        OrderError::Core(error) => {
            tracing::error!(error = ?error, "checkout order payment settlement invariant failed");
            PortError::new(
                PortErrorKind::InvariantViolation,
                "order.invariant_violation",
                "order payment settlement failed an internal invariant",
                false,
            )
        }
    }
}
