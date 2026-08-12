use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ListOrderChangesInput, ListOrderReturnsInput, ListOrdersInput, OrderChangeResponse, OrderError,
    OrderResponse, OrderReturnResponse, OrderService,
};

const ORDER_READ_OWNER: &str = "rustok_order";
const ORDER_READ_BOUNDARY: &str = "order_read_port";

/// Transport-neutral order-owner boundary for complete order and post-order projection reads.
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

    async fn read_order_return_projection(
        &self,
        context: PortContext,
        request: ReadOrderReturnProjectionRequest,
    ) -> Result<OrderReturnResponse, PortError>;

    async fn list_order_return_projections(
        &self,
        context: PortContext,
        request: ListOrderReturnProjectionsRequest,
    ) -> Result<OrderReturnProjectionPage, PortError>;

    async fn read_order_change_projection(
        &self,
        context: PortContext,
        request: ReadOrderChangeProjectionRequest,
    ) -> Result<OrderChangeResponse, PortError>;

    async fn list_order_change_projections(
        &self,
        context: PortContext,
        request: ListOrderChangeProjectionsRequest,
    ) -> Result<OrderChangeProjectionPage, PortError>;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadOrderReturnProjectionRequest {
    pub return_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListOrderReturnProjectionsRequest {
    pub page: u64,
    pub per_page: u64,
    pub order_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderReturnProjectionPage {
    pub items: Vec<OrderReturnResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadOrderChangeProjectionRequest {
    pub change_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListOrderChangeProjectionsRequest {
    pub page: u64,
    pub per_page: u64,
    pub order_id: Option<Uuid>,
    pub status: Option<String>,
    pub change_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderChangeProjectionPage {
    pub items: Vec<OrderChangeResponse>,
    pub total: u64,
}

pub struct InProcessOrderReadPort {
    inner: OrderService,
}

impl InProcessOrderReadPort {
    pub fn new(db: sea_orm::DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
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
                    None,
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
                    None,
                    None,
                    customer_id,
                    status_length,
                    None,
                    fallback_locale_length,
                    error,
                )
            })?;

        Ok(OrderProjectionPage { items, total })
    }

    async fn read_order_return_projection(
        &self,
        context: PortContext,
        request: ReadOrderReturnProjectionRequest,
    ) -> Result<OrderReturnResponse, PortError> {
        const OPERATION: &str = "read_order_return_projection";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;

        self.inner
            .get_return(tenant_id, request.return_id)
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    OPERATION,
                    None,
                    Some(request.return_id),
                    None,
                    None,
                    None,
                    None,
                    None,
                    error,
                )
            })
    }

    async fn list_order_return_projections(
        &self,
        context: PortContext,
        request: ListOrderReturnProjectionsRequest,
    ) -> Result<OrderReturnProjectionPage, PortError> {
        const OPERATION: &str = "list_order_return_projections";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        let order_id = request.order_id;
        let status_length = request.status.as_deref().map(str::len);

        let (items, total) = self
            .inner
            .list_returns(
                tenant_id,
                ListOrderReturnsInput {
                    page: request.page,
                    per_page: request.per_page,
                    order_id,
                    status: request.status,
                },
            )
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    OPERATION,
                    order_id,
                    None,
                    None,
                    None,
                    status_length,
                    None,
                    None,
                    error,
                )
            })?;

        Ok(OrderReturnProjectionPage { items, total })
    }

    async fn read_order_change_projection(
        &self,
        context: PortContext,
        request: ReadOrderChangeProjectionRequest,
    ) -> Result<OrderChangeResponse, PortError> {
        const OPERATION: &str = "read_order_change_projection";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;

        self.inner
            .get_order_change(tenant_id, request.change_id)
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    OPERATION,
                    None,
                    None,
                    Some(request.change_id),
                    None,
                    None,
                    None,
                    None,
                    error,
                )
            })
    }

    async fn list_order_change_projections(
        &self,
        context: PortContext,
        request: ListOrderChangeProjectionsRequest,
    ) -> Result<OrderChangeProjectionPage, PortError> {
        const OPERATION: &str = "list_order_change_projections";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        let order_id = request.order_id;
        let status_length = request.status.as_deref().map(str::len);
        let change_type_length = request.change_type.as_deref().map(str::len);

        let (items, total) = self
            .inner
            .list_order_changes(
                tenant_id,
                ListOrderChangesInput {
                    page: request.page,
                    per_page: request.per_page,
                    order_id,
                    status: request.status,
                    change_type: request.change_type,
                },
            )
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    OPERATION,
                    order_id,
                    None,
                    None,
                    None,
                    status_length,
                    change_type_length,
                    None,
                    error,
                )
            })?;

        Ok(OrderChangeProjectionPage { items, total })
    }
}

