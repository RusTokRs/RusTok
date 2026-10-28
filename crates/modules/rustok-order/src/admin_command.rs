use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{OrderError, OrderResponse, OrderService};

const ORDER_ADMIN_COMMAND_OWNER: &str = "rustok_order";
const ORDER_ADMIN_COMMAND_BOUNDARY: &str = "order_admin_command_port";

#[async_trait]
pub trait OrderAdminCommandPort: Send + Sync {
    async fn mark_paid(
        &self,
        context: PortContext,
        request: MarkOrderPaidRequest,
    ) -> Result<OrderResponse, PortError>;

    async fn ship(
        &self,
        context: PortContext,
        request: ShipOrderRequest,
    ) -> Result<OrderResponse, PortError>;

    async fn deliver(
        &self,
        context: PortContext,
        request: DeliverOrderRequest,
    ) -> Result<OrderResponse, PortError>;

    async fn cancel(
        &self,
        context: PortContext,
        request: CancelOrderRequest,
    ) -> Result<OrderResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkOrderPaidRequest {
    pub order_id: Uuid,
    pub payment_id: String,
    pub payment_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShipOrderRequest {
    pub order_id: Uuid,
    pub tracking_number: String,
    pub carrier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliverOrderRequest {
    pub order_id: Uuid,
    pub delivered_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancelOrderRequest {
    pub order_id: Uuid,
    pub reason: Option<String>,
}

pub struct InProcessOrderAdminCommandPort {
    inner: OrderService,
}

impl InProcessOrderAdminCommandPort {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: OrderService::new(db, event_bus),
        }
    }
}

pub fn in_process_order_admin_command_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn OrderAdminCommandPort> {
    Arc::new(InProcessOrderAdminCommandPort::new(db, event_bus))
}

#[derive(Clone)]
pub struct OrderAdminCommandRuntime {
    command_port: Arc<dyn OrderAdminCommandPort>,
}

impl OrderAdminCommandRuntime {
    pub fn new(command_port: Arc<dyn OrderAdminCommandPort>) -> Self {
        Self { command_port }
    }

    pub fn in_process(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self::new(in_process_order_admin_command_port(db, event_bus))
    }

    pub fn command_port(&self) -> Arc<dyn OrderAdminCommandPort> {
        self.command_port.clone()
    }
}

fn validate_non_nil_uuid(id: Uuid, field: &'static str) -> Result<(), PortError> {
    if id.is_nil() {
        Err(PortError::validation(
            format!("order.{field}_nil"),
            format!("{field} must not be a nil UUID"),
        ))
    } else {
        Ok(())
    }
}

#[async_trait]
impl OrderAdminCommandPort for InProcessOrderAdminCommandPort {
    async fn mark_paid(
        &self,
        context: PortContext,
        request: MarkOrderPaidRequest,
    ) -> Result<OrderResponse, PortError> {
        let operation = "mark_paid";
        validate_non_nil_uuid(request.order_id, "order_id")?;
        let (tenant_id, actor_id) = require_admin_command_context(&context, operation)?;
        tracing::debug!(
            boundary = ORDER_ADMIN_COMMAND_BOUNDARY,
            operation,
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            order_id = %request.order_id,
            payment_id = %request.payment_id,
            payment_method = %request.payment_method,
            "order admin mark_paid entered"
        );
        self.inner
            .mark_paid(
                tenant_id,
                actor_id,
                request.order_id,
                request.payment_id,
                request.payment_method,
            )
            .await
            .map_err(|error| map_order_error(&context, operation, request.order_id, error))
    }

    async fn ship(
        &self,
        context: PortContext,
        request: ShipOrderRequest,
    ) -> Result<OrderResponse, PortError> {
        let operation = "ship";
        validate_non_nil_uuid(request.order_id, "order_id")?;
        let (tenant_id, actor_id) = require_admin_command_context(&context, operation)?;
        tracing::debug!(
            boundary = ORDER_ADMIN_COMMAND_BOUNDARY,
            operation,
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            order_id = %request.order_id,
            tracking_number = %request.tracking_number,
            carrier = %request.carrier,
            "order admin ship entered"
        );
        self.inner
            .ship_order(
                tenant_id,
                actor_id,
                request.order_id,
                request.tracking_number,
                request.carrier,
            )
            .await
            .map_err(|error| map_order_error(&context, operation, request.order_id, error))
    }

    async fn deliver(
        &self,
        context: PortContext,
        request: DeliverOrderRequest,
    ) -> Result<OrderResponse, PortError> {
        let operation = "deliver";
        validate_non_nil_uuid(request.order_id, "order_id")?;
        let (tenant_id, actor_id) = require_admin_command_context(&context, operation)?;
        tracing::debug!(
            boundary = ORDER_ADMIN_COMMAND_BOUNDARY,
            operation,
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            order_id = %request.order_id,
            delivered_signature = ?request.delivered_signature,
            "order admin deliver entered"
        );
        self.inner
            .deliver_order(
                tenant_id,
                actor_id,
                request.order_id,
                request.delivered_signature,
            )
            .await
            .map_err(|error| map_order_error(&context, operation, request.order_id, error))
    }

    async fn cancel(
        &self,
        context: PortContext,
        request: CancelOrderRequest,
    ) -> Result<OrderResponse, PortError> {
        let operation = "cancel";
        validate_non_nil_uuid(request.order_id, "order_id")?;
        let (tenant_id, actor_id) = require_admin_command_context(&context, operation)?;
        tracing::debug!(
            boundary = ORDER_ADMIN_COMMAND_BOUNDARY,
            operation,
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            order_id = %request.order_id,
            reason = ?request.reason,
            "order admin cancel entered"
        );
        self.inner
            .cancel_order(tenant_id, actor_id, request.order_id, request.reason)
            .await
            .map_err(|error| map_order_error(&context, operation, request.order_id, error))
    }
}

fn require_admin_command_context(
    context: &PortContext,
    operation: &'static str,
) -> Result<(Uuid, Uuid), PortError> {
    context.require_policy(PortCallPolicy::write())?;
    let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        log_invalid_context(context, operation, "tenant_id");
        PortError::validation(
            "order.admin_command_context_invalid",
            "order command context is invalid",
        )
    })?;
    if tenant_id.is_nil() {
        log_invalid_context(context, operation, "tenant_id_nil");
        return Err(PortError::validation(
            "order.tenant_id_nil",
            "order tenant_id must not be a nil UUID",
        ));
    }
    let actor_id = Uuid::parse_str(&context.actor.id).map_err(|_| {
        log_invalid_context(context, operation, "actor_id");
        PortError::validation(
            "order.admin_command_context_invalid",
            "order command context is invalid",
        )
    })?;
    if actor_id.is_nil() {
        log_invalid_context(context, operation, "actor_id_nil");
        return Err(PortError::validation(
            "order.actor_id_nil",
            "order actor_id must not be a nil UUID",
        ));
    }
    Ok((tenant_id, actor_id))
}

