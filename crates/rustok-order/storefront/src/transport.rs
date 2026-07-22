mod graphql_adapter;
mod native_server_adapter;

use std::fmt::{Display, Formatter};

use rustok_ui_core::normalize_required_ui_text;
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutCompletionCommandMetadata {
    pub source_module: String,
    pub source_surface: String,
    pub command: String,
    pub owner_module: String,
    pub create_fulfillment: bool,
}

impl CheckoutCompletionCommandMetadata {
    pub fn storefront_complete() -> Self {
        Self {
            source_module: "rustok-commerce".into(),
            source_surface: "storefront_checkout_workspace".into(),
            command: "complete_checkout".into(),
            owner_module: "rustok-order".into(),
            create_fulfillment: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompleteCheckoutRequest {
    pub cart_id: String,
    pub idempotency_key: String,
    pub metadata: CheckoutCompletionCommandMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutAdjustment {
    pub id: String,
    pub line_item_id: Option<String>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub scope: Option<String>,
    pub amount: String,
    pub currency_code: String,
    pub metadata: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutCompletion {
    pub order_id: String,
    pub order_status: String,
    pub currency_code: String,
    pub shipping_total: String,
    pub adjustment_total: String,
    pub total_amount: String,
    pub adjustments: Vec<CheckoutAdjustment>,
    pub payment_collection_id: String,
    pub payment_collection_status: String,
    pub fulfillment_count: u64,
    pub context_locale: String,
    pub context_currency_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckoutCompletionTransportError {
    Graphql(String),
    ServerFn(String),
    Validation(String),
}

impl CheckoutCompletionTransportError {
    pub fn message(&self) -> &str {
        match self {
            Self::Graphql(message) | Self::ServerFn(message) | Self::Validation(message) => message,
        }
    }
}

impl Display for CheckoutCompletionTransportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for CheckoutCompletionTransportError {}

pub async fn complete_checkout(
    request: CompleteCheckoutRequest,
) -> Result<CheckoutCompletion, UiTransportError> {
    let native_request = request.clone();
    execute_selected_transport(
        "order",
        selected_transport_path(),
        move || native_server_adapter::complete_checkout(native_request),
        move || graphql_adapter::complete_checkout(request),
    )
    .await
}

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub fn build_complete_checkout_request(cart_id: String) -> CompleteCheckoutRequest {
    CompleteCheckoutRequest {
        cart_id: normalize_required_ui_text(cart_id),
        idempotency_key: format!("storefront-checkout:{}", uuid::Uuid::new_v4()),
        metadata: CheckoutCompletionCommandMetadata::storefront_complete(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_request_trims_cart_id_and_creates_stable_key() {
        let request = build_complete_checkout_request(" cart-1 ".into());
        let replay = request.clone();
        assert_eq!(request.cart_id, "cart-1");
        assert!(request.idempotency_key.starts_with("storefront-checkout:"));
        assert_eq!(request.idempotency_key, replay.idempotency_key);
    }

    #[test]
    fn complete_request_carries_order_owned_command_metadata() {
        let request = build_complete_checkout_request("cart-1".into());
        assert_eq!(request.metadata.owner_module, "rustok-order");
        assert_eq!(request.metadata.command, "complete_checkout");
        assert!(request.metadata.create_fulfillment);
    }

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }
}
