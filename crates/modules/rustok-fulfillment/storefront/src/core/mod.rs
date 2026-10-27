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

impl ShippingSelectionLabels {
    pub fn resolve(locale: Option<&str>) -> Self {
        Self {
            badge: crate::i18n::t(locale, "fulfillment.shipping.badge", "Shipping"),
            title: crate::i18n::t(locale, "fulfillment.shipping.title", "Delivery options"),
            subtitle: crate::i18n::t(
                locale,
                "fulfillment.shipping.subtitle",
                "Choose shipping options for each delivery group.",
            ),
            empty: crate::i18n::t(
                locale,
                "fulfillment.shipping.empty",
                "No delivery groups are available for this cart.",
            ),
            group_label: crate::i18n::t(locale, "fulfillment.shipping.group", "Delivery group"),
            line_items_label: crate::i18n::t(
                locale,
                "fulfillment.shipping.lineItems",
                "line items",
            ),
            provider_label: crate::i18n::t(locale, "fulfillment.shipping.provider", "Provider"),
            selected_label: crate::i18n::t(locale, "fulfillment.shipping.selected", "Selected"),
            select_label: crate::i18n::t(locale, "fulfillment.shipping.select", "Select"),
            pending_label: crate::i18n::t(locale, "fulfillment.shipping.pending", "Processing..."),
            no_selection_label: crate::i18n::t(
                locale,
                "fulfillment.shipping.noSelection",
                "No shipping option",
            ),
        }
    }
}

impl SelectShippingOptionRequest {
    pub fn is_valid(&self) -> bool {
        !self.shipping_profile_slug.trim().is_empty()
    }
}

pub fn delivery_group_key(group: &StorefrontDeliveryGroup) -> String {
    delivery_group_key_raw(&group.shipping_profile_slug, group.seller_id.as_deref())
}

pub fn delivery_group_key_raw(shipping_profile_slug: &str, seller_id: Option<&str>) -> String {
    match seller_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(seller) => format!("{}::{seller}", shipping_profile_slug.trim()),
        None => format!("{}::none", shipping_profile_slug.trim()),
    }
}

pub fn format_line_items_label(line_items_label: &str, count: u64) -> String {
    format!("{line_items_label}: {count}")
}

pub fn format_provider_label(provider_label: &str, provider_id: &str) -> String {
    format!("{provider_label}: {}", provider_id.trim())
}

pub fn is_shipping_option_selected(selected_id: Option<&str>, option_id: &str) -> bool {
    let option_id = option_id.trim();
    if option_id.is_empty() {
        return false;
    }
    selected_id.map(str::trim).filter(|s| !s.is_empty()) == Some(option_id)
}

pub fn selection_action_label(is_selected: bool, labels: &ShippingSelectionLabels) -> &str {
    if is_selected {
        &labels.selected_label
    } else {
        &labels.select_label
    }
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
        assert!(request.is_valid());
    }

    #[test]
    fn select_request_validity_requires_non_empty_slug() {
        let valid_request = SelectShippingOptionRequest {
            shipping_profile_slug: "standard".into(),
            seller_id: None,
            shipping_option_id: None,
        };
        assert!(valid_request.is_valid());

        let invalid_request = SelectShippingOptionRequest {
            shipping_profile_slug: "   ".into(),
            seller_id: None,
            shipping_option_id: None,
        };
        assert!(!invalid_request.is_valid());
    }

    #[test]
    fn price_formatter_handles_missing_parts() {
        assert_eq!(format_shipping_option_price("10.00", "usd"), "10.00 USD");
        assert_eq!(format_shipping_option_price("", "usd"), "USD");
        assert_eq!(format_shipping_option_price("10.00", ""), "10.00");
        assert_eq!(format_shipping_option_price("   ", "   "), "");
    }

    #[test]
    fn delivery_group_key_formats_with_or_without_seller() {
        let with_seller = StorefrontDeliveryGroup {
            shipping_profile_slug: " express ".into(),
            seller_id: Some(" seller-42 ".into()),
            line_item_count: 3,
            selected_shipping_option_id: None,
            available_shipping_options: Vec::new(),
        };
        assert_eq!(delivery_group_key(&with_seller), "express::seller-42");

        let without_seller = StorefrontDeliveryGroup {
            shipping_profile_slug: "express".into(),
            seller_id: None,
            line_item_count: 1,
            selected_shipping_option_id: None,
            available_shipping_options: Vec::new(),
        };
        assert_eq!(delivery_group_key(&without_seller), "express::none");

        let blank_seller = StorefrontDeliveryGroup {
            shipping_profile_slug: "express".into(),
            seller_id: Some("   ".into()),
            line_item_count: 1,
            selected_shipping_option_id: None,
            available_shipping_options: Vec::new(),
        };
        assert_eq!(delivery_group_key(&blank_seller), "express::none");
    }

    #[test]
    fn formatting_and_selection_helpers_work_consistently() {
        assert_eq!(format_line_items_label("line items", 5), "line items: 5");
        assert_eq!(
            format_provider_label("Carrier", " fedex "),
            "Carrier: fedex"
        );

        assert!(is_shipping_option_selected(Some(" opt-1 "), "opt-1"));
        assert!(!is_shipping_option_selected(Some("opt-1"), "opt-2"));
        assert!(!is_shipping_option_selected(None, "opt-1"));
        assert!(!is_shipping_option_selected(Some("   "), "   "));
        assert!(!is_shipping_option_selected(Some(""), ""));
        assert!(!is_shipping_option_selected(Some("opt-1"), "   "));

        let labels = ShippingSelectionLabels {
            badge: "Badge".into(),
            title: "Title".into(),
            subtitle: "Subtitle".into(),
            empty: "Empty".into(),
            group_label: "Group".into(),
            line_items_label: "Items".into(),
            provider_label: "Provider".into(),
            selected_label: "Selected".into(),
            select_label: "Select".into(),
            pending_label: "Pending".into(),
            no_selection_label: "None".into(),
        };

        assert_eq!(selection_action_label(true, &labels), "Selected");
        assert_eq!(selection_action_label(false, &labels), "Select");
    }

    #[test]
    fn labels_resolve_supports_english_and_russian() {
        let en_labels = ShippingSelectionLabels::resolve(Some("en"));
        assert_eq!(en_labels.badge, "Shipping");
        assert_eq!(en_labels.title, "Delivery options");

        let ru_labels = ShippingSelectionLabels::resolve(Some("ru"));
        assert_eq!(ru_labels.badge, "Доставка");
        assert_eq!(ru_labels.title, "Варианты доставки");
    }
}
