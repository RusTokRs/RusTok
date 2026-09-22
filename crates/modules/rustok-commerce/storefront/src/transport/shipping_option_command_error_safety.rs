use rustok_ui_transport::{UiTransportError, UiTransportPath};
use uuid::Uuid;

use crate::core::SelectShippingOptionRequest;

use super::shared_adapter::ApiError;

const COMMERCE_STOREFRONT_SHIPPING_OWNER: &str = "rustok_commerce.storefront";
const COMMERCE_STOREFRONT_SHIPPING_OPERATION: &str = "select_storefront_shipping_option";
const COMMERCE_STOREFRONT_SHIPPING_BOUNDARY: &str =
    "commerce_storefront_shipping_option_public_transport";
const CART_ID_UUID_VALIDATION: &str = "cart_id must be a valid UUID";
const SELECTED_SHIPPING_OPTION_ID_UUID_VALIDATION: &str =
    "selected_shipping_option_id must be a valid UUID";
const INVALID_SHIPPING_SELECTION: &str = "Invalid shipping selection";
const SHIPPING_SELECTION_UNAVAILABLE: &str = "Shipping selection is temporarily unavailable";

pub(super) struct ShippingOptionCommandErrorContext {
    correlation_id: String,
    tenant_slug_length: Option<usize>,
    cart_id_length: usize,
    delivery_group_count: usize,
    available_shipping_option_count: usize,
    shipping_profile_slug_length: usize,
    seller_id_length: Option<usize>,
    shipping_option_id_length: Option<usize>,
}

impl ShippingOptionCommandErrorContext {
    pub(super) fn new(request: &SelectShippingOptionRequest) -> Self {
        let owner_request = &request.owner_request;
        Self {
            correlation_id: format!(
                "commerce-storefront-shipping:{COMMERCE_STOREFRONT_SHIPPING_OPERATION}:{}",
                Uuid::new_v4()
            ),
            tenant_slug_length: configured_tenant_slug_length(),
            cart_id_length: owner_request.cart_id.chars().count(),
            delivery_group_count: owner_request.delivery_groups.len(),
            available_shipping_option_count: owner_request
                .delivery_groups
                .iter()
                .map(|group| group.available_shipping_option_ids.len())
                .sum(),
            shipping_profile_slug_length: owner_request.shipping_profile_slug.chars().count(),
            seller_id_length: owner_request
                .seller_id
                .as_deref()
                .map(|value| value.chars().count()),
            shipping_option_id_length: owner_request
                .shipping_option_id
                .as_deref()
                .map(|value| value.chars().count()),
        }
    }

    pub(super) fn map_error(&self, error: UiTransportError) -> ApiError {
        if is_invalid_shipping_selection(&error) {
            tracing::warn!(
                error = ?error,
                owner = COMMERCE_STOREFRONT_SHIPPING_OWNER,
                owner_operation = COMMERCE_STOREFRONT_SHIPPING_OPERATION,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                cart_id_length = self.cart_id_length,
                delivery_group_count = self.delivery_group_count,
                available_shipping_option_count = self.available_shipping_option_count,
                shipping_profile_slug_length = self.shipping_profile_slug_length,
                seller_id_present = self.seller_id_length.is_some(),
                seller_id_length = ?self.seller_id_length,
                shipping_option_id_present = self.shipping_option_id_length.is_some(),
                shipping_option_id_length = ?self.shipping_option_id_length,
                failed_path = error.failed_path.as_str(),
                fallback_attempted = error.fallback_attempted,
                code = "commerce.storefront_shipping_selection_invalid",
                boundary = COMMERCE_STOREFRONT_SHIPPING_BOUNDARY,
                "commerce storefront shipping selection validation failed"
            );
            return ApiError::Validation(INVALID_SHIPPING_SELECTION.to_string());
        }

        tracing::error!(
            error = ?error,
            owner = COMMERCE_STOREFRONT_SHIPPING_OWNER,
            owner_operation = COMMERCE_STOREFRONT_SHIPPING_OPERATION,
            correlation_id = %self.correlation_id,
            tenant_slug_configured = self.tenant_slug_length.is_some(),
            tenant_slug_length = ?self.tenant_slug_length,
            cart_id_length = self.cart_id_length,
            delivery_group_count = self.delivery_group_count,
            available_shipping_option_count = self.available_shipping_option_count,
            shipping_profile_slug_length = self.shipping_profile_slug_length,
            seller_id_present = self.seller_id_length.is_some(),
            seller_id_length = ?self.seller_id_length,
            shipping_option_id_present = self.shipping_option_id_length.is_some(),
            shipping_option_id_length = ?self.shipping_option_id_length,
            failed_path = error.failed_path.as_str(),
            fallback_attempted = error.fallback_attempted,
            code = "commerce.storefront_shipping_selection_unavailable",
            boundary = COMMERCE_STOREFRONT_SHIPPING_BOUNDARY,
            "commerce storefront shipping selection command failed"
        );

        match error.failed_path {
            UiTransportPath::NativeServer => {
                ApiError::ServerFn(SHIPPING_SELECTION_UNAVAILABLE.to_string())
            }
            UiTransportPath::Graphql => {
                ApiError::Graphql(SHIPPING_SELECTION_UNAVAILABLE.to_string())
            }
        }
    }
}

fn is_invalid_shipping_selection(error: &UiTransportError) -> bool {
    [
        error.native_error.as_deref(),
        error.graphql_error.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(is_shipping_selection_validation_message)
}

fn is_shipping_selection_validation_message(message: &str) -> bool {
    matches!(
        message,
        CART_ID_UUID_VALIDATION
            | SELECTED_SHIPPING_OPTION_ID_UUID_VALIDATION
            | INVALID_SHIPPING_SELECTION
    ) || (message.starts_with("delivery group `")
        && message.ends_with(" is not present in the checkout cart"))
        || (message.starts_with("shipping option ")
            && message.contains(" is not available for shipping profile "))
}

fn configured_tenant_slug_length() -> Option<usize> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let value = value.trim();
            (!value.is_empty()).then_some(value.chars().count())
        })
    })
}
