use serde::{Deserialize, Serialize};

pub type StorefrontCheckoutAdjustment = rustok_order_storefront::transport::CheckoutAdjustment;
pub type StorefrontCheckoutCompletion = rustok_order_storefront::transport::CheckoutCompletion;
pub type StorefrontCheckoutPaymentCollection =
    rustok_payment_storefront::transport::PaymentCollection;

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
pub struct StorefrontCheckoutDeliveryGroup {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
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
