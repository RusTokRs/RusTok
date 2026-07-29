use std::sync::Arc;

use ::rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use ::rustok_order::{
    ListOrderChangesInput, ListOrderProjectionsRequest, ListOrderReturnsInput, ListOrdersInput,
    OrderChangeResponse, OrderReadPort, OrderResponse, OrderReturnResponse,
    ReadOrderProjectionRequest,
};
use ::rustok_outbox::TransactionalEventBus;
use ::sea_orm::{DatabaseConnection, DbErr};
use ::uuid::Uuid;

pub(crate) mod error {
    pub use ::rustok_order::error::*;
}

use error::{OrderError, OrderResult};

const GRAPHQL_ORDER_READ_BOUNDARY: &str = "commerce_graphql_order_read_shim";

/// Compatibility facade for the legacy safe-query source.
///
/// Complete order detail/list projections are routed through the typed owner
/// boundary. Return and order-change reads remain on the concrete owner service
/// until their wider owner contracts are published.
pub(crate) struct OrderService {
    inner: ::rustok_order::OrderService,
    order_reads: Arc<dyn OrderReadPort>,
}

impl OrderService {
    pub(crate) fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        let order_reads =
            crate::graphql_runtime::CommerceOrderReadRuntime::in_process(db.clone(), event_bus.clone())
                .order_read_port();
        Self {
            inner: ::rustok_order::OrderService::new(db, event_bus),
            order_reads,
        }
    }

    pub(crate) async fn get_order_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> OrderResult<OrderResponse> {
        let context = graphql_order_read_context(
            tenant_id,
            locale,
            "read_order_projection",
            order_id,
        );
        self.order_reads
            .read_order_projection(
                context.clone(),
                ReadOrderProjectionRequest {
                    order_id,
                    tenant_default_locale: fallback_locale.map(str::to_owned),
                },
            )
            .await
            .map_err(|error| map_order_read_port_error(error, &context, Some(order_id)))
    }

    pub(crate) async fn list_orders_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        input: ListOrdersInput,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> OrderResult<(Vec<OrderResponse>, u64)> {
        let context = graphql_order_read_context(
            tenant_id,
            locale,
            "list_order_projections",
            tenant_id,
        );
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
            .map_err(|error| map_order_read_port_error(error, &context, None))?;
        Ok((page.items, page.total))
    }

    pub(crate) async fn get_order_change(
        &self,
        tenant_id: Uuid,
        change_id: Uuid,
    ) -> OrderResult<OrderChangeResponse> {
        self.inner.get_order_change(tenant_id, change_id).await
    }

    pub(crate) async fn list_order_changes(
        &self,
        tenant_id: Uuid,
        input: ListOrderChangesInput,
    ) -> OrderResult<(Vec<OrderChangeResponse>, u64)> {
        self.inner.list_order_changes(tenant_id, input).await
    }

    pub(crate) async fn get_return(
        &self,
        tenant_id: Uuid,
        return_id: Uuid,
    ) -> OrderResult<OrderReturnResponse> {
        self.inner.get_return(tenant_id, return_id).await
    }

    pub(crate) async fn list_returns(
        &self,
        tenant_id: Uuid,
        input: ListOrderReturnsInput,
    ) -> OrderResult<(Vec<OrderReturnResponse>, u64)> {
        self.inner.list_returns(tenant_id, input).await
    }
}

fn graphql_order_read_context(
    tenant_id: Uuid,
    locale: &str,
    operation: &'static str,
    resource_id: Uuid,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.graphql-order-query"),
        locale,
        format!("commerce-graphql-order:{operation}:{resource_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

fn map_order_read_port_error(
    error: PortError,
    context: &PortContext,
    order_id: Option<Uuid>,
) -> OrderError {
    let error_kind = match error.kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    };
    tracing::error!(
        owner = "rustok_order",
        owner_operation = %context.correlation_id,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        order_id = ?order_id,
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
        PortErrorKind::NotFound => OrderError::OrderNotFound(order_id.unwrap_or_else(Uuid::nil)),
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
