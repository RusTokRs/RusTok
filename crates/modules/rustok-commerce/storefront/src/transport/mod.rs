mod aggregate_error_safety;
mod checkout_completion_command_error_safety;
mod graphql_adapter;
mod native_server_adapter;
mod payment_collection_command_error_safety;
mod shared_adapter;
mod shipping_option_command_error_safety;

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
use rustok_ui_transport::{UiTransportPath, execute_selected_transport};
use shared_adapter::ApiError;

pub async fn fetch_storefront_commerce(
    request: FetchCommerceRequest,
) -> Result<StorefrontCommerceData, ApiError> {
    let error_context = aggregate_error_safety::AggregateFetchErrorContext::new(&request);
    let native_request = request.clone();
    execute_selected_transport(
        "commerce",
        selected_transport_path(),
        move || native_server_adapter::fetch_storefront_commerce(native_request),
        move || graphql_adapter::fetch_storefront_commerce(request),
    )
    .await
    .map_err(|error| error_context.map_error(error))
}

pub async fn create_storefront_payment_collection(
    request: PaymentCollectionCommandRequest,
) -> Result<StorefrontCheckoutPaymentCollection, ApiError> {
    let error_context =
        payment_collection_command_error_safety::PaymentCollectionCommandErrorContext::new(
            &request,
        );
    create_payment_collection(request)
        .await
        .map_err(|error| error_context.map_error(error))
}

#[allow(dead_code)]
pub async fn select_storefront_shipping_option(
    request: SelectShippingOptionRequest,
) -> Result<(), ApiError> {
    let error_context =
        shipping_option_command_error_safety::ShippingOptionCommandErrorContext::new(&request);
    select_shipping_option(request.owner_request)
        .await
        .map_err(|error| error_context.map_error(error))
}

pub async fn complete_storefront_checkout(
    request: CheckoutCompletionCommandRequest,
) -> Result<StorefrontCheckoutCompletion, ApiError> {
    let error_context =
        checkout_completion_command_error_safety::CheckoutCompletionCommandErrorContext::new(
            &request,
        );
    complete_checkout(request)
        .await
        .map_err(|error| error_context.map_error(error))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }
}
