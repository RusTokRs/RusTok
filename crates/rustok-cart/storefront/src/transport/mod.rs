mod graphql_adapter;
mod native_server_adapter;

use crate::api::ApiError;
use crate::core::{CartFetchRequest, CartLineItemDecrementRequest, CartLineItemMutationRequest};
use crate::model::StorefrontCartData;

fn should_try_graphql_fallback(error: &ApiError) -> bool {
    !matches!(error, ApiError::Validation(_))
}

pub async fn fetch_cart(request: CartFetchRequest) -> Result<StorefrontCartData, ApiError> {
    match native_server_adapter::fetch_cart(request.clone()).await {
        Ok(data) => Ok(data),
        Err(err) if should_try_graphql_fallback(&err) => graphql_adapter::fetch_cart(request).await,
        Err(err) => Err(err),
    }
}

pub async fn decrement_line_item(request: CartLineItemDecrementRequest) -> Result<(), ApiError> {
    match native_server_adapter::decrement_line_item(request.clone()).await {
        Ok(()) => Ok(()),
        Err(err) if should_try_graphql_fallback(&err) => {
            graphql_adapter::decrement_line_item(request).await
        }
        Err(err) => Err(err),
    }
}

pub async fn remove_line_item(request: CartLineItemMutationRequest) -> Result<(), ApiError> {
    match native_server_adapter::remove_line_item(request.clone()).await {
        Ok(()) => Ok(()),
        Err(err) if should_try_graphql_fallback(&err) => {
            graphql_adapter::remove_line_item(request).await
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_errors_are_not_fallback_candidates() {
        let error = ApiError::Validation("cart_id must be a valid UUID".to_string());

        assert!(!should_try_graphql_fallback(&error));
    }

    #[test]
    fn server_and_graphql_errors_remain_fallback_candidates() {
        assert!(should_try_graphql_fallback(&ApiError::ServerFn(
            "server function unavailable".to_string(),
        )));
        assert!(should_try_graphql_fallback(&ApiError::Graphql(
            "network fallback candidate".to_string(),
        )));
    }
}
