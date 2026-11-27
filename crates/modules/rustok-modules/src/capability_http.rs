use std::collections::HashMap;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::{Client, Method};
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use url::Url;

use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    SandboxError, SandboxResult,
};

use crate::artifact_capability_router::{
    ArtifactCapabilityBrokerResolver, ArtifactCapabilityExecution,
    resolve_granted_artifact_capability,
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
    pub fn new() -> SandboxResult<Self> {
        Self::with_timeout(Duration::from_secs(10))
    }

    pub fn with_timeout(timeout: Duration) -> SandboxResult<Self> {
        Self::with_timeout_and_limit(timeout, DEFAULT_MAX_RESPONSE_BYTES)
    }

    pub fn with_timeout_and_limit(
        timeout: Duration,
        max_response_bytes: usize,
    ) -> SandboxResult<Self> {
        if timeout.is_zero() || max_response_bytes == 0 {
            return Err(SandboxError::InvalidRequest(
                "HTTP capability timeout and response limit must be positive".to_string(),
            ));
        }
        // Process/system proxies must not redirect a DNS-pinned request through
        // a proxy that performs its own destination lookup.
        let capability = CapabilityName::new("platform.http")?;
        let client = restricted_http_client_builder(timeout)
            .build()
            .map_err(|error| SandboxError::HostCapability {
                capability: capability.clone(),
                message: format!("failed to construct HTTP capability client: {error}"),
            })?;
        Ok(Self {
            client,
            timeout,
            max_response_bytes,
        })
    }
}

fn is_disallowed_ipv4(ipv4: Ipv4Addr) -> bool {
    let octets = ipv4.octets();
    ipv4.is_loopback()
        || ipv4.is_private()
        || ipv4.is_link_local()
        || ipv4.is_broadcast()
        || ipv4.is_unspecified()
        || ipv4.is_multicast()
        || ipv4.is_documentation()
        // 0.0.0.0/8 "this network", 100.64.0.0/10 CGNAT shared space,
        // 192.0.0.0/24 IETF protocol assignments, 198.18.0.0/15
        // benchmarking, 240.0.0.0/4 reserved.
        || octets[0] == 0
        || (octets[0] == 100 && (octets[1] & 0xc0) == 0x40)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 198 && (octets[1] & 0xfe) == 18)
        || octets[0] >= 240
}

/// Extracts the IPv4 address embedded in transition/compatibility prefixes
/// whose traffic can be delivered to that IPv4 destination: the NAT64
/// well-known prefix `64:ff9b::/96`, the RFC 8215 local-use prefix
/// `64:ff9b:1::/48`, deprecated IPv4-compatible `::/96`, and 6to4
/// `2002::/16`.
fn embedded_ipv4(ipv6: std::net::Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = ipv6.segments();
    if segments[..6] == [0x0064, 0xff9b, 0, 0, 0, 0]
        || segments[..3] == [0x0064, 0xff9b, 0x0001]
        || segments[..6] == [0; 6]
    {
        return Some(Ipv4Addr::from(
            u32::from(segments[6]) << 16 | u32::from(segments[7]),
        ));
    }
    if segments[0] == 0x2002 {
        return Some(Ipv4Addr::from(
            u32::from(segments[1]) << 16 | u32::from(segments[2]),
        ));
    }
    None
}

fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => is_disallowed_ipv4(ipv4),
        IpAddr::V6(ipv6) => {
            // IPv4-mapped IPv6 literals connect to the embedded IPv4
            // address on dual-stack hosts, so they must pass the IPv4 rules.
            if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                return is_disallowed_ipv4(ipv4);
            }
            // NAT64/6to4 traffic is delivered to the embedded IPv4
            // destination, so it must pass the same rules.
            if let Some(ipv4) = embedded_ipv4(ipv6) {
                return is_disallowed_ipv4(ipv4);
            }
            let segments = ipv6.segments();
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || ipv6.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
                // Deprecated site-local unicast remains locally routable on
                // networks that still configure it.
                || (segments[0] & 0xffc0) == 0xfec0
                // Reject non-globally-reachable special-purpose ranges,
                // but retain more-specific IANA allocations that are marked
                // globally reachable (for example PCP anycast and AMT).
                || (segments[0] == 0x2001 && segments[1] == 0x0000) // Teredo 2001::/32
                || (segments[0] == 0x2001
                    && segments[1] == 0x0002
                    && segments[2] == 0x0000) // Benchmarking 2001:2::/48
                || (segments[0] == 0x2001
                    && segments[1] == 0x0010
                    && (segments[2] & 0xfff0) == 0) // Deprecated ORCHID 2001:10::/28
                || (segments[0] == 0x2001 && segments[1] == 0x0db8) // Documentation
                || (segments[0] == 0x3fff && (segments[1] & 0xf000) == 0) // Documentation 3fff::/20
                || (segments[0] == 0x0100
                    && segments[1] == 0
                    && segments[2] == 0
                    && segments[3] == 0) // Discard-only 100::/64
                || (segments[0] == 0x0100
                    && segments[1] == 0
                    && segments[2] == 0
                    && segments[3] == 1) // Dummy prefix 100:0:0:1::/64
                || segments[0] == 0x5f00 // Segment Routing SIDs 5f00::/16
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

