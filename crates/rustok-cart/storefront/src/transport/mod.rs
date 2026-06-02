mod graphql_adapter;
mod native_server_adapter;

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::api::ApiError;
use crate::core::{CartFetchRequest, CartLineItemDecrementRequest, CartLineItemMutationRequest};
use crate::model::StorefrontCartData;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartTransportPath {
    NativeServer,
    Graphql,
}

impl CartTransportPath {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeServer => "native_server",
            Self::Graphql => "graphql",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartTransportError {
    pub failed_path: CartTransportPath,
    pub fallback_attempted: bool,
    pub native_error: Option<String>,
    pub graphql_error: Option<String>,
}

impl CartTransportError {
    fn native(error: ApiError) -> Self {
        Self {
            failed_path: CartTransportPath::NativeServer,
            fallback_attempted: false,
            native_error: Some(error.to_string()),
            graphql_error: None,
        }
    }

    fn fallback_failed(native_error: ApiError, graphql_error: ApiError) -> Self {
        Self {
            failed_path: CartTransportPath::Graphql,
            fallback_attempted: true,
            native_error: Some(native_error.to_string()),
            graphql_error: Some(graphql_error.to_string()),
        }
    }
}

impl Display for CartTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (&self.native_error, &self.graphql_error) {
            (Some(native), Some(graphql)) => write!(
                f,
                "cart transport fallback failed: native_server={native}; graphql={graphql}"
            ),
            (Some(native), None) => write!(
                f,
                "cart transport failed on {}: {native}",
                self.failed_path.as_str()
            ),
            (None, Some(graphql)) => write!(
                f,
                "cart transport failed on {}: {graphql}",
                self.failed_path.as_str()
            ),
            (None, None) => write!(f, "cart transport failed on {}", self.failed_path.as_str()),
        }
    }
}

impl std::error::Error for CartTransportError {}

pub type TransportResult<T> = Result<T, CartTransportError>;

fn should_try_graphql_fallback(error: &ApiError) -> bool {
    !matches!(error, ApiError::Validation(_))
}

pub async fn fetch_cart(request: CartFetchRequest) -> TransportResult<StorefrontCartData> {
    match native_server_adapter::fetch_cart(request.clone()).await {
        Ok(data) => Ok(data),
        Err(native_error) if should_try_graphql_fallback(&native_error) => {
            match graphql_adapter::fetch_cart(request).await {
                Ok(data) => Ok(data),
                Err(graphql_error) => Err(CartTransportError::fallback_failed(
                    native_error,
                    graphql_error,
                )),
            }
        }
        Err(native_error) => Err(CartTransportError::native(native_error)),
    }
}

pub async fn decrement_line_item(request: CartLineItemDecrementRequest) -> TransportResult<()> {
    match native_server_adapter::decrement_line_item(request.clone()).await {
        Ok(()) => Ok(()),
        Err(native_error) if should_try_graphql_fallback(&native_error) => {
            match graphql_adapter::decrement_line_item(request).await {
                Ok(()) => Ok(()),
                Err(graphql_error) => Err(CartTransportError::fallback_failed(
                    native_error,
                    graphql_error,
                )),
            }
        }
        Err(native_error) => Err(CartTransportError::native(native_error)),
    }
}

pub async fn remove_line_item(request: CartLineItemMutationRequest) -> TransportResult<()> {
    match native_server_adapter::remove_line_item(request.clone()).await {
        Ok(()) => Ok(()),
        Err(native_error) if should_try_graphql_fallback(&native_error) => {
            match graphql_adapter::remove_line_item(request).await {
                Ok(()) => Ok(()),
                Err(graphql_error) => Err(CartTransportError::fallback_failed(
                    native_error,
                    graphql_error,
                )),
            }
        }
        Err(native_error) => Err(CartTransportError::native(native_error)),
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

    #[test]
    fn native_validation_error_keeps_single_path_evidence() {
        let error = CartTransportError::native(ApiError::Validation(
            "cart_id must be a valid UUID".to_string(),
        ));

        assert_eq!(error.failed_path, CartTransportPath::NativeServer);
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
            ApiError::ServerFn("server function unavailable".to_string()),
            ApiError::Graphql("network unavailable".to_string()),
        );

        assert_eq!(error.failed_path, CartTransportPath::Graphql);
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
