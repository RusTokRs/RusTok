/// Rate Limiting Middleware for RusToK
///
/// Implements a sliding window rate limiter to protect endpoints from abuse.
/// Supports per-IP rate limiting with configurable limits.
use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use moka::future::Cache;
use once_cell::sync::Lazy;
use redis::Script;
use rustok_telemetry::metrics::{
    record_rate_limit_backend_unavailable, record_rate_limit_exceeded, update_rate_limit_runtime,
};
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use rustok_cache::CacheService;

use crate::auth::{AuthConfig, decode_access_token};
use crate::common::{
    extract_effective_client_ip, peer_ip_from_extensions,
    settings::{RateLimitBackendKind, RequestTrustSettings},
};

const RATE_LIMIT_REDIS_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

/// Configuration for rate limiting
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: usize,
    /// Time window duration
    pub window: Duration,
    /// Whether to enable rate limiting (can be disabled in dev)
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window: Duration::from_secs(60),
            enabled: true,
        }
    }
}

impl RateLimitConfig {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            max_requests,
            window: Duration::from_secs(window_secs),
            enabled: true,
        }
    }

    pub fn per_minute(requests_per_minute: u32, burst: u32) -> Self {
        Self {
            max_requests: requests_per_minute.saturating_add(burst).max(1) as usize,
            window: Duration::from_secs(60),
            enabled: true,
        }
    }

    pub fn disabled() -> Self {
        Self {
            max_requests: 0,
            window: Duration::from_secs(0),
            enabled: false,
        }
    }
}

#[derive(Clone, Debug)]
struct RequestCounter {
    count: usize,
    window_start: Instant,
}

const MEMORY_BACKEND_MAX_ENTRIES: u64 = 100_000;

#[derive(Clone)]
enum RateLimiterBackend {
    Memory {
        requests: Cache<String, RequestCounter>,
    },
    Redis {
        client: redis::Client,
        key_prefix: String,
    },
}

