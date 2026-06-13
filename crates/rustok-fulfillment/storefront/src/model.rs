use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StorefrontShippingOption {
    pub id: String,
    pub name: String,
    pub currency_code: String,
    pub amount: String,
    pub provider_id: String,
    pub active: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StorefrontDeliveryGroup {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub line_item_count: u64,
    pub selected_shipping_option_id: Option<String>,
    pub available_shipping_options: Vec<StorefrontShippingOption>,
}
