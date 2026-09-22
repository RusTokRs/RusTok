use rustok_ui_transport::{UiTransportError, UiTransportPath};
use uuid::Uuid;

use crate::core::FetchCommerceRequest;

use super::shared_adapter::ApiError;

const COMMERCE_STOREFRONT_AGGREGATE_OWNER: &str = "rustok_commerce.storefront";
const COMMERCE_STOREFRONT_AGGREGATE_OPERATION: &str = "fetch_storefront_commerce";
const COMMERCE_STOREFRONT_AGGREGATE_BOUNDARY: &str =
    "commerce_storefront_aggregate_public_transport";
const INVALID_CART_SELECTION: &str = "Invalid cart selection";
const CART_ID_UUID_VALIDATION: &str = "cart_id must be a valid UUID";
const STOREFRONT_COMMERCE_UNAVAILABLE: &str = "Storefront commerce data is temporarily unavailable";

pub(super) struct AggregateFetchErrorContext {
    correlation_id: String,
    tenant_slug_length: Option<usize>,
    selected_cart_id_length: Option<usize>,
    locale_length: Option<usize>,
}

impl AggregateFetchErrorContext {
    pub(super) fn new(request: &FetchCommerceRequest) -> Self {
        Self {
            correlation_id: format!(
                "commerce-storefront-aggregate:{COMMERCE_STOREFRONT_AGGREGATE_OPERATION}:{}",
                Uuid::new_v4()
            ),
            tenant_slug_length: configured_tenant_slug_length(),
            selected_cart_id_length: request
                .selected_cart_id
                .as_deref()
                .map(|value| value.chars().count()),
            locale_length: request.locale.as_deref().map(|value| value.chars().count()),
        }
    }

    pub(super) fn map_error(&self, error: UiTransportError) -> ApiError {
        if is_invalid_cart_selection(&error) {
            tracing::warn!(
                error = ?error,
                owner = COMMERCE_STOREFRONT_AGGREGATE_OWNER,
                owner_operation = COMMERCE_STOREFRONT_AGGREGATE_OPERATION,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                selected_cart_id_present = self.selected_cart_id_length.is_some(),
                selected_cart_id_length = ?self.selected_cart_id_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                failed_path = error.failed_path.as_str(),
                fallback_attempted = error.fallback_attempted,
                code = "commerce.storefront_aggregate_cart_selection_invalid",
                boundary = COMMERCE_STOREFRONT_AGGREGATE_BOUNDARY,
                "commerce storefront aggregate request validation failed"
            );
            return ApiError::Validation(INVALID_CART_SELECTION.to_string());
        }

        tracing::error!(
            error = ?error,
            owner = COMMERCE_STOREFRONT_AGGREGATE_OWNER,
            owner_operation = COMMERCE_STOREFRONT_AGGREGATE_OPERATION,
            correlation_id = %self.correlation_id,
            tenant_slug_configured = self.tenant_slug_length.is_some(),
            tenant_slug_length = ?self.tenant_slug_length,
            selected_cart_id_present = self.selected_cart_id_length.is_some(),
            selected_cart_id_length = ?self.selected_cart_id_length,
            locale_present = self.locale_length.is_some(),
            locale_length = ?self.locale_length,
            failed_path = error.failed_path.as_str(),
            fallback_attempted = error.fallback_attempted,
            code = "commerce.storefront_aggregate_unavailable",
            boundary = COMMERCE_STOREFRONT_AGGREGATE_BOUNDARY,
            "commerce storefront aggregate transport failed"
        );

        match error.failed_path {
            UiTransportPath::NativeServer => {
                ApiError::ServerFn(STOREFRONT_COMMERCE_UNAVAILABLE.to_string())
            }
            UiTransportPath::Graphql => {
                ApiError::Graphql(STOREFRONT_COMMERCE_UNAVAILABLE.to_string())
            }
        }
    }
}

fn is_invalid_cart_selection(error: &UiTransportError) -> bool {
    [
        error.native_error.as_deref(),
        error.graphql_error.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|message| message == INVALID_CART_SELECTION || message == CART_ID_UUID_VALIDATION)
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
