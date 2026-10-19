use rustok_ui_transport::{UiTransportError, UiTransportPath};
use uuid::Uuid;

use crate::core::CheckoutCompletionCommandRequest;

use super::shared_adapter::ApiError;

const COMMERCE_STOREFRONT_CHECKOUT_OWNER: &str = "rustok_commerce.storefront";
const COMMERCE_STOREFRONT_CHECKOUT_OPERATION: &str = "complete_storefront_checkout";
const COMMERCE_STOREFRONT_CHECKOUT_BOUNDARY: &str =
    "commerce_storefront_checkout_completion_public_transport";
const CART_ID_UUID_VALIDATION: &str = "cart_id must be a valid UUID";
const IDEMPOTENCY_KEY_VALIDATION: &str = "checkout idempotency key must contain 1 to 191 bytes";
const OWNER_INVALID_CHECKOUT_REQUEST: &str = "Checkout request is invalid";
const INVALID_CHECKOUT_REQUEST: &str = "Invalid checkout request";
const CHECKOUT_COMPLETION_UNAVAILABLE: &str = "Checkout completion is temporarily unavailable";

pub(super) struct CheckoutCompletionCommandErrorContext {
    correlation_id: String,
    tenant_slug_length: Option<usize>,
    cart_id_length: usize,
    idempotency_key_length: usize,
    source_module_length: usize,
    source_surface_length: usize,
    command_length: usize,
    owner_module_length: usize,
    create_fulfillment: bool,
}

impl CheckoutCompletionCommandErrorContext {
    pub(super) fn new(request: &CheckoutCompletionCommandRequest) -> Self {
        Self {
            correlation_id: format!(
                "commerce-storefront-checkout:{COMMERCE_STOREFRONT_CHECKOUT_OPERATION}:{}",
                Uuid::new_v4()
            ),
            tenant_slug_length: configured_tenant_slug_length(),
            cart_id_length: request.cart_id.chars().count(),
            idempotency_key_length: request.idempotency_key.chars().count(),
            source_module_length: request.metadata.source_module.chars().count(),
            source_surface_length: request.metadata.source_surface.chars().count(),
            command_length: request.metadata.command.chars().count(),
            owner_module_length: request.metadata.owner_module.chars().count(),
            create_fulfillment: request.metadata.create_fulfillment,
        }
    }

    pub(super) fn map_error(&self, error: UiTransportError) -> ApiError {
        if is_invalid_checkout_request(&error) {
            tracing::warn!(
                error = ?error,
                owner = COMMERCE_STOREFRONT_CHECKOUT_OWNER,
                owner_operation = COMMERCE_STOREFRONT_CHECKOUT_OPERATION,
                correlation_id = %self.correlation_id,
                tenant_slug_configured = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                cart_id_length = self.cart_id_length,
                idempotency_key_length = self.idempotency_key_length,
                source_module_length = self.source_module_length,
                source_surface_length = self.source_surface_length,
                command_length = self.command_length,
                owner_module_length = self.owner_module_length,
                create_fulfillment = self.create_fulfillment,
                failed_path = error.failed_path.as_str(),
                fallback_attempted = error.fallback_attempted,
                code = "commerce.storefront_checkout_request_invalid",
                boundary = COMMERCE_STOREFRONT_CHECKOUT_BOUNDARY,
                "commerce storefront checkout completion validation failed"
            );
            return ApiError::Validation(INVALID_CHECKOUT_REQUEST.to_string());
        }

        tracing::error!(
            error = ?error,
            owner = COMMERCE_STOREFRONT_CHECKOUT_OWNER,
            owner_operation = COMMERCE_STOREFRONT_CHECKOUT_OPERATION,
            correlation_id = %self.correlation_id,
            tenant_slug_configured = self.tenant_slug_length.is_some(),
            tenant_slug_length = ?self.tenant_slug_length,
            cart_id_length = self.cart_id_length,
            idempotency_key_length = self.idempotency_key_length,
            source_module_length = self.source_module_length,
            source_surface_length = self.source_surface_length,
            command_length = self.command_length,
            owner_module_length = self.owner_module_length,
            create_fulfillment = self.create_fulfillment,
            failed_path = error.failed_path.as_str(),
            fallback_attempted = error.fallback_attempted,
            code = "commerce.storefront_checkout_completion_unavailable",
            boundary = COMMERCE_STOREFRONT_CHECKOUT_BOUNDARY,
            "commerce storefront checkout completion command failed"
        );

        match error.failed_path {
            UiTransportPath::NativeServer => {
                ApiError::ServerFn(CHECKOUT_COMPLETION_UNAVAILABLE.to_string())
            }
            UiTransportPath::Graphql => {
                ApiError::Graphql(CHECKOUT_COMPLETION_UNAVAILABLE.to_string())
            }
        }
    }
}

