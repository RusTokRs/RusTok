use crate::model::StorefrontDeliveryGroup;
use rustok_ui_core::{normalize_optional_ui_text, normalize_required_ui_text};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectShippingOptionRequest {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub shipping_option_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShippingSelectionLabels {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub empty: String,
    pub group_label: String,
    pub line_items_label: String,
    pub provider_label: String,
    pub selected_label: String,
    pub select_label: String,
    pub pending_label: String,
    pub no_selection_label: String,
}

pub fn build_select_shipping_option_request(
    group: &StorefrontDeliveryGroup,
    shipping_option_id: Option<String>,
) -> SelectShippingOptionRequest {
    SelectShippingOptionRequest {
        shipping_profile_slug: normalize_required_ui_text(group.shipping_profile_slug.clone()),
        seller_id: normalize_optional_ui_text(group.seller_id.clone()),
        shipping_option_id: normalize_optional_ui_text(shipping_option_id),
    }
}

pub fn format_shipping_option_price(amount: &str, currency_code: &str) -> String {
    let amount = amount.trim();
    let currency_code = currency_code.trim().to_ascii_uppercase();

    match (amount.is_empty(), currency_code.is_empty()) {
        (true, true) => String::new(),
        (true, false) => currency_code,
        (false, true) => amount.to_string(),
        (false, false) => format!("{amount} {currency_code}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::StorefrontDeliveryGroup;

    #[test]
    fn select_request_normalizes_group_and_option_values() {
        let group = StorefrontDeliveryGroup {
            shipping_profile_slug: " default ".into(),
            seller_id: Some(" seller-1 ".into()),
            line_item_count: 2,
            selected_shipping_option_id: None,
            available_shipping_options: Vec::new(),
        };

        let request = build_select_shipping_option_request(&group, Some(" option-1 ".into()));

        assert_eq!(request.shipping_profile_slug, "default");
        assert_eq!(request.seller_id.as_deref(), Some("seller-1"));
        assert_eq!(request.shipping_option_id.as_deref(), Some("option-1"));
    }

    #[test]
    fn price_formatter_handles_missing_parts() {
        assert_eq!(format_shipping_option_price("10.00", "usd"), "10.00 USD");
        assert_eq!(format_shipping_option_price("", "usd"), "USD");
        assert_eq!(format_shipping_option_price("10.00", ""), "10.00");
    }
}
