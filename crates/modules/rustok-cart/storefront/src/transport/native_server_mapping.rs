use serde_json::Value;

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
        channel_id: value.channel_id.map(|id| id.to_string()),
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
                fulfillment_requirement: item.fulfillment_requirement.as_str().to_string(),
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