fn log_invalid_context(context: &PortContext, operation: &'static str, field: &'static str) {
    tracing::warn!(
        owner = ORDER_ADMIN_COMMAND_OWNER,
        operation,
        field,
        correlation_id = %context.correlation_id,
        tenant_id_length = context.tenant_id.chars().count(),
        actor_id_length = context.actor.id.chars().count(),
        boundary = ORDER_ADMIN_COMMAND_BOUNDARY,
        "order admin command context was rejected"
    );
}

fn map_order_error(
    context: &PortContext,
    operation: &'static str,
    order_id: Uuid,
    error: OrderError,
) -> PortError {
    let (kind, code, message, retryable, error_variant, technical) = match &error {
        OrderError::Validation(_) => (
            PortErrorKind::Validation,
            "order.admin_command_validation",
            "order request is invalid",
            false,
            "validation",
            false,
        ),
        OrderError::OrderNotFound(_)
        | OrderError::OrderReturnNotFound(_)
        | OrderError::OrderChangeNotFound(_) => (
            PortErrorKind::NotFound,
            "order.admin_order_not_found",
            "order was not found",
            false,
            "not_found",
            false,
        ),
        OrderError::InvalidTransition { .. } | OrderError::IdempotencyConflict => (
            PortErrorKind::Conflict,
            "order.admin_command_state_conflict",
            "order lifecycle transition conflicts with the current state",
            false,
            "invalid_transition",
            false,
        ),
        OrderError::CommandReceiptCorrupt => (
            PortErrorKind::InvariantViolation,
            "order.admin_command_receipt_invalid",
            "order command state requires operator review",
            false,
            "receipt_corrupt",
            true,
        ),
        OrderError::Database(_) => (
            PortErrorKind::Unavailable,
            "order.admin_command_storage_unavailable",
            "order storage is temporarily unavailable",
            true,
            "database",
            true,
        ),
        OrderError::Core(_) => (
            PortErrorKind::InvariantViolation,
            "order.admin_command_invariant",
            "order operation could not be completed safely",
            false,
            "core",
            true,
        ),
    };

    if technical {
        tracing::error!(
            owner = ORDER_ADMIN_COMMAND_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            order_id_non_nil = !order_id.is_nil(),
            error_variant,
            code,
            retryable,
            boundary = ORDER_ADMIN_COMMAND_BOUNDARY,
            "order admin command failed"
        );
    } else {
        tracing::warn!(
            owner = ORDER_ADMIN_COMMAND_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            order_id_non_nil = !order_id.is_nil(),
            error_variant,
            code,
            retryable,
            boundary = ORDER_ADMIN_COMMAND_BOUNDARY,
            "order admin command was rejected"
        );
    }

    PortError::new(kind, code, message, retryable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;
    use std::time::Duration;

    #[test]
    fn test_require_admin_command_context_success() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(actor_id.to_string()),
            "en",
            "corr-1",
        )
        .with_idempotency_key("idem-1")
        .with_deadline(Duration::from_millis(5000));

        let (parsed_tenant, parsed_actor) =
            require_admin_command_context(&context, "test_admin_cmd").unwrap();
        assert_eq!(parsed_tenant, tenant_id);
        assert_eq!(parsed_actor, actor_id);
    }

    #[test]
    fn test_require_admin_command_context_rejects_nil_tenant() {
        let actor_id = Uuid::new_v4();
        let context = PortContext::new(
            Uuid::nil().to_string(),
            PortActor::user(actor_id.to_string()),
            "en",
            "corr-1",
        )
        .with_idempotency_key("idem-1")
        .with_deadline(Duration::from_millis(5000));

        let err = require_admin_command_context(&context, "test_admin_cmd").unwrap_err();
        assert_eq!(err.code, "order.tenant_id_nil");
    }

    #[test]
    fn test_require_admin_command_context_rejects_nil_actor() {
        let tenant_id = Uuid::new_v4();
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(Uuid::nil().to_string()),
            "en",
            "corr-1",
        )
        .with_idempotency_key("idem-1")
        .with_deadline(Duration::from_millis(5000));

        let err = require_admin_command_context(&context, "test_admin_cmd").unwrap_err();
        assert_eq!(err.code, "order.actor_id_nil");
    }

    #[test]
    fn test_validate_non_nil_uuid() {
        assert!(validate_non_nil_uuid(Uuid::new_v4(), "order_id").is_ok());
        let err = validate_non_nil_uuid(Uuid::nil(), "order_id").unwrap_err();
        assert_eq!(err.code, "order.order_id_nil");
    }

    #[test]
    fn test_map_order_error() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(actor_id.to_string()),
            "en",
            "corr-1",
        );

        let err = map_order_error(
            &context,
            "test_op",
            order_id,
            OrderError::Validation("invalid".to_string()),
        );
        assert_eq!(err.code, "order.admin_command_validation");

        let err = map_order_error(
            &context,
            "test_op",
            order_id,
            OrderError::OrderNotFound(order_id),
        );
        assert_eq!(err.code, "order.admin_order_not_found");

        let err = map_order_error(
            &context,
            "test_op",
            order_id,
            OrderError::InvalidTransition {
                from: "pending".to_string(),
                to: "delivered".to_string(),
            },
        );
        assert_eq!(err.code, "order.admin_command_state_conflict");
    }
}
