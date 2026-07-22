mod graphql_adapter;
mod native_server_adapter;

use crate::core::{CartFetchRequest, CartLineItemDecrementRequest, CartLineItemMutationRequest};
use crate::model::StorefrontCartData;
use rustok_ui_transport::{
    UiTransportError, UiTransportPath, UiTransportResult, execute_selected_transport,
};

pub type CartTransportError = UiTransportError;
pub type TransportResult<T> = UiTransportResult<T>;

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub async fn fetch_cart(request: CartFetchRequest) -> TransportResult<StorefrontCartData> {
    let native_request = request.clone();
    execute_selected_transport(
        "cart",
        selected_transport_path(),
        move || native_server_adapter::fetch_cart(native_request),
        move || graphql_adapter::fetch_cart(request),
    )
    .await
}

pub async fn decrement_line_item(request: CartLineItemDecrementRequest) -> TransportResult<()> {
    let native_request = request.clone();
    execute_selected_transport(
        "cart",
        selected_transport_path(),
        move || native_server_adapter::decrement_line_item(native_request),
        move || graphql_adapter::decrement_line_item(request),
    )
    .await
}

pub async fn remove_line_item(request: CartLineItemMutationRequest) -> TransportResult<()> {
    let native_request = request.clone();
    execute_selected_transport(
        "cart",
        selected_transport_path(),
        move || native_server_adapter::remove_line_item(native_request),
        move || graphql_adapter::remove_line_item(request),
    )
    .await
}

#[cfg(test)]
mod tests {
    use native_server_adapter::ApiError;

    use super::*;

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }

    #[test]
    fn native_validation_error_keeps_single_path_evidence() {
        let error = CartTransportError::native(
            "cart",
            ApiError::Validation("cart_id must be a valid UUID".to_string()),
        );

        assert_eq!(
            error.failed_path,
            rustok_ui_transport::UiTransportPath::NativeServer
        );
        assert!(!error.fallback_attempted);
        assert_eq!(
            error.native_error,
            Some("cart_id must be a valid UUID".to_string())
        );
        assert_eq!(error.graphql_error, None);
    }

    #[test]
    fn transport_path_serializes_as_stable_snake_case() {
        let serialized = serde_json::to_string(&CartTransportError::fallback_failed(
            "cart",
            ApiError::ServerFn("server function unavailable".to_string()),
            ApiError::Graphql("network unavailable".to_string()),
        ))
        .expect("transport error should serialize");

        assert!(serialized.contains(r#""failed_path":"graphql""#));
        assert!(serialized.contains(r#""fallback_attempted":true"#));
    }

    #[test]
    fn failed_fallback_keeps_both_path_errors() {
        let error = CartTransportError::fallback_failed(
            "cart",
            ApiError::ServerFn("server function unavailable".to_string()),
            ApiError::Graphql("network unavailable".to_string()),
        );

        assert_eq!(
            error.failed_path,
            rustok_ui_transport::UiTransportPath::Graphql
        );
        assert!(error.fallback_attempted);
        assert_eq!(
            error.native_error,
            Some("server function unavailable".to_string())
        );
        assert_eq!(error.graphql_error, Some("network unavailable".to_string()));
        assert!(
            error
                .to_string()
                .contains("native_server=server function unavailable")
        );
        assert!(error.to_string().contains("graphql=network unavailable"));
    }
}
