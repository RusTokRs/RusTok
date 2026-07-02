mod graphql_adapter;
mod native_server_adapter;

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::core::ProductStorefrontFetchRequest;
use crate::model::{ProductCatalogSearchOptions, StorefrontProductsData};
use native_server_adapter::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductTransportPath {
    NativeServer,
    Graphql,
}

impl ProductTransportPath {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeServer => "native_server",
            Self::Graphql => "graphql",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductTransportError {
    pub failed_path: ProductTransportPath,
    pub fallback_attempted: bool,
    pub native_error: Option<String>,
    pub graphql_error: Option<String>,
}

impl ProductTransportError {
    fn fallback_failed(native_error: ApiError, graphql_error: ApiError) -> Self {
        Self {
            failed_path: ProductTransportPath::Graphql,
            fallback_attempted: true,
            native_error: Some(native_error.to_string()),
            graphql_error: Some(graphql_error.to_string()),
        }
    }
}

impl Display for ProductTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (&self.native_error, &self.graphql_error) {
            (Some(native), Some(graphql)) => write!(
                f,
                "product transport fallback failed: native_server={native}; graphql={graphql}"
            ),
            (Some(native), None) => write!(
                f,
                "product transport failed on {}: {native}",
                self.failed_path.as_str()
            ),
            (None, Some(graphql)) => write!(
                f,
                "product transport failed on {}: {graphql}",
                self.failed_path.as_str()
            ),
            (None, None) => write!(
                f,
                "product transport failed on {}",
                self.failed_path.as_str()
            ),
        }
    }
}

impl std::error::Error for ProductTransportError {}

pub type TransportResult<T> = Result<T, ProductTransportError>;

pub async fn fetch_products(
    request: ProductStorefrontFetchRequest,
) -> TransportResult<StorefrontProductsData> {
    match native_server_adapter::fetch_products(request.clone()).await {
        Ok(data) => Ok(data),
        Err(native_error) => match graphql_adapter::fetch_products(request).await {
            Ok(data) => Ok(data),
            Err(graphql_error) => Err(ProductTransportError::fallback_failed(
                native_error,
                graphql_error,
            )),
        },
    }
}

pub async fn fetch_catalog_search_options(
    locale: String,
) -> TransportResult<ProductCatalogSearchOptions> {
    match native_server_adapter::fetch_catalog_search_options(locale.clone()).await {
        Ok(data) => Ok(data),
        Err(native_error) => match graphql_adapter::fetch_catalog_search_options(locale).await {
            Ok(data) => Ok(data),
            Err(graphql_error) => Err(ProductTransportError::fallback_failed(
                native_error,
                graphql_error,
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_path_serializes_as_stable_snake_case() {
        let serialized = serde_json::to_string(&ProductTransportError::fallback_failed(
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
            ApiError::ServerFn("server function unavailable".to_string()),
            ApiError::Graphql("network unavailable".to_string()),
        );

        assert_eq!(error.failed_path, ProductTransportPath::Graphql);
        assert!(error.fallback_attempted);
        assert_eq!(
            error.native_error,
            Some("server function unavailable".to_string())
        );
        assert_eq!(error.graphql_error, Some("network unavailable".to_string()));
        assert!(error
            .to_string()
            .contains("native_server=server function unavailable"));
        assert!(error.to_string().contains("graphql=network unavailable"));
    }
}
