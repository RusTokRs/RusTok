mod graphql_adapter;
mod native_server_adapter;

use crate::core::FetchRequest;
use crate::model::{ProductCatalogSearchOptions, StorefrontProductsData};
use rustok_ui_transport::{
    UiTransportError, UiTransportPath, UiTransportResult, execute_selected_transport,
};

pub type ProductTransportError = UiTransportError;
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

pub async fn fetch_products(request: FetchRequest) -> TransportResult<StorefrontProductsData> {
    let native_request = request.clone();
    execute_selected_transport(
        "product",
        selected_transport_path(),
        move || native_server_adapter::fetch_products(native_request),
        move || graphql_adapter::fetch_products(request),
    )
    .await
}

pub async fn fetch_catalog_search_options(
    locale: String,
) -> TransportResult<ProductCatalogSearchOptions> {
    let native_locale = locale.clone();
    execute_selected_transport(
        "product",
        selected_transport_path(),
        move || native_server_adapter::fetch_catalog_search_options(native_locale),
        move || graphql_adapter::fetch_catalog_search_options(locale),
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
    fn transport_path_serializes_as_stable_snake_case() {
        let serialized = serde_json::to_string(&ProductTransportError::fallback_failed(
            "product",
            ApiError::ServerFn("server function unavailable".to_string()),
            ApiError::Graphql("network unavailable".to_string()),
        ))
        .expect("transport error should serialize");

        assert!(serialized.contains(r#""failed_path":"graphql""#));
        assert!(serialized.contains(r#""fallback_attempted":true"#));
    }

    #[test]
    fn failed_fallback_keeps_both_path_errors() {
        let error = ProductTransportError::fallback_failed(
            "product",
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
