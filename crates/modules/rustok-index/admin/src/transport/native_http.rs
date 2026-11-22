use leptos::{prelude::ServerFnError, server_fn::ServerFn};
use reqwest::{
    Client, Url,
    header::{ACCEPT, AUTHORIZATION, HeaderMap},
};
use serde::{Serialize, de::DeserializeOwned};

use super::native_server_adapter::{
    CancelJobNative, FetchBootstrapNative, RetryJobNative, TriggerReplayNative,
};

#[derive(Clone, Copy)]
pub(super) enum IndexEndpoint {
    Bootstrap,
    Trigger,
    Cancel,
    Retry,
}

impl IndexEndpoint {
    fn path(self) -> &'static str {
        match self {
            Self::Bootstrap => FetchBootstrapNative::PATH,
            Self::Trigger => TriggerReplayNative::PATH,
            Self::Cancel => CancelJobNative::PATH,
            Self::Retry => RetryJobNative::PATH,
        }
    }
}

const TENANT_SLUG: &str = "x-tenant-slug";
const TENANT_ID: &str = "x-tenant-id";
const RELAY_HEADER: &str = "x-rustok-index-relay";

/// Standalone SSR relay configuration. The backend comes from trusted host configuration, never
/// a request. Both halves of the relay use the generated Index server functions' wire contract.
#[derive(Clone)]
pub struct IndexAdminUpstream {
    target: Option<(Url, Client)>,
}

impl IndexAdminUpstream {
    pub fn new(base: &str) -> Self {
        let target = Url::parse(base)
            .ok()
            .filter(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.has_host()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.path() == "/"
                    && url.query().is_none()
                    && url.fragment().is_none()
            })
            .and_then(|origin| {
                Client::builder()
                    .timeout(std::time::Duration::from_secs(180))
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .ok()
                    .map(|client| (origin, client))
            });
        Self { target }
    }

    /// These request routes use bearer authentication, not the host's SSR refresh cookie.
    pub fn is_request_path(path: &str) -> bool {
        [
            IndexEndpoint::Bootstrap,
            IndexEndpoint::Trigger,
            IndexEndpoint::Cancel,
            IndexEndpoint::Retry,
        ]
        .into_iter()
        .any(|endpoint| endpoint.path() == path)
    }

    pub(super) async fn call<T: DeserializeOwned>(
        &self,
        endpoint: IndexEndpoint,
        token: Option<String>,
        tenant: Option<String>,
        body: impl Serialize,
    ) -> Result<T, ServerFnError> {
        let (origin, client) = self
            .target
            .as_ref()
            .ok_or_else(|| ServerFnError::new("Index upstream is not configured"))?;
        let url = origin
            .join(endpoint.path())
            .map_err(|_| ServerFnError::new("Invalid Index upstream address"))?;
        let token = token
            .filter(|token| !token.is_empty())
            .ok_or_else(|| ServerFnError::new("Authentication required"))?;
        let tenant = tenant
            .filter(|tenant| !tenant.is_empty())
            .ok_or_else(|| ServerFnError::new("Index tenant required"))?;
        let response = client
            .post(url)
            .bearer_auth(token)
            .header(TENANT_ID, tenant.as_str())
            .header(TENANT_SLUG, tenant)
            .header(ACCEPT, "application/json")
            .header(RELAY_HEADER, "1")
            .json(&body)
            .send()
            .await
            .map_err(|_| ServerFnError::new("Index backend could not be reached"))?;
        if !response.status().is_success() {
            let status = response.status();
            if let Ok(text) = response.text().await {
                // Preserve our controlled server-function error, not an HTML error page or
                // a driver error that may contain credential-bearing request metadata.
                if let Some(message) = text
                    .strip_prefix("ServerError|")
                    .filter(|text| text.len() <= 512)
                {
                    return Err(ServerFnError::new(message));
                }
            }
            return Err(ServerFnError::new(format!(
                "Index backend returned HTTP {status}"
            )));
        }
        response
            .json()
            .await
            .map_err(|_| ServerFnError::new("Invalid Index backend response"))
    }

    pub(super) async fn relay<T: DeserializeOwned>(
        &self,
        endpoint: IndexEndpoint,
        body: impl Serialize,
    ) -> Result<T, ServerFnError> {
        let headers = leptos_axum::extract::<HeaderMap>()
            .await
            .map_err(|_| ServerFnError::new("Index request headers are missing"))?;
        let (token, tenant) = request_scope(&headers)?;
        self.call(endpoint, Some(token), Some(tenant), body).await
    }
}

fn request_scope(headers: &HeaderMap) -> Result<(String, String), ServerFnError> {
    let invalid_scope =
        || ServerFnError::new("Index relay requires one authenticated request scope");
    if headers.contains_key(RELAY_HEADER)
        || headers.get_all(AUTHORIZATION).iter().count() != 1
        || headers.get_all(TENANT_SLUG).iter().count() != 1
        || headers.get_all(TENANT_ID).iter().count() > 1
    {
        return Err(invalid_scope());
    }
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or_else(invalid_scope)?;
    let tenant = headers
        .get(TENANT_SLUG)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.trim() == *value)
        .ok_or_else(invalid_scope)?;
    if headers
        .get(TENANT_ID)
        .is_some_and(|identifier| identifier.as_bytes() != tenant.as_bytes())
    {
        return Err(invalid_scope());
    }
    Ok((token.to_owned(), tenant.to_owned()))
}

#[cfg(test)]
mod tests {
    use reqwest::header::HeaderValue;

    use super::*;

    #[test]
    fn relay_refuses_missing_duplicate_conflicting_and_relayed_scope() {
        let mut request = HeaderMap::new();
        request.insert(AUTHORIZATION, HeaderValue::from_static("Bearer fixture"));
        assert!(request_scope(&request).is_err());
        request.insert(TENANT_SLUG, HeaderValue::from_static("fixture-tenant"));
        request.insert(TENANT_ID, HeaderValue::from_static("fixture-tenant"));
        assert_eq!(
            request_scope(&request).unwrap(),
            ("fixture".into(), "fixture-tenant".into())
        );
        request.insert(TENANT_ID, HeaderValue::from_static("other-tenant"));
        assert!(request_scope(&request).is_err());
        request.remove(TENANT_ID);
        request.append(TENANT_SLUG, HeaderValue::from_static("fixture-tenant"));
        assert!(request_scope(&request).is_err());
        request.insert(TENANT_SLUG, HeaderValue::from_static("fixture-tenant"));
        request.insert(RELAY_HEADER, HeaderValue::from_static("1"));
        assert!(request_scope(&request).is_err());
    }

    #[test]
    fn only_the_index_server_fn_paths_opt_out_of_ssr_cookie_rotation() {
        assert!(IndexAdminUpstream::is_request_path(
            "/api/fn/index/bootstrap"
        ));
        assert!(IndexAdminUpstream::is_request_path(
            "/api/fn/index/cancel-job"
        ));
        assert!(!IndexAdminUpstream::is_request_path("/modules/index"));
        assert!(!IndexAdminUpstream::is_request_path("/api/fn/index/other"));
    }
}
