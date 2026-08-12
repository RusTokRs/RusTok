use std::sync::Arc;

use ::rustok_api::{PortContext, PortError, PortErrorKind};
use ::rustok_order::{
    ListOrderChangeProjectionsRequest, ListOrderChangesInput, ListOrderProjectionsRequest,
    ListOrderReturnProjectionsRequest, ListOrderReturnsInput, ListOrdersInput, OrderChangeResponse,
    OrderReadPort, OrderResponse, OrderReturnResponse, ReadOrderChangeProjectionRequest,
    ReadOrderProjectionRequest, ReadOrderReturnProjectionRequest,
};
use ::rustok_outbox::TransactionalEventBus;
use ::sea_orm::{DatabaseConnection, DbErr};
use ::uuid::Uuid;

pub(crate) mod dto {
    pub use ::rustok_order::dto::*;
}

pub(crate) mod error {
    pub use ::rustok_order::error::*;
}

use error::{OrderError, OrderResult};

const GRAPHQL_ORDER_READ_BOUNDARY: &str = "commerce_graphql_order_read_shim";

/// Compatibility facade for the legacy safe-query source.
///
/// Complete order, return, and order-change projection reads are routed through
/// the typed owner boundary. The facade stores no concrete owner service.
pub(crate) struct OrderService {
    order_reads: Arc<dyn OrderReadPort>,
}

impl OrderService {
    pub(crate) fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        let order_reads =
            crate::graphql_runtime::order_read_runtime_for_current_graphql_scope(db, event_bus)
                .order_read_port();
        Self { order_reads }
    }

    pub(crate) async fn get_order_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> OrderResult<OrderResponse> {
        const OPERATION: &str = "read_order_projection";
        let context = graphql_order_read_context(tenant_id, Some(locale), OPERATION, order_id);
        self.order_reads
            .read_order_projection(
                context.clone(),
                ReadOrderProjectionRequest {
                    order_id,
                    tenant_default_locale: fallback_locale.map(str::to_owned),
                },
            )
            .await
            .map_err(|error| {
                map_order_read_port_error(
                    error,
                    &context,
                    OPERATION,
                    GraphqlOrderReadResource::Order(order_id),
                )
            })
    }

    pub(crate) async fn list_orders_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        input: ListOrdersInput,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> OrderResult<(Vec<OrderResponse>, u64)> {
        const OPERATION: &str = "list_order_projections";
        let context = graphql_order_read_context(tenant_id, Some(locale), OPERATION, tenant_id);
        let page = self
            .order_reads
            .list_order_projections(
                context.clone(),
                ListOrderProjectionsRequest {
                    page: input.page,
                    per_page: input.per_page,
                    status: input.status,
                    customer_id: input.customer_id,
                    tenant_default_locale: fallback_locale.map(str::to_owned),
                },
            )
            .await
            .map_err(|error| {
                map_order_read_port_error(
                    error,
                    &context,
                    OPERATION,
                    GraphqlOrderReadResource::None,
                )
            })?;
        Ok((page.items, page.total))
    }

    pub(crate) async fn get_order_change(
        &self,
        tenant_id: Uuid,
        change_id: Uuid,
    ) -> OrderResult<OrderChangeResponse> {
        const OPERATION: &str = "read_order_change_projection";
        let context = graphql_order_read_context(tenant_id, None, OPERATION, change_id);
        self.order_reads
            .read_order_change_projection(
                context.clone(),
                ReadOrderChangeProjectionRequest { change_id },
            )
            .await
            .map_err(|error| {
                map_order_read_port_error(
                    error,
                    &context,
                    OPERATION,
                    GraphqlOrderReadResource::Change(change_id),
                )
            })
    }

    pub(crate) async fn list_order_changes(
        &self,
        tenant_id: Uuid,
        input: ListOrderChangesInput,
    ) -> OrderResult<(Vec<OrderChangeResponse>, u64)> {
        const OPERATION: &str = "list_order_change_projections";
        let context = graphql_order_read_context(tenant_id, None, OPERATION, tenant_id);
        let page = self
            .order_reads
            .list_order_change_projections(
                context.clone(),
                ListOrderChangeProjectionsRequest {
                    page: input.page,
                    per_page: input.per_page,
                    order_id: input.order_id,
                    status: input.status,
                    change_type: input.change_type,
                },
            )
            .await
            .map_err(|error| {
                map_order_read_port_error(
                    error,
                    &context,
                    OPERATION,
                    GraphqlOrderReadResource::None,
                )
            })?;
        Ok((page.items, page.total))
    }

    pub(crate) async fn get_return(
        &self,
        tenant_id: Uuid,
        return_id: Uuid,
    ) -> OrderResult<OrderReturnResponse> {
        const OPERATION: &str = "read_order_return_projection";
        let context = graphql_order_read_context(tenant_id, None, OPERATION, return_id);
        self.order_reads
            .read_order_return_projection(
                context.clone(),
                ReadOrderReturnProjectionRequest { return_id },
            )
            .await
            .map_err(|error| {
                map_order_read_port_error(
                    error,
                    &context,
                    OPERATION,
                    GraphqlOrderReadResource::Return(return_id),
                )
            })
    }

    pub(crate) async fn list_returns(
        &self,
        tenant_id: Uuid,
        input: ListOrderReturnsInput,
    ) -> OrderResult<(Vec<OrderReturnResponse>, u64)> {
        const OPERATION: &str = "list_order_return_projections";
        let context = graphql_order_read_context(tenant_id, None, OPERATION, tenant_id);
        let page = self
            .order_reads
            .list_order_return_projections(
                context.clone(),
                ListOrderReturnProjectionsRequest {
                    page: input.page,
                    per_page: input.per_page,
                    order_id: input.order_id,
                    status: input.status,
                },
            )
            .await
            .map_err(|error| {
                map_order_read_port_error(
                    error,
                    &context,
                    OPERATION,
                    GraphqlOrderReadResource::None,
                )
            })?;
        Ok((page.items, page.total))
    }
}

