use crate::model::StorefrontCheckoutCart;
use rustok_fulfillment_storefront::transport::{
    SelectShippingOptionRequest as FulfillmentSelectShippingOptionRequest,
    ShippingSelectionDeliveryGroup,
    build_select_shipping_option_request as build_fulfillment_select_shipping_option_request,
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

#[allow(dead_code)]
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
        cart.id.clone(),
        cart.delivery_groups
            .iter()
            .map(|group| ShippingSelectionDeliveryGroup {
                shipping_profile_slug: group.shipping_profile_slug.clone(),
                seller_id: group.seller_id.clone(),
                selected_shipping_option_id: group.selected_shipping_option_id.clone(),
                available_shipping_option_ids: group
                    .available_shipping_options
                    .iter()
                    .map(|option| option.id.clone())
                    .collect(),
            })
            .collect(),
        shipping_profile_slug.clone(),
        seller_id.clone(),
        shipping_option_id.clone(),
    );

    let _ = cart;
    SelectShippingOptionRequest { owner_request }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
