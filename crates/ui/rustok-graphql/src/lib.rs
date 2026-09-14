/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::Duration;

pub const GRAPHQL_ENDPOINT: &str = "/api/graphql";
pub const TENANT_HEADER: &str = "X-Tenant-Slug";
pub const AUTH_HEADER: &str = "Authorization";
pub const ACCEPT_LANGUAGE_HEADER: &str = "Accept-Language";
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub fn default_client() -> &'static ClientWithMiddleware {
    static CLIENT: OnceLock<ClientWithMiddleware> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let mut builder = reqwest::Client::builder().timeout(DEFAULT_TIMEOUT);

        #[cfg(not(target_arch = "wasm32"))]
        {
            builder = builder.pool_max_idle_per_host(10);
        }

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

        ClientBuilder::new(builder.build().expect("failed to build reqwest client"))
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build()
    })
}

pub fn graphql_endpoint_from_base(base: &str) -> String {
    if let Ok(base_url) = url::Url::parse(base) {
        if let Ok(joined) = base_url.join(GRAPHQL_ENDPOINT) {
            return joined.to_string();
        }
    }
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        GRAPHQL_ENDPOINT.trim_start_matches('/')
    )
}

pub fn default_graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        graphql_endpoint_from_base(&origin)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(url) = std::env::var("RUSTOK_GRAPHQL_URL") {
            return url;
        }
        let base = std::env::var("RUSTOK_API_URL")
            .unwrap_or_else(|_| "http://localhost:5150".to_string());
        graphql_endpoint_from_base(&base)
    }
}

