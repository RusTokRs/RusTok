use leptos::prelude::ServerFnError;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use serde::{Serialize, de::DeserializeOwned};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    Graphql(GraphqlHttpError),
    ServerFn(ServerFnError),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value)
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value)
    }
}

pub fn get_graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{}/api/graphql", origin)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{}/api/graphql", base)
    }
}

pub async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: DeserializeOwned,
{
    execute_graphql(
        &get_graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(ApiError::from)
}

// A routing hint, not authorization. Once a nonempty value is chosen, reject
// malformed input rather than silently falling back to a different tenant.
fn select_tenant_slug<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    let value = values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())?;
    if value.len() > 255 || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.to_string())
}

pub fn configured_tenant_slug() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        // Browser WASM has no process environment. These are public build-time
        // routing hints and intentionally bind a standalone build to one tenant.
        select_tenant_slug([
            option_env!("RUSTOK_TENANT_SLUG"),
            option_env!("NEXT_PUBLIC_TENANT_SLUG"),
            option_env!("NEXT_PUBLIC_DEFAULT_TENANT_SLUG"),
        ])
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let values = [
            std::env::var("RUSTOK_TENANT_SLUG").ok(),
            std::env::var("NEXT_PUBLIC_TENANT_SLUG").ok(),
            std::env::var("NEXT_PUBLIC_DEFAULT_TENANT_SLUG").ok(),
        ];
        select_tenant_slug(values.iter().map(|value| value.as_deref()))
    }
}

#[cfg(test)]
mod tests {
    use super::select_tenant_slug;

    #[test]
    fn tenant_hint_is_bounded_and_never_falls_back_after_invalid_selection() {
        assert_eq!(
            select_tenant_slug([None, Some("  "), Some(" shop-a ")]),
            Some("shop-a".to_string())
        );
        assert_eq!(select_tenant_slug([None, Some("")]), None);
        assert_eq!(
            select_tenant_slug([Some("shop\nother"), Some("shop-b")]),
            None
        );
        let too_long = "a".repeat(256);
        assert_eq!(
            select_tenant_slug([Some(too_long.as_str()), Some("shop-b")]),
            None
        );
    }
}
