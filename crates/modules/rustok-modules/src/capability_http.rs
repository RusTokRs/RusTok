use std::collections::HashMap;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::{Client, Method};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, Statement, TransactionTrait, Value as SqlValue,
};
use serde_json::{Value, json};
use url::Url;

use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    SandboxError, SandboxResult, SandboxSubject,
};

use crate::artifact_capability_router::{
    ArtifactCapabilityBrokerResolver, ArtifactCapabilityExecution,
    begin_artifact_capability_effect, commit_artifact_capability_effect,
    resolve_granted_artifact_capability,
};
use crate::backend::SupportedBackend;
use uuid::Uuid;

pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024; // 10 MB

#[derive(Clone)]
struct HttpEffectReceiptKey {
    tenant_id: Uuid,
    installation_id: Uuid,
    execution_idempotency_key: Uuid,
    request_idempotency_key: Uuid,
    request_digest: String,
}

fn http_effect_receipt_key(
    call: &CapabilityCall,
    input: &serde_json::Map<String, Value>,
) -> SandboxResult<HttpEffectReceiptKey> {
    let tenant_id = call
        .context
        .tenant_id
        .filter(|id| !id.is_nil())
        .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))?;
    let execution_idempotency_key = call
        .context
        .idempotency_key
        .filter(|id| !id.is_nil())
        .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "mutating HTTP calls require a stable execution idempotency key".to_string(),
        })?;
    let installation_id = match &call.subject {
        SandboxSubject::ModuleArtifact { installation_id, .. } if !installation_id.is_nil() => {
            *installation_id
        }
        _ => return Err(SandboxError::CapabilityDenied(call.capability.clone())),
    };
    let request_digest = crate::idempotency::semantic_request_digest(&call.input)
        .map_err(|_| SandboxError::InvalidRequest("HTTP request could not be fingerprinted".to_string()))?;
    let request_idempotency_key = match input
        .get("idempotency_key")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .filter(|id| !id.is_nil())
    {
        Some(key) => key,
        None => deterministic_http_request_key(&request_digest),
    };
    Ok(HttpEffectReceiptKey {
        tenant_id,
        installation_id,
        execution_idempotency_key,
        request_idempotency_key,
        request_digest,
    })
}

