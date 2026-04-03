pub mod queries;

use leptos::prelude::*;
use leptos_graphql::{
    execute as execute_graphql, persisted_query_extension, GraphqlHttpError, GraphqlRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ApiRequestContext {
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ServerGraphqlRequest {
    query: String,
    variables: Value,
    persisted_query_sha256: Option<String>,
    context: ApiRequestContext,
}

pub fn get_graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|w| w.location().origin().ok())
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

pub fn api_base_url() -> String {
    get_graphql_url()
        .trim_end_matches("/api/graphql")
        .trim_end_matches('/')
        .to_string()
}

pub fn get_graphql_ws_url() -> String {
    let graphql_url = get_graphql_url();
    let ws_base = if let Some(rest) = graphql_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = graphql_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        graphql_url
    };

    format!("{}/ws", ws_base.trim_end_matches('/'))
}

pub type ApiError = GraphqlHttpError;

/// Read the admin UI locale from LocalStorage.
/// Returns None in non-WASM environments or if the key is absent.
pub fn get_stored_locale() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        gloo_storage::LocalStorage::get::<String>("rustok-admin-locale").ok()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

fn build_request_context(token: Option<String>, tenant_slug: Option<String>) -> ApiRequestContext {
    ApiRequestContext {
        token,
        tenant_slug,
        locale: get_stored_locale(),
    }
}

async fn execute_server_graphql(request: ServerGraphqlRequest) -> Result<Value, GraphqlHttpError> {
    let mut graphql_request = GraphqlRequest::new(request.query, Some(request.variables));

    if let Some(sha256_hash) = request.persisted_query_sha256.as_deref() {
        graphql_request = graphql_request.with_extensions(persisted_query_extension(sha256_hash));
    }

    execute_graphql(
        &get_graphql_url(),
        graphql_request,
        request.context.token,
        request.context.tenant_slug,
        request.context.locale,
    )
    .await
}

fn map_server_fn_error(error: ServerFnError) -> ApiError {
    let message = error.to_string();

    if message == "Unauthorized" {
        ApiError::Unauthorized
    } else if message == "Network error" {
        ApiError::Network
    } else if let Some(value) = message.strip_prefix("Http error: ") {
        ApiError::Http(value.to_string())
    } else if let Some(value) = message.strip_prefix("GraphQL error: ") {
        ApiError::Graphql(value.to_string())
    } else {
        ApiError::Graphql(message)
    }
}

pub async fn extract_http_error(response: reqwest::Response) -> String {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return format!("request failed with status {status}");
    }

    if let Ok(payload) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(message) = payload
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| payload.get("error").and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
    }

    trimmed.to_string()
}

#[server(prefix = "/api/fn", endpoint = "admin/graphql")]
async fn admin_graphql(request: ServerGraphqlRequest) -> Result<Value, ServerFnError> {
    execute_server_graphql(request)
        .await
        .map_err(|err| ServerFnError::ServerError(err.to_string()))
}

pub async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    let response = admin_graphql(ServerGraphqlRequest {
        query: query.to_string(),
        variables: serde_json::to_value(variables)
            .map_err(|err| ApiError::Graphql(err.to_string()))?,
        persisted_query_sha256: None,
        context: build_request_context(token, tenant_slug),
    })
    .await
    .map_err(map_server_fn_error)?;

    serde_json::from_value(response).map_err(|err| ApiError::Graphql(err.to_string()))
}

pub async fn request_with_persisted<V, T>(
    query: &str,
    variables: V,
    sha256_hash: &str,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    let response = admin_graphql(ServerGraphqlRequest {
        query: query.to_string(),
        variables: serde_json::to_value(variables)
            .map_err(|err| ApiError::Graphql(err.to_string()))?,
        persisted_query_sha256: Some(sha256_hash.to_string()),
        context: build_request_context(token, tenant_slug),
    })
    .await
    .map_err(map_server_fn_error)?;

    serde_json::from_value(response).map_err(|err| ApiError::Graphql(err.to_string()))
}