#[derive(Clone, Copy, Debug)]
struct OrderReadContextFacts {
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

#[derive(Clone, Copy, Debug)]
struct OrderReadRequestFacts {
    order_id_present: bool,
    order_id_non_nil: Option<bool>,
    return_id_present: bool,
    return_id_non_nil: Option<bool>,
    change_id_present: bool,
    change_id_non_nil: Option<bool>,
    customer_id_present: bool,
    customer_id_non_nil: Option<bool>,
    status_length: Option<usize>,
    change_type_length: Option<usize>,
    fallback_locale_length: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
struct OrderReadOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

fn order_read_context_facts(context: &PortContext) -> OrderReadContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    OrderReadContextFacts {
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

fn order_read_request_facts(
    order_id: Option<Uuid>,
    return_id: Option<Uuid>,
    change_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    status_length: Option<usize>,
    change_type_length: Option<usize>,
    fallback_locale_length: Option<usize>,
) -> OrderReadRequestFacts {
    OrderReadRequestFacts {
        order_id_present: order_id.is_some(),
        order_id_non_nil: order_id.map(|value| !value.is_nil()),
        return_id_present: return_id.is_some(),
        return_id_non_nil: return_id.map(|value| !value.is_nil()),
        change_id_present: change_id.is_some(),
        change_id_non_nil: change_id.map(|value| !value.is_nil()),
        customer_id_present: customer_id.is_some(),
        customer_id_non_nil: customer_id.map(|value| !value.is_nil()),
        status_length,
        change_type_length,
        fallback_locale_length,
    }
}

fn order_read_owner_error_facts(error: &OrderError) -> OrderReadOwnerErrorFacts {
    let (
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    ) = match error {
        OrderError::Validation(value) => ("validation", 1, value.chars().count(), 0, 0, false),
        OrderError::OrderNotFound(id) => (
            "order_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        OrderError::OrderReturnNotFound(id) => (
            "return_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        OrderError::OrderChangeNotFound(id) => (
            "change_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        OrderError::InvalidTransition { from, to } => (
            "invalid_transition",
            2,
            from.chars().count() + to.chars().count(),
            0,
            0,
            false,
        ),
        OrderError::Database(_) => ("database", 0, 0, 0, 0, true),
        OrderError::Core(_) => ("core", 0, 0, 0, 0, true),
    };
    OrderReadOwnerErrorFacts {
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    }
}

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let facts = order_read_context_facts(context);
        tracing::warn!(
            owner = ORDER_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = facts.tenant_id_length,
            tenant_id_parseable = false,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            locale_length = facts.locale_length,
            causation_id_present = facts.causation_id_present,
            causation_id_length = ?facts.causation_id_length,
            traceparent_present = facts.traceparent_present,
            traceparent_length = ?facts.traceparent_length,
            idempotency_key_present = facts.idempotency_key_present,
            idempotency_key_length = ?facts.idempotency_key_length,
            deadline_ms = ?facts.deadline_ms,
            code = "order.context_invalid",
            boundary = ORDER_READ_BOUNDARY,
            "order read context was rejected with bounded diagnostics"
        );
        PortError::validation("order.context_invalid", "order request context is invalid")
    })
}

