use crate::model::StorefrontCheckoutCart;
use rustok_fulfillment_storefront::transport::{
    build_select_shipping_option_request as build_fulfillment_select_shipping_option_request,
    SelectShippingOptionRequest as FulfillmentSelectShippingOptionRequest,
    ShippingSelectionDeliveryGroup,
};
use rustok_order_storefront::transport::CompleteCheckoutRequest;
use rustok_payment_storefront::transport::PaymentCollectionCreateRequest;
use rustok_ui_core::{normalize_optional_ui_text, normalize_required_ui_text};

pub const SELECTED_CART_QUERY_KEY: &str = "cart_id";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommerceStorefrontRouteState {
    pub selected_cart_id: Option<String>,
    pub selected_cart_query_key: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchCommerceRequest {
    pub selected_cart_id: Option<String>,
    pub locale: Option<String>,
}

pub type PaymentCollectionCommandRequest = PaymentCollectionCreateRequest;
pub type CheckoutCompletionCommandRequest = CompleteCheckoutRequest;

#[derive(Clone, Debug)]
pub struct SelectShippingOptionRequest {
    pub owner_request: FulfillmentSelectShippingOptionRequest,
}

pub fn build_storefront_route_state(
    selected_cart_id: Option<String>,
) -> CommerceStorefrontRouteState {
    CommerceStorefrontRouteState {
        selected_cart_id: normalize_optional_ui_text(selected_cart_id),
        selected_cart_query_key: SELECTED_CART_QUERY_KEY,
    }
}

pub fn build_fetch_commerce_request(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> FetchCommerceRequest {
    FetchCommerceRequest {
        selected_cart_id: normalize_optional_ui_text(selected_cart_id),
        locale: normalize_optional_ui_text(locale),
    }
}

pub fn build_select_shipping_option_request(
    cart: StorefrontCheckoutCart,
    shipping_profile_slug: String,
    seller_id: Option<String>,
    shipping_option_id: Option<String>,
) -> SelectShippingOptionRequest {
    let shipping_profile_slug = normalize_required_ui_text(shipping_profile_slug);
    let seller_id = normalize_optional_ui_text(seller_id);
    let shipping_option_id = normalize_optional_ui_text(shipping_option_id);
    let owner_request = build_fulfillment_select_shipping_option_request(
        cart.id,
        cart.delivery_groups
            .into_iter()
            .map(|group| ShippingSelectionDeliveryGroup {
                shipping_profile_slug: group.shipping_profile_slug,
                seller_id: group.seller_id,
                selected_shipping_option_id: group.selected_shipping_option_id,
                available_shipping_option_ids: group
                    .available_shipping_options
                    .into_iter()
                    .map(|option| option.id)
                    .collect(),
            })
            .collect(),
        shipping_profile_slug,
        seller_id,
        shipping_option_id,
    );

    SelectShippingOptionRequest { owner_request }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::StorefrontCheckoutShippingOption;

    #[test]
    fn route_state_normalizes_blank_cart_id() {
        let state = build_storefront_route_state(Some("  ".to_string()));
        assert_eq!(state.selected_cart_id, None);
        assert_eq!(state.selected_cart_query_key, SELECTED_CART_QUERY_KEY);
    }

    #[test]
    fn route_state_trims_cart_id() {
        let state = build_storefront_route_state(Some(" cart-1 ".to_string()));
        assert_eq!(state.selected_cart_id.as_deref(), Some("cart-1"));
    }

    #[test]
    fn fetch_request_normalizes_route_context_inputs() {
        let request = build_fetch_commerce_request(Some(" cart-1 ".into()), Some(" ru ".into()));
        assert_eq!(request.selected_cart_id.as_deref(), Some("cart-1"));
        assert_eq!(request.locale.as_deref(), Some("ru"));
    }

    #[test]
    fn select_shipping_option_request_normalizes_and_maps_cart() {
        let cart = StorefrontCheckoutCart {
            id: " cart-123 ".into(),
            status: "active".into(),
            currency_code: "USD".into(),
            subtotal_amount: "50.00".into(),
            adjustment_total: "0.00".into(),
            shipping_total: "5.00".into(),
            total_amount: "55.00".into(),
            channel_slug: Some("default".into()),
            email: None,
            customer_id: None,
            region_id: None,
            country_code: None,
            locale_code: None,
            selected_shipping_option_id: Some("opt-1".into()),
            line_item_count: 1,
            adjustment_count: 0,
            delivery_group_count: 1,
            adjustments: vec![],
            delivery_groups: vec![crate::model::StorefrontCheckoutDeliveryGroup {
                shipping_profile_slug: "standard".into(),
                seller_id: Some("seller-1".into()),
                line_item_count: 1,
                selected_shipping_option_id: Some("opt-1".into()),
                available_shipping_options: vec![StorefrontCheckoutShippingOption {
                    id: "opt-1".into(),
                    name: "Ground".into(),
                    currency_code: "USD".into(),
                    amount: "5.00".into(),
                    provider_id: "prov-1".into(),
                    active: true,
                }],
            }],
        };

        let request = build_select_shipping_option_request(
            cart,
            " standard ".into(),
            Some(" seller-1 ".into()),
            Some(" opt-1 ".into()),
        );

        assert_eq!(request.owner_request.cart_id, "cart-123");
        assert_eq!(request.owner_request.shipping_profile_slug, "standard");
        assert_eq!(request.owner_request.seller_id.as_deref(), Some("seller-1"));
        assert_eq!(
            request.owner_request.shipping_option_id.as_deref(),
            Some("opt-1")
        );
        assert_eq!(request.owner_request.delivery_groups.len(), 1);
        assert_eq!(
            request.owner_request.delivery_groups[0].available_shipping_option_ids,
            vec!["opt-1".to_string()]
        );
    }
}
