use crate::api::{self, ApiError};
use crate::model::StorefrontCartData;

pub async fn fetch_cart(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ApiError> {
    api::fetch_storefront_cart_server(selected_cart_id, locale).await
}

pub async fn decrement_line_item(cart_id: String, line_item_id: String) -> Result<(), ApiError> {
    api::decrement_storefront_cart_line_item_server(cart_id, line_item_id).await
}

pub async fn remove_line_item(cart_id: String, line_item_id: String) -> Result<(), ApiError> {
    api::remove_storefront_cart_line_item_server(cart_id, line_item_id).await
}
