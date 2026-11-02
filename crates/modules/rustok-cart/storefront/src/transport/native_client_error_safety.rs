use uuid::Uuid;

use crate::core::{CartFetchRequest, CartLineItemDecrementRequest, CartLineItemMutationRequest};

use super::native_server_adapter::ApiError;

const CART_STOREFRONT_NATIVE_CLIENT_OWNER: &str = "rustok_cart.storefront";
const CART_STOREFRONT_NATIVE_CLIENT_BOUNDARY: &str = "cart_storefront_native_client_transport";
const CART_STOREFRONT_NATIVE_CLIENT_PUBLIC_MESSAGE: &str =
    "Cart storefront request could not be completed";

pub(super) struct NativeClientErrorContext {
    operation: &'static str,
    correlation_id: String,
    selected_cart_id_length: Option<usize>,
    locale_length: Option<usize>,
    cart_id_length: Option<usize>,
    line_item_id_length: Option<usize>,
}

impl NativeClientErrorContext {
    pub(super) fn fetch_cart(request: &CartFetchRequest) -> Self {
        Self {
            operation: "fetch_cart",
            correlation_id: native_client_correlation_id("fetch_cart"),
            selected_cart_id_length: request
                .selected_cart_id
                .as_deref()
                .map(|value| value.chars().count()),
            locale_length: request.locale.as_deref().map(|value| value.chars().count()),
            cart_id_length: None,
            line_item_id_length: None,
        }
    }

    pub(super) fn decrement_line_item(request: &CartLineItemDecrementRequest) -> Self {
        Self::line_item_operation(
            "decrement_line_item",
            request.cart_id.as_str(),
            request.line_item_id.as_str(),
        )
    }

    pub(super) fn remove_line_item(request: &CartLineItemMutationRequest) -> Self {
        Self::line_item_operation(
            "remove_line_item",
            request.cart_id.as_str(),
            request.line_item_id.as_str(),
        )
    }

    fn line_item_operation(operation: &'static str, cart_id: &str, line_item_id: &str) -> Self {
        Self {
            operation,
            correlation_id: native_client_correlation_id(operation),
            selected_cart_id_length: None,
            locale_length: None,
            cart_id_length: Some(cart_id.chars().count()),
            line_item_id_length: Some(line_item_id.chars().count()),
        }
    }

    pub(super) fn create_cart(request: &crate::core::CartCreateRequest) -> Self {
        Self {
            operation: "create_cart",
            correlation_id: native_client_correlation_id("create_cart"),
            selected_cart_id_length: None,
            locale_length: request.locale.as_deref().map(|value| value.chars().count()),
            cart_id_length: None,
            line_item_id_length: None,
        }
    }
    pub(super) fn add_variant(request: &crate::core::CartAddVariantRequest) -> Self {
        Self {
            operation: "add_variant",
            correlation_id: native_client_correlation_id("add_variant"),
            selected_cart_id_length: None,
            locale_length: request.locale.as_deref().map(|value| value.chars().count()),
            cart_id_length: Some(request.cart_id.chars().count()),
            line_item_id_length: None,
        }
    }

    pub(super) fn map_error(&self, error: ApiError) -> ApiError {
        match error {
            ApiError::Validation(message) => ApiError::Validation(message),
            error => {
                let (error_kind, raw_error_length) = match &error {
                    ApiError::Graphql(message) => ("graphql", message.chars().count()),
                    ApiError::ServerFn(message) => ("server_fn", message.chars().count()),
                    ApiError::Validation(message) => ("validation", message.chars().count()),
                };
                tracing::error!(
                    error_kind, raw_error_length,
                    owner = CART_STOREFRONT_NATIVE_CLIENT_OWNER,
                    owner_operation = self.operation,
                    correlation_id = %self.correlation_id,
                    selected_cart_id_present = self.selected_cart_id_length.is_some(),
                    selected_cart_id_length = ?self.selected_cart_id_length,
                    locale_present = self.locale_length.is_some(),
                    locale_length = ?self.locale_length,
                    cart_id_present = self.cart_id_length.is_some(),
                    cart_id_length = ?self.cart_id_length,
                    line_item_id_present = self.line_item_id_length.is_some(),
                    line_item_id_length = ?self.line_item_id_length,
                    code = "cart.storefront_native_client_transport_failed",
                    boundary = CART_STOREFRONT_NATIVE_CLIENT_BOUNDARY,
                    "cart storefront native client transport request failed"
                );

                ApiError::ServerFn(CART_STOREFRONT_NATIVE_CLIENT_PUBLIC_MESSAGE.to_string())
            }
        }
    }
}

fn native_client_correlation_id(operation: &'static str) -> String {
    format!(
        "cart-storefront-native-client:{operation}:{}",
        Uuid::new_v4()
    )
}
