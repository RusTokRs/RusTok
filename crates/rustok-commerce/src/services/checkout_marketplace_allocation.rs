use std::{sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_marketplace_allocation::{
    AllocateMarketplaceOrderLineInput, AllocateMarketplaceOrderLinesInput,
    AllocateMarketplaceOrderLinesResponse, MarketplaceAllocationCommandPort,
};
use rustok_order::OrderResponse;
use thiserror::Error;
use uuid::Uuid;

use super::CheckoutOrderPlanPayload;

const ALLOCATION_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum CheckoutMarketplaceAllocationError {
    #[error("marketplace allocation snapshot is invalid: {0}")]
    Validation(String),
    #[error("marketplace allocation boundary `{code}` failed: {message}")]
    Boundary {
        code: String,
        message: String,
        retryable: bool,
    },
}

pub type CheckoutMarketplaceAllocationResult<T> =
    Result<T, CheckoutMarketplaceAllocationError>;

pub struct CheckoutMarketplaceAllocationStage {
    allocation_port: Arc<dyn MarketplaceAllocationCommandPort>,
}

impl CheckoutMarketplaceAllocationStage {
    pub fn new(allocation_port: Arc<dyn MarketplaceAllocationCommandPort>) -> Self {
        Self { allocation_port }
    }

    pub async fn allocate_if_present(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        plan: &CheckoutOrderPlanPayload,
        order: &OrderResponse,
    ) -> CheckoutMarketplaceAllocationResult<Option<AllocateMarketplaceOrderLinesResponse>> {
        let request = build_request(plan, order, operation_id)?;
        let Some(request) = request else {
            return Ok(None);
        };
        let mut context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(actor_id.to_string()),
            plan.context.locale.clone(),
            format!("checkout-marketplace-allocation-{operation_id}"),
        )
        .with_deadline(ALLOCATION_DEADLINE)
        .with_idempotency_key(format!(
            "checkout:{operation_id}:marketplace-allocation:v1"
        ));
        if let Some(channel) = plan.channel_slug.clone() {
            context = context.with_channel(channel);
        }
        self.allocation_port
            .allocate_order_lines(context, request)
            .await
            .map(Some)
            .map_err(map_port_error)
    }
}

pub fn order_contains_marketplace_lines(order: &OrderResponse) -> bool {
    order.line_items.iter().any(|line| line.seller_id.is_some())
}

fn build_request(
    plan: &CheckoutOrderPlanPayload,
    order: &OrderResponse,
    operation_id: Uuid,
) -> CheckoutMarketplaceAllocationResult<Option<AllocateMarketplaceOrderLinesInput>> {
    if plan.marketplace_lines.is_empty() {
        return Ok(None);
    }
    if order.line_items.len() != plan.order_input.line_items.len() {
        return Err(CheckoutMarketplaceAllocationError::Validation(format!(
            "created order {} line count does not match immutable checkout plan",
            order.id
        )));
    }

    let mut lines = Vec::with_capacity(plan.marketplace_lines.len());
    for planned in &plan.marketplace_lines {
        let order_line = order.line_items.get(planned.order_line_index).ok_or_else(|| {
            CheckoutMarketplaceAllocationError::Validation(format!(
                "marketplace snapshot references missing created order line index {}",
                planned.order_line_index
            ))
        })?;
        let snapshot = &planned.snapshot;
        if order_line.product_id != Some(snapshot.master_product_id)
            || order_line.variant_id != Some(snapshot.master_variant_id)
            || i64::from(order_line.quantity) * snapshot.unit_amount != snapshot.subtotal_amount
        {
            return Err(CheckoutMarketplaceAllocationError::Validation(format!(
                "created order line {} does not match typed marketplace snapshot for cart line {}",
                order_line.id, snapshot.cart_line_item_id
            )));
        }
        let seller_id = order_line
            .seller_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value.trim()).ok())
            .ok_or_else(|| {
                CheckoutMarketplaceAllocationError::Validation(format!(
                    "created marketplace order line {} is missing a UUID seller identity",
                    order_line.id
                ))
            })?;
        if seller_id != snapshot.seller_id {
            return Err(CheckoutMarketplaceAllocationError::Validation(format!(
                "created order line {} seller does not match typed marketplace snapshot",
                order_line.id
            )));
        }

        lines.push(AllocateMarketplaceOrderLineInput {
            order_line_item_id: order_line.id,
            seller_id: snapshot.seller_id,
            listing_id: snapshot.listing_id,
            master_product_id: snapshot.master_product_id,
            master_variant_id: snapshot.master_variant_id,
            quantity: i64::from(order_line.quantity),
            unit_amount: snapshot.unit_amount,
            subtotal_amount: snapshot.subtotal_amount,
            discount_amount: snapshot.discount_amount,
            tax_amount: snapshot.tax_amount,
            total_amount: snapshot.total_amount,
            listing_terms_version: snapshot.listing_terms_version,
            pricing_reference: snapshot.pricing_reference.clone(),
            inventory_reference: snapshot.inventory_reference.clone(),
            fulfillment_profile_slug: snapshot
                .fulfillment_profile_slug
                .clone()
                .or_else(|| Some(order_line.shipping_profile_slug.clone())),
            metadata: serde_json::json!({
                "source": "checkout",
                "checkout_operation_id": operation_id,
                "cart_line_item_id": snapshot.cart_line_item_id,
                "order_line_index": planned.order_line_index,
            }),
        });
    }

    Ok(Some(AllocateMarketplaceOrderLinesInput {
        order_id: order.id,
        currency_code: order.currency_code.clone(),
        lines,
    }))
}

fn map_port_error(error: PortError) -> CheckoutMarketplaceAllocationError {
    let message = match error.kind {
        PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict => {
            error.message
        }
        PortErrorKind::Forbidden => "marketplace allocation permission denied".to_string(),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            "marketplace allocation owner is temporarily unavailable".to_string()
        }
        PortErrorKind::InvariantViolation => {
            "marketplace allocation receipt requires operator review".to_string()
        }
    };
    CheckoutMarketplaceAllocationError::Boundary {
        code: error.code,
        message,
        retryable: error.retryable,
    }
}