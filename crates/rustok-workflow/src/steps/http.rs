use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;

use async_trait::async_trait;
use reqwest::header::{HeaderName, HeaderValue};
use serde_json::Value;
use tracing::info;

use super::{StepContext, StepOutput, WorkflowStep};
use crate::error::{WorkflowError, WorkflowResult};

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT_SECS: u64 = 60;
const MAX_URL_LEN: usize = 2_048;
const MAX_HEADERS: usize = 32;
const MAX_HEADER_VALUE_LEN: usize = 8 * 1024;
const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const ALLOW_HTTP_ENV: &str = "RUSTOK_WORKFLOW_ALLOW_INSECURE_HTTP";

/// HTTP step — performs a policy-constrained outbound HTTP request.
///
/// Tenant-managed workflows may contact only public HTTP(S) destinations.
/// DNS is resolved before the request and pinned into the client to prevent
/// rebinding between policy validation and connection establishment.
pub struct HttpStep;

impl HttpStep {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HttpStep {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct ValidatedTarget {
    url: reqwest::Url,
    host: String,
    address: SocketAddr,
}

#[async_trait]
impl WorkflowStep for HttpStep {
    fn step_type(&self) -> &'static str {
        "http"
    }

    async fn execute(&self, config: &Value, context: StepContext) -> WorkflowResult<StepOutput> {
        let method = config
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("POST")
            .to_uppercase();
        let method = match method.as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "PATCH" => reqwest::Method::PATCH,
            "DELETE" => reqwest::Method::DELETE,
            value => {
                return Err(WorkflowError::InvalidStepConfig(format!(
                    "http: unsupported method '{value}'"
                )));
            }
        };

        let raw_url = config
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| WorkflowError::InvalidStepConfig("http: missing 'url'".into()))?;
        let target = validate_target(raw_url).await?;
        let timeout_secs = config
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .clamp(1, MAX_TIMEOUT_SECS);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .resolve(&target.host, target.address)
            .build()
            .map_err(|error| {
                WorkflowError::StepFailed(format!("http: failed to build secure client: {error}"))
            })?;

        let mut log_url = target.url.clone();
        log_url.set_query(None);
        log_url.set_fragment(None);
        info!(method = %method, url = %log_url, "Executing policy-constrained HTTP step");

        let mut request = client.request(method.clone(), target.url);
        if let Some(headers) = config.get("headers").and_then(Value::as_object) {
            if headers.len() > MAX_HEADERS {
                return Err(WorkflowError::InvalidStepConfig(format!(
                    "http: at most {MAX_HEADERS} headers are allowed"
                )));
            }
            for (key, value) in headers {
                let Some(value) = value.as_str() else {
                    return Err(WorkflowError::InvalidStepConfig(format!(
                        "http: header '{key}' must be a string"
                    )));
                };
                if value.len() > MAX_HEADER_VALUE_LEN {
                    return Err(WorkflowError::InvalidStepConfig(format!(
                        "http: header '{key}' exceeds the size limit"
                    )));
                }
                let name = HeaderName::from_bytes(key.as_bytes()).map_err(|_| {
                    WorkflowError::InvalidStepConfig(format!("http: invalid header name '{key}'"))
                })?;
                if is_forbidden_header(&name) {
                    return Err(WorkflowError::InvalidStepConfig(format!(
                        "http: header '{name}' is controlled by the HTTP client"
                    )));
                }
                let value = HeaderValue::from_str(value).map_err(|_| {
                    WorkflowError::InvalidStepConfig(format!(
                        "http: invalid value for header '{name}'"
                    ))
                })?;
                request = request.header(name, value);
            }
        }

        if method != reqwest::Method::GET {
            if let Some(body) = config.get("body") {
                let serialized = serde_json::to_vec(body).map_err(|error| {
                    WorkflowError::InvalidStepConfig(format!(
                        "http: failed to serialize request body: {error}"
                    ))
                })?;
                if serialized.len() > MAX_REQUEST_BODY_BYTES {
                    return Err(WorkflowError::InvalidStepConfig(
                        "http: request body exceeds the 1 MiB size limit".to_string(),
                    ));
                }
                request = request.json(body);
            }
        }

        let mut response = request
            .send()
            .await
            .map_err(|error| WorkflowError::StepFailed(format!("http: request failed: {error}")))?;
        let status = response.status();
        if status.is_redirection() {
            return Err(WorkflowError::StepFailed(format!(
                "http: redirects are not allowed (status {})",
                status.as_u16()
            )));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(WorkflowError::StepFailed(
                "http: response exceeds the 1 MiB size limit".to_string(),
            ));
        }

        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| {
            WorkflowError::StepFailed(format!("http: response read failed: {error}"))
        })? {
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(WorkflowError::StepFailed(
                    "http: response exceeds the 1 MiB size limit".to_string(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }

        if !status.is_success() {
            return Err(WorkflowError::StepFailed(format!(
                "http: request returned status {}",
                status.as_u16()
            )));
        }

        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).map_err(|_| {
                WorkflowError::StepFailed(
                    "http: successful response must contain valid JSON".to_string(),
                )
            })?
        };
        let mut new_context = context.clone();
        new_context.set("http_response", body.clone());

        Ok(StepOutput::continue_with(
            new_context,
            serde_json::json!({ "status": status.as_u16(), "body": body }),
        ))
    }
}

