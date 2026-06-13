#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShippingSelectionDeliveryGroup {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub selected_shipping_option_id: Option<String>,
    pub available_shipping_option_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectShippingOptionRequest {
    pub cart_id: String,
    pub delivery_groups: Vec<ShippingSelectionDeliveryGroup>,
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub shipping_option_id: Option<String>,
}

pub fn build_select_shipping_option_request(
    cart_id: String,
    delivery_groups: Vec<ShippingSelectionDeliveryGroup>,
    shipping_profile_slug: String,
    seller_id: Option<String>,
    seller_scope: Option<String>,
    shipping_option_id: Option<String>,
) -> SelectShippingOptionRequest {
    SelectShippingOptionRequest {
        cart_id: normalize_required(cart_id),
        delivery_groups,
        shipping_profile_slug: normalize_required(shipping_profile_slug),
        seller_id: normalize_optional(seller_id),
        seller_scope: normalize_optional(seller_scope),
        shipping_option_id: normalize_optional(shipping_option_id),
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn normalize_required(value: String) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_request_normalizes_ids() {
        let request = build_select_shipping_option_request(
            " cart-1 ".into(),
            Vec::new(),
            " default ".into(),
            Some(" seller-1 ".into()),
            Some(" ".into()),
            Some(" ship-1 ".into()),
        );

        assert_eq!(request.cart_id, "cart-1");
        assert_eq!(request.shipping_profile_slug, "default");
        assert_eq!(request.seller_id.as_deref(), Some("seller-1"));
        assert_eq!(request.seller_scope, None);
        assert_eq!(request.shipping_option_id.as_deref(), Some("ship-1"));
    }
}
