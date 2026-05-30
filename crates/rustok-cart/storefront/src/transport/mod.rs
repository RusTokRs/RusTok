mod graphql_adapter;
mod native_server_adapter;

use crate::api::ApiError;
use crate::model::StorefrontCartData;

pub async fn fetch_cart(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ApiError> {
    match native_server_adapter::fetch_cart(selected_cart_id.clone(), locale.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => graphql_adapter::fetch_cart(selected_cart_id, locale).await,
    }
}

pub async fn decrement_line_item(
    cart_id: String,
    line_item_id: String,
    current_quantity: i32,
) -> Result<(), ApiError> {
    match native_server_adapter::decrement_line_item(cart_id.clone(), line_item_id.clone()).await {
        Ok(()) => Ok(()),
        Err(_) => {
            graphql_adapter::decrement_line_item(cart_id, line_item_id, current_quantity).await
        }
    }
}

pub async fn remove_line_item(cart_id: String, line_item_id: String) -> Result<(), ApiError> {
    match native_server_adapter::remove_line_item(cart_id.clone(), line_item_id.clone()).await {
        Ok(()) => Ok(()),
        Err(_) => graphql_adapter::remove_line_item(cart_id, line_item_id).await,
    }
}
