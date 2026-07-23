use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Transport-neutral owner boundary for checkout shipping selection.
#[async_trait]
pub trait ShippingSelectionPort: Send + Sync {
    async fn list_seller_shipping_options(
        &self,
        context: PortContext,
        request: ListSellerShippingOptionsRequest,
    ) -> Result<SellerShippingOptionsSnapshot, PortError>;

    async fn select_shipping_option(
        &self,
        context: PortContext,
        request: SelectShippingOptionPortRequest,
    ) -> Result<SelectedShippingOptionSnapshot, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListSellerShippingOptionsRequest {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub shipping_profile_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectShippingOptionPortRequest {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub shipping_option_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SellerShippingOptionsSnapshot {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub options: Vec<ShippingOptionProjection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShippingOptionProjection {
    pub id: Uuid,
    pub provider_id: String,
    pub name: String,
    pub currency_code: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedShippingOptionSnapshot {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub option: ShippingOptionProjection,
}

#[async_trait]
impl ShippingSelectionPort for crate::FulfillmentService {
    async fn list_seller_shipping_options(
        &self,
        context: PortContext,
        request: ListSellerShippingOptionsRequest,
    ) -> Result<SellerShippingOptionsSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, "list_seller_shipping_options")?;
        let options = self
            .list_shipping_options(tenant_id, Some(&context.locale), Some(&context.locale))
            .await
            .map_err(|error| {
                fulfillment_error_to_port_error(
                    &context,
                    "list_seller_shipping_options",
                    error,
                )
            })?
            .into_iter()
            .filter(|option| {
                request
                    .shipping_profile_slug
                    .as_deref()
                    .map(|profile| {
                        option
                            .allowed_shipping_profile_slugs
                            .as_ref()
                            .map(|profiles| profiles.iter().any(|item| item == profile))
                            .unwrap_or(true)
                    })
                    .unwrap_or(true)
            })
            .map(ShippingOptionProjection::from_response)
            .collect();

        Ok(SellerShippingOptionsSnapshot {
            cart_id: request.cart_id,
            seller_id: request.seller_id,
            options,
        })
    }

    async fn select_shipping_option(
        &self,
        context: PortContext,
        request: SelectShippingOptionPortRequest,
    ) -> Result<SelectedShippingOptionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, "select_shipping_option")?;
        let option = self
            .get_shipping_option(
                tenant_id,
                request.shipping_option_id,
                Some(&context.locale),
                Some(&context.locale),
            )
            .await
            .map_err(|error| {
                fulfillment_error_to_port_error(&context, "select_shipping_option", error)
            })?;

        Ok(SelectedShippingOptionSnapshot {
            cart_id: request.cart_id,
            seller_id: request.seller_id,
            option: ShippingOptionProjection::from_response(option),
        })
    }
}

impl ShippingOptionProjection {
    pub fn from_response(response: crate::ShippingOptionResponse) -> Self {
        Self {
            id: response.id,
            provider_id: response.provider_id,
            name: response.name,
            currency_code: response.currency_code,
            amount: response.amount,
        }
    }
}

fn parse_port_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "fulfillment.context_invalid",
            "fulfillment port request context is invalid"
        );
        PortError::validation(
            "fulfillment.context_invalid",
            "fulfillment request context is invalid",
        )
    })
}

fn fulfillment_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: crate::FulfillmentError,
) -> PortError {
    match error {
        crate::FulfillmentError::Validation(message) => {
            tracing::warn!(
                error = %message,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "fulfillment.validation",
                "fulfillment owner validation failed"
            );
            PortError::validation("fulfillment.validation", "fulfillment request is invalid")
        }
        crate::FulfillmentError::ShippingOptionNotFound(id) => {
            tracing::warn!(
                resource_id = %id,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "fulfillment.shipping_option_not_found",
                "fulfillment shipping option was not found"
            );
            PortError::new(
                rustok_api::PortErrorKind::NotFound,
                "fulfillment.shipping_option_not_found",
                "shipping option was not found",
                false,
            )
        }
        crate::FulfillmentError::FulfillmentNotFound(id) => {
            tracing::warn!(
                resource_id = %id,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "fulfillment.fulfillment_not_found",
                "fulfillment resource was not found"
            );
            PortError::new(
                rustok_api::PortErrorKind::NotFound,
                "fulfillment.fulfillment_not_found",
                "fulfillment was not found",
                false,
            )
        }
        crate::FulfillmentError::InvalidTransition { from, to } => {
            tracing::warn!(
                from = %from,
                to = %to,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "fulfillment.invalid_transition",
                "fulfillment lifecycle transition was rejected"
            );
            PortError::conflict(
                "fulfillment.invalid_transition",
                "fulfillment lifecycle transition conflicts with the current state",
            )
        }
        crate::FulfillmentError::Database(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "fulfillment.database_unavailable",
                "fulfillment storage operation failed"
            );
            PortError::unavailable(
                "fulfillment.database_unavailable",
                "fulfillment storage is temporarily unavailable",
            )
        }
    }
}
