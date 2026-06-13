use crate::api::{self, ApiError};
use crate::core::{
    CheckoutCompletionCommandRequest, FetchCommerceRequest, PaymentCollectionCommandRequest,
    SelectShippingOptionRequest,
};
use crate::model::{
    StorefrontCheckoutCompletion, StorefrontCheckoutPaymentCollection, StorefrontCommerceData,
};

pub async fn fetch_storefront_commerce(
    request: FetchCommerceRequest,
) -> Result<StorefrontCommerceData, ApiError> {
    api::fetch_storefront_commerce_server(request.selected_cart_id, request.locale).await
}

pub async fn create_storefront_payment_collection(
    request: PaymentCollectionCommandRequest,
) -> Result<StorefrontCheckoutPaymentCollection, ApiError> {
    api::create_storefront_payment_collection_server(request.cart_id).await
}

#[allow(dead_code)]
pub async fn select_storefront_shipping_option(
    request: SelectShippingOptionRequest,
) -> Result<(), ApiError> {
    api::select_storefront_shipping_option_server(
        request.cart.id,
        request.shipping_profile_slug,
        request.seller_id,
        request.seller_scope,
        request.shipping_option_id,
    )
    .await
}

pub async fn complete_storefront_checkout(
    request: CheckoutCompletionCommandRequest,
) -> Result<StorefrontCheckoutCompletion, ApiError> {
    api::complete_storefront_checkout_server(request.cart_id).await
}