#[derive(Clone)]
pub struct RateLimiter {
    backend: RateLimiterBackend,
    config: RateLimitConfig,
    namespace: String,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self::new_with_namespace(config, "default")
    }

    pub fn new_with_namespace(config: RateLimitConfig, namespace: impl Into<String>) -> Self {
        let requests = Cache::builder()
            .max_capacity(MEMORY_BACKEND_MAX_ENTRIES)
            .time_to_idle(config.window)
            .build();
        Self {
            backend: RateLimiterBackend::Memory { requests },
            config,
            namespace: namespace.into(),
        }
    }

    pub fn with_redis(config: RateLimitConfig, client: redis::Client, key_prefix: String) -> Self {
        Self::with_redis_in_namespace(config, client, key_prefix, "default")
    }

    pub fn with_redis_in_namespace(
        config: RateLimitConfig,
        client: redis::Client,
        key_prefix: String,
        namespace: impl Into<String>,
    ) -> Self {
        Self {
            backend: RateLimiterBackend::Redis { client, key_prefix },
            config,
            namespace: namespace.into(),
        }
    }

    pub fn window_secs(&self) -> u64 {
        self.config.window.as_secs()
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn max_requests(&self) -> usize {
        self.config.max_requests
    }

    pub fn is_distributed(&self) -> bool {
        matches!(self.backend, RateLimiterBackend::Redis { .. })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn backend_kind(&self) -> &'static str {
        match self.backend {
            RateLimiterBackend::Memory { .. } => "memory",
            RateLimiterBackend::Redis { .. } => "redis",
        }
    }

    pub async fn check_rate_limit(&self, key: &str) -> Result<RateLimitInfo, RateLimitCheckError> {
        if !self.config.enabled {
            return Ok(RateLimitInfo::unlimited());
        }

        match &self.backend {
            RateLimiterBackend::Memory { requests } => {
                self.check_rate_limit_memory(requests, key).await
            }
            RateLimiterBackend::Redis { client, key_prefix } => {
                self.check_rate_limit_redis(client, key_prefix, key).await
            }
        }
    }

    async fn check_rate_limit_memory(
        &self,
        requests: &Cache<String, RequestCounter>,
        key: &str,
    ) -> Result<RateLimitInfo, RateLimitCheckError> {
        let now = Instant::now();
        let max_requests = self.config.max_requests;
        let window = self.config.window;

        // Atomically read-modify-write the counter so there is no TOCTOU race.
        let counter = requests
            .entry(key.to_string())
            .and_upsert_with(|maybe| async move {
                let mut c = maybe.map(|e| e.into_value()).unwrap_or(RequestCounter {
                    count: 0,
                    window_start: now,
                });
                if now.duration_since(c.window_start) > window {
                    c.count = 0;
                    c.window_start = now;
                }
                c.count = c.count.saturating_add(1);
                c
            })
            .await
            .into_value();

        if counter.count > max_requests {
            let retry_after = window
                .saturating_sub(now.duration_since(counter.window_start))
                .as_secs();
            warn!(
                key = %key,
                count = counter.count,
                limit = max_requests,
                retry_after = retry_after,
                "Rate limit exceeded"
            );
            return Err(RateLimitCheckError::Exceeded(RateLimitExceeded::new(
                max_requests,
                retry_after,
            )));
        }

        let reset_at = counter.window_start + window;
        let reset_secs = reset_at.saturating_duration_since(now).as_secs();

        Ok(RateLimitInfo {
            limit: max_requests,
            remaining: max_requests.saturating_sub(counter.count),
            reset: reset_secs,
        })
    }

    async fn check_rate_limit_redis(
        &self,
        client: &redis::Client,
        key_prefix: &str,
        key: &str,
    ) -> Result<RateLimitInfo, RateLimitCheckError> {
        static RATE_LIMIT_REDIS_SCRIPT: Lazy<Script> = Lazy::new(|| {
            Script::new(
                r#"
local current = redis.call('INCR', KEYS[1])
if current == 1 then
    redis.call('EXPIRE', KEYS[1], ARGV[1])
end
local ttl = redis.call('TTL', KEYS[1])
return {current, ttl}
"#,
            )
        });

        let redis_key = redis_rate_limit_key(key_prefix, key);
        let window_secs = bounded_redis_window_seconds(self.config.window);
        let mut connection = redis_with_timeout(
            RATE_LIMIT_REDIS_OPERATION_TIMEOUT,
            "redis rate-limit connection",
            client.get_multiplexed_async_connection(),
        )
        .await
        .map_err(RateLimitCheckError::BackendUnavailable)?;

        let (current, ttl): (i64, i64) = redis_with_timeout(
            RATE_LIMIT_REDIS_OPERATION_TIMEOUT,
            "redis rate-limit script",
            RATE_LIMIT_REDIS_SCRIPT
                .key(redis_key.as_str())
                .arg(window_secs)
                .invoke_async(&mut connection),
        )
        .await
        .map_err(RateLimitCheckError::BackendUnavailable)?;

        let current = current.max(0) as usize;
        let retry_after = ttl.max(1) as u64;

        if current > self.config.max_requests {
            warn!(
                redis_key = %redis_key,
                limit = self.config.max_requests,
                current,
                retry_after,
                "Distributed rate limit exceeded"
            );
            return Err(RateLimitCheckError::Exceeded(RateLimitExceeded::new(
                self.config.max_requests,
                retry_after,
            )));
        }

        Ok(RateLimitInfo {
            limit: self.config.max_requests,
            remaining: self.config.max_requests.saturating_sub(current),
            reset: retry_after,
        })
    }

    pub async fn cleanup_expired(&self) {
        let RateLimiterBackend::Memory { requests } = &self.backend else {
            return;
        };
        requests.run_pending_tasks().await;
        debug!(
            retained = requests.entry_count(),
            "Cleaned up expired rate limit entries"
        );
    }

    pub async fn check_backend_health(&self) -> Result<(), String> {
        match &self.backend {
            RateLimiterBackend::Memory { .. } => Ok(()),
            RateLimiterBackend::Redis { client, .. } => {
                let mut connection = redis_with_timeout(
                    RATE_LIMIT_REDIS_OPERATION_TIMEOUT,
                    "redis rate-limit health connection",
                    client.get_multiplexed_async_connection(),
                )
                .await?;

                let response: String = redis_with_timeout(
                    RATE_LIMIT_REDIS_OPERATION_TIMEOUT,
                    "redis rate-limit health PING",
                    redis::cmd("PING").query_async(&mut connection),
                )
                .await?;

                if response.eq_ignore_ascii_case("PONG") {
                    Ok(())
                } else {
                    Err(format!(
                        "unexpected redis rate-limit ping response: {response}"
                    ))
                }
            }
        }
    }

    pub async fn get_stats(&self) -> RateLimitStats {
        match &self.backend {
            RateLimiterBackend::Memory { requests } => {
                let count = requests.entry_count() as usize;
                RateLimitStats {
                    active_clients: count,
                    total_entries: count,
                    distributed: false,
                }
            }
            RateLimiterBackend::Redis { .. } => RateLimitStats {
                active_clients: 0,
                total_entries: 0,
                distributed: true,
            },
        }
    }

    pub async fn sync_runtime_metrics(&self) -> Result<(), String> {
        let stats = self.get_stats().await;
        let backend = self.backend_kind();
        let namespace = self.namespace();

        match self.check_backend_health().await {
            Ok(()) => {
                update_rate_limit_runtime(
                    namespace,
                    backend,
                    stats.distributed,
                    stats.active_clients,
                    stats.total_entries,
                    true,
                );
                Ok(())
            }
            Err(error) => {
                update_rate_limit_runtime(
                    namespace,
                    backend,
                    stats.distributed,
                    stats.active_clients,
                    stats.total_entries,
                    false,
                );
                Err(error)
            }
        }
    }

    pub fn build_for_backend(
        config: RateLimitConfig,
        backend: RateLimitBackendKind,
        redis_key_prefix: &str,
        namespace: &str,
        cache_service: &CacheService,
    ) -> Result<Self, String> {
        if !config.enabled {
            return Ok(Self::new_with_namespace(config, namespace));
        }

        match backend {
            RateLimitBackendKind::Memory => Ok(Self::new_with_namespace(config, namespace)),
            RateLimitBackendKind::Redis => {
                let client = cache_service.redis_client().cloned().ok_or_else(|| {
                    "rate_limit.backend=redis requires a configured Redis runtime".to_string()
                })?;
                Ok(Self::with_redis_in_namespace(
                    config,
                    client,
                    format!("{redis_key_prefix}:{namespace}"),
                    namespace,
                ))
            }
        }
    }
}

