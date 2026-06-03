use crate::api::{self, ApiError};
use crate::model::{
    StorefrontCheckoutCart, StorefrontCheckoutCompletion, StorefrontCheckoutPaymentCollection,
    StorefrontCommerceData,
};

pub async fn fetch_storefront_commerce(
    cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
    api::fetch_storefront_commerce(cart_id, locale).await
}

pub async fn create_storefront_payment_collection(
    cart_id: String,
) -> Result<StorefrontCheckoutPaymentCollection, ApiError> {
    api::create_storefront_payment_collection(cart_id).await
}

pub async fn select_storefront_shipping_option(
    cart: StorefrontCheckoutCart,
    shipping_profile_slug: String,
    seller_id: Option<String>,
    seller_scope: Option<String>,
    shipping_option_id: Option<String>,
) -> Result<(), ApiError> {
    api::select_storefront_shipping_option(
        cart,
        shipping_profile_slug,
        seller_id,
        seller_scope,
        shipping_option_id,
    )
    .await
}

pub async fn complete_storefront_checkout(
    cart_id: String,
) -> Result<StorefrontCheckoutCompletion, ApiError> {
    api::complete_storefront_checkout(cart_id).await
}
