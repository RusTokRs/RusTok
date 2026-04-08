use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderAdminBootstrap {
    #[serde(rename = "currentTenant")]
    pub current_tenant: CurrentTenant,
    pub me: CurrentUser,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderList {
    pub items: Vec<OrderListItem>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderListItem {
    pub id: String,
    #[serde(rename = "customerId")]
    pub customer_id: Option<String>,
    pub status: String,
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    #[serde(rename = "totalAmount")]
    pub total_amount: String,
    #[serde(rename = "trackingNumber")]
    pub tracking_number: Option<String>,
    pub carrier: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "confirmedAt")]
    pub confirmed_at: Option<String>,
    #[serde(rename = "paidAt")]
    pub paid_at: Option<String>,
    #[serde(rename = "shippedAt")]
    pub shipped_at: Option<String>,
    #[serde(rename = "deliveredAt")]
    pub delivered_at: Option<String>,
    #[serde(rename = "cancelledAt")]
    pub cancelled_at: Option<String>,
    #[serde(rename = "lineItems")]
    pub line_items: Vec<OrderLineItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderDetailEnvelope {
    pub order: OrderDetail,
    #[serde(rename = "paymentCollection")]
    pub payment_collection: Option<PaymentCollection>,
    pub fulfillment: Option<Fulfillment>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderDetail {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "channelId")]
    pub channel_id: Option<String>,
    #[serde(rename = "channelSlug")]
    pub channel_slug: Option<String>,
    #[serde(rename = "customerId")]
    pub customer_id: Option<String>,
    pub status: String,
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    #[serde(rename = "totalAmount")]
    pub total_amount: String,
    pub metadata: String,
    #[serde(rename = "paymentId")]
    pub payment_id: Option<String>,
    #[serde(rename = "paymentMethod")]
    pub payment_method: Option<String>,
    #[serde(rename = "trackingNumber")]
    pub tracking_number: Option<String>,
    pub carrier: Option<String>,
    #[serde(rename = "cancellationReason")]
    pub cancellation_reason: Option<String>,
    #[serde(rename = "deliveredSignature")]
    pub delivered_signature: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "confirmedAt")]
    pub confirmed_at: Option<String>,
    #[serde(rename = "paidAt")]
    pub paid_at: Option<String>,
    #[serde(rename = "shippedAt")]
    pub shipped_at: Option<String>,
    #[serde(rename = "deliveredAt")]
    pub delivered_at: Option<String>,
    #[serde(rename = "cancelledAt")]
    pub cancelled_at: Option<String>,
    #[serde(rename = "lineItems")]
    pub line_items: Vec<OrderLineItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderLineItem {
    pub id: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "productId")]
    pub product_id: Option<String>,
    #[serde(rename = "variantId")]
    pub variant_id: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: String,
    pub sku: Option<String>,
    pub title: String,
    pub quantity: i32,
    #[serde(rename = "unitPrice")]
    pub unit_price: String,
    #[serde(rename = "totalPrice")]
    pub total_price: String,
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub metadata: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PaymentCollection {
    pub id: String,
    pub status: String,
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "authorizedAmount")]
    pub authorized_amount: String,
    #[serde(rename = "capturedAmount")]
    pub captured_amount: String,
    #[serde(rename = "providerId")]
    pub provider_id: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "authorizedAt")]
    pub authorized_at: Option<String>,
    #[serde(rename = "capturedAt")]
    pub captured_at: Option<String>,
    #[serde(rename = "cancelledAt")]
    pub cancelled_at: Option<String>,
    pub payments: Vec<Payment>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Payment {
    pub id: String,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    #[serde(rename = "providerPaymentId")]
    pub provider_payment_id: String,
    pub status: String,
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "capturedAmount")]
    pub captured_amount: String,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "authorizedAt")]
    pub authorized_at: Option<String>,
    #[serde(rename = "capturedAt")]
    pub captured_at: Option<String>,
    #[serde(rename = "cancelledAt")]
    pub cancelled_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Fulfillment {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "shippingOptionId")]
    pub shipping_option_id: Option<String>,
    #[serde(rename = "customerId")]
    pub customer_id: Option<String>,
    pub status: String,
    pub carrier: Option<String>,
    #[serde(rename = "trackingNumber")]
    pub tracking_number: Option<String>,
    #[serde(rename = "deliveredNote")]
    pub delivered_note: Option<String>,
    #[serde(rename = "cancellationReason")]
    pub cancellation_reason: Option<String>,
    pub metadata: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "shippedAt")]
    pub shipped_at: Option<String>,
    #[serde(rename = "deliveredAt")]
    pub delivered_at: Option<String>,
    #[serde(rename = "cancelledAt")]
    pub cancelled_at: Option<String>,
}
