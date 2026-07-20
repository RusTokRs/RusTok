use std::{sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_marketplace_allocation::{
    AllocateMarketplaceOrderLineInput, AllocateMarketplaceOrderLinesInput,
    AllocateMarketplaceOrderLinesResponse, MarketplaceAllocationCommandPort,
};
use rustok_order::OrderResponse;
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use super::CheckoutOrderPlanPayload;

const ALLOCATION_DEADLINE: Duration = Duration::from_secs(5);
const MARKETPLACE_METADATA_KEY: &str = "marketplace";

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
        let request = build_request(order, operation_id)?;
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
    order
        .line_items
        .iter()
        .any(|line| line.metadata.get(MARKETPLACE_METADATA_KEY).is_some())
}

fn build_request(
    order: &OrderResponse,
    operation_id: Uuid,
) -> CheckoutMarketplaceAllocationResult<Option<AllocateMarketplaceOrderLinesInput>> {
    let mut lines = Vec::new();
    for line in &order.line_items {
        let Some(raw) = line.metadata.get(MARKETPLACE_METADATA_KEY) else {
            continue;
        };
        let snapshot: MarketplaceCheckoutLineSnapshot = serde_json::from_value(raw.clone())
            .map_err(|error| {
                CheckoutMarketplaceAllocationError::Validation(format!(
                    "order line {} has malformed marketplace metadata: {error}",
                    line.id
                ))
            })?;
        let product_id = line.product_id.ok_or_else(|| {
            CheckoutMarketplaceAllocationError::Validation(format!(
                "marketplace order line {} is missing product_id",
                line.id
            ))
        })?;
        let variant_id = line.variant_id.ok_or_else(|| {
            CheckoutMarketplaceAllocationError::Validation(format!(
                "marketplace order line {} is missing variant_id",
                line.id
            ))
        })?;
        lines.push(AllocateMarketplaceOrderLineInput {
            order_line_item_id: line.id,
            seller_id: snapshot.seller_id,
            listing_id: snapshot.listing_id,
            master_product_id: product_id,
            master_variant_id: variant_id,
            quantity: i64::from(line.quantity),
            unit_amount: snapshot.unit_amount,
            subtotal_amount: snapshot.subtotal_amount,
            discount_amount: snapshot.discount_amount,
            tax_amount: snapshot.tax_amount,
            total_amount: snapshot.total_amount,
            listing_terms_version: snapshot.listing_terms_version,
            pricing_reference: snapshot.pricing_reference,
            inventory_reference: snapshot.inventory_reference,
            fulfillment_profile_slug: snapshot
                .fulfillment_profile_slug
                .or_else(|| Some(line.shipping_profile_slug.clone())),
            metadata: serde_json::json!({
                "source": "checkout",
                "checkout_operation_id": operation_id,
                "cart_line_item_id": line
                    .metadata
                    .pointer("/checkout/cart_line_item_id")
                    .cloned()
                    .unwrap_or(Value::Null),
            }),
        });
    }
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(AllocateMarketplaceOrderLinesInput {
        order_id: order.id,
        currency_code: order.currency_code.clone(),
        lines,
    }))
}

#[derive(Debug, Deserialize)]
struct MarketplaceCheckoutLineSnapshot {
    seller_id: Uuid,
    listing_id: Uuid,
    listing_terms_version: i32,
    unit_amount: i64,
    subtotal_amount: i64,
    #[serde(default)]
    discount_amount: i64,
    #[serde(default)]
    tax_amount: i64,
    total_amount: i64,
    pricing_reference: Option<String>,
    inventory_reference: Option<String>,
    fulfillment_profile_slug: Option<String>,
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