/// Builds the one restricted client policy used for both shared and
/// DNS-pinned destinations. Disabling process/system proxies is essential:
/// otherwise reqwest may send the URL to a proxy which performs its own DNS
/// lookup, bypassing the validated and pinned destination address.
fn restricted_http_client_builder(timeout: Duration) -> reqwest::ClientBuilder {
    Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
}

/// Resolves one hostname within the remaining capability deadline. The generic
/// future keeps timeout behavior deterministic in tests without mutating global
/// resolver configuration.
async fn resolve_dns_with_timeout<F, I>(
    host: &str,
    capability: &CapabilityName,
    timeout: Duration,
    lookup: F,
) -> SandboxResult<Vec<SocketAddr>>
where
    F: Future<Output = std::io::Result<I>>,
    I: IntoIterator<Item = SocketAddr>,
{
    let addrs = tokio::time::timeout(timeout, lookup)
        .await
        .map_err(|_| SandboxError::HostCapability {
            capability: capability.clone(),
            message: format!("HTTP destination host `{host}` DNS lookup timed out"),
        })?
        .map_err(|error| SandboxError::HostCapability {
            capability: capability.clone(),
            message: format!("HTTP destination host `{host}` could not be resolved: {error}"),
        })?
        .into_iter()
        .collect::<Vec<_>>();
    if addrs.is_empty() {
        return Err(SandboxError::HostCapability {
            capability: capability.clone(),
            message: format!("HTTP destination host `{host}` resolved to no addresses"),
        });
    }
    Ok(addrs)
}

fn remaining_request_timeout(
    started_at: Instant,
    timeout: Duration,
    capability: &CapabilityName,
) -> SandboxResult<Duration> {
    timeout
        .checked_sub(started_at.elapsed())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| SandboxError::HostCapability {
            capability: capability.clone(),
            message: "HTTP capability deadline elapsed before request dispatch".to_string(),
        })
}

fn response_body_len_within_limit(current: usize, chunk: usize, limit: usize) -> bool {
    current
        .checked_add(chunk)
        .is_some_and(|next| next <= limit)
}

