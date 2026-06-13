use crate::api::{self, ApiError};
use crate::core::{CartCommandRequest, FetchCommerceRequest};
use crate::model::{
    StorefrontCheckoutCompletion, StorefrontCheckoutPaymentCollection, StorefrontCommerceData,
};

pub async fn fetch_storefront_commerce(
    request: FetchCommerceRequest,
) -> Result<StorefrontCommerceData, ApiError> {
    api::fetch_storefront_commerce_server(request.selected_cart_id, request.locale).await
}

pub async fn create_storefront_payment_collection(
    request: CartCommandRequest,
) -> Result<StorefrontCheckoutPaymentCollection, ApiError> {
    api::create_storefront_payment_collection_server(request.cart_id).await
}

pub async fn complete_storefront_checkout(
    request: CartCommandRequest,
) -> Result<StorefrontCheckoutCompletion, ApiError> {
    api::complete_storefront_checkout_server(request.cart_id).await
}