#[derive(Clone)]
pub struct SharedApiRateLimiter(pub Arc<RateLimiter>);

#[derive(Clone)]
pub struct SharedAuthRateLimiter(pub Arc<RateLimiter>);

#[derive(Clone)]
pub struct SharedOAuthRateLimiter(pub Arc<RateLimiter>);

#[derive(Clone)]
pub struct SharedSearchRateLimiter(pub Arc<RateLimiter>);

#[derive(Clone)]
pub struct RateLimitMiddlewareState {
    pub limiter: Arc<RateLimiter>,
    pub auth_config: Option<AuthConfig>,
    pub trusted_auth_dimensions: bool,
}

#[derive(Clone)]
pub struct PathRateLimitMiddlewareState {
    pub policies: Arc<Vec<PathRateLimitPolicy>>,
    pub auth_config: Option<AuthConfig>,
    pub trusted_auth_dimensions: bool,
    pub request_trust: RequestTrustSettings,
}

#[derive(Clone)]
pub struct PathRateLimitPolicy {
    pub limiter: Arc<RateLimiter>,
    pub prefixes: Arc<Vec<&'static str>>,
}

fn matching_path_policy<'a>(
    policies: &'a [PathRateLimitPolicy],
    path: &str,
) -> Option<&'a PathRateLimitPolicy> {
    policies.iter().find(|policy| {
        policy
            .prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix))
    })
}

#[derive(Debug, Clone)]
pub struct RateLimitStats {
    pub active_clients: usize,
    pub total_entries: usize,
    pub distributed: bool,
}

#[derive(Debug)]
pub struct RateLimitInfo {
    pub limit: usize,
    pub remaining: usize,
    pub reset: u64,
}