#[derive(Clone, Copy)]
enum GraphqlOrderReadResource {
    Order(Uuid),
    Return(Uuid),
    Change(Uuid),
    None,
}

fn graphql_order_read_context(
    tenant_id: Uuid,
    explicit_locale: Option<&str>,
    operation: &'static str,
    resource_id: Uuid,
) -> PortContext {
    let call_context = crate::graphql_runtime::order_read_call_context_for_current_graphql_scope();
    let locale = explicit_locale
        .or_else(|| call_context.locale())
        .unwrap_or("und");
    let context = PortContext::new(
        tenant_id.to_string(),
        call_context.actor(),
        locale,
        format!("commerce-graphql-order:{operation}:{resource_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match call_context.channel() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn map_order_read_port_error(
    error: PortError,
    context: &PortContext,
    operation: &'static str,
    resource: GraphqlOrderReadResource,
) -> OrderError {
    let error_kind = match &error.kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    };
    let (order_id, return_id, change_id) = match resource {
        GraphqlOrderReadResource::Order(id) => (Some(id), None, None),
        GraphqlOrderReadResource::Return(id) => (None, Some(id), None),
        GraphqlOrderReadResource::Change(id) => (None, None, Some(id)),
        GraphqlOrderReadResource::None => (None, None, None),
    };
    tracing::error!(
        owner = "rustok_order",
        owner_operation = operation,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        order_id = ?order_id,
        return_id = ?return_id,
        change_id = ?change_id,
        actor = ?context.actor,
        channel = ?context.channel,
        locale = %context.locale,
        deadline_ms = ?context.deadline_ms,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        boundary = GRAPHQL_ORDER_READ_BOUNDARY,
        "commerce GraphQL order owner read failed"
    );

    match error.kind {
        PortErrorKind::Validation | PortErrorKind::Forbidden => {
            OrderError::Validation(error.message)
        }
        PortErrorKind::NotFound => match resource {
            GraphqlOrderReadResource::Order(id) => OrderError::OrderNotFound(id),
            GraphqlOrderReadResource::Return(id) => OrderError::OrderReturnNotFound(id),
            GraphqlOrderReadResource::Change(id) => OrderError::OrderChangeNotFound(id),
            GraphqlOrderReadResource::None => OrderError::OrderNotFound(Uuid::nil()),
        },
        PortErrorKind::Conflict => OrderError::InvalidTransition {
            from: "current".to_string(),
            to: "requested".to_string(),
        },
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            OrderError::Database(DbErr::Custom(error.message))
        }
        PortErrorKind::InvariantViolation => {
            OrderError::Core(rustok_core::Error::Validation(error.message))
        }
    }
}
