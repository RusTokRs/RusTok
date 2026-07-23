use super::*;
use reqwest::blocking::Client;
use reqwest::Url;
use serde::de::DeserializeOwned;
use std::net::IpAddr;

pub(crate) fn post_registry_json<T>(endpoint: &str, payload: &T) -> Result<String>
where
    T: Serialize,
{
    let value: serde_json::Value = post_registry_json_parsed(endpoint, payload, None)?;
    pretty_json(&value)
}

pub(crate) fn post_registry_json_parsed<T, U>(
    endpoint: &str,
    payload: &T,
    auth_token: Option<&str>,
) -> Result<U>
where
    T: Serialize,
    U: DeserializeOwned,
{
    let client = build_registry_http_client(endpoint)?;
    let mut request = client.post(endpoint).json(payload);
    if let Some(auth_token) = auth_token {
        request = request.bearer_auth(auth_token);
    }
    let response = request
        .send()
        .with_context(|| format!("Failed to call registry endpoint {endpoint}"))?;
    parse_registry_json_response(endpoint, response)
}

pub(crate) fn post_registry_json_with_runner_token_parsed<T, U>(
    endpoint: &str,
    payload: &T,
    runner_token: &str,
) -> Result<U>
where
    T: Serialize,
    U: DeserializeOwned,
{
    let client = build_registry_http_client(endpoint)?;
    let response = client
        .post(endpoint)
        .header("x-rustok-runner-token", runner_token)
        .json(payload)
        .send()
        .with_context(|| format!("Failed to call registry runner endpoint {endpoint}"))?;
    parse_registry_json_response(endpoint, response)
}

pub(crate) fn put_registry_bytes_parsed<U>(
    endpoint: &str,
    payload: &[u8],
    content_type: &str,
    auth_token: Option<&str>,
) -> Result<U>
where
    U: DeserializeOwned,
{
    let client = build_registry_http_client(endpoint)?;
    let mut request = client
        .put(endpoint)
        .header("content-type", content_type)
        .body(payload.to_vec());
    if let Some(auth_token) = auth_token {
        request = request.bearer_auth(auth_token);
    }
    let response = request
        .send()
        .with_context(|| format!("Failed to call registry upload endpoint {endpoint}"))?;
    parse_registry_json_response(endpoint, response)
}

pub(crate) fn get_registry_json_parsed<U>(endpoint: &str, auth_token: Option<&str>) -> Result<U>
where
    U: DeserializeOwned,
{
    let client = build_registry_http_client(endpoint)?;
    let mut request = client.get(endpoint);
    if let Some(auth_token) = auth_token {
        request = request.bearer_auth(auth_token);
    }
    let response = request
        .send()
        .with_context(|| format!("Failed to call registry endpoint {endpoint}"))?;
    parse_registry_json_response(endpoint, response)
}

fn parse_registry_json_response<U>(
    endpoint: &str,
    response: reqwest::blocking::Response,
) -> Result<U>
where
    U: DeserializeOwned,
{
    let response = response
        .error_for_status()
        .with_context(|| format!("Registry endpoint {endpoint} returned an error status"))?;
    response
        .json::<U>()
        .with_context(|| format!("Failed to parse registry response from {endpoint}"))
}

pub(crate) fn pretty_json<T>(value: &T) -> Result<String>
where
    T: Serialize,
{
    serde_json::to_string_pretty(value).context("Failed to pretty-print registry payload")
}

fn build_registry_http_client(endpoint: &str) -> Result<Client> {
    let mut builder = Client::builder().timeout(std::time::Duration::from_secs(15));
    if registry_endpoint_uses_loopback(endpoint) {
        builder = builder.no_proxy();
    }

    builder
        .build()
        .context("Failed to build registry HTTP client")
}

pub(crate) fn registry_endpoint_uses_loopback(endpoint: &str) -> bool {
    Url::parse(endpoint)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_string()))
        .is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .trim_matches(|ch| ch == '[' || ch == ']')
                    .parse::<IpAddr>()
                    .map(|address| address.is_loopback())
                    .unwrap_or(false)
        })
}
