use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{FulfillmentError, FulfillmentResponse, FulfillmentService, ListFulfillmentsInput};

const FULFILLMENT_OWNER: &str = "rustok_fulfillment";
const FULFILLMENT_LIFECYCLE_READ_BOUNDARY: &str = "fulfillment_lifecycle_read_port";

/// Transport-neutral owner boundary for fulfillment lifecycle projection reads.
#[async_trait]
pub trait FulfillmentReadPort: Send + Sync {
    async fn read_fulfillment_projection(
        &self,
        context: PortContext,
        request: ReadFulfillmentProjectionRequest,
    ) -> Result<FulfillmentResponse, PortError>;

    async fn list_fulfillment_projections(
        &self,
        context: PortContext,
        request: ListFulfillmentProjectionsRequest,
    ) -> Result<FulfillmentProjectionPage, PortError>;

    async fn find_latest_fulfillment_by_order_projection(
        &self,
        context: PortContext,
        request: FindLatestFulfillmentByOrderProjectionRequest,
    ) -> Result<Option<FulfillmentResponse>, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadFulfillmentProjectionRequest {
    pub fulfillment_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListFulfillmentProjectionsRequest {
    pub page: u64,
    pub per_page: u64,
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentProjectionPage {
    pub items: Vec<FulfillmentResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindLatestFulfillmentByOrderProjectionRequest {
    pub order_id: Uuid,
}

pub struct InProcessFulfillmentReadPort {
    inner: FulfillmentService,
}

impl InProcessFulfillmentReadPort {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            inner: FulfillmentService::new(db),
        }
    }

    pub fn from_service(inner: FulfillmentService) -> Self {
        Self { inner }
    }
}

pub fn in_process_fulfillment_read_port(
    db: sea_orm::DatabaseConnection,
) -> Arc<dyn FulfillmentReadPort> {
    Arc::new(InProcessFulfillmentReadPort::new(db))
}

struct FulfillmentLifecycleReadContextFacts {
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

struct FulfillmentLifecycleOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

struct FulfillmentLifecycleReadRequestFacts {
    fulfillment_id_present: bool,
    fulfillment_id_non_nil: bool,
    order_id_present: bool,
    order_id_non_nil: bool,
    customer_id_present: bool,
    customer_id_non_nil: bool,
    status_present: bool,
    status_length: Option<usize>,
}

#[async_trait]
impl FulfillmentReadPort for InProcessFulfillmentReadPort {
    async fn read_fulfillment_projection(
        &self,
        context: PortContext,
        request: ReadFulfillmentProjectionRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, "read_fulfillment_projection")?;

        self.inner
            .get_fulfillment(tenant_id, request.fulfillment_id)
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    "read_fulfillment_projection",
                    Some(request.fulfillment_id),
                    None,
                    None,
                    None,
                    error,
                )
            })
    }

    async fn list_fulfillment_projections(
        &self,
        context: PortContext,
        request: ListFulfillmentProjectionsRequest,
    ) -> Result<FulfillmentProjectionPage, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, "list_fulfillment_projections")?;
        let status_length = request.status.as_deref().map(str::len);
        let order_id = request.order_id;
        let customer_id = request.customer_id;
        let (items, total) = self
            .inner
            .list_fulfillments(
                tenant_id,
                ListFulfillmentsInput {
                    page: request.page,
                    per_page: request.per_page,
                    status: request.status,
                    order_id,
                    customer_id,
                },
            )
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    "list_fulfillment_projections",
                    None,
                    order_id,
                    status_length,
                    customer_id,
                    error,
                )
            })?;

        Ok(FulfillmentProjectionPage { items, total })
    }

    async fn find_latest_fulfillment_by_order_projection(
        &self,
        context: PortContext,
        request: FindLatestFulfillmentByOrderProjectionRequest,
    ) -> Result<Option<FulfillmentResponse>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, "find_latest_fulfillment_by_order_projection")?;

        self.inner
            .find_by_order(tenant_id, request.order_id)
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    "find_latest_fulfillment_by_order_projection",
                    None,
                    Some(request.order_id),
                    None,
                    None,
                    error,
                )
            })
    }
}

