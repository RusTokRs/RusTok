use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::OrderResponse;

/// Transport-neutral owner boundary for checkout completion/result reads.
#[async_trait]
pub trait CheckoutCompletionPort: Send + Sync {
    async fn complete_checkout(
        &self,
        context: PortContext,
        request: CompleteCheckoutPortRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError>;

    async fn read_checkout_result(
        &self,
        context: PortContext,
        request: CheckoutResultRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError>;

    async fn read_order_status(
        &self,
        context: PortContext,
        request: OrderStatusRequest,
    ) -> Result<OrderStatusSnapshot, PortError>;
}

#[async_trait]
impl CheckoutCompletionPort for crate::OrderService {
    async fn complete_checkout(
        &self,
        context: PortContext,
        request: CompleteCheckoutPortRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let actor_id = parse_port_actor_id(&context)?;
        let CompleteCheckoutPortRequest {
            cart_id: _,
            customer_id,
            payment_collection_id,
            shipping_option_id: _,
            channel_id,
            channel_slug,
            locale,
            fallback_locale,
            currency_code,
            shipping_total,
            line_items,
            adjustments,
            tax_lines,
            metadata,
        } = request;
        let mut response = self
            .create_order_with_channel(
                tenant_id,
                actor_id,
                crate::CreateOrderInput {
                    customer_id,
                    currency_code,
                    shipping_total,
                    line_items,
                    adjustments,
                    tax_lines,
                    metadata,
                },
                channel_id,
                channel_slug,
            )
            .await
            .map_err(order_error_to_port_error)?;
        response = self
            .confirm_order(tenant_id, actor_id, response.id)
            .await
            .map_err(order_error_to_port_error)?;
        if let Some(locale) = locale.as_deref() {
            response = self
                .get_order_with_locale_fallback(
                    tenant_id,
                    response.id,
                    locale,
                    fallback_locale.as_deref(),
                )
                .await
                .map_err(order_error_to_port_error)?;
        }
        Ok(CheckoutCompletionSnapshot::from_response(
            &response,
            payment_collection_id,
        ))
    }

    async fn read_checkout_result(
        &self,
        context: PortContext,
        request: CheckoutResultRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let _tenant_id = parse_port_tenant_id(&context)?;
        let _cart_id = request.cart_id;
        Err(PortError::unavailable(
            "order.checkout_result_projection_unavailable",
            "checkout result lookup by cart id is not exposed by the current order storage projection",
        ))
    }

    async fn read_order_status(
        &self,
        context: PortContext,
        request: OrderStatusRequest,
    ) -> Result<OrderStatusSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let response = self
            .get_order(tenant_id, request.order_id)
            .await
            .map_err(order_error_to_port_error)?;
        Ok(OrderStatusSnapshot::from_response(&response))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteCheckoutPortRequest {
    pub cart_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub payment_collection_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub currency_code: String,
    pub shipping_total: Decimal,
    pub line_items: Vec<crate::CreateOrderLineItemInput>,
    pub adjustments: Vec<crate::CreateOrderAdjustmentInput>,
    pub tax_lines: Vec<crate::CreateOrderTaxLineInput>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckoutResultRequest {
    pub cart_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderStatusRequest {
    pub order_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckoutCompletionSnapshot {
    pub order_id: Uuid,
    pub status: String,
    pub currency_code: String,
    pub total: Decimal,
    pub payment_collection_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderStatusSnapshot {
    pub order_id: Uuid,
    pub status: String,
    pub paid: bool,
    pub shipped: bool,
    pub delivered: bool,
    pub total_amount: Decimal,
}

impl OrderStatusSnapshot {
    pub fn from_response(response: &OrderResponse) -> Self {
        Self {
            order_id: response.id,
            status: response.status.clone(),
            paid: response.paid_at.is_some(),
            shipped: response.shipped_at.is_some(),
            delivered: response.delivered_at.is_some(),
            total_amount: response.total_amount,
        }
    }
}

impl CheckoutCompletionSnapshot {
    pub fn from_response(response: &OrderResponse, payment_collection_id: Option<Uuid>) -> Self {
        Self {
            order_id: response.id,
            status: response.status.clone(),
            currency_code: response.currency_code.clone(),
            total: response.total_amount,
            payment_collection_id,
        }
    }
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "order.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for order ports",
        )
    })
}

fn parse_port_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.actor.id).map_err(|_| {
        PortError::validation(
            "order.actor_id_invalid",
            "PortContext.actor.id must be a UUID for order write ports",
        )
    })
}

fn order_error_to_port_error(error: crate::OrderError) -> PortError {
    match error {
        crate::OrderError::Database(error) => PortError::unavailable(
            "order.database_unavailable",
            format!("order storage unavailable: {error}"),
        ),
        crate::OrderError::OrderNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "order.order_not_found",
            format!("order {id} not found"),
            false,
        ),
        crate::OrderError::Validation(message) => {
            PortError::validation("order.validation", message)
        }
        other => PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "order.invariant_violation",
            other.to_string(),
            false,
        ),
    }
}
