use crate::api::{self, ApiError};
use crate::core::{CartFetchRequest, CartLineItemDecrementRequest, CartLineItemMutationRequest};
use crate::model::StorefrontCartData;

pub async fn fetch_cart(request: CartFetchRequest) -> Result<StorefrontCartData, ApiError> {
    api::fetch_storefront_cart_server(request.selected_cart_id, request.locale).await
}

pub async fn decrement_line_item(request: CartLineItemDecrementRequest) -> Result<(), ApiError> {
    api::decrement_storefront_cart_line_item_server(request.cart_id, request.line_item_id).await
}

pub async fn remove_line_item(request: CartLineItemMutationRequest) -> Result<(), ApiError> {
    api::remove_storefront_cart_line_item_server(request.cart_id, request.line_item_id).await
}
