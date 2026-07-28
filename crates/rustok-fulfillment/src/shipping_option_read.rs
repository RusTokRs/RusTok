use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{FulfillmentError, FulfillmentService, ShippingOptionResponse};

/// Transport-neutral owner boundary for complete shipping-option reads.
#[async_trait]
pub trait ShippingOptionReadPort: Send + Sync {
    async fn list_shipping_option_projections(
        &self,
        context: PortContext,
        request: ListShippingOptionProjectionsRequest,
    ) -> Result<Vec<ShippingOptionResponse>, PortError>;

    async fn list_all_shipping_option_projections(
        &self,
        context: PortContext,
        request: ListAllShippingOptionProjectionsRequest,
    ) -> Result<Vec<ShippingOptionResponse>, PortError>;

    async fn read_shipping_option_projection(
        &self,
        context: PortContext,
        request: ReadShippingOptionProjectionRequest,
    ) -> Result<ShippingOptionResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListShippingOptionProjectionsRequest {
    pub requested_locale: Option<String>,
    pub tenant_default_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListAllShippingOptionProjectionsRequest {
    pub requested_locale: Option<String>,
    pub tenant_default_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadShippingOptionProjectionRequest {
    pub shipping_option_id: Uuid,
    pub requested_locale: Option<String>,
    pub tenant_default_locale: Option<String>,
}

pub struct InProcessShippingOptionReadPort {
    inner: FulfillmentService,
}

impl InProcessShippingOptionReadPort {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            inner: FulfillmentService::new(db),
        }
    }

    pub fn from_service(inner: FulfillmentService) -> Self {
        Self { inner }
    }
}

pub fn in_process_shipping_option_read_port(
    db: sea_orm::DatabaseConnection,
) -> Arc<dyn ShippingOptionReadPort> {
    Arc::new(InProcessShippingOptionReadPort::new(db))
}

#[async_trait]
impl ShippingOptionReadPort for InProcessShippingOptionReadPort {
    async fn list_shipping_option_projections(
        &self,
        context: PortContext,
        request: ListShippingOptionProjectionsRequest,
    ) -> Result<Vec<ShippingOptionResponse>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, "list_shipping_option_projections")?;
        let requested_locale_length = request.requested_locale.as_deref().map(str::len);
        let tenant_default_locale_length = request.tenant_default_locale.as_deref().map(str::len);

        self.inner
            .list_shipping_options(
                tenant_id,
                request.requested_locale.as_deref(),
                request.tenant_default_locale.as_deref(),
            )
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    "list_shipping_option_projections",
                    None,
                    requested_locale_length,
                    tenant_default_locale_length,
                    error,
                )
            })
    }

    async fn list_all_shipping_option_projections(
        &self,
        context: PortContext,
        request: ListAllShippingOptionProjectionsRequest,
    ) -> Result<Vec<ShippingOptionResponse>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, "list_all_shipping_option_projections")?;
        let requested_locale_length = request.requested_locale.as_deref().map(str::len);
        let tenant_default_locale_length = request.tenant_default_locale.as_deref().map(str::len);

        self.inner
            .list_all_shipping_options(
                tenant_id,
                request.requested_locale.as_deref(),
                request.tenant_default_locale.as_deref(),
            )
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    "list_all_shipping_option_projections",
                    None,
                    requested_locale_length,
                    tenant_default_locale_length,
                    error,
                )
            })
    }

    async fn read_shipping_option_projection(
        &self,
        context: PortContext,
        request: ReadShippingOptionProjectionRequest,
    ) -> Result<ShippingOptionResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, "read_shipping_option_projection")?;
        let requested_locale_length = request.requested_locale.as_deref().map(str::len);
        let tenant_default_locale_length = request.tenant_default_locale.as_deref().map(str::len);

        self.inner
            .get_shipping_option(
                tenant_id,
                request.shipping_option_id,
                request.requested_locale.as_deref(),
                request.tenant_default_locale.as_deref(),
            )
            .await
            .map_err(|error| {
                map_owner_error(
                    &context,
                    "read_shipping_option_projection",
                    Some(request.shipping_option_id),
                    requested_locale_length,
                    tenant_default_locale_length,
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
            boundary = "fulfillment_shipping_option_read_port",
            "fulfillment shipping-option read context is invalid"
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
    shipping_option_id: Option<Uuid>,
    requested_locale_length: Option<usize>,
    tenant_default_locale_length: Option<usize>,
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
            shipping_option_id = ?shipping_option_id,
            requested_locale_length = ?requested_locale_length,
            tenant_default_locale_length = ?tenant_default_locale_length,
            error_kind,
            code,
            retryable,
            boundary = "fulfillment_shipping_option_read_port",
            "fulfillment shipping-option read failed"
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
            shipping_option_id = ?shipping_option_id,
            requested_locale_length = ?requested_locale_length,
            tenant_default_locale_length = ?tenant_default_locale_length,
            error_kind,
            code,
            retryable,
            boundary = "fulfillment_shipping_option_read_port",
            "fulfillment shipping-option read was rejected"
        );
    }

    PortError::new(kind, code, message, retryable)
}
