use crate::api::{self, ApiError};
use crate::core::{CartCommandRequest, FetchCommerceRequest, SelectShippingOptionRequest};
use crate::model::{
    StorefrontCheckoutCompletion, StorefrontCheckoutPaymentCollection, StorefrontCommerceData,
};

pub async fn fetch_storefront_commerce(
    request: FetchCommerceRequest,
) -> Result<StorefrontCommerceData, ApiError> {
    api::fetch_storefront_commerce_graphql(request.selected_cart_id, request.locale).await
}

pub async fn create_storefront_payment_collection(
    request: CartCommandRequest,
) -> Result<StorefrontCheckoutPaymentCollection, ApiError> {
    api::create_storefront_payment_collection_graphql(request.cart_id).await
}

pub async fn select_storefront_shipping_option(
    request: SelectShippingOptionRequest,
) -> Result<(), ApiError> {
    api::select_storefront_shipping_option_graphql(
        request.cart,
        request.shipping_profile_slug,
        request.seller_id,
        request.seller_scope,
        request.shipping_option_id,
    )
    .await
}

pub async fn complete_storefront_checkout(
    request: CartCommandRequest,
) -> Result<StorefrontCheckoutCompletion, ApiError> {
    api::complete_storefront_checkout_graphql(request.cart_id).await
}
