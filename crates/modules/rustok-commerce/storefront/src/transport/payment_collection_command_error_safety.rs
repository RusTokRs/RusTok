use rustok_ui_transport::{UiTransportError, UiTransportPath};
use uuid::Uuid;

use crate::core::PaymentCollectionCommandRequest;

use super::shared_adapter::ApiError;

const COMMERCE_STOREFRONT_PAYMENT_OWNER: &str = "rustok_commerce.storefront";
const COMMERCE_STOREFRONT_PAYMENT_OPERATION: &str = "create_storefront_payment_collection";
const COMMERCE_STOREFRONT_PAYMENT_BOUNDARY: &str =
    "commerce_storefront_payment_collection_public_transport";
const CART_ID_UUID_VALIDATION: &str = "cart_id must be a valid UUID";
const INVALID_CART_SELECTION: &str = "Invalid cart selection";
const STOREFRONT_PAYMENT_COLLECTION_UNAVAILABLE: &str =
    "Storefront payment collection is temporarily unavailable";

pub(super) struct PaymentCollectionCommandErrorContext {
    correlation_id: String,
    tenant_slug_length: Option<usize>,
    cart_id_length: usize,
    source_module_length: usize,
    source_surface_length: usize,
    command_length: usize,
    owner_module_length: usize,
}

impl PaymentCollectionCommandErrorContext {
    pub(super) fn new(request: &PaymentCollectionCommandRequest) -> Self {
        Self {
            correlation_id: format!(
                "commerce-storefront-payment:{COMMERCE_STOREFRONT_PAYMENT_OPERATION}:{}",
                Uuid::new_v4()
            ),
            tenant_slug_length: configured_tenant_slug_length(),
            cart_id_length: request.cart_id.chars().count(),
            source_module_length: request.metadata.source_module.chars().count(),
            source_surface_length: request.metadata.source_surface.chars().count(),
            command_length: request.metadata.command.chars().count(),
            owner_module_length: request.metadata.owner_module.chars().count(),
        }
    }

    pub(super) fn map_error(&self, error: UiTransportError) -> ApiError {
        if is_invalid_cart_selection(&error) {
            tracing::warn!(
                error = ?error,
                owner = COMMERCE_STOREFRONT_PAYMENT_OWNER,
                owner_operation = COMMERCE_STOREFRONT_PAYMENT_OPERATION,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                cart_id_length = self.cart_id_length,
                source_module_length = self.source_module_length,
                source_surface_length = self.source_surface_length,
                command_length = self.command_length,
                owner_module_length = self.owner_module_length,
                failed_path = error.failed_path.as_str(),
                fallback_attempted = error.fallback_attempted,
                code = "commerce.storefront_payment_collection_cart_id_invalid",
                boundary = COMMERCE_STOREFRONT_PAYMENT_BOUNDARY,
                "commerce storefront payment collection validation failed"
            );
            return ApiError::Validation(INVALID_CART_SELECTION.to_string());
        }

        tracing::error!(
            error = ?error,
            owner = COMMERCE_STOREFRONT_PAYMENT_OWNER,
            owner_operation = COMMERCE_STOREFRONT_PAYMENT_OPERATION,
            correlation_id = %self.correlation_id,
            tenant_slug_configured = self.tenant_slug_length.is_some(),
            tenant_slug_length = ?self.tenant_slug_length,
            cart_id_length = self.cart_id_length,
            source_module_length = self.source_module_length,
            source_surface_length = self.source_surface_length,
            command_length = self.command_length,
            owner_module_length = self.owner_module_length,
            failed_path = error.failed_path.as_str(),
            fallback_attempted = error.fallback_attempted,
            code = "commerce.storefront_payment_collection_unavailable",
            boundary = COMMERCE_STOREFRONT_PAYMENT_BOUNDARY,
            "commerce storefront payment collection command failed"
        );

        match error.failed_path {
            UiTransportPath::NativeServer => {
                ApiError::ServerFn(STOREFRONT_PAYMENT_COLLECTION_UNAVAILABLE.to_string())
            }
            UiTransportPath::Graphql => {
                ApiError::Graphql(STOREFRONT_PAYMENT_COLLECTION_UNAVAILABLE.to_string())
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
    .any(|message| message == CART_ID_UUID_VALIDATION)
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