fn is_invalid_checkout_request(error: &UiTransportError) -> bool {
    [
        error.native_error.as_deref(),
        error.graphql_error.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|message| {
        message == CART_ID_UUID_VALIDATION
            || message == IDEMPOTENCY_KEY_VALIDATION
            || message == OWNER_INVALID_CHECKOUT_REQUEST
            || message == INVALID_CHECKOUT_REQUEST
            || message.contains(CART_ID_UUID_VALIDATION)
            || message.contains(IDEMPOTENCY_KEY_VALIDATION)
            || message.contains(OWNER_INVALID_CHECKOUT_REQUEST)
            || message.contains(INVALID_CHECKOUT_REQUEST)
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_order_storefront::transport::CheckoutCompletionCommandMetadata;

    fn sample_checkout_request() -> CheckoutCompletionCommandRequest {
        CheckoutCompletionCommandRequest {
            cart_id: "cart-123".into(),
            idempotency_key: "idem-key-1".into(),
            metadata: CheckoutCompletionCommandMetadata {
                source_module: "rustok_commerce".into(),
                source_surface: "storefront".into(),
                command: "complete_checkout".into(),
                owner_module: "rustok_order".into(),
                create_fulfillment: true,
            },
        }
    }

    #[test]
    fn maps_validation_error_on_native_and_graphql() {
        let req = sample_checkout_request();
        let ctx = CheckoutCompletionCommandErrorContext::new(&req);

        let err_native = UiTransportError {
            surface: "commerce".into(),
            failed_path: UiTransportPath::NativeServer,
            native_error: Some("checkout idempotency key must contain 1 to 191 bytes".into()),
            graphql_error: None,
            fallback_attempted: false,
        };
        match ctx.map_error(err_native) {
            ApiError::Validation(msg) => assert_eq!(msg, "Invalid checkout request"),
            other => panic!("expected ApiError::Validation, got {other:?}"),
        }

        let err_graphql = UiTransportError {
            surface: "commerce".into(),
            failed_path: UiTransportPath::Graphql,
            native_error: None,
            graphql_error: Some("Checkout request is invalid".into()),
            fallback_attempted: false,
        };
        match ctx.map_error(err_graphql) {
            ApiError::Validation(msg) => assert_eq!(msg, "Invalid checkout request"),
            other => panic!("expected ApiError::Validation, got {other:?}"),
        }
    }

    #[test]
    fn maps_transport_errors_to_safe_public_message() {
        let req = sample_checkout_request();
        let ctx = CheckoutCompletionCommandErrorContext::new(&req);

        let err_native = UiTransportError {
            surface: "commerce".into(),
            failed_path: UiTransportPath::NativeServer,
            native_error: Some("connection pool exhausted".into()),
            graphql_error: None,
            fallback_attempted: false,
        };
        match ctx.map_error(err_native) {
            ApiError::ServerFn(msg) => {
                assert_eq!(msg, "Checkout completion is temporarily unavailable");
            }
            other => panic!("expected ApiError::ServerFn, got {other:?}"),
        }

        let err_graphql = UiTransportError {
            surface: "commerce".into(),
            failed_path: UiTransportPath::Graphql,
            native_error: None,
            graphql_error: Some("internal server error".into()),
            fallback_attempted: false,
        };
        match ctx.map_error(err_graphql) {
            ApiError::Graphql(msg) => {
                assert_eq!(msg, "Checkout completion is temporarily unavailable");
            }
            other => panic!("expected ApiError::Graphql, got {other:?}"),
        }
    }
}
