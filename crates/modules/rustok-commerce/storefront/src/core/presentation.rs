use crate::i18n::t;
use crate::model::{
    StorefrontCheckoutCompletion, StorefrontCheckoutDeliveryGroup,
    StorefrontCheckoutPaymentCollection, StorefrontCommerceData,
};
use rustok_cart_storefront::core::CartCheckoutHandoffLabels;
use rustok_fulfillment_storefront::core::ShippingSelectionLabels;
use rustok_fulfillment_storefront::{
    StorefrontDeliveryGroup as FulfillmentDeliveryGroup,
    StorefrontShippingOption as FulfillmentShippingOption,
};
use rustok_order_storefront::core::{
    OrderCheckoutActionLabels, OrderCheckoutResultData, OrderCheckoutResultLabels,
};
use rustok_payment_storefront::core::{
    PaymentCollectionActionLabels, PaymentCollectionCardData, PaymentCollectionCardLabels,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommerceStorefrontShellViewModel {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub load_error: String,
    pub action_error: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommerceStorefrontContextViewModel {
    pub effective_locale: String,
    pub tenant: String,
    pub tenant_default_locale: String,
    pub channel: String,
    pub channel_resolution_source: String,
}

pub fn build_storefront_shell_view_model(locale: Option<&str>) -> CommerceStorefrontShellViewModel {
    CommerceStorefrontShellViewModel {
        badge: t(locale, "commerce.badge", "commerce"),
        title: t(locale, "commerce.title", "Commerce orchestration hub"),
        subtitle: t(
            locale,
            "commerce.subtitle",
            "Catalog, pricing, regions, and cart line-item handling now live in module-owned storefront packages. Commerce remains the aggregate storefront handoff for checkout context and cross-domain flow.",
        ),
        load_error: t(
            locale,
            "commerce.error.load",
            "Failed to load commerce storefront data",
        ),
        action_error: t(
            locale,
            "commerce.error.action",
            "Failed to update aggregate checkout state",
        ),
    }
}

pub fn build_storefront_context_view_model(
    data: StorefrontCommerceData,
    locale: Option<&str>,
) -> CommerceStorefrontContextViewModel {
    let empty_value = t(locale, "commerce.context.empty", "not resolved");

    CommerceStorefrontContextViewModel {
        effective_locale: data.effective_locale,
        tenant: data
            .tenant_slug
            .unwrap_or_else(|| t(locale, "commerce.context.tenantMissing", "host tenant")),
        tenant_default_locale: data.tenant_default_locale,
        channel: data.channel_slug.unwrap_or_else(|| empty_value.clone()),
        channel_resolution_source: data
            .channel_resolution_source
            .unwrap_or_else(|| empty_value.clone()),
    }
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

pub fn build_fulfillment_delivery_groups(
    groups: Vec<StorefrontCheckoutDeliveryGroup>,
) -> Vec<FulfillmentDeliveryGroup> {
    groups
        .into_iter()
        .map(|group| FulfillmentDeliveryGroup {
            shipping_profile_slug: group.shipping_profile_slug,
            seller_id: group.seller_id,
            line_item_count: group.line_item_count,
            selected_shipping_option_id: group.selected_shipping_option_id,
            available_shipping_options: group
                .available_shipping_options
                .into_iter()
                .map(|option| FulfillmentShippingOption {
                    id: option.id,
                    name: option.name,
                    currency_code: option.currency_code,
                    amount: option.amount,
                    provider_id: option.provider_id,
                    active: option.active,
                })
                .collect(),
        })
        .collect()
}

pub fn build_fulfillment_shipping_selection_labels(
    locale: Option<&str>,
) -> ShippingSelectionLabels {
    ShippingSelectionLabels {
        badge: t(locale, "commerce.delivery.badge", "delivery"),
        title: t(locale, "commerce.delivery.title", "Shipping selection"),
        subtitle: t(
            locale,
            "commerce.delivery.subtitle",
            "Select fulfillment-owned shipping options for each seller-aware delivery group.",
        ),
        empty: t(
            locale,
            "commerce.delivery.empty",
            "No delivery groups are available for this cart yet.",
        ),
        group_label: t(locale, "commerce.delivery.group", "Delivery group"),
        line_items_label: t(locale, "commerce.delivery.lineItems", "line items"),
        provider_label: t(locale, "commerce.delivery.provider", "Provider"),
        selected_label: t(locale, "commerce.delivery.selected", "Selected"),
        select_label: t(locale, "commerce.delivery.select", "Select"),
        pending_label: t(locale, "commerce.checkout.pending", "Processing..."),
        no_selection_label: t(
            locale,
            "commerce.delivery.noSelection",
            "No shipping option",
        ),
    }
}

pub fn build_cart_checkout_handoff_labels(locale: Option<&str>) -> CartCheckoutHandoffLabels {
    CartCheckoutHandoffLabels {
        cart_label: t(locale, "commerce.checkout.cart.id", "Cart"),
        status_label: t(locale, "commerce.checkout.cart.status", "Cart status"),
        module_ownership: t(
            locale,
            "commerce.checkout.cart.moduleOwnership",
            "Cart totals, line items and adjustments stay in the cart module workspace.",
        ),
    }
}

pub fn build_payment_collection_action_labels(
    locale: Option<&str>,
) -> PaymentCollectionActionLabels {
    PaymentCollectionActionLabels {
        pending: t(locale, "commerce.checkout.pending", "Processing..."),
        create_or_reuse: t(
            locale,
            "commerce.checkout.createCollection",
            "Create or reuse payment collection",
        ),
    }
}

pub fn build_order_checkout_action_labels(locale: Option<&str>) -> OrderCheckoutActionLabels {
    OrderCheckoutActionLabels {
        pending: t(locale, "commerce.checkout.pending", "Processing..."),
        complete: t(locale, "commerce.checkout.complete", "Complete checkout"),
    }
}

pub fn build_payment_collection_card_data(
    payment_collection: StorefrontCheckoutPaymentCollection,
) -> PaymentCollectionCardData {
    PaymentCollectionCardData {
        id: payment_collection.id,
        status: payment_collection.status,
    }
}

pub fn build_payment_collection_card_labels(locale: Option<&str>) -> PaymentCollectionCardLabels {
    PaymentCollectionCardLabels {
        badge: t(locale, "commerce.payment.badge", "payment collection"),
        module_ownership: t(
            locale,
            "commerce.payment.moduleOwnership",
            "Payment collection details stay in payment-owned UI; commerce only shows checkout orchestration handoff state.",
        ),
        empty_id: t(locale, "commerce.payment.emptyId", "not attached"),
        empty_status: t(locale, "commerce.payment.emptyStatus", "pending"),
    }
}

pub fn build_order_checkout_result_data(
    result: StorefrontCheckoutCompletion,
) -> OrderCheckoutResultData {
    OrderCheckoutResultData {
        order_id: result.order_id,
        order_status: result.order_status,
    }
}

pub fn build_order_checkout_result_labels(locale: Option<&str>) -> OrderCheckoutResultLabels {
    OrderCheckoutResultLabels {
        badge: t(locale, "commerce.checkout.result.badge", "checkout result"),
        module_ownership: t(
            locale,
            "commerce.checkout.result.moduleOwnership",
            "Order, payment, fulfillment and adjustment details remain in their module-owned workspaces; commerce shows only the aggregate checkout outcome.",
        ),
        order_status_label: t(
            locale,
            "commerce.checkout.result.orderStatus",
            "Order status",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storefront_data(
        tenant_slug: Option<&str>,
        channel_slug: Option<&str>,
        channel_resolution_source: Option<&str>,
    ) -> StorefrontCommerceData {
        StorefrontCommerceData {
            effective_locale: "ru".to_string(),
            tenant_slug: tenant_slug.map(str::to_string),
            tenant_default_locale: "en".to_string(),
            channel_slug: channel_slug.map(str::to_string),
            channel_resolution_source: channel_resolution_source.map(str::to_string),
            selected_cart_id: None,
            checkout: None,
        }
    }

    #[test]
    fn context_view_model_preserves_resolved_context() {
        let view_model = build_storefront_context_view_model(
            storefront_data(Some("main"), Some("web"), Some("domain")),
            Some("en"),
        );

        assert_eq!(view_model.effective_locale, "ru");
        assert_eq!(view_model.tenant, "main");
        assert_eq!(view_model.tenant_default_locale, "en");
        assert_eq!(view_model.channel, "web");
        assert_eq!(view_model.channel_resolution_source, "domain");
    }

    #[test]
    fn context_view_model_applies_missing_context_fallbacks() {
        let view_model =
            build_storefront_context_view_model(storefront_data(None, None, None), Some("en"));

        assert_eq!(view_model.tenant, "host tenant");
        assert_eq!(view_model.channel, "not resolved");
        assert_eq!(view_model.channel_resolution_source, "not resolved");
    }

    #[test]
    fn fulfillment_delivery_groups_drop_legacy_seller_scope() {
        let groups = build_fulfillment_delivery_groups(vec![StorefrontCheckoutDeliveryGroup {
            shipping_profile_slug: "default".into(),
            seller_id: Some("seller-1".into()),
            line_item_count: 1,
            selected_shipping_option_id: Some("ship-1".into()),
            available_shipping_options: Vec::new(),
        }]);

        assert_eq!(groups[0].shipping_profile_slug, "default");
        assert_eq!(groups[0].seller_id.as_deref(), Some("seller-1"));
    }
}
