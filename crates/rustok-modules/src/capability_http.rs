use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, Method};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use url::Url;

use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    SandboxError, SandboxResult,
};

use crate::artifact_capability_router::{
    resolve_granted_artifact_capability, ArtifactCapabilityBrokerResolver,
    ArtifactCapabilityExecution,
};

pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024; // 10 MB

/// Production broker for the `platform.http` sandbox capability.
///
/// Executes outbound HTTP requests on behalf of admitted guest modules within
/// the bounds enforced by `HttpCapabilityConstraints` with redirect and SSRF isolation.
#[derive(Clone)]
pub struct ArtifactHttpCapabilityBroker {
    client: Client,
    timeout: Duration,
    max_response_bytes: usize,
}

impl ArtifactHttpCapabilityBroker {
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(10))
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self::with_timeout_and_limit(timeout, DEFAULT_MAX_RESPONSE_BYTES)
    }

    pub fn with_timeout_and_limit(timeout: Duration, max_response_bytes: usize) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_default();
        Self {
            client,
            timeout,
            max_response_bytes,
        }
    }
}

impl Default for ArtifactHttpCapabilityBroker {
    fn default() -> Self {
        Self::new()
    }
}

fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback()
                || ipv4.is_private()
                || ipv4.is_link_local()
                || ipv4.is_broadcast()
                || ipv4.is_unspecified()
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

fn is_disallowed_hostname(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    lower == "localhost"
        || lower.ends_with(".localhost")
        || lower.ends_with(".local")
        || lower.ends_with(".internal")
        || lower.ends_with(".lan")
}

async fn validate_target_address(url: &Url, capability: &CapabilityName) -> SandboxResult<()> {
    let host = url
        .host_str()
        .ok_or_else(|| SandboxError::InvalidRequest("HTTP url must include a host".into()))?;

    if is_disallowed_hostname(host) {
        return Err(SandboxError::HostCapability {
            capability: capability.clone(),
            message: format!("HTTP destination host `{host}` is disallowed (local/internal hostname)"),
        });
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_disallowed_ip(ip) {
            return Err(SandboxError::HostCapability {
                capability: capability.clone(),
                message: format!("HTTP destination IP `{ip}` is disallowed (private/loopback/metadata)"),
            });
        }
        return Ok(());
    }

    let port = url.port_or_known_default().unwrap_or(443);
    if let Ok(addrs) = tokio::net::lookup_host((host, port)).await {
        for addr in addrs {
            if is_disallowed_ip(addr.ip()) {
                return Err(SandboxError::HostCapability {
                    capability: capability.clone(),
                    message: format!(
                        "HTTP destination host `{host}` resolves to disallowed IP `{}`",
                        addr.ip()
                    ),
                });
            }
        }
    }

    Ok(())
}

#[async_trait]
impl CapabilityBroker for ArtifactHttpCapabilityBroker {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        let input = call
            .input
            .as_object()
            .ok_or_else(|| SandboxError::InvalidRequest("HTTP input must be an object".into()))?;

        let method_str = input
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| SandboxError::InvalidRequest("HTTP method is required".into()))?;

        let url_str = input
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| SandboxError::InvalidRequest("HTTP url is required".into()))?;

        let parsed_url = Url::parse(url_str).map_err(|err| {
            SandboxError::InvalidRequest(format!("HTTP url is invalid: {err}"))
        })?;

        validate_target_address(&parsed_url, &call.capability).await?;

        let method = match method_str.to_ascii_uppercase().as_str() {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "PUT" => Method::PUT,
            "PATCH" => Method::PATCH,
            "DELETE" => Method::DELETE,
            "HEAD" => Method::HEAD,
            "OPTIONS" => Method::OPTIONS,
            other => {
                return Err(SandboxError::InvalidRequest(format!(
                    "unsupported HTTP method `{other}`"
                )));
            }
        };

        let mut request_builder = self.client.request(method, url_str).timeout(self.timeout);

        if let Some(headers_obj) = input.get("headers").and_then(Value::as_object) {
            for (key, val) in headers_obj {
                if let Some(val_str) = val.as_str() {
                    request_builder = request_builder.header(key.as_str(), val_str);
                }
            }
        }

        if let Some(body_val) = input.get("body") {
            if let Some(body_str) = body_val.as_str() {
                request_builder = request_builder.body(body_str.to_string());
            } else if !body_val.is_null() {
                request_builder = request_builder.json(body_val);
            }
        }

        let response = request_builder.send().await.map_err(|err| {
            SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!("HTTP request failed: {err}"),
            }
        })?;

        let status = response.status().as_u16();

        if let Some(content_length) = response.content_length() {
            if content_length > self.max_response_bytes as u64 {
                return Err(SandboxError::HostCapability {
                    capability: call.capability.clone(),
                    message: format!(
                        "HTTP response content length ({content_length} bytes) exceeds limit of {} bytes",
                        self.max_response_bytes
                    ),
                });
            }
        }

        let mut response_headers = HashMap::new();
        for (key, val) in response.headers() {
            if let Ok(val_str) = val.to_str() {
                response_headers.insert(key.as_str().to_string(), val_str.to_string());
            }
        }

        let bytes = response.bytes().await.map_err(|err| {
            SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!("reading HTTP response body failed: {err}"),
            }
        })?;

        if bytes.len() > self.max_response_bytes {
            return Err(SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!(
                    "HTTP response body size ({} bytes) exceeds limit of {} bytes",
                    bytes.len(),
                    self.max_response_bytes
                ),
            });
        }

        let body_text = String::from_utf8_lossy(&bytes).to_string();

        Ok(CapabilityResponse {
            output: json!({
                "status": status,
                "headers": response_headers,
                "body": body_text,
            }),
        })
    }
}

/// Deployment-owned resolver for the `platform.http` capability broker.
#[derive(Clone)]
pub struct SeaOrmArtifactHttpCapabilityBrokerResolver {
    db: DatabaseConnection,
    broker: Arc<ArtifactHttpCapabilityBroker>,
}

impl SeaOrmArtifactHttpCapabilityBrokerResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            broker: Arc::new(ArtifactHttpCapabilityBroker::new()),
        }
    }

    pub fn with_timeout(db: DatabaseConnection, timeout: Duration) -> Self {
        Self {
            db,
            broker: Arc::new(ArtifactHttpCapabilityBroker::with_timeout(timeout)),
        }
    }
}

#[async_trait]
impl ArtifactCapabilityBrokerResolver for SeaOrmArtifactHttpCapabilityBrokerResolver {
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
        if capability.as_str() != "platform.http" {
            return Err(SandboxError::CapabilityDenied(capability.clone()));
        }
        resolve_granted_artifact_capability(&self.db, execution, capability).await?;
        Ok(self.broker.clone())
    }
}