impl RateLimitInfo {
    fn unlimited() -> Self {
        Self {
            limit: usize::MAX,
            remaining: usize::MAX,
            reset: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitExceeded {
    pub limit: usize,
    pub retry_after: u64,
}

impl RateLimitExceeded {
    fn new(limit: usize, retry_after: u64) -> Self {
        Self { limit, retry_after }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitCheckError {
    Exceeded(RateLimitExceeded),
    BackendUnavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrustedRateLimitClaims {
    tenant_id: uuid::Uuid,
    oauth_app_id: Option<uuid::Uuid>,
}

/// Extract client identifier from the request.
///
/// Priority:
/// 1. X-Forwarded-For — first IP in the list (behind a proxy)
/// 2. X-Real-IP (behind nginx)
/// 3. "ip:unknown" fallback
///
/// Security note: user identity MUST NOT be sourced from client-supplied headers
/// such as X-User-ID. Any client can set an arbitrary value, which would allow
/// them to exhaust another user's rate-limit bucket or bypass their own.
/// User-scoped rate limiting must be implemented after JWT verification using
/// the verified claims from a trusted middleware layer.
fn extract_client_id(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| IpAddr::from_str(value).is_ok())
        .map(|value| format!("ip:{value}"))
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| IpAddr::from_str(value).is_ok())
                .map(|value| format!("ip:{value}"))
        })
        .unwrap_or_else(|| "ip:unknown".to_string())
}

pub fn extract_client_id_pub(headers: &HeaderMap) -> String {
    extract_client_id(headers)
}

fn extract_client_id_for_request(
    headers: &HeaderMap,
    request: &Request,
    request_trust: &RequestTrustSettings,
) -> String {
    extract_effective_client_ip(
        headers,
        peer_ip_from_extensions(request.extensions()),
        request_trust,
    )
    .map(|ip| format!("ip:{ip}"))
    .unwrap_or_else(|| "ip:unknown".to_string())
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        })
}

fn extract_trusted_rate_limit_claims(
    headers: &HeaderMap,
    auth_config: Option<&AuthConfig>,
) -> Option<TrustedRateLimitClaims> {
    let auth_config = auth_config?;
    let token = extract_bearer_token(headers)?;
    let claims = decode_access_token(auth_config, token).ok()?;

    Some(TrustedRateLimitClaims {
        tenant_id: claims.tenant_id,
        oauth_app_id: claims.client_id,
    })
}

fn build_rate_limit_key(
    headers: &HeaderMap,
    request: &Request,
    auth_config: Option<&AuthConfig>,
    trusted_auth_dimensions: bool,
    request_trust: &RequestTrustSettings,
) -> String {
    let mut key = extract_client_id_for_request(headers, request, request_trust);

    if !trusted_auth_dimensions {
        return key;
    }

    if let Some(claims) = extract_trusted_rate_limit_claims(headers, auth_config) {
        key.push_str("|tenant:");
        key.push_str(&claims.tenant_id.to_string());

        if let Some(oauth_app_id) = claims.oauth_app_id {
            key.push_str("|oauth_app:");
            key.push_str(&oauth_app_id.to_string());
        }
    }

    key
}

fn redis_rate_limit_key(key_prefix: &str, identity: &str) -> String {
    format!(
        "{key_prefix}:v1:sha256:{}",
        hex::encode(Sha256::digest(identity.as_bytes()))
    )
}

fn bounded_redis_window_seconds(window: Duration) -> i64 {
    window.as_secs().clamp(1, i64::MAX as u64) as i64
}

async fn redis_with_timeout<T, E, F>(
    timeout: Duration,
    operation: &str,
    future: F,
) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| format!("{operation} timed out after {} ms", timeout.as_millis()))?
        .map_err(|error| format!("{operation} failed: {error}"))
}

fn insert_header_if_valid(headers: &mut axum::http::HeaderMap, key: &'static str, value: String) {
    match axum::http::HeaderValue::from_str(&value) {
        Ok(header_value) => {
            headers.insert(key, header_value);
        }
        Err(error) => {
            tracing::warn!(%key, %value, %error, "Skipping invalid rate limit header value");
        }
    }
}

