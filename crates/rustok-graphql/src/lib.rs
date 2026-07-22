/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::str::FromStr;

pub const GRAPHQL_ENDPOINT: &str = "/api/graphql";
pub const TENANT_HEADER: &str = "X-Tenant-Slug";
pub const AUTH_HEADER: &str = "Authorization";
pub const ACCEPT_LANGUAGE_HEADER: &str = "Accept-Language";

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
    let client = reqwest::Client::new();
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

    if let Some(errors) = body.errors {
        if let Some(error) = errors.first() {
            return Err(GraphqlHttpError::Graphql(error.message.clone()));
        }
    }

    body.data
        .ok_or_else(|| GraphqlHttpError::Graphql("No data".to_string()))
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
}
