mod graphql_adapter;
mod native_server_adapter;
mod raw_adapter;

use crate::core::{
    CheckoutCompletionCommandRequest, FetchCommerceRequest, PaymentCollectionCommandRequest,
    SelectShippingOptionRequest,
};
use crate::model::{
    StorefrontCheckoutCompletion, StorefrontCheckoutPaymentCollection, StorefrontCommerceData,
};
use raw_adapter::ApiError;
use rustok_fulfillment_storefront::transport::{
    select_shipping_option, ShippingSelectionTransportError,
};
use rustok_order_storefront::transport::{complete_checkout, CheckoutCompletionTransportError};
use rustok_payment_storefront::transport::{create_payment_collection, PaymentTransportError};

pub async fn fetch_storefront_commerce(
    request: FetchCommerceRequest,
) -> Result<StorefrontCommerceData, ApiError> {
    match native_server_adapter::fetch_storefront_commerce(request.clone()).await {
        Ok(data) => Ok(data),
        Err(error) if should_fallback_to_graphql(&error) => {
            graphql_adapter::fetch_storefront_commerce(request).await
        }
        Err(error) => Err(error),
    }
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

fn should_fallback_to_graphql(error: &ApiError) -> bool {
    matches!(
        error,
        ApiError::ServerFn(server_error)
            if server_error.contains("MissingServer")
                || server_error.contains("missing server")
                || server_error.contains("not available on this target")
    )
}

impl From<PaymentTransportError> for ApiError {
    fn from(value: PaymentTransportError) -> Self {
        match value {
            PaymentTransportError::Graphql(message) => Self::Graphql(message),
            PaymentTransportError::ServerFn(message) => Self::ServerFn(message),
            PaymentTransportError::Validation(message) => Self::Validation(message),
        }
    }
}

impl From<CheckoutCompletionTransportError> for ApiError {
    fn from(value: CheckoutCompletionTransportError) -> Self {
        match value {
            CheckoutCompletionTransportError::Graphql(message) => Self::Graphql(message),
            CheckoutCompletionTransportError::ServerFn(message) => Self::ServerFn(message),
            CheckoutCompletionTransportError::Validation(message) => Self::Validation(message),
        }
    }
}

impl From<ShippingSelectionTransportError> for ApiError {
    fn from(value: ShippingSelectionTransportError) -> Self {
        match value {
            ShippingSelectionTransportError::Graphql(message) => Self::Graphql(message),
            ShippingSelectionTransportError::ServerFn(message) => Self::ServerFn(message),
            ShippingSelectionTransportError::Validation(message) => Self::Validation(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_missing_server_errors_can_fallback_to_graphql() {
        assert!(should_fallback_to_graphql(&ApiError::ServerFn(
            "MissingServerFunction".into()
        )));
        assert!(should_fallback_to_graphql(&ApiError::ServerFn(
            "server function is not available on this target".into()
        )));
    }

    #[test]
    fn validation_and_graphql_errors_do_not_trigger_compatibility_fallback() {
        assert!(!should_fallback_to_graphql(&ApiError::Validation(
            "cart_id must be a valid UUID".into()
        )));
        assert!(!should_fallback_to_graphql(&ApiError::Graphql(
            "network unavailable".into()
        )));
    }
}