fn apply_rate_limit_headers(headers: &mut axum::http::HeaderMap, info: &RateLimitInfo) {
    insert_header_if_valid(headers, "x-ratelimit-limit", info.limit.to_string());
    insert_header_if_valid(headers, "x-ratelimit-remaining", info.remaining.to_string());
    insert_header_if_valid(headers, "x-ratelimit-reset", info.reset.to_string());
}

fn rate_limited_response(exceeded: &RateLimitExceeded) -> Response {
    let mut response = Response::new(Body::from("Rate limit exceeded"));
    *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;

    let headers = response.headers_mut();
    insert_header_if_valid(headers, "retry-after", exceeded.retry_after.to_string());
    insert_header_if_valid(headers, "x-ratelimit-limit", exceeded.limit.to_string());
    insert_header_if_valid(headers, "x-ratelimit-remaining", "0".to_string());
    insert_header_if_valid(
        headers,
        "x-ratelimit-reset",
        exceeded.retry_after.to_string(),
    );

    response
}

fn rate_limit_backend_unavailable_response(reason: &str) -> Response {
    let mut response = Response::new(Body::from("Rate limit backend unavailable"));
    *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
    insert_header_if_valid(response.headers_mut(), "retry-after", "1".to_string());
    tracing::error!(reason, "Rate limit backend unavailable");
    response
}

pub async fn rate_limit_middleware(
    State(state): State<RateLimitMiddlewareState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, impl IntoResponse> {
    let rate_limit_key = build_rate_limit_key(
        &headers,
        &request,
        state.auth_config.as_ref(),
        state.trusted_auth_dimensions,
        &RequestTrustSettings::default(),
    );

    debug!(rate_limit_key = %rate_limit_key, "Checking rate limit");

    match state.limiter.check_rate_limit(&rate_limit_key).await {
        Ok(info) => {
            let mut response = next.run(request).await;
            apply_rate_limit_headers(response.headers_mut(), &info);

            Ok(response)
        }
        Err(RateLimitCheckError::Exceeded(exceeded)) => {
            record_rate_limit_exceeded(state.limiter.namespace());
            Err(rate_limited_response(&exceeded))
        }
        Err(RateLimitCheckError::BackendUnavailable(reason)) => {
            record_rate_limit_backend_unavailable(state.limiter.namespace());
            Err(rate_limit_backend_unavailable_response(&reason))
        }
    }
}

/// Path-aware rate limiting middleware.
///
/// Applies rate limiting only to requests whose URI path starts with one of the
/// provided `prefixes`. All other requests are passed through unchanged.
///
/// This is useful to protect specific endpoint groups (e.g. `/api/auth`) without
/// creating separate Axum sub-routers for each group.
pub async fn rate_limit_for_paths(
    State(state): State<PathRateLimitMiddlewareState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, impl IntoResponse> {
    let path = request.uri().path().to_owned();
    let Some(policy) = matching_path_policy(&state.policies, &path) else {
        return Ok(next.run(request).await);
    };

    let rate_limit_key = build_rate_limit_key(
        &headers,
        &request,
        state.auth_config.as_ref(),
        state.trusted_auth_dimensions,
        &state.request_trust,
    );

    debug!(
        rate_limit_key = %rate_limit_key,
        path = %path,
        namespace = policy.limiter.namespace(),
        "Checking rate limit for matched path policy"
    );

    match policy.limiter.check_rate_limit(&rate_limit_key).await {
        Ok(info) => {
            let mut response = next.run(request).await;
            apply_rate_limit_headers(response.headers_mut(), &info);

            Ok(response)
        }
        Err(RateLimitCheckError::Exceeded(exceeded)) => {
            record_rate_limit_exceeded(policy.limiter.namespace());
            Err(rate_limited_response(&exceeded))
        }
        Err(RateLimitCheckError::BackendUnavailable(reason)) => {
            record_rate_limit_backend_unavailable(policy.limiter.namespace());
            Err(rate_limit_backend_unavailable_response(&reason))
        }
    }
}

pub async fn cleanup_task(limiter: Arc<RateLimiter>) {
    let mut interval = tokio::time::interval(Duration::from_secs(300));

    loop {
        interval.tick().await;
        limiter.cleanup_expired().await;
    }
}

#[cfg(test)]
#[path = "rate_limit_tests.rs"]
mod tests;
