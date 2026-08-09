use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CancelOrderChangeInput, CancelOrderReturnInput, CreateOrderChangeInput,
    CreateOrderReturnInput, OrderChangeResponse, OrderError, OrderReturnResponse, OrderService,
};

const ORDER_POST_ORDER_COMMAND_OWNER: &str = "rustok_order";
const ORDER_POST_ORDER_COMMAND_BOUNDARY: &str = "order_post_order_command_port";

#[async_trait]
pub trait OrderPostOrderCommandPort: Send + Sync {
    async fn create_change(
        &self,
        context: PortContext,
        request: CreateOrderChangeRequest,
    ) -> Result<OrderChangeResponse, PortError>;

    async fn cancel_change(
        &self,
        context: PortContext,
        request: CancelOrderChangeRequest,
    ) -> Result<OrderChangeResponse, PortError>;

    async fn create_return(
        &self,
        context: PortContext,
        request: CreateOrderReturnRequest,
    ) -> Result<OrderReturnResponse, PortError>;

    async fn cancel_return(
        &self,
        context: PortContext,
        request: CancelOrderReturnRequest,
    ) -> Result<OrderReturnResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderChangeRequest {
    pub order_id: Uuid,
    pub input: CreateOrderChangeInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrderChangeRequest {
    pub change_id: Uuid,
    pub input: CancelOrderChangeInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderReturnRequest {
    pub order_id: Uuid,
    pub input: CreateOrderReturnInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrderReturnRequest {
    pub return_id: Uuid,
    pub input: CancelOrderReturnInput,
}

pub struct InProcessOrderPostOrderCommandPort {
    inner: OrderService,
}

impl InProcessOrderPostOrderCommandPort {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: OrderService::new(db, event_bus),
        }
    }
}

pub fn in_process_order_post_order_command_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn OrderPostOrderCommandPort> {
    Arc::new(InProcessOrderPostOrderCommandPort::new(db, event_bus))
}

#[derive(Clone)]
pub struct OrderPostOrderCommandRuntime {
    command_port: Arc<dyn OrderPostOrderCommandPort>,
}

impl OrderPostOrderCommandRuntime {
    pub fn new(command_port: Arc<dyn OrderPostOrderCommandPort>) -> Self {
        Self { command_port }
    }

    pub fn in_process(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self::new(in_process_order_post_order_command_port(db, event_bus))
    }

    pub fn command_port(&self) -> Arc<dyn OrderPostOrderCommandPort> {
        self.command_port.clone()
    }
}

#[async_trait]
impl OrderPostOrderCommandPort for InProcessOrderPostOrderCommandPort {
    async fn create_change(
        &self,
        context: PortContext,
        request: CreateOrderChangeRequest,
    ) -> Result<OrderChangeResponse, PortError> {
        const OPERATION: &str = "create_change";
        let (tenant_id, actor_id) = require_post_order_command_context(&context, OPERATION)?;
        self.inner
            .create_order_change(tenant_id, actor_id, request.order_id, request.input)
            .await
            .map_err(|error| map_order_error(&context, OPERATION, request.order_id, error))
    }

    async fn cancel_change(
        &self,
        context: PortContext,
        request: CancelOrderChangeRequest,
    ) -> Result<OrderChangeResponse, PortError> {
        const OPERATION: &str = "cancel_change";
        let (tenant_id, _) = require_post_order_command_context(&context, OPERATION)?;
        self.inner
            .cancel_order_change(tenant_id, request.change_id, request.input)
            .await
            .map_err(|error| map_order_error(&context, OPERATION, request.change_id, error))
    }

    async fn create_return(
        &self,
        context: PortContext,
        request: CreateOrderReturnRequest,
    ) -> Result<OrderReturnResponse, PortError> {
        const OPERATION: &str = "create_return";
        let (tenant_id, _) = require_post_order_command_context(&context, OPERATION)?;
        self.inner
            .create_return(tenant_id, request.order_id, request.input)
            .await
            .map_err(|error| map_order_error(&context, OPERATION, request.order_id, error))
    }

    async fn cancel_return(
        &self,
        context: PortContext,
        request: CancelOrderReturnRequest,
    ) -> Result<OrderReturnResponse, PortError> {
        const OPERATION: &str = "cancel_return";
        let (tenant_id, _) = require_post_order_command_context(&context, OPERATION)?;
        self.inner
            .cancel_return(tenant_id, request.return_id, request.input)
            .await
            .map_err(|error| map_order_error(&context, OPERATION, request.return_id, error))
    }
}

fn require_post_order_command_context(
    context: &PortContext,
    operation: &'static str,
) -> Result<(Uuid, Uuid), PortError> {
    context.require_policy(PortCallPolicy::write()).inspect_err(|error| {
        log_context_error(context, operation, "policy", error);
    })?;
    let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let error = PortError::validation(
            "order.post_order_command_context_invalid",
            "order command context is invalid",
        );
        log_context_error(context, operation, "tenant_id", &error);
        error
    })?;
    let actor_id = Uuid::parse_str(&context.actor.id).map_err(|_| {
        let error = PortError::validation(
            "order.post_order_command_context_invalid",
            "order command context is invalid",
        );
        log_context_error(context, operation, "actor_id", &error);
        error
    })?;
    Ok((tenant_id, actor_id))
}

