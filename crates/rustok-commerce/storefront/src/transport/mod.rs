mod graphql_adapter;
mod native_server_adapter;
mod shared_adapter;

use crate::core::{
    CheckoutCompletionCommandRequest, FetchCommerceRequest, PaymentCollectionCommandRequest,
    SelectShippingOptionRequest,
};
use crate::model::{
    StorefrontCheckoutCompletion, StorefrontCheckoutPaymentCollection, StorefrontCommerceData,
};
use rustok_fulfillment_storefront::transport::select_shipping_option;
use rustok_order_storefront::transport::complete_checkout;
use rustok_payment_storefront::transport::create_payment_collection;
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};
use shared_adapter::ApiError;

pub async fn fetch_storefront_commerce(
    request: FetchCommerceRequest,
) -> Result<StorefrontCommerceData, ApiError> {
    let native_request = request.clone();
    execute_selected_transport(
        "commerce",
        selected_transport_path(),
        move || native_server_adapter::fetch_storefront_commerce(native_request),
        move || graphql_adapter::fetch_storefront_commerce(request),
    )
    .await
    .map_err(ApiError::from)
}

pub async fn create_storefront_payment_collection(
    request: PaymentCollectionCommandRequest,
) -> Result<StorefrontCheckoutPaymentCollection, ApiError> {
    create_payment_collection(request)
        .await
        .map_err(ApiError::from)
}

#[allow(dead_code)]
pub async fn select_storefront_shipping_option(
    request: SelectShippingOptionRequest,
) -> Result<(), ApiError> {
    select_shipping_option(request.owner_request)
        .await
        .map_err(ApiError::from)
}

pub async fn complete_storefront_checkout(
    request: CheckoutCompletionCommandRequest,
) -> Result<StorefrontCheckoutCompletion, ApiError> {
    complete_checkout(request).await.map_err(ApiError::from)
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

impl From<UiTransportError> for ApiError {
    fn from(value: UiTransportError) -> Self {
        match value.failed_path {
            UiTransportPath::NativeServer => Self::ServerFn(value.to_string()),
            UiTransportPath::Graphql => Self::Graphql(value.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }
}