fn order_read_owner_error_policy(
    error: &OrderError,
) -> (
    PortErrorKind,
    &'static str,
    &'static str,
    bool,
    &'static str,
) {
    match error {
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
    }
}

fn log_order_read_owner_error(
    context: &PortContext,
    operation: &'static str,
    request_facts: &OrderReadRequestFacts,
    error_facts: &OrderReadOwnerErrorFacts,
    error_kind: &'static str,
    code: &'static str,
    retryable: bool,
    technical_failure: bool,
) {
    let context_facts = order_read_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = ORDER_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            fallback_locale_length = ?request_facts.fallback_locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            order_id_present = request_facts.order_id_present,
            order_id_non_nil = ?request_facts.order_id_non_nil,
            return_id_present = request_facts.return_id_present,
            return_id_non_nil = ?request_facts.return_id_non_nil,
            change_id_present = request_facts.change_id_present,
            change_id_non_nil = ?request_facts.change_id_non_nil,
            customer_id_present = request_facts.customer_id_present,
            customer_id_non_nil = ?request_facts.customer_id_non_nil,
            status_length = ?request_facts.status_length,
            change_type_length = ?request_facts.change_type_length,
            owner_error_variant = error_facts.error_variant,
            owner_error_text_field_count = error_facts.text_field_count,
            owner_error_text_total_length = error_facts.text_total_length,
            owner_error_uuid_field_count = error_facts.uuid_field_count,
            owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
            owner_error_opaque_payload_present = error_facts.opaque_payload_present,
            error_kind,
            code,
            retryable,
            boundary = ORDER_READ_BOUNDARY,
            "order projection read failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = ORDER_READ_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            fallback_locale_length = ?request_facts.fallback_locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            order_id_present = request_facts.order_id_present,
            order_id_non_nil = ?request_facts.order_id_non_nil,
            return_id_present = request_facts.return_id_present,
            return_id_non_nil = ?request_facts.return_id_non_nil,
            change_id_present = request_facts.change_id_present,
            change_id_non_nil = ?request_facts.change_id_non_nil,
            customer_id_present = request_facts.customer_id_present,
            customer_id_non_nil = ?request_facts.customer_id_non_nil,
            status_length = ?request_facts.status_length,
            change_type_length = ?request_facts.change_type_length,
            owner_error_variant = error_facts.error_variant,
            owner_error_text_field_count = error_facts.text_field_count,
            owner_error_text_total_length = error_facts.text_total_length,
            owner_error_uuid_field_count = error_facts.uuid_field_count,
            owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
            owner_error_opaque_payload_present = error_facts.opaque_payload_present,
            error_kind,
            code,
            retryable,
            boundary = ORDER_READ_BOUNDARY,
            "order projection read was rejected with bounded diagnostics"
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn map_owner_error(
    context: &PortContext,
    operation: &'static str,
    order_id: Option<Uuid>,
    return_id: Option<Uuid>,
    change_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    status_length: Option<usize>,
    change_type_length: Option<usize>,
    fallback_locale_length: Option<usize>,
    error: OrderError,
) -> PortError {
    let request_facts = order_read_request_facts(
        order_id,
        return_id,
        change_id,
        customer_id,
        status_length,
        change_type_length,
        fallback_locale_length,
    );
    let error_facts = order_read_owner_error_facts(&error);
    let (kind, code, message, retryable, error_kind) = order_read_owner_error_policy(&error);
    let technical_failure = matches!(&error, OrderError::Database(_) | OrderError::Core(_));
    log_order_read_owner_error(
        context,
        operation,
        &request_facts,
        &error_facts,
        error_kind,
        code,
        retryable,
        technical_failure,
    );
    PortError::new(kind, code, message, retryable)
}