pub use default_graphql_url as graphql_url;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GraphqlRequest<V = Value> {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<V>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

impl<V> GraphqlRequest<V> {
    pub fn new(query: impl Into<String>, variables: Option<V>) -> Self {
        Self {
            query: query.into(),
            variables,
            extensions: None,
        }
    }

    pub fn with_extensions(mut self, extensions: Value) -> Self {
        self.extensions = Some(extensions);
        self
    }

    pub fn is_mutation(&self) -> bool {
        self.query.trim_start().starts_with("mutation")
    }

    pub fn is_query(&self) -> bool {
        !self.is_mutation()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GraphqlResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphqlError>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphqlError {
    pub message: String,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphqlHttpError {
    #[error("Network error")]
    Network,
    #[error("GraphQL error: {0}")]
    Graphql(String),
    #[error("Http error: {0}")]
    Http(String),
    #[error("Unauthorized")]
    Unauthorized,
}

impl FromStr for GraphqlHttpError {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "Network error" {
            return Ok(Self::Network);
        }

        if value == "Unauthorized" {
            return Ok(Self::Unauthorized);
        }

        if let Some(message) = value.strip_prefix("GraphQL error: ") {
            return Ok(Self::Graphql(message.to_string()));
        }

        if let Some(message) = value.strip_prefix("Http error: ") {
            return Ok(Self::Http(message.to_string()));
        }

        Err(format!("Unknown GraphqlHttpError: {value}"))
    }
}

pub fn persisted_query_extension(sha256_hash: &str) -> Value {
    serde_json::json!({
        "persistedQuery": {
            "version": 1,
            "sha256Hash": sha256_hash,
        }
    })
}

pub async fn execute<V, T>(
    endpoint: &str,
    request: GraphqlRequest<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<T, GraphqlHttpError>
where
    V: Serialize,
    T: DeserializeOwned,
{
    execute_with_client(
        default_client(),
        endpoint,
        request,
        token,
        tenant_slug,
        locale,
    )
    .await
}

pub async fn execute_with_client<V, T>(
    client: &ClientWithMiddleware,
    endpoint: &str,
    request: GraphqlRequest<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<T, GraphqlHttpError>
where
    V: Serialize,
    T: DeserializeOwned,
{
    let mut req = client.post(endpoint).json(&request);

    if let Some(token) = token {
        req = req.header(AUTH_HEADER, format!("Bearer {token}"));
    }

    if let Some(tenant_slug) = tenant_slug {
        req = req.header(TENANT_HEADER, tenant_slug);
    }

    if let Some(locale) = locale {
        req = req.header(ACCEPT_LANGUAGE_HEADER, locale);
    }

    let response = req.send().await.map_err(|_| GraphqlHttpError::Network)?;

    if response.status() == 401 {
        return Err(GraphqlHttpError::Unauthorized);
    }

    if !response.status().is_success() {
        return Err(GraphqlHttpError::Http(response.status().to_string()));
    }

    let body: GraphqlResponse<T> = response
        .json()
        .await
        .map_err(|_| GraphqlHttpError::Network)?;

    if let Some(errors) = body.errors
        && let Some(error) = errors.first()
    {
        return Err(GraphqlHttpError::Graphql(error.message.clone()));
    }

    body.data
        .ok_or_else(|| GraphqlHttpError::Graphql("No data".to_string()))
}

pub async fn execute_with_raw_client<V, T>(
    client: &reqwest::Client,
    endpoint: &str,
    request: GraphqlRequest<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<T, GraphqlHttpError>
where
    V: Serialize,
    T: DeserializeOwned,
{
    let client = ClientBuilder::new(client.clone()).build();
    execute_with_client(&client, endpoint, request, token, tenant_slug, locale).await
}


#[cfg(test)]
mod tests {
    use super::{GraphqlHttpError, GraphqlRequest, persisted_query_extension};
    use serde_json::json;
    use std::str::FromStr;

    #[test]
    fn request_omits_empty_optional_fields() {
        let request = GraphqlRequest::<serde_json::Value>::new("query Test { ok }", None);
        let value = serde_json::to_value(request).expect("request serializes");

        assert_eq!(value, json!({ "query": "query Test { ok }" }));
    }

    #[test]
    fn persisted_query_extension_uses_apq_shape() {
        assert_eq!(
            persisted_query_extension("abc"),
            json!({
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": "abc"
                }
            })
        );
    }

    #[test]
    fn graphql_http_error_round_trips_display_strings() {
        assert_eq!(
            GraphqlHttpError::from_str("GraphQL error: denied"),
            Ok(GraphqlHttpError::Graphql("denied".to_string()))
        );
        assert_eq!(
            GraphqlHttpError::from_str("Http error: 500"),
            Ok(GraphqlHttpError::Http("500".to_string()))
        );
        assert_eq!(
            GraphqlHttpError::from_str("Unauthorized"),
            Ok(GraphqlHttpError::Unauthorized)
        );
    }

    #[test]
    fn default_client_returns_same_instance() {
        let client1 = super::default_client();
        let client2 = super::default_client();
        assert!(std::ptr::eq(client1, client2));
    }

    #[test]
    fn graphql_endpoint_from_base_normalizes_correctly() {
        assert_eq!(
            super::graphql_endpoint_from_base("http://localhost:5150"),
            "http://localhost:5150/api/graphql"
        );
        assert_eq!(
            super::graphql_endpoint_from_base("http://localhost:5150/"),
            "http://localhost:5150/api/graphql"
        );
        assert_eq!(
            super::graphql_endpoint_from_base("/prefix"),
            "/prefix/api/graphql"
        );
    }

    #[test]
    fn graphql_request_identifies_operation_type() {
        let query = GraphqlRequest::<()>::new("query GetItems { items { id } }", None);
        assert!(query.is_query());
        assert!(!query.is_mutation());

        let anonymous_query = GraphqlRequest::<()>::new("{ items { id } }", None);
        assert!(anonymous_query.is_query());
        assert!(!anonymous_query.is_mutation());

        let mutation = GraphqlRequest::<()>::new("mutation CreateItem { create { id } }", None);
        assert!(mutation.is_mutation());
        assert!(!mutation.is_query());
    }
}

