mod graphql_adapter;
mod native_server_adapter;

use std::fmt::{Display, Formatter};

use rustok_ui_core::normalize_required_ui_text;
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentCollectionCommandMetadata {
    pub source_module: String,
    pub source_surface: String,
    pub command: String,
    pub owner_module: String,
}

impl PaymentCollectionCommandMetadata {
    pub fn storefront_create() -> Self {
        Self {
            source_module: "rustok-commerce".into(),
            source_surface: "storefront_checkout_workspace".into(),
            command: "create_or_reuse_payment_collection".into(),
            owner_module: "rustok-payment".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentCollectionCreateRequest {
    pub cart_id: String,
    pub metadata: PaymentCollectionCommandMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentCollectionFetchRequest {
    pub cart_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefundSummaryFetchRequest {
    pub order_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefundSummary {
    pub total: u64,
    pub refunded_amount: Option<String>,
    pub latest_status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentCollection {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentTransportError {
    Graphql(String),
    ServerFn(String),
    Validation(String),
}

impl PaymentTransportError {
    pub fn message(&self) -> &str {
        match self {
            Self::Graphql(message) | Self::ServerFn(message) | Self::Validation(message) => message,
        }
    }
}

impl Display for PaymentTransportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for PaymentTransportError {}

pub type PaymentFacadeError = UiTransportError;

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

pub async fn create_payment_collection(
    request: PaymentCollectionCreateRequest,
) -> Result<PaymentCollection, PaymentFacadeError> {
    let native_request = request.clone();
    execute_selected_transport(
        "payment",
        selected_transport_path(),
        move || native_server_adapter::create_payment_collection(native_request),
        move || graphql_adapter::create_payment_collection(request),
    )
    .await
}

pub async fn fetch_payment_collection(
    request: PaymentCollectionFetchRequest,
) -> Result<Option<PaymentCollection>, PaymentFacadeError> {
    let native_request = request.clone();
    execute_selected_transport(
        "payment",
        selected_transport_path(),
        move || native_server_adapter::fetch_payment_collection(native_request),
        move || graphql_adapter::fetch_payment_collection(request),
    )
    .await
}

pub async fn fetch_refund_summary(
    request: RefundSummaryFetchRequest,
) -> Result<RefundSummary, PaymentFacadeError> {
    let native_request = request.clone();
    execute_selected_transport(
        "payment",
        selected_transport_path(),
        move || native_server_adapter::fetch_refund_summary(native_request),
        move || graphql_adapter::fetch_refund_summary(request),
    )
    .await
}

pub fn build_payment_collection_create_request(cart_id: String) -> PaymentCollectionCreateRequest {
    PaymentCollectionCreateRequest {
        cart_id: normalize_required_ui_text(cart_id),
        metadata: PaymentCollectionCommandMetadata::storefront_create(),
    }
}

pub fn build_payment_collection_fetch_request(cart_id: String) -> PaymentCollectionFetchRequest {
    PaymentCollectionFetchRequest {
        cart_id: normalize_required_ui_text(cart_id),
    }
}

pub fn build_refund_summary_fetch_request(order_id: String) -> RefundSummaryFetchRequest {
    RefundSummaryFetchRequest {
        order_id: normalize_required_ui_text(order_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_request_trims_cart_id() {
        let request = build_payment_collection_create_request(" cart-1 ".into());
        assert_eq!(request.cart_id, "cart-1");
    }

    #[test]
    fn fetch_request_trims_cart_id() {
        let request = build_payment_collection_fetch_request(" cart-1 ".into());
        assert_eq!(request.cart_id, "cart-1");
    }

    #[test]
    fn refund_summary_request_trims_order_id() {
        let request = build_refund_summary_fetch_request(" order-1 ".into());
        assert_eq!(request.order_id, "order-1");
    }

    #[test]
    fn create_request_carries_payment_owned_command_metadata() {
        let request = build_payment_collection_create_request("cart-1".into());
        assert_eq!(request.metadata.owner_module, "rustok-payment");
        assert_eq!(
            request.metadata.command,
            "create_or_reuse_payment_collection"
        );
    }

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }
}
