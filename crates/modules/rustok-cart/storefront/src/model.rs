use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCartData {
    pub selected_cart_id: Option<String>,
    pub cart: Option<StorefrontCart>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCart {
    pub id: String,
    pub status: String,
    pub currency_code: String,
    pub subtotal_amount: String,
    pub adjustment_total: String,
    pub shipping_total: String,
    pub total_amount: String,
    pub channel_slug: Option<String>,
    pub email: Option<String>,
    pub customer_id: Option<String>,
    pub region_id: Option<String>,
    pub country_code: Option<String>,
    pub locale_code: Option<String>,
    pub line_items: Vec<StorefrontCartLineItem>,
    pub adjustments: Vec<StorefrontCartAdjustment>,
    pub delivery_groups: Vec<StorefrontCartDeliveryGroup>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCartLineItem {
    pub id: String,
    pub title: String,
    pub sku: Option<String>,
    pub quantity: i32,
    pub unit_price: String,
    pub total_price: String,
    pub currency_code: String,
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCartAdjustment {
    pub id: String,
    pub line_item_id: Option<String>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub scope: Option<String>,
    pub amount: String,
    pub currency_code: String,
    pub metadata: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCartDeliveryGroup {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub line_item_count: u64,
    pub selected_shipping_option_id: Option<String>,
    pub available_shipping_options: Vec<StorefrontCartShippingOption>,
    pub available_option_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCartShippingOption {
    pub id: String,
    pub name: String,
    pub currency_code: String,
    pub amount: String,
    pub provider_id: String,
    pub active: bool,
}