/// Validates the destination and, for hostnames, resolves it exactly once so
/// the caller can pin the request to the validated address.
///
/// Resolution is fail-closed: a lookup error, timeout or empty answer rejects
/// the call instead of skipping the SSRF check, and any disallowed address in
/// the answer rejects the whole call. Pinning the returned address closes
/// the DNS-rebinding gap between a check-time lookup and reqwest's own
/// resolution. Returns `None` for IP-literal destinations (no DNS involved).
async fn resolve_public_target(
    url: &Url,
    capability: &CapabilityName,
    timeout: Duration,
) -> SandboxResult<Option<(String, SocketAddr)>> {
    let host = url
        .host_str()
        .ok_or_else(|| SandboxError::InvalidRequest("HTTP url must include a host".into()))?;

    if is_disallowed_hostname(host) {
        return Err(SandboxError::HostCapability {
            capability: capability.clone(),
            message: format!(
                "HTTP destination host `{host}` is disallowed (local/internal hostname)"
            ),
        });
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_disallowed_ip(ip) {
            return Err(SandboxError::HostCapability {
                capability: capability.clone(),
                message: format!(
                    "HTTP destination IP `{ip}` is disallowed (private/loopback/metadata)"
                ),
            });
        }
        return Ok(None);
    }

    let port = url.port_or_known_default().unwrap_or(443);
    let addrs = resolve_dns_with_timeout(
        host,
        capability,
        timeout,
        tokio::net::lookup_host((host, port)),
    )
    .await?;
    let first = addrs.first().ok_or_else(|| SandboxError::HostCapability {
        capability: capability.clone(),
        message: format!("HTTP destination host `{host}` resolved to no addresses"),
    })?;
    for addr in &addrs {
        if is_disallowed_ip(addr.ip()) {
            // The resolved address itself is not echoed to the guest: it
            // can disclose internal addressing learned through DNS.
            return Err(SandboxError::HostCapability {
                capability: capability.clone(),
                message: format!("HTTP destination host `{host}` resolves to a disallowed address"),
            });
        }
    }
    Ok(Some((host.to_ascii_lowercase(), *first)))
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

        let parsed_url = Url::parse(url_str)
            .map_err(|err| SandboxError::InvalidRequest(format!("HTTP url is invalid: {err}")))?;
        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err(SandboxError::InvalidRequest(format!(
                "HTTP url scheme `{}` is not supported",
                parsed_url.scheme()
            )));
        }

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

        // DNS resolution and the HTTP exchange share one deadline. The
        // request receives only the time left after the bounded DNS lookup.
        let started_at = Instant::now();
        let target =
            resolve_public_target(&parsed_url, &call.capability, self.timeout).await?;
        let remaining_timeout =
            remaining_request_timeout(started_at, self.timeout, &call.capability)?;

        // Pin hostname destinations to the address validated above so the
        // connection cannot be rebound to a different (private) address by
        // a second DNS answer. Both clients use the same no-proxy policy.
        let client = match target {
            Some((host, addr)) => restricted_http_client_builder(remaining_timeout)
                .resolve(&host, addr)
                .build()
                .map_err(|error| SandboxError::HostCapability {
                    capability: call.capability.clone(),
                    message: format!("failed to construct pinned HTTP client: {error}"),
                })?,
            None => self.client.clone(),
        };

        let mut request_builder = client.request(method, url_str).timeout(remaining_timeout);

        if let Some(headers_obj) = input.get("headers").and_then(Value::as_object) {
            for (key, val) in headers_obj {
                // Hop-by-hop and framing headers are owned by the HTTP
                // client; a guest must not override Host or body framing.
                if matches!(
                    key.to_ascii_lowercase().as_str(),
                    "host"
                        | "content-length"
                        | "transfer-encoding"
                        | "connection"
                        | "keep-alive"
                        | "proxy-connection"
                        | "te"
                        | "trailer"
                        | "upgrade"
                ) {
                    return Err(SandboxError::InvalidRequest(format!(
                        "HTTP header `{key}` is managed by the platform and must not be set"
                    )));
                }
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

        let mut response =
            request_builder
                .send()
                .await
                .map_err(|err| SandboxError::HostCapability {
                    capability: call.capability.clone(),
                    message: format!("HTTP request failed: {err}"),
                })?;

        let status = response.status().as_u16();

        if response
            .content_length()
            .is_some_and(|len| len > self.max_response_bytes as u64)
        {
            let content_length = response.content_length().unwrap_or_default();
            return Err(SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!(
                    "HTTP response content length ({content_length} bytes) exceeds limit of {} bytes",
                    self.max_response_bytes
                ),
            });
        }

        let mut response_headers = HashMap::new();
        for (key, val) in response.headers() {
            if let Ok(val_str) = val.to_str() {
                response_headers.insert(key.as_str().to_string(), val_str.to_string());
            }
        }

        // Stream the body so the size cap bounds memory during the read;
        // a chunked response without Content-Length must not be buffered
        // in full before the limit is checked.
        let mut bytes = Vec::new();
        while let Some(chunk) =
            response
                .chunk()
                .await
                .map_err(|err| SandboxError::HostCapability {
                    capability: call.capability.clone(),
                    message: format!("reading HTTP response body failed: {err}"),
                })?
        {
            if !response_body_len_within_limit(
                bytes.len(),
                chunk.len(),
                self.max_response_bytes,
            ) {
                return Err(SandboxError::HostCapability {
                    capability: call.capability.clone(),
                    message: format!(
                        "HTTP response body size exceeds limit of {} bytes",
                        self.max_response_bytes
                    ),
                });
            }
            bytes.extend_from_slice(&chunk);
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
    pub fn new(db: DatabaseConnection) -> SandboxResult<Self> {
        Ok(Self {
            db,
            broker: Arc::new(ArtifactHttpCapabilityBroker::new()?),
        })
    }

    pub fn with_timeout(db: DatabaseConnection, timeout: Duration) -> SandboxResult<Self> {
        Ok(Self {
            db,
            broker: Arc::new(ArtifactHttpCapabilityBroker::with_timeout(timeout)?),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_capability_rejects_zero_timeout_and_response_limit() {
        assert!(matches!(
            ArtifactHttpCapabilityBroker::with_timeout_and_limit(Duration::ZERO, 1),
            Err(SandboxError::InvalidRequest(_))
        ));
        assert!(matches!(
            ArtifactHttpCapabilityBroker::with_timeout_and_limit(Duration::from_secs(1), 0),
            Err(SandboxError::InvalidRequest(_))
        ));
    }

    #[tokio::test]
    async fn dns_lookup_is_bounded_by_the_capability_timeout() {
        let capability = CapabilityName::new("platform.http").expect("capability");
        let result = resolve_dns_with_timeout(
            "example.com",
            &capability,
            Duration::from_millis(1),
            std::future::pending::<std::io::Result<Vec<SocketAddr>>>(),
        )
        .await;

        assert!(matches!(
            result,
            Err(SandboxError::HostCapability { message, .. })
                if message.contains("DNS lookup timed out")
        ));
    }

    #[test]
    fn dns_and_request_share_one_nonzero_deadline() {
        let capability = CapabilityName::new("platform.http").expect("capability");
        assert!(
            remaining_request_timeout(Instant::now(), Duration::from_secs(1), &capability)
                .is_ok()
        );
        let expired = Instant::now() - Duration::from_secs(2);
        assert!(matches!(
            remaining_request_timeout(expired, Duration::from_secs(1), &capability),
            Err(SandboxError::HostCapability { message, .. })
                if message.contains("deadline elapsed")
        ));
    }

    #[test]
    fn response_body_limit_check_cannot_overflow() {
        assert!(response_body_len_within_limit(8, 2, 10));
        assert!(!response_body_len_within_limit(8, 3, 10));
        assert!(!response_body_len_within_limit(usize::MAX, 1, usize::MAX));
    }

    #[test]
    fn transition_prefixes_cannot_smuggle_private_ipv4() {
        // NAT64 64:ff9b::a9fe:a9fe embeds 169.254.169.254 (metadata).
        assert!(is_disallowed_ip(
            "64:ff9b::a9fe:a9fe".parse().expect("nat64 address")
        ));
        // 6to4 2002:0a00:0001:: embeds 10.0.0.1.
        assert!(is_disallowed_ip(
            "2002:0a00:0001::".parse().expect("6to4 address")
        ));
        // Public embedded destinations remain allowed.
        assert!(!is_disallowed_ip(
            "64:ff9b::808:808".parse().expect("nat64 public address")
        ));
        assert!(!is_disallowed_ip(
            "2002:0808:0808::".parse().expect("6to4 public address")
        ));
        // RFC 8215 local-use NAT64 must screen its embedded IPv4 too.
        assert!(is_disallowed_ip(
            "64:ff9b:1::a9fe:a9fe".parse().expect("local-use nat64 private address")
        ));
        assert!(!is_disallowed_ip(
            "64:ff9b:1::808:808".parse().expect("local-use nat64 public address")
        ));
        // Deprecated IPv4-compatible addresses cannot bypass IPv4 screening.
        assert!(is_disallowed_ip(
            "::a9fe:a9fe".parse().expect("ipv4-compatible metadata address")
        ));
        assert!(!is_disallowed_ip(
            "::808:808".parse().expect("ipv4-compatible public address")
        ));
        // Deprecated site-local and IETF special-purpose IPv6 ranges are not
        // valid public destinations (the latter includes Teredo).
        assert!(is_disallowed_ip("fec0::1".parse().expect("site-local ipv6")));
        assert!(is_disallowed_ip(
            "2001:0:4136:e378:8000:63bf:3fff:fdd2"
                .parse()
                .expect("teredo ipv6")
        ));
        assert!(is_disallowed_ip(
            "2001:2::1".parse().expect("benchmarking ipv6")
        ));
        assert!(is_disallowed_ip(
            "2001:10::1".parse().expect("deprecated orchid ipv6")
        ));
        assert!(is_disallowed_ip(
            "3fff::1".parse().expect("new documentation ipv6")
        ));
        assert!(is_disallowed_ip(
            "100::1".parse().expect("discard-only ipv6")
        ));
        assert!(is_disallowed_ip(
            "100:0:0:1::1".parse().expect("dummy ipv6")
        ));
        assert!(is_disallowed_ip(
            "5f00::1".parse().expect("segment routing sid")
        ));
        // More-specific IANA assignments in 2001::/23 that are explicitly
        // globally reachable remain usable when an artifact grants them.
        assert!(!is_disallowed_ip(
            "2001:1::1".parse().expect("pcp anycast")
        ));
        assert!(!is_disallowed_ip(
            "2001:3::1".parse().expect("amt ipv6")
        ));
        assert!(!is_disallowed_ip(
            "2001:20::1".parse().expect("orchid v2 ipv6")
        ));
        // Ordinary public IPv6 is unaffected.
        assert!(!is_disallowed_ip(
            "2606:4700:4700::1111".parse().expect("public ipv6")
        ));
    }
}
