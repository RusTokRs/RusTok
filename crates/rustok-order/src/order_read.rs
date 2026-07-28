use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ListOrdersInput, OrderError, OrderResponse, OrderService};

/// Transport-neutral order-owner boundary for complete order projection reads.
#[async_trait]
pub trait OrderReadPort: Send + Sync {
    async fn read_order_projection(
        &self,
        context: PortContext,
        request: ReadOrderProjectionRequest,
    ) -> Result<OrderResponse, PortError>;

    async fn list_order_projections(
        &self,
        context: PortContext,
        request: ListOrderProjectionsRequest,
    ) -> Result<OrderProjectionPage, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadOrderProjectionRequest {
    pub order_id: Uuid,
    pub tenant_default_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListOrderProjectionsRequest {
    pub page: u64,
    pub per_page: u64,
    pub status: Option<String>,
    pub customer_id: Option<Uuid>,
    pub tenant_default_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderProjectionPage {
    pub items: Vec<OrderResponse>,
    pub total: u64,
}

pub struct InProcessOrderReadPort {
    inner: OrderService,
}

impl InProcessOrderReadPort {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> Self {
        Self {
            inner: OrderService::new(db, event_bus),
        }
    }

    pub fn from_service(inner: OrderService) -> Self {
        Self { inner }
    }
}

pub fn in_process_order_read_port(
    db: sea_orm::DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn OrderReadPort> {
    Arc::new(InProcessOrderReadPort::new(db, event_bus))
}

#[async_trait]
impl OrderReadPort for InProcessOrderReadPort {
    async fn read_order_projection(
        &self,
        context: PortContext,
        request: ReadOrderProjectionRequest,
    ) -> Result<OrderResponse, PortError> {
        const OPERATION: &str = "read_order_projection";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        let fallback_locale_length = request.tenant_default_locale.as_deref().map(str::len);

        self.inner
            .get_order_with_locale_fallback(
                tenant_id,
                request.order_id,
                context.locale.as_str(),
                request.tenant_default_locale.as_deref(),
            )
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    OPERATION,
                    Some(request.order_id),
                    None,
                    None,
                    fallback_locale_length,
                    error,
                )
            })
    }

    async fn list_order_projections(
        &self,
        context: PortContext,
        request: ListOrderProjectionsRequest,
    ) -> Result<OrderProjectionPage, PortError> {
        const OPERATION: &str = "list_order_projections";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        let status_length = request.status.as_deref().map(str::len);
        let customer_id = request.customer_id;
        let fallback_locale_length = request.tenant_default_locale.as_deref().map(str::len);

        let (items, total) = self
            .inner
            .list_orders_with_locale_fallback(
                tenant_id,
                ListOrdersInput {
                    page: request.page,
                    per_page: request.per_page,
                    status: request.status,
                    customer_id,
                },
                context.locale.as_str(),
                request.tenant_default_locale.as_deref(),
            )
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    OPERATION,
                    None,
                    customer_id,
                    status_length,
                    fallback_locale_length,
                    error,
                )
            })?;

        Ok(OrderProjectionPage { items, total })
    }
}

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            owner = "rustok_order",
            operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel_length = context.channel.as_deref().map(str::len),
            locale_length = context.locale.len(),
            causation_id_present = context.causation_id.is_some(),
            traceparent_present = context.traceparent.is_some(),
            deadline_ms = ?context.deadline_ms,
            code = "order.context_invalid",
            boundary = "order_read_port",
            "order read context is invalid"
        );
        PortError::validation(
            "order.context_invalid",
            "order request context is invalid",
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn map_owner_error(
    context: &PortContext,
    operation: &'static str,
    order_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    status_length: Option<usize>,
    fallback_locale_length: Option<usize>,
    error: OrderError,
) -> PortError {
    let (kind, code, message, retryable, error_kind) = match &error {
        OrderError::Validation(_) => (
            PortErrorKind::Validation,
            "order.validation",
            "order request is invalid",
            false,
            "validation",
        ),
        OrderError::OrderNotFound(_) => (
            PortErrorKind::NotFound,
            "order.order_not_found",
            "order was not found",
            false,
            "order_not_found",
        ),
        OrderError::OrderReturnNotFound(_) => (
            PortErrorKind::NotFound,
            "order.return_not_found",
            "order return was not found",
            false,
            "return_not_found",
        ),
        OrderError::OrderChangeNotFound(_) => (
            PortErrorKind::NotFound,
            "order.change_not_found",
            "order change was not found",
            false,
            "change_not_found",
        ),
        OrderError::InvalidTransition { .. } => (
            PortErrorKind::Conflict,
            "order.invalid_transition",
            "order lifecycle transition conflicts with the current state",
            false,
            "invalid_transition",
        ),
        OrderError::Database(_) => (
            PortErrorKind::Unavailable,
            "order.database_unavailable",
            "order storage is temporarily unavailable",
            true,
            "database",
        ),
        OrderError::Core(_) => (
            PortErrorKind::InvariantViolation,
            "order.operation_failed",
            "order operation could not be completed safely",
            false,
            "core",
        ),
    };

    if matches!(&error, OrderError::Database(_) | OrderError::Core(_)) {
        tracing::error!(
            error = ?error,
            owner = "rustok_order",
            operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel_length = context.channel.as_deref().map(str::len),
            locale_length = context.locale.len(),
            fallback_locale_length = ?fallback_locale_length,
            causation_id_present = context.causation_id.is_some(),
            traceparent_present = context.traceparent.is_some(),
            deadline_ms = ?context.deadline_ms,
            order_id = ?order_id,
            customer_id = ?customer_id,
            status_length = ?status_length,
            error_kind,
            code,
            retryable,
            boundary = "order_read_port",
            "order projection read failed"
        );
    } else {
        tracing::warn!(
            owner = "rustok_order",
            operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel_length = context.channel.as_deref().map(str::len),
            locale_length = context.locale.len(),
            fallback_locale_length = ?fallback_locale_length,
            causation_id_present = context.causation_id.is_some(),
            traceparent_present = context.traceparent.is_some(),
            deadline_ms = ?context.deadline_ms,
            order_id = ?order_id,
            customer_id = ?customer_id,
            status_length = ?status_length,
            error_kind,
            code,
            retryable,
            boundary = "order_read_port",
            "order projection read was rejected"
        );
    }

    PortError::new(kind, code, message, retryable)
}