fn map_order_error(
    context: &PortContext,
    operation: &'static str,
    resource_id: Uuid,
    error: OrderError,
) -> PortError {
    let (kind, code, message, retryable, error_variant, technical) = match &error {
        OrderError::Validation(_) => (
            PortErrorKind::Validation,
            "order.post_order_command_validation",
            "order request is invalid",
            false,
            "validation",
            false,
        ),
        OrderError::OrderNotFound(_)
        | OrderError::OrderReturnNotFound(_)
        | OrderError::OrderChangeNotFound(_) => (
            PortErrorKind::NotFound,
            "order.post_order_resource_not_found",
            "order resource was not found",
            false,
            "not_found",
            false,
        ),
        OrderError::InvalidTransition { .. } => (
            PortErrorKind::Conflict,
            "order.post_order_command_state_conflict",
            "order lifecycle transition conflicts with the requested operation",
            false,
            "invalid_transition",
            false,
        ),
        OrderError::Database(_) => (
            PortErrorKind::Unavailable,
            "order.post_order_storage_unavailable",
            "order storage is temporarily unavailable",
            true,
            "database",
            true,
        ),
        OrderError::Core(_) => (
            PortErrorKind::InvariantViolation,
            "order.post_order_command_invariant",
            "order operation could not be completed safely",
            false,
            "core",
            true,
        ),
    };

    if technical {
        tracing::error!(
            owner = ORDER_POST_ORDER_COMMAND_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            resource_id_non_nil = !resource_id.is_nil(),
            error_variant,
            public_code = code,
            retryable,
            boundary = ORDER_POST_ORDER_COMMAND_BOUNDARY,
            "order post-order command failed"
        );
    } else {
        tracing::warn!(
            owner = ORDER_POST_ORDER_COMMAND_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            resource_id_non_nil = !resource_id.is_nil(),
            error_variant,
            public_code = code,
            retryable,
            boundary = ORDER_POST_ORDER_COMMAND_BOUNDARY,
            "order post-order command was rejected"
        );
    }

    PortError::new(kind, code, message, retryable)
}

fn log_context_error(
    context: &PortContext,
    operation: &'static str,
    admission: &'static str,
    error: &PortError,
) {
    tracing::warn!(
        owner = ORDER_POST_ORDER_COMMAND_OWNER,
        operation,
        admission,
        correlation_id = %context.correlation_id,
        tenant_id_length = context.tenant_id.chars().count(),
        actor_id_length = context.actor.id.chars().count(),
        code = %error.code,
        retryable = error.retryable,
        boundary = ORDER_POST_ORDER_COMMAND_BOUNDARY,
        "order post-order command context was rejected"
    );
}