fn deterministic_http_request_key(request_digest: &str) -> Uuid {
    let digest_prefix = request_digest
        .strip_prefix("sha256:")
        .and_then(|digest| digest.get(..32))
        .and_then(|digest| u128::from_str_radix(digest, 16).ok())
        .expect("semantic request digest is a SHA-256 hex value");
    let mut bytes = digest_prefix.to_be_bytes();
    // Mark the deterministic value as an RFC 4122 name-based UUID.
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn http_receipt_identity_values(
    key: &HttpEffectReceiptKey,
    backend: SupportedBackend,
) -> Vec<sea_orm::Value> {
    vec![
        backend.uuid_value(key.tenant_id),
        backend.uuid_value(key.installation_id),
        backend.uuid_value(key.execution_idempotency_key),
        backend.uuid_value(key.request_idempotency_key),
        key.request_digest.clone().into(),
    ]
}

fn http_receipt_scope_values(
    key: &HttpEffectReceiptKey,
    backend: SupportedBackend,
) -> Vec<sea_orm::Value> {
    vec![
        backend.uuid_value(key.tenant_id),
        backend.uuid_value(key.installation_id),
        backend.uuid_value(key.execution_idempotency_key),
        backend.uuid_value(key.request_idempotency_key),
    ]
}

fn http_host_error(call: &CapabilityCall, message: &str) -> SandboxError {
    SandboxError::HostCapability {
        capability: call.capability.clone(),
        message: message.to_string(),
    }
}

/// Production broker for the `platform.http` sandbox capability.
///
/// Executes outbound HTTP requests on behalf of admitted guest modules within
/// the bounds enforced by `HttpCapabilityConstraints` with redirect and SSRF isolation.
#[derive(Clone)]
pub struct ArtifactHttpCapabilityBroker {
    client: Client,
    timeout: Duration,
    max_response_bytes: usize,
    db: Option<DatabaseConnection>,
    effect_authorizer: Option<Arc<dyn crate::ArtifactSqlEffectAuthorizer>>,
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
            db: None,
            effect_authorizer: None,
        })
    }

    fn with_database(db: DatabaseConnection, timeout: Duration) -> SandboxResult<Self> {
        let mut broker = Self::with_timeout(timeout)?;
        broker.db = Some(db);
        Ok(broker)
    }

    fn effect_dependencies(
        &self,
        call: &CapabilityCall,
    ) -> SandboxResult<(&DatabaseConnection, &dyn crate::ArtifactSqlEffectAuthorizer)> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))?;
        let authorizer = self
            .effect_authorizer
            .as_deref()
            .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))?;
        Ok((db, authorizer))
    }

    async fn authorize_dispatch(&self, call: &CapabilityCall) -> SandboxResult<()> {
        let (db, authorizer) = self.effect_dependencies(call)?;
        let (transaction, guard) =
            begin_artifact_capability_effect(db, call, authorizer).await?;
        commit_artifact_capability_effect(transaction, guard, &call.capability).await
    }

    async fn claim_effect_receipt(
        &self,
        call: &CapabilityCall,
        key: &HttpEffectReceiptKey,
    ) -> SandboxResult<Option<Value>> {
        let (db, authorizer) = self.effect_dependencies(call)?;
        let (transaction, guard) =
            begin_artifact_capability_effect(db, call, authorizer).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|_| http_host_error(call, "HTTP receipt backend is unavailable"))?;
        let inserted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_http_effect_receipts
                     (tenant_id, installation_id, execution_idempotency_key,
                      request_idempotency_key, request_digest, status, created_at, updated_at)
                     VALUES ({}, {}, {}, {}, {}, 'dispatching', {}, {})
                     ON CONFLICT (tenant_id, installation_id, execution_idempotency_key,
                                  request_idempotency_key) DO NOTHING",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.now_sql(),
                    backend.now_sql(),
                ),
                http_receipt_identity_values(key, backend),
            ))
            .await
            .map_err(|_| http_host_error(call, "HTTP effect receipt could not be claimed"))?;
        if inserted.rows_affected() == 1 {
            commit_artifact_capability_effect(transaction, guard, &call.capability).await?;
            return Ok(None);
        }

        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT request_digest, status, CAST(response AS TEXT) AS response
                     FROM module_artifact_http_effect_receipts
                     WHERE tenant_id = {} AND installation_id = {}
                       AND execution_idempotency_key = {} AND request_idempotency_key = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                ),
                http_receipt_scope_values(key, backend),
            ))
            .await
            .map_err(|_| http_host_error(call, "HTTP effect receipt could not be read"))?;
        commit_artifact_capability_effect(transaction, guard, &call.capability).await?;
        let row = row.ok_or_else(|| {
            http_host_error(call, "HTTP effect receipt disappeared during idempotency claim")
        })?;
        let stored_digest: String = row
            .try_get("", "request_digest")
            .map_err(|_| http_host_error(call, "HTTP effect receipt is invalid"))?;
        if stored_digest != key.request_digest {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "HTTP idempotency key was reused for a different request".to_string(),
            });
        }
        let status: String = row
            .try_get("", "status")
            .map_err(|_| http_host_error(call, "HTTP effect receipt is invalid"))?;
        if status == "completed" {
            let response: String = row
                .try_get("", "response")
                .map_err(|_| http_host_error(call, "HTTP replay response is unavailable"))?;
            let output = serde_json::from_str(&response)
                .map_err(|_| http_host_error(call, "HTTP replay response is invalid"))?;
            return Ok(Some(output));
        }
        Err(http_host_error(
            call,
            "the previous HTTP request outcome is unresolved; automatic replay is disabled",
        ))
    }

    async fn complete_effect_receipt(
        &self,
        call: &CapabilityCall,
        key: &HttpEffectReceiptKey,
        output: &Value,
    ) -> SandboxResult<()> {
        let (db, authorizer) = self.effect_dependencies(call)?;
        let (transaction, guard) =
            begin_artifact_capability_effect(db, call, authorizer).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|_| http_host_error(call, "HTTP receipt backend is unavailable"))?;
        let completed = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_http_effect_receipts
                     SET status = 'completed', response = {}, updated_at = {}, completed_at = {}
                     WHERE tenant_id = {} AND installation_id = {}
                       AND execution_idempotency_key = {} AND request_idempotency_key = {}
                       AND request_digest = {} AND status = 'dispatching'",
                    backend.placeholder(1),
                    backend.now_sql(),
                    backend.now_sql(),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                ),
                vec![
                    SqlValue::Json(Some(Box::new(output.clone()))),
                    backend.uuid_value(key.tenant_id),
                    backend.uuid_value(key.installation_id),
                    backend.uuid_value(key.execution_idempotency_key),
                    backend.uuid_value(key.request_idempotency_key),
                    key.request_digest.clone().into(),
                ],
            ))
            .await
            .map_err(|_| http_host_error(call, "HTTP effect receipt could not be completed"))?;
        if completed.rows_affected() != 1 {
            return Err(http_host_error(call, "HTTP effect receipt is no longer dispatchable"));
        }
        commit_artifact_capability_effect(transaction, guard, &call.capability).await
    }

    async fn mark_effect_receipt_ambiguous(
        &self,
        call: &CapabilityCall,
        key: &HttpEffectReceiptKey,
    ) {
        let Ok((db, authorizer)) = self.effect_dependencies(call) else {
            return;
        };
        let Ok((transaction, guard)) =
            begin_artifact_capability_effect(db, call, authorizer).await
        else {
            return;
        };
        let Ok(backend) = SupportedBackend::new(transaction.get_database_backend()) else {
            return;
        };
        let result = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_http_effect_receipts
                     SET status = 'ambiguous', updated_at = {}
                     WHERE tenant_id = {} AND installation_id = {}
                       AND execution_idempotency_key = {} AND request_idempotency_key = {}
                       AND request_digest = {} AND status = 'dispatching'",
                    backend.now_sql(),
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                ),
                vec![
                    backend.uuid_value(key.tenant_id),
                    backend.uuid_value(key.installation_id),
                    backend.uuid_value(key.execution_idempotency_key),
                    backend.uuid_value(key.request_idempotency_key),
                    key.request_digest.clone().into(),
                ],
            ))
            .await;
        if result.is_ok() {
            let _ = commit_artifact_capability_effect(transaction, guard, &call.capability).await;
        }
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