async fn validate_target(raw_url: &str) -> WorkflowResult<ValidatedTarget> {
    if raw_url.len() > MAX_URL_LEN {
        return Err(WorkflowError::InvalidStepConfig(
            "http: URL exceeds the size limit".to_string(),
        ));
    }
    let mut url = reqwest::Url::parse(raw_url)
        .map_err(|error| WorkflowError::InvalidStepConfig(format!("http: invalid URL: {error}")))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(WorkflowError::InvalidStepConfig(
            "http: URL userinfo is not allowed".to_string(),
        ));
    }
    match url.scheme() {
        "https" => {}
        "http" if insecure_http_allowed() => {}
        "http" => {
            return Err(WorkflowError::InvalidStepConfig(format!(
                "http: insecure HTTP is disabled; host may opt in with {ALLOW_HTTP_ENV}"
            )));
        }
        scheme => {
            return Err(WorkflowError::InvalidStepConfig(format!(
                "http: unsupported URL scheme '{scheme}'"
            )));
        }
    }

    let host = canonicalize_request_host(&mut url)?;
    if host.is_empty()
        || host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".home.arpa")
    {
        return Err(WorkflowError::InvalidStepConfig(
            "http: local or internal hostnames are not allowed".to_string(),
        ));
    }

    let port = url.port_or_known_default().ok_or_else(|| {
        WorkflowError::InvalidStepConfig("http: destination port is required".to_string())
    })?;
    let addresses = if let Ok(ip) = IpAddr::from_str(&host) {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::net::lookup_host((host.as_str(), port))
            .await
            .map_err(|error| {
                WorkflowError::StepFailed(format!("http: DNS resolution failed: {error}"))
            })?
            .collect::<Vec<_>>()
    };
    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(WorkflowError::InvalidStepConfig(
            "http: destination resolves to a private, local, reserved, or multicast address"
                .to_string(),
        ));
    }

    Ok(ValidatedTarget {
        url,
        host,
        address: addresses[0],
    })
}

fn canonicalize_request_host(url: &mut reqwest::Url) -> WorkflowResult<String> {
    let host = url
        .host_str()
        .ok_or_else(|| WorkflowError::InvalidStepConfig("http: URL host is required".into()))?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() {
        return Err(WorkflowError::InvalidStepConfig(
            "http: URL host is required".to_string(),
        ));
    }
    url.set_host(Some(&host)).map_err(|_| {
        WorkflowError::InvalidStepConfig("http: URL host could not be canonicalized".to_string())
    })?;
    Ok(host)
}

fn insecure_http_allowed() -> bool {
    cfg!(test)
        || std::env::var(ALLOW_HTTP_ENV)
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

fn is_forbidden_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "host"
            | "content-length"
            | "transfer-encoding"
            | "connection"
            | "proxy-authorization"
            | "proxy-authenticate"
            | "upgrade"
            | "te"
            | "trailer"
    )
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || a == 0
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 192 && b == 0)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && ip.octets()[2] == 100)
        || (a == 203 && b == 0 && ip.octets()[2] == 113))
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(ipv4) = ip.to_ipv4() {
        return is_public_ipv4(ipv4);
    }

    let segments = ip.segments();
    let first = segments[0];
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
        || (first & 0xffc0) == 0xfec0
        || (first & 0xff00) == 0xff00
        || (segments[0] == 0x0064 && segments[1] == 0xff9b)
        || (segments[0] == 0x0100 && segments[1] == 0)
        || (segments[0] == 0x2001 && segments[1] == 0)
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || segments[0] == 0x2002)
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_request_host, is_public_ip, validate_target};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn blocks_metadata_private_and_local_addresses() {
        for ip in [
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6("fd00::1".parse().unwrap()),
            IpAddr::V6("::ffff:127.0.0.1".parse().unwrap()),
            IpAddr::V6("::127.0.0.1".parse().unwrap()),
        ] {
            assert!(!is_public_ip(ip), "{ip} must be blocked");
        }
        assert!(is_public_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }

    #[test]
    fn canonical_host_matches_the_host_used_for_dns_pinning() {
        let mut url = reqwest::Url::parse("https://Example.COM./hook").unwrap();
        let host = canonicalize_request_host(&mut url).unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(url.host_str(), Some("example.com"));
    }

    #[tokio::test]
    async fn rejects_literal_metadata_endpoint_before_request() {
        assert!(
            validate_target("http://169.254.169.254/latest/meta-data")
                .await
                .is_err()
        );
        assert!(validate_target("file:///etc/passwd").await.is_err());
        assert!(validate_target("http://localhost:8080/hook").await.is_err());
    }
}
