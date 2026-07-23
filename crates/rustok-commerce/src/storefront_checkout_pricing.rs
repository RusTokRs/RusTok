use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_cart::{
    AtomicCartCheckoutPricingResolver, CartCheckoutLineItemPricingUpdate, CartCheckoutPricingPlan,
    CartPricingAdjustmentUpdate, CartResponse, PrepareCartCheckoutSnapshotRequest,
};
use rustok_outbox::TransactionalEventBus;
use rustok_pricing::{
    ResolveProductPriceRequest, ResolvedProductPriceSnapshot, in_process_pricing_read_port,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

/// Request-scoped storefront pricing resolver used by the durable checkout
/// binding after the checkout operation lease has been acquired.
#[derive(Clone)]
pub(crate) struct StorefrontCheckoutPricingResolver {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl StorefrontCheckoutPricingResolver {
    pub(crate) fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        _request_channel_id: Option<Uuid>,
        _request_channel_slug: Option<String>,
    ) -> Self {
        Self { db, event_bus }
    }
}

#[async_trait]
impl AtomicCartCheckoutPricingResolver for StorefrontCheckoutPricingResolver {
    async fn resolve_checkout_pricing(
        &self,
        tenant_id: Uuid,
        cart: &CartResponse,
        request: &PrepareCartCheckoutSnapshotRequest,
    ) -> Result<CartCheckoutPricingPlan, PortError> {
        let pricing_read_port =
            in_process_pricing_read_port(self.db.clone(), self.event_bus.clone());
        let effective_region_id = cart.region_id.or(request.input.region_id);
        let cart_channel_slug = normalize_channel_slug(cart.channel_slug.as_deref());
        let currency_code = cart.currency_code.trim().to_ascii_uppercase();
        let mut line_items = Vec::new();

        for line_item in &cart.line_items {
            let Some(variant_id) = line_item.variant_id else {
                continue;
            };
            let resolved_price = pricing_read_port
                .resolve_product_price(
                    checkout_pricing_port_context(tenant_id, cart, line_item.id),
                    ResolveProductPriceRequest {
                        product_id: line_item.product_id,
                        variant_id,
                        region_id: effective_region_id,
                        channel_id: cart.channel_id,
                        channel_slug: cart_channel_slug.clone(),
                        price_list_id: None,
                        quantity: Some(line_item.quantity),
                        currency_code: currency_code.clone(),
                    },
                )
                .await
                .map_err(|_| {
                    PortError::new(
                        PortErrorKind::Unavailable,
                        "pricing.checkout_resolution_unavailable",
                        "checkout pricing is temporarily unavailable",
                        true,
                    )
                })?;
            if !resolved_price
                .currency_code
                .eq_ignore_ascii_case(currency_code.as_str())
            {
                return Err(PortError::invariant_violation(
                    "pricing.checkout_currency_mismatch",
                    "checkout pricing returned an incompatible currency",
                ));
            }

            line_items.push(checkout_line_item_pricing_update(
                line_item.id,
                variant_id,
                line_item.quantity,
                &resolved_price,
            ));
        }

        Ok(CartCheckoutPricingPlan {
            currency_code,
            effective_region_id,
            cart_channel_id: cart.channel_id,
            cart_channel_slug,
            line_items,
        })
    }
}

fn checkout_pricing_port_context(
    tenant_id: Uuid,
    cart: &CartResponse,
    line_item_id: Uuid,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.checkout-pricing"),
        cart.locale_code.as_deref().unwrap_or("en"),
        format!("checkout-pricing:{}:{line_item_id}", cart.id),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    cart.channel_slug
        .as_deref()
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context)
}

fn checkout_line_item_pricing_update(
    line_item_id: Uuid,
    variant_id: Uuid,
    quantity: i32,
    resolved_price: &ResolvedProductPriceSnapshot,
) -> CartCheckoutLineItemPricingUpdate {
    let base_unit_price = resolved_price
        .compare_at_amount
        .filter(|compare_at| *compare_at > resolved_price.amount)
        .unwrap_or(resolved_price.amount);
    let pricing_adjustment = if base_unit_price > resolved_price.amount {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "kind".to_string(),
            Value::from(if resolved_price.price_list_id.is_some() {
                "price_list"
            } else {
                "sale"
            }),
        );
        metadata.insert(
            "base_amount".to_string(),
            Value::from(base_unit_price.normalize().to_string()),
        );
        metadata.insert(
            "effective_amount".to_string(),
            Value::from(resolved_price.amount.normalize().to_string()),
        );
        if let Some(compare_at_amount) = resolved_price.compare_at_amount {
            metadata.insert(
                "compare_at_amount".to_string(),
                Value::from(compare_at_amount.normalize().to_string()),
            );
        }
        if let Some(discount_percent) = resolved_price.discount_percent {
            metadata.insert(
                "discount_percent".to_string(),
                Value::from(discount_percent.normalize().to_string()),
            );
        }
        if let Some(price_list_id) = resolved_price.price_list_id {
            metadata.insert(
                "price_list_id".to_string(),
                Value::from(price_list_id.to_string()),
            );
        }
        if let Some(channel_id) = resolved_price.channel_id {
            metadata.insert(
                "channel_id".to_string(),
                Value::from(channel_id.to_string()),
            );
        }
        if let Some(channel_slug) = resolved_price.channel_slug.as_deref() {
            metadata.insert("channel_slug".to_string(), Value::from(channel_slug));
        }

        Some(CartPricingAdjustmentUpdate {
            source_id: resolved_price.price_list_id.map(|value| value.to_string()),
            amount: (base_unit_price - resolved_price.amount) * Decimal::from(quantity),
            metadata: Value::Object(metadata),
        })
    } else {
        None
    };

    CartCheckoutLineItemPricingUpdate {
        line_item_id,
        variant_id,
        quantity,
        unit_price: base_unit_price,
        pricing_adjustment,
    }
}

fn normalize_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}
