use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCommerceData {
    pub effective_locale: String,
    pub tenant_slug: Option<String>,
    pub tenant_default_locale: String,
    pub channel_slug: Option<String>,
    pub channel_resolution_source: Option<String>,
    pub selected_cart_id: Option<String>,
    pub checkout: Option<StorefrontCheckoutWorkspace>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCheckoutWorkspace {
    pub cart: Option<StorefrontCheckoutCart>,
    pub payment_collection: Option<StorefrontCheckoutPaymentCollection>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCheckoutCart {
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
    pub selected_shipping_option_id: Option<String>,
    pub line_item_count: u64,
    pub adjustment_count: u64,
    pub delivery_group_count: u64,
    pub adjustments: Vec<StorefrontCheckoutAdjustment>,
    pub delivery_groups: Vec<StorefrontCheckoutDeliveryGroup>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCheckoutAdjustment {
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
pub struct StorefrontCheckoutDeliveryGroup {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub line_item_count: u64,
    pub selected_shipping_option_id: Option<String>,
    pub available_shipping_options: Vec<StorefrontCheckoutShippingOption>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCheckoutShippingOption {
    pub id: String,
    pub name: String,
    pub currency_code: String,
    pub amount: String,
    pub provider_id: String,
    pub active: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCheckoutPaymentCollection {
    pub id: String,
    pub status: String,
    pub currency_code: String,
    pub amount: String,
    pub authorized_amount: String,
    pub captured_amount: String,
    pub order_id: Option<String>,
    pub provider_id: Option<String>,
    pub payment_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCheckoutCompletion {
    pub order_id: String,
    pub order_status: String,
    pub currency_code: String,
    pub shipping_total: String,
    pub adjustment_total: String,
    pub total_amount: String,
    pub adjustments: Vec<StorefrontCheckoutAdjustment>,
    pub payment_collection_id: String,
    pub payment_collection_status: String,
    pub fulfillment_count: u64,
    pub context_locale: String,
    pub context_currency_code: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontOrderRefundSummary {
    pub total: u64,
    pub refunded_amount: Option<String>,
    pub latest_status: Option<String>,
}
