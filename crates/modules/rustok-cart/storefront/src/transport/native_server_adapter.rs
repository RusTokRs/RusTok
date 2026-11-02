//! One native server-function contract for SSR and browser clients. SSR handlers live in the
//! server-only module; client builds generate authenticated requests from these same signatures.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::core::CartCoreError;
use crate::model::StorefrontCartData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
    Validation(String),
}
impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) | Self::ServerFn(error) | Self::Validation(error) => {
                write!(f, "{error}")
            }
        }
    }
}
impl std::error::Error for ApiError {}
impl From<CartCoreError> for ApiError {
    fn from(value: CartCoreError) -> Self {
        match value {
            CartCoreError::Validation(error) => Self::Validation(error),
        }
    }
}
impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_cart(
    request: crate::core::CartFetchRequest,
) -> Result<StorefrontCartData, ApiError> {
    storefront_cart_native(request.selected_cart_id, request.locale)
        .await
        .map_err(ApiError::from)
}
pub async fn decrement_line_item(
    request: crate::core::CartLineItemDecrementRequest,
) -> Result<(), ApiError> {
    storefront_cart_decrement_line_item(request.cart_id, request.line_item_id)
        .await
        .map_err(ApiError::from)
}
pub async fn remove_line_item(
    request: crate::core::CartLineItemMutationRequest,
) -> Result<(), ApiError> {
    storefront_cart_remove_line_item(request.cart_id, request.line_item_id)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "cart/storefront-data", client = leptos_auth::AuthorizedBrowserClient)]
async fn storefront_cart_native(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ServerFnError> {
    super::native_server_adapter_ssr::storefront_cart_native(selected_cart_id, locale).await
}
#[server(prefix = "/api/fn", endpoint = "cart/decrement-line-item", client = leptos_auth::AuthorizedBrowserClient)]
async fn storefront_cart_decrement_line_item(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ServerFnError> {
    super::native_server_adapter_ssr::storefront_cart_decrement_line_item(cart_id, line_item_id)
        .await
}
#[server(prefix = "/api/fn", endpoint = "cart/remove-line-item", client = leptos_auth::AuthorizedBrowserClient)]
async fn storefront_cart_remove_line_item(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ServerFnError> {
    super::native_server_adapter_ssr::storefront_cart_remove_line_item(cart_id, line_item_id).await
}
