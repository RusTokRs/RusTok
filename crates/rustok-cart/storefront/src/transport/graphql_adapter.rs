use crate::api::{self, ApiError};
use crate::core::{
    CartFetchRequest, CartLineItemDecrementRequest, CartLineItemMutationRequest,
    CartLineItemQuantityCommand,
};
use crate::model::StorefrontCartData;

pub async fn fetch_cart(request: CartFetchRequest) -> Result<StorefrontCartData, ApiError> {
    api::fetch_storefront_cart_graphql(request.selected_cart_id, request.locale).await
}

pub async fn decrement_line_item(request: CartLineItemDecrementRequest) -> Result<(), ApiError> {
    match request.command {
        CartLineItemQuantityCommand::Remove => {
            api::remove_storefront_cart_line_item_graphql(request.cart_id, request.line_item_id)
                .await
        }
        CartLineItemQuantityCommand::Update { next_quantity } => {
            api::update_storefront_cart_line_item_quantity_graphql(
                request.cart_id,
                request.line_item_id,
                next_quantity,
            )
            .await
        }
    }
}

pub async fn remove_line_item(request: CartLineItemMutationRequest) -> Result<(), ApiError> {
    api::remove_storefront_cart_line_item_graphql(request.cart_id, request.line_item_id).await
}
