pub mod hooks;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

pub use hooks::{use_lazy_query, use_mutation, use_query, MutationResult, QueryResult};

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

    if let Some(t) = token {
        req = req.header(AUTH_HEADER, format!("Bearer {}", t));
    }

    if let Some(slug) = tenant_slug {
        req = req.header(TENANT_HEADER, slug);
    }

    if let Some(locale) = locale {
        req = req.header(ACCEPT_LANGUAGE_HEADER, locale);
    }

    let res = req.send().await.map_err(|_| GraphqlHttpError::Network)?;

    if res.status() == 401 {
        return Err(GraphqlHttpError::Unauthorized);
    }

    if !res.status().is_success() {
        return Err(GraphqlHttpError::Http(res.status().to_string()));
    }

    let body: GraphqlResponse<T> = res.json().await.map_err(|_| GraphqlHttpError::Network)?;

    if let Some(errors) = body.errors {
        if let Some(err) = errors.first() {
            return Err(GraphqlHttpError::Graphql(err.message.clone()));
        }
    }

    body.data
        .ok_or_else(|| GraphqlHttpError::Graphql("No data".to_string()))
}