fn fulfillment_lifecycle_read_context_facts(
    context: &PortContext,
) -> FulfillmentLifecycleReadContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    FulfillmentLifecycleReadContextFacts {
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

fn fulfillment_lifecycle_owner_error_facts(
    error: &FulfillmentError,
) -> FulfillmentLifecycleOwnerErrorFacts {
    let (
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    ) = match error {
        FulfillmentError::Validation(value) => {
            ("validation", 1, value.chars().count(), 0, 0, false)
        }
        FulfillmentError::ShippingOptionNotFound(id) => (
            "shipping_option_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        FulfillmentError::FulfillmentNotFound(id) => (
            "fulfillment_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        FulfillmentError::InvalidTransition { from, to } => (
            "invalid_transition",
            2,
            from.chars().count() + to.chars().count(),
            0,
            0,
            false,
        ),
        FulfillmentError::Database(_) => ("database", 0, 0, 0, 0, true),
    };
    FulfillmentLifecycleOwnerErrorFacts {
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    }
}

fn fulfillment_lifecycle_read_request_facts(
    fulfillment_id: Option<Uuid>,
    order_id: Option<Uuid>,
    status_length: Option<usize>,
    customer_id: Option<Uuid>,
) -> FulfillmentLifecycleReadRequestFacts {
    FulfillmentLifecycleReadRequestFacts {
        fulfillment_id_present: fulfillment_id.is_some(),
        fulfillment_id_non_nil: fulfillment_id.map(|value| !value.is_nil()).unwrap_or(false),
        order_id_present: order_id.is_some(),
        order_id_non_nil: order_id.map(|value| !value.is_nil()).unwrap_or(false),
        customer_id_present: customer_id.is_some(),
        customer_id_non_nil: customer_id.map(|value| !value.is_nil()).unwrap_or(false),
        status_present: status_length.is_some(),
        status_length,
    }
}

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let context_facts = fulfillment_lifecycle_read_context_facts(context);
        tracing::warn!(
            owner = FULFILLMENT_OWNER,
            operation,
            correlation_id = %context.correlation_id,
            tenant_id_parse_failed = true,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            code = "fulfillment.context_invalid",
            boundary = FULFILLMENT_LIFECYCLE_READ_BOUNDARY,
            "fulfillment lifecycle read context is invalid"
        );
        PortError::validation(
            "fulfillment.context_invalid",
            "fulfillment request context is invalid",
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn map_owner_error(
    context: &PortContext,
    operation: &'static str,
    fulfillment_id: Option<Uuid>,
    order_id: Option<Uuid>,
    status_length: Option<usize>,
    customer_id: Option<Uuid>,
    error: FulfillmentError,
) -> PortError {
    let error_facts = fulfillment_lifecycle_owner_error_facts(&error);
    let request_facts = fulfillment_lifecycle_read_request_facts(
        fulfillment_id,
        order_id,
        status_length,
        customer_id,
    );
    let (kind, code, message, retryable, technical_failure) = match &error {
        FulfillmentError::Validation(_) => (
            PortErrorKind::Validation,
            "fulfillment.validation",
            "fulfillment request is invalid",
            false,
            false,
        ),
        FulfillmentError::ShippingOptionNotFound(_) => (
            PortErrorKind::NotFound,
            "fulfillment.shipping_option_not_found",
            "shipping option was not found",
            false,
            false,
        ),
        FulfillmentError::FulfillmentNotFound(_) => (
            PortErrorKind::NotFound,
            "fulfillment.fulfillment_not_found",
            "fulfillment was not found",
            false,
            false,
        ),
        FulfillmentError::InvalidTransition { .. } => (
            PortErrorKind::Conflict,
            "fulfillment.invalid_transition",
            "fulfillment lifecycle transition conflicts with the current state",
            false,
            false,
        ),
        FulfillmentError::Database(_) => (
            PortErrorKind::Unavailable,
            "fulfillment.database_unavailable",
            "fulfillment storage is temporarily unavailable",
            true,
            true,
        ),
    };
    let context_facts = fulfillment_lifecycle_read_context_facts(context);

    if technical_failure {
        tracing::error!(
            owner = FULFILLMENT_OWNER,
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
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            fulfillment_id_present = request_facts.fulfillment_id_present,
            fulfillment_id_non_nil = request_facts.fulfillment_id_non_nil,
            order_id_present = request_facts.order_id_present,
            order_id_non_nil = request_facts.order_id_non_nil,
            customer_id_present = request_facts.customer_id_present,
            customer_id_non_nil = request_facts.customer_id_non_nil,
            status_present = request_facts.status_present,
            status_length = ?request_facts.status_length,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            code,
            retryable,
            boundary = FULFILLMENT_LIFECYCLE_READ_BOUNDARY,
            "fulfillment lifecycle read failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = FULFILLMENT_OWNER,
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
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            fulfillment_id_present = request_facts.fulfillment_id_present,
            fulfillment_id_non_nil = request_facts.fulfillment_id_non_nil,
            order_id_present = request_facts.order_id_present,
            order_id_non_nil = request_facts.order_id_non_nil,
            customer_id_present = request_facts.customer_id_present,
            customer_id_non_nil = request_facts.customer_id_non_nil,
            status_present = request_facts.status_present,
            status_length = ?request_facts.status_length,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            code,
            retryable,
            boundary = FULFILLMENT_LIFECYCLE_READ_BOUNDARY,
            "fulfillment lifecycle read was rejected with bounded diagnostics"
        );
    }

    PortError::new(kind, code, message, retryable)
}
