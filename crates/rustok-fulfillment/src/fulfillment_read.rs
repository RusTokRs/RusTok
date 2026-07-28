use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    FulfillmentError, FulfillmentResponse, FulfillmentService, ListFulfillmentsInput,
};

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
        let tenant_id = parse_tenant_id(
            &context,
            "find_latest_fulfillment_by_order_projection",
        )?;

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

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            owner = "rustok_fulfillment",
            operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel_length = context.channel.as_deref().map(str::len),
            locale_length = context.locale.len(),
            causation_id_present = context.causation_id.is_some(),
            traceparent_present = context.traceparent.is_some(),
            deadline_ms = ?context.deadline_ms,
            code = "fulfillment.context_invalid",
            boundary = "fulfillment_lifecycle_read_port",
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
    let (kind, code, message, retryable, error_kind) = match &error {
        FulfillmentError::Validation(_) => (
            PortErrorKind::Validation,
            "fulfillment.validation",
            "fulfillment request is invalid",
            false,
            "validation",
        ),
        FulfillmentError::ShippingOptionNotFound(_) => (
            PortErrorKind::NotFound,
            "fulfillment.shipping_option_not_found",
            "shipping option was not found",
            false,
            "shipping_option_not_found",
        ),
        FulfillmentError::FulfillmentNotFound(_) => (
            PortErrorKind::NotFound,
            "fulfillment.fulfillment_not_found",
            "fulfillment was not found",
            false,
            "fulfillment_not_found",
        ),
        FulfillmentError::InvalidTransition { .. } => (
            PortErrorKind::Conflict,
            "fulfillment.invalid_transition",
            "fulfillment lifecycle transition conflicts with the current state",
            false,
            "invalid_transition",
        ),
        FulfillmentError::Database(_) => (
            PortErrorKind::Unavailable,
            "fulfillment.database_unavailable",
            "fulfillment storage is temporarily unavailable",
            true,
            "database",
        ),
    };

    if matches!(&error, FulfillmentError::Database(_)) {
        tracing::error!(
            error = ?error,
            owner = "rustok_fulfillment",
            operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel_length = context.channel.as_deref().map(str::len),
            locale_length = context.locale.len(),
            causation_id_present = context.causation_id.is_some(),
            traceparent_present = context.traceparent.is_some(),
            deadline_ms = ?context.deadline_ms,
            fulfillment_id = ?fulfillment_id,
            order_id = ?order_id,
            customer_id = ?customer_id,
            status_length = ?status_length,
            error_kind,
            code,
            retryable,
            boundary = "fulfillment_lifecycle_read_port",
            "fulfillment lifecycle read failed"
        );
    } else {
        tracing::warn!(
            owner = "rustok_fulfillment",
            operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel_length = context.channel.as_deref().map(str::len),
            locale_length = context.locale.len(),
            causation_id_present = context.causation_id.is_some(),
            traceparent_present = context.traceparent.is_some(),
            deadline_ms = ?context.deadline_ms,
            fulfillment_id = ?fulfillment_id,
            order_id = ?order_id,
            customer_id = ?customer_id,
            status_length = ?status_length,
            error_kind,
            code,
            retryable,
            boundary = "fulfillment_lifecycle_read_port",
            "fulfillment lifecycle read was rejected"
        );
    }

    PortError::new(kind, code, message, retryable)
}
