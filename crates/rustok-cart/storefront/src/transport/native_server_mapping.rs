use serde_json::Value;
use uuid::Uuid;

use crate::model::{
    StorefrontCart, StorefrontCartAdjustment, StorefrontCartDeliveryGroup, StorefrontCartLineItem,
    StorefrontCartShippingOption,
};

pub(super) fn map_native_cart(value: rustok_cart::CartResponse) -> StorefrontCart {
    StorefrontCart {
        id: value.id.to_string(),
        status: value.status,
        currency_code: value.currency_code,
        subtotal_amount: value.subtotal_amount.normalize().to_string(),
        adjustment_total: value.adjustment_total.normalize().to_string(),
        shipping_total: value.shipping_total.normalize().to_string(),
        total_amount: value.total_amount.normalize().to_string(),
        channel_slug: value.channel_slug,
        email: value.email,
        customer_id: value.customer_id.map(|value| value.to_string()),
        region_id: value.region_id.map(|value| value.to_string()),
        country_code: value.country_code,
        locale_code: value.locale_code,
        line_items: value
            .line_items
            .into_iter()
            .map(|item| StorefrontCartLineItem {
                id: item.id.to_string(),
                title: item.title,
                sku: item.sku,
                quantity: item.quantity,
                unit_price: item.unit_price.normalize().to_string(),
                total_price: item.total_price.normalize().to_string(),
                currency_code: item.currency_code,
                shipping_profile_slug: item.shipping_profile_slug,
                seller_id: item.seller_id,
            })
            .collect(),
        adjustments: value
            .adjustments
            .into_iter()
            .map(|adjustment| StorefrontCartAdjustment {
                id: adjustment.id.to_string(),
                line_item_id: adjustment.line_item_id.map(|value| value.to_string()),
                source_type: adjustment.source_type,
                source_id: adjustment.source_id,
                scope: adjustment
                    .metadata
                    .get("scope")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                amount: adjustment.amount.normalize().to_string(),
                currency_code: adjustment.currency_code,
                metadata: adjustment.metadata.to_string(),
            })
            .collect(),
        delivery_groups: value
            .delivery_groups
            .into_iter()
            .map(|group| StorefrontCartDeliveryGroup {
                shipping_profile_slug: group.shipping_profile_slug,
                seller_id: group.seller_id,
                line_item_count: group.line_item_ids.len() as u64,
                selected_shipping_option_id: group
                    .selected_shipping_option_id
                    .map(|value| value.to_string()),
                available_option_count: group.available_shipping_options.len() as u64,
                available_shipping_options: group
                    .available_shipping_options
                    .into_iter()
                    .map(|option| StorefrontCartShippingOption {
                        id: option.id.to_string(),
                        name: option.name,
                        currency_code: option.currency_code,
                        amount: option.amount.normalize().to_string(),
                        provider_id: option.provider_id,
                        active: option.active,
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub(super) fn storefront_cart_pricing_update(
    line_item_id: Uuid,
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedProductPriceSnapshot,
) -> rustok_cart::services::cart::CartLineItemPricingUpdate {
    let base_unit_price = resolved_price
        .compare_at_amount
        .filter(|compare_at| *compare_at > resolved_price.amount)
        .unwrap_or(resolved_price.amount);
    let pricing_adjustment = if base_unit_price > resolved_price.amount {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "kind".to_string(),
            serde_json::Value::from(if resolved_price.price_list_id.is_some() {
                "price_list"
            } else {
                "sale"
            }),
        );
        metadata.insert(
            "base_amount".to_string(),
            serde_json::Value::from(base_unit_price.normalize().to_string()),
        );
        metadata.insert(
            "effective_amount".to_string(),
            serde_json::Value::from(resolved_price.amount.normalize().to_string()),
        );
        if let Some(compare_at_amount) = resolved_price.compare_at_amount {
            metadata.insert(
                "compare_at_amount".to_string(),
                serde_json::Value::from(compare_at_amount.normalize().to_string()),
            );
        }
        if let Some(discount_percent) = resolved_price.discount_percent {
            metadata.insert(
                "discount_percent".to_string(),
                serde_json::Value::from(discount_percent.normalize().to_string()),
            );
        }
        if let Some(price_list_id) = resolved_price.price_list_id {
            metadata.insert(
                "price_list_id".to_string(),
                serde_json::Value::from(price_list_id.to_string()),
            );
        }
        if let Some(channel_id) = resolved_price.channel_id {
            metadata.insert(
                "channel_id".to_string(),
                serde_json::Value::from(channel_id.to_string()),
            );
        }
        if let Some(channel_slug) = resolved_price.channel_slug.as_deref() {
            metadata.insert(
                "channel_slug".to_string(),
                serde_json::Value::from(channel_slug),
            );
        }

        Some(rustok_cart::services::cart::CartPricingAdjustmentUpdate {
            source_id: resolved_price.price_list_id.map(|value| value.to_string()),
            amount: (base_unit_price - resolved_price.amount)
                * rust_decimal::Decimal::from(quantity),
            metadata: serde_json::Value::Object(metadata),
        })
    } else {
        None
    };

    rustok_cart::services::cart::CartLineItemPricingUpdate {
        line_item_id,
        unit_price: base_unit_price,
        pricing_adjustment,
    }
}
