mod graphql_adapter;
mod native_server_adapter;

use crate::api::ApiError;
use crate::core::{
    CheckoutCompletionCommandRequest, FetchCommerceRequest, PaymentCollectionCommandRequest,
    SelectShippingOptionRequest,
};
use crate::model::{
    StorefrontCheckoutCompletion, StorefrontCheckoutPaymentCollection, StorefrontCommerceData,
};
use rustok_fulfillment_storefront::transport::{
    select_shipping_option_with_fallback, ShippingSelectionTransportError,
};

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
    match native_server_adapter::create_storefront_payment_collection(request.clone()).await {
        Ok(collection) => Ok(collection),
        Err(_) => graphql_adapter::create_storefront_payment_collection(request).await,
    }
}

#[allow(dead_code)]
pub async fn select_storefront_shipping_option(
    request: SelectShippingOptionRequest,
) -> Result<(), ApiError> {
    select_shipping_option_with_fallback(
        request.owner_request,
        |owner_request| async move {
            native_server_adapter::select_storefront_shipping_option(SelectShippingOptionRequest {
                owner_request,
            })
            .await
            .map_err(ShippingSelectionTransportError::from)
        },
        |owner_request| async move {
            graphql_adapter::select_storefront_shipping_option(SelectShippingOptionRequest {
                owner_request,
            })
            .await
            .map_err(ShippingSelectionTransportError::from)
        },
    )
    .await
    .map_err(ApiError::from)
}

pub async fn complete_storefront_checkout(
    request: CheckoutCompletionCommandRequest,
) -> Result<StorefrontCheckoutCompletion, ApiError> {
    match native_server_adapter::complete_storefront_checkout(request.clone()).await {
        Ok(completion) => Ok(completion),
        Err(_) => graphql_adapter::complete_storefront_checkout(request).await,
    }
}

fn should_fallback_to_graphql(error: &ApiError) -> bool {
    ShippingSelectionTransportError::from(error.clone()).should_fallback_to_graphql()
}

impl From<ApiError> for ShippingSelectionTransportError {
    fn from(value: ApiError) -> Self {
        match value {
            ApiError::Graphql(message) => Self::Graphql(message),
            ApiError::ServerFn(message) => Self::ServerFn(message),
            ApiError::Validation(message) => Self::Validation(message),
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
    fn validation_and_graphql_errors_do_not_trigger_fetch_fallback() {
        assert!(!should_fallback_to_graphql(&ApiError::Validation(
            "cart_id must be a valid UUID".into()
        )));
        assert!(!should_fallback_to_graphql(&ApiError::Graphql(
            "network unavailable".into()
        )));
    }
}