/// Extracts the IPv4 address embedded in transition-mechanism prefixes
/// whose traffic is ultimately delivered to that IPv4 destination:
/// NAT64 well-known prefix `64:ff9b::/96` and 6to4 `2002::/16`.
fn embedded_ipv4(ipv6: std::net::Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = ipv6.segments();
    if segments[..6] == [0x0064, 0xff9b, 0, 0, 0, 0] {
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
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || ipv6.is_multicast()
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
                || (ipv6.segments()[0] == 0x2001 && ipv6.segments()[1] == 0x0db8)
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
        grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if grant.name != call.capability
            || call.capability.as_str() != "platform.http"
            || call.operation != "request"
        {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
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
            .map_err(|_| SandboxError::InvalidRequest("HTTP url is invalid".into()))?;
        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err(SandboxError::InvalidRequest(
                "HTTP url scheme is not supported".to_string(),
            ));
        }
        let method = match method_str.to_ascii_uppercase().as_str() {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "PUT" => Method::PUT,
            "PATCH" => Method::PATCH,
            "DELETE" => Method::DELETE,
            "HEAD" => Method::HEAD,
            "OPTIONS" => Method::OPTIONS,
            _ => return Err(SandboxError::InvalidRequest("HTTP method is unsupported".into())),
        };
        let mutating = matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");
        let receipt_key = if mutating {
            Some(http_effect_receipt_key(call, input)?)
        } else {
            None
        };

        // DNS resolution and the HTTP exchange share one deadline. Build the
        // complete request before claiming a durable receipt, so malformed
        // headers or body encoding never leave a false dispatch in progress.
        let started_at = Instant::now();
        let target =
            resolve_public_target(&parsed_url, &call.capability, self.timeout).await?;
        let remaining_timeout =
            remaining_request_timeout(started_at, self.timeout, &call.capability)?;
        let client = match target {
            Some((host, addr)) => restricted_http_client_builder(remaining_timeout)
                .resolve(&host, addr)
                .build()
                .map_err(|_| http_host_error(call, "pinned HTTP client could not be constructed"))?,
            None => self.client.clone(),
        };
        let mut request_builder = client.request(method.clone(), url_str).timeout(remaining_timeout);
        if let Some(headers_obj) = input.get("headers") {
            let headers = headers_obj.as_object().ok_or_else(|| {
                SandboxError::InvalidRequest("HTTP headers must be an object".to_string())
            })?;
            for (key, value) in headers {
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
                        | "idempotency-key"
                ) {
                    return Err(SandboxError::InvalidRequest(format!(
                        "HTTP header `{key}` is managed by the platform and must not be set"
                    )));
                }
                let value = value.as_str().ok_or_else(|| {
                    SandboxError::InvalidRequest("HTTP header values must be strings".to_string())
                })?;
                request_builder = request_builder.header(key.as_str(), value);
            }
        }
        if let Some(key) = receipt_key.as_ref() {
            let remote_key = format!(
                "{}:{}",
                key.execution_idempotency_key, key.request_idempotency_key
            );
            request_builder = request_builder.header("Idempotency-Key", remote_key);
        }
        if let Some(body_val) = input.get("body") {
            if let Some(body_str) = body_val.as_str() {
                request_builder = request_builder.body(body_str.to_string());
            } else if !body_val.is_null() {
                request_builder = request_builder.json(body_val);
            }
        }
        let request = request_builder
            .build()
            .map_err(|_| SandboxError::InvalidRequest("HTTP request could not be built".into()))?;

        if let Some(key) = receipt_key.as_ref() {
            if let Some(output) = self.claim_effect_receipt(call, key).await? {
                return Ok(CapabilityResponse { output });
            }
        } else {
            self.authorize_dispatch(call).await?;
        }

        let mut response = match client.execute(request).await {
            Ok(response) => response,
            Err(_) => {
                if let Some(key) = receipt_key.as_ref() {
                    self.mark_effect_receipt_ambiguous(call, key).await;
                }
                return Err(http_host_error(
                    call,
                    "HTTP request failed; its remote outcome may be ambiguous",
                ));
            }
        };
        let status = response.status().as_u16();
        if response
            .content_length()
            .is_some_and(|length| length > self.max_response_bytes as u64)
        {
            if let Some(key) = receipt_key.as_ref() {
                self.mark_effect_receipt_ambiguous(call, key).await;
            }
            return Err(http_host_error(
                call,
                "HTTP response content length exceeds the configured limit",
            ));
        }
        let mut response_headers = HashMap::new();
        for (key, value) in response.headers() {
            if let Ok(value) = value.to_str() {
                response_headers.insert(key.as_str().to_string(), value.to_string());
            }
        }
        let mut bytes = Vec::new();
        loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) => {
                    if let Some(key) = receipt_key.as_ref() {
                        self.mark_effect_receipt_ambiguous(call, key).await;
                    }
                    return Err(http_host_error(
                        call,
                        "reading the HTTP response failed; its remote outcome is ambiguous",
                    ));
                }
            };
            if !response_body_len_within_limit(
                bytes.len(),
                chunk.len(),
                self.max_response_bytes,
            ) {
                if let Some(key) = receipt_key.as_ref() {
                    self.mark_effect_receipt_ambiguous(call, key).await;
                }
                return Err(http_host_error(
                    call,
                    "HTTP response body exceeds the configured limit",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let output = json!({
            "status": status,
            "headers": response_headers,
            "body": String::from_utf8_lossy(&bytes).to_string(),
        });
        if let Some(key) = receipt_key.as_ref() {
            self.complete_effect_receipt(call, key, &output).await?;
        }
        Ok(CapabilityResponse { output })
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
        let broker =
            ArtifactHttpCapabilityBroker::with_database(db.clone(), Duration::from_secs(10))?;
        Ok(Self {
            db,
            broker: Arc::new(broker),
        })
    }

    pub fn with_timeout(db: DatabaseConnection, timeout: Duration) -> SandboxResult<Self> {
        let broker = ArtifactHttpCapabilityBroker::with_database(db.clone(), timeout)?;
        Ok(Self {
            db,
            broker: Arc::new(broker),
        })
    }

    pub fn with_sql_effect_authorizer(
        mut self,
        authorizer: Arc<dyn crate::ArtifactSqlEffectAuthorizer>,
    ) -> Self {
        let mut broker = (*self.broker).clone();
        broker.effect_authorizer = Some(authorizer);
        self.broker = Arc::new(broker);
        self
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



    #[derive(Clone)]
    struct AllowHttpEffectAuthorizer;

    struct AllowHttpEffectFence;

    struct AllowHttpEffectGuard;

    #[async_trait]
    impl crate::ArtifactSqlEffectAuthorizer for AllowHttpEffectAuthorizer {
        async fn prepare(
            &self,
            _call: &CapabilityCall,
        ) -> SandboxResult<Box<dyn crate::ArtifactSqlEffectFence>> {
            Ok(Box::new(AllowHttpEffectFence))
        }
    }

    #[async_trait]
    impl crate::ArtifactSqlEffectFence for AllowHttpEffectFence {
        async fn lock_on(
            &self,
            _tx: &sea_orm::DatabaseTransaction,
        ) -> SandboxResult<Box<dyn crate::ArtifactSqlEffectCommitGuard>> {
            Ok(Box::new(AllowHttpEffectGuard))
        }
    }

    #[async_trait]
    impl crate::ArtifactSqlEffectCommitGuard for AllowHttpEffectGuard {
        async fn check_before_commit(
            &self,
            _tx: &sea_orm::DatabaseTransaction,
        ) -> SandboxResult<()> {
            Ok(())
        }
    }

    async fn http_receipt_fixture() -> (DatabaseConnection, ArtifactHttpCapabilityBroker) {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        db.execute_unprepared(
            "CREATE TABLE module_artifact_http_effect_receipts (
                tenant_id TEXT NOT NULL,
                installation_id TEXT NOT NULL,
                execution_idempotency_key TEXT NOT NULL,
                request_idempotency_key TEXT NOT NULL,
                request_digest TEXT NOT NULL,
                status TEXT NOT NULL,
                response TEXT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT NULL,
                PRIMARY KEY (tenant_id, installation_id, execution_idempotency_key, request_idempotency_key)
            )",
        )
        .await
        .expect("HTTP receipt table");
        let mut broker =
            ArtifactHttpCapabilityBroker::with_database(db.clone(), Duration::from_secs(1))
                .expect("HTTP broker");
        broker.effect_authorizer = Some(Arc::new(AllowHttpEffectAuthorizer));
        (db, broker)
    }

    fn receipt_test_call(
        tenant_id: Uuid,
        installation_id: Uuid,
        execution_key: Uuid,
        request_key: Option<Uuid>,
        body: &str,
    ) -> CapabilityCall {
        let mut input = json!({
            "method": "POST",
            "url": "https://api.example.test/v1/items",
            "headers": {},
            "body": body,
        });
        if let Some(request_key) = request_key {
            input["idempotency_key"] = json!(request_key.to_string());
        }
        CapabilityCall {
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id,
                slug: "http-receipt-test".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:test".to_string(),
            },
            context: rustok_sandbox::CapabilityCallContext {
                phase: rustok_sandbox::ExecutionPhase::Event,
                tenant_id: Some(tenant_id),
                actor_id: None,
                trace_id: None,
                idempotency_key: Some(execution_key),
            },
            capability: CapabilityName::new("platform.http").expect("capability"),
            operation: "request".to_string(),
            input,
        }
    }

    #[tokio::test]
    async fn mutating_http_receipts_never_replay_an_unresolved_dispatch() {
        let (_db, broker) = http_receipt_fixture().await;
        let call = receipt_test_call(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            "payload",
        );
        let key = http_effect_receipt_key(&call, call.input.as_object().expect("input"))
            .expect("receipt key");
        assert!(broker.claim_effect_receipt(&call, &key).await.unwrap().is_none());
        assert!(broker.claim_effect_receipt(&call, &key).await.is_err());
        broker.mark_effect_receipt_ambiguous(&call, &key).await;
        assert!(broker.claim_effect_receipt(&call, &key).await.is_err());
    }

    #[tokio::test]
    async fn completed_http_receipt_replays_response_and_rejects_key_reuse() {
        let (_db, broker) = http_receipt_fixture().await;
        let tenant_id = Uuid::new_v4();
        let installation_id = Uuid::new_v4();
        let execution_key = Uuid::new_v4();
        let request_key = Uuid::new_v4();
        let call = receipt_test_call(
            tenant_id,
            installation_id,
            execution_key,
            Some(request_key),
            "first",
        );
        let key = http_effect_receipt_key(&call, call.input.as_object().expect("input"))
            .expect("receipt key");
        assert!(broker.claim_effect_receipt(&call, &key).await.unwrap().is_none());
        let response = json!({ "status": 201, "headers": {}, "body": "created" });
        broker
            .complete_effect_receipt(&call, &key, &response)
            .await
            .expect("complete receipt");
        assert_eq!(
            broker.claim_effect_receipt(&call, &key).await.unwrap(),
            Some(response)
        );

        let changed = receipt_test_call(
            tenant_id,
            installation_id,
            execution_key,
            Some(request_key),
            "different",
        );
        let changed_key =
            http_effect_receipt_key(&changed, changed.input.as_object().expect("input"))
                .expect("changed receipt key");
        assert!(matches!(
            broker.claim_effect_receipt(&changed, &changed_key).await,
            Err(SandboxError::CapabilityConstraintDenied { .. })
        ));
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
        // Ordinary public IPv6 is unaffected.
        assert!(!is_disallowed_ip(
            "2606:4700:4700::1111".parse().expect("public ipv6")
        ));
    }
}
