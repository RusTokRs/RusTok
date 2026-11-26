/// Rate Limiting Middleware for RusToK
///
/// Implements a fixed-window rate limiter to protect endpoints from abuse.
/// Supports per-IP rate limiting with configurable limits.
use axum::{
    Json,
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
use rustok_web::ErrorBody;
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::OnceCell;
use tracing::{debug, warn};

use rustok_cache::CacheService;

use crate::auth::{AuthConfig, decode_access_token};
use crate::common::{
    extract_effective_client_ip, peer_ip_from_extensions,
    settings::{RateLimitBackendKind, RequestTrustSettings},
};

const RATE_LIMIT_REDIS_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

// Repair legacy or restored counters that exist without an expiry. A counter
// that cannot be given a TTL must fail closed rather than rate-limit forever.
const RATE_LIMIT_REDIS_SCRIPT_SOURCE: &str = r#"
-- Repair a persistent key before GET or INCR can fail. Redis does not
-- roll back script writes on error. Never extend an existing expiry.
local previous_ttl = redis.call('PTTL', KEYS[1])
if previous_ttl == -1 then
    if redis.call('PEXPIRE', KEYS[1], ARGV[1]) ~= 1 then
        return redis.error_reply('rate-limit key expiry could not be established')
    end
end
-- Corrupt counters must not advance toward an admissible count.
local previous = redis.call('GET', KEYS[1])
if previous and not string.match(previous, '^[1-9][0-9]*$') then
    return redis.error_reply('invalid rate-limit counter')
end
local current = redis.call('INCR', KEYS[1])
local ttl = redis.call('PTTL', KEYS[1])
if ttl == -1 then
    redis.call('PEXPIRE', KEYS[1], ARGV[1])
    ttl = redis.call('PTTL', KEYS[1])
end
if ttl < 0 then
    return redis.error_reply('rate-limit key expiry could not be established')
end
return {current, ttl}
"#;

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
        normalize_rate_limit_config(Self {
            max_requests,
            window: Duration::from_secs(window_secs),
            enabled: true,
        })
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

const MIN_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(1);
const MAX_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(365 * 24 * 60 * 60);

fn normalize_rate_limit_config(mut config: RateLimitConfig) -> RateLimitConfig {
    if config.enabled {
        let bounded = config
            .window
            .clamp(MIN_RATE_LIMIT_WINDOW, MAX_RATE_LIMIT_WINDOW);
        // Redis expiry has millisecond precision. Round up once for both
        // backends so switching storage cannot shorten the configured window.
        let millis = bounded.as_millis()
            + u128::from(bounded.subsec_nanos() % 1_000_000 != 0);
        let normalized = Duration::from_millis(millis as u64);
        if normalized != config.window {
            tracing::warn!(
                configured_window_ms = %config.window.as_millis(),
                normalized_window_ms = %normalized.as_millis(),
                "Normalizing active rate-limit window to supported bounds and precision"
            );
            config.window = normalized;
        }
    }
    config
}

#[derive(Clone, Debug)]
struct RequestCounter {
    count: usize,
    window_start: Instant,
}

/// Advance the fixed window using the time at which the serialized cache
/// mutation executes, not the time at which a request began waiting for it.
fn advance_request_counter(
    previous: Option<RequestCounter>,
    now: Instant,
    window: Duration,
) -> RequestCounter {
    let mut counter = previous.unwrap_or(RequestCounter {
        count: 0,
        window_start: now,
    });
    // A window ends at its deadline, not one tick after it.
    if now.duration_since(counter.window_start) >= window {
        counter.count = 0;
        counter.window_start = now;
    }
    counter.count = counter.count.saturating_add(1);
    counter
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
        /// One reconnecting manager per limiter, shared by all clones and
        /// namespaces built from the same backend instance. Opening a fresh
        /// multiplexed connection per request turns Redis outages and normal
        /// traffic into connection churn.
        connection: Arc<OnceCell<redis::aio::ConnectionManager>>,
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
        let config = normalize_rate_limit_config(config);
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
        let config = normalize_rate_limit_config(config);
        Self {
            backend: RateLimiterBackend::Redis {
                client,
                key_prefix,
                connection: Arc::new(OnceCell::new()),
            },
            config,
            namespace: namespace.into(),
        }
    }

    async fn redis_connection(
        &self,
        client: &redis::Client,
        connection: &Arc<OnceCell<redis::aio::ConnectionManager>>,
    ) -> Result<redis::aio::ConnectionManager, String> {
        // Bound acquisition including time queued behind another initializer.
        // Failed or cancelled initialization leaves the shared slot retryable.
        let manager = redis_with_timeout(
            RATE_LIMIT_REDIS_OPERATION_TIMEOUT,
            "redis rate-limit connection",
            connection.get_or_try_init(|| client.get_connection_manager()),
        )
        .await?;
        Ok(manager.clone())
    }

    pub fn window_secs(&self) -> u64 {
        rate_limit_reset_after(self.config.window)
    }

    /// Auth credentials share one IP-only budget across REST and GraphQL.
    pub(crate) fn uses_trusted_auth_dimensions(&self, configured: bool) -> bool {
        configured && self.namespace != "auth"
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
            RateLimiterBackend::Redis {
                client,
                key_prefix,
                connection,
            } => {
                self.check_rate_limit_redis(client, connection, key_prefix, key)
                    .await
            }
        }
    }

    async fn check_rate_limit_memory(
        &self,
        requests: &Cache<String, RequestCounter>,
        key: &str,
    ) -> Result<RateLimitInfo, RateLimitCheckError> {
        let max_requests = self.config.max_requests;
        let window = self.config.window;

        // Moka serializes updates per key. Sampling time inside the update
        // ensures a delayed request cannot use an older window deadline.
        let counter = requests
            .entry(key.to_string())
            .and_upsert_with(|maybe| async move {
                advance_request_counter(
                    maybe.map(|entry| entry.into_value()),
                    Instant::now(),
                    window,
                )
            })
            .await
            .into_value();
        let now = Instant::now();

        if counter.count > max_requests {
            let retry_after = rate_limit_retry_after(
                window.saturating_sub(now.duration_since(counter.window_start)),
            );
            let key_fingerprint = rate_limit_key_fingerprint(key);
            warn!(
                key_fingerprint = %key_fingerprint,
                count = counter.count,
                limit = max_requests,
                retry_after,
                "Rate limit exceeded"
            );
            return Err(RateLimitCheckError::Exceeded(RateLimitExceeded::new(
                max_requests,
                retry_after,
            )));
        }

        let reset_at = counter.window_start + window;
        let reset_secs = rate_limit_reset_after(reset_at.saturating_duration_since(now));

        Ok(RateLimitInfo {
            limit: max_requests,
            remaining: max_requests.saturating_sub(counter.count),
            reset: reset_secs,
        })
    }

    async fn check_rate_limit_redis(
        &self,
        client: &redis::Client,
        connection: &Arc<OnceCell<redis::aio::ConnectionManager>>,
        key_prefix: &str,
        key: &str,
    ) -> Result<RateLimitInfo, RateLimitCheckError> {
        static RATE_LIMIT_REDIS_SCRIPT: Lazy<Script> =
            Lazy::new(|| Script::new(RATE_LIMIT_REDIS_SCRIPT_SOURCE));

        let redis_key = redis_rate_limit_key(key_prefix, key);
        let window_ms = bounded_redis_window_millis(self.config.window);
        let mut connection = self
            .redis_connection(client, connection)
            .await
            .map_err(RateLimitCheckError::BackendUnavailable)?;

        let (current, ttl): (i64, i64) = redis_with_timeout(
            RATE_LIMIT_REDIS_OPERATION_TIMEOUT,
            "redis rate-limit script",
            RATE_LIMIT_REDIS_SCRIPT
                .key(redis_key.as_str())
                .arg(window_ms)
                .invoke_async(&mut connection),
        )
        .await
        .map_err(RateLimitCheckError::BackendUnavailable)?;

        let (current, reset_after) = decode_redis_rate_limit_result(current, ttl)?;
        let retry_after = reset_after.max(1);

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
            reset: reset_after,
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
            RateLimiterBackend::Redis {
                client, connection, ..
            } => {
                let mut connection = self.redis_connection(client, connection).await?;

                let response: String = redis_with_timeout(
                    RATE_LIMIT_REDIS_OPERATION_TIMEOUT,
                    "redis rate-limit health PING",
                    redis::cmd("PING").query_async(&mut connection),
                )
                .await?;

                if response.eq_ignore_ascii_case("PONG") {
                    Ok(())
                } else {
                    Err("unexpected redis rate-limit ping response".to_string())
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

fn path_matches_prefix(path: &str, prefix: &str) -> bool {
    // A group prefix must stop on a path-segment boundary. Otherwise an
    // /api/auth limiter also affects unrelated /api/authentication routes.
    path == prefix
        || (prefix.ends_with('/') && path.starts_with(prefix))
        || path
            .strip_prefix(prefix)
            .is_some_and(|remaining| remaining.starts_with('/'))
}

fn matching_path_policy<'a>(
    policies: &'a [PathRateLimitPolicy],
    path: &str,
) -> Option<&'a PathRateLimitPolicy> {
    policies.iter().find(|policy| {
        policy
            .prefixes
            .iter()
            .any(|prefix| path_matches_prefix(path, prefix))
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
    if headers.get_all(header::AUTHORIZATION).iter().count() > 1 {
        return None;
    }
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            let (scheme, token) = value.trim().split_once(' ')?;
            scheme
                .eq_ignore_ascii_case("bearer")
                .then_some(token.trim())
        })
        .filter(|token| !token.is_empty())
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

fn decode_redis_rate_limit_result(
    current: i64,
    ttl: i64,
) -> Result<(usize, u64), RateLimitCheckError> {
    let current = usize::try_from(current)
        .ok()
        .filter(|count| *count > 0)
        .ok_or_else(|| {
            RateLimitCheckError::BackendUnavailable("invalid Redis rate-limit counter".to_string())
        })?;
    let ttl = u64::try_from(ttl).map_err(|_| {
        RateLimitCheckError::BackendUnavailable("invalid Redis rate-limit expiry".to_string())
    })?;
    Ok((current, ttl.div_ceil(1_000)))
}

fn bounded_redis_window_millis(window: Duration) -> i64 {
    window.as_millis().clamp(1, i64::MAX as u128) as i64
}

fn rate_limit_reset_after(remaining: Duration) -> u64 {
    remaining
        .as_secs()
        .saturating_add(u64::from(remaining.subsec_nanos() > 0))
}

fn rate_limit_retry_after(remaining: Duration) -> u64 {
    rate_limit_reset_after(remaining).max(1)
}

fn rate_limit_key_fingerprint(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))[..16].to_string()
}

async fn redis_with_timeout<T, E, F>(
    timeout: Duration,
    operation: &'static str,
    future: F,
) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| format!("{operation} timed out after {} ms", timeout.as_millis()))?
        // Redis errors can include server replies and connection details.
        // These diagnostics cross into module GraphQL logs; retain only the
        // fixed operation label, never the backend's uncontrolled text.
        .map_err(|_| format!("{operation} failed"))
}

fn insert_header_if_valid(headers: &mut axum::http::HeaderMap, key: &'static str, value: String) {
    match axum::http::HeaderValue::from_str(&value) {
        Ok(header_value) => {
            headers.insert(key, header_value);
        }
        Err(_error) => {
            tracing::warn!(
                header = key,
                failure_code = "rate_limit_header_value_invalid",
                "Skipping invalid rate limit header value"
            );
        }
    }
}

fn apply_rate_limit_headers(headers: &mut axum::http::HeaderMap, info: &RateLimitInfo) {
    insert_header_if_valid(headers, "x-ratelimit-limit", info.limit.to_string());
    insert_header_if_valid(headers, "x-ratelimit-remaining", info.remaining.to_string());
    insert_header_if_valid(headers, "x-ratelimit-reset", info.reset.to_string());
}

fn rate_limited_response(exceeded: &RateLimitExceeded) -> Response {
    let mut response =
        Json(ErrorBody::new("rate_limit_exceeded", "Rate limit exceeded")).into_response();
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

fn rate_limit_backend_unavailable_response() -> Response {
    let mut response = Json(ErrorBody::new(
        "rate_limit_backend_unavailable",
        "Rate limit backend unavailable",
    ))
    .into_response();
    *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
    insert_header_if_valid(response.headers_mut(), "retry-after", "1".to_string());
    tracing::error!(
        error_code = "rate_limit_backend_unavailable",
        "Rate limit backend unavailable"
    );
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
        state.limiter.uses_trusted_auth_dimensions(state.trusted_auth_dimensions),
        &RequestTrustSettings::default(),
    );

    let rate_limit_key_fingerprint = rate_limit_key_fingerprint(&rate_limit_key);
    debug!(
        rate_limit_key_fingerprint = %rate_limit_key_fingerprint,
        "Checking rate limit"
    );

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
        Err(RateLimitCheckError::BackendUnavailable(_reason)) => {
            record_rate_limit_backend_unavailable(state.limiter.namespace());
            Err(rate_limit_backend_unavailable_response())
        }
    }
}

/// Path-aware rate limiting middleware.
///
/// Applies rate limiting only to paths within one of the provided `prefixes`
/// (matching an exact path or a slash-delimited descendant). Unrelated paths
/// with the same character prefix are passed through unchanged.
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
        // The Auth namespace shares an IP-only budget with GraphQL. A valid
        // JWT must not partition the credential budget by tenant or app.
        policy.limiter.uses_trusted_auth_dimensions(state.trusted_auth_dimensions),
        &state.request_trust,
    );

    let rate_limit_key_fingerprint = rate_limit_key_fingerprint(&rate_limit_key);
    debug!(
        rate_limit_key_fingerprint = %rate_limit_key_fingerprint,
        path_group = crate::middleware::http_stack::trace_path_family(&path),
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
        Err(RateLimitCheckError::BackendUnavailable(_reason)) => {
            record_rate_limit_backend_unavailable(policy.limiter.namespace());
            Err(rate_limit_backend_unavailable_response())
        }
    }
}

#[cfg(test)]
mod fixed_window_tests {
    use super::*;

    #[test]
    fn fixed_window_starts_new_window_at_exact_deadline() {
        let start = Instant::now();
        let window = Duration::from_secs(60);
        let previous = advance_request_counter(None, start, window);
        assert_eq!(previous.count, 1);

        let before_end = advance_request_counter(
            Some(previous),
            start + window - Duration::from_nanos(1),
            window,
        );
        assert_eq!(before_end.count, 2);
        assert_eq!(before_end.window_start, start);

        let deadline = start + window;
        let new_window = advance_request_counter(Some(before_end), deadline, window);
        assert_eq!(new_window.count, 1);
        assert_eq!(new_window.window_start, deadline);
    }

    #[test]
    fn fixed_window_uses_actual_update_instant_for_elapsed_window() {
        let start = Instant::now();
        let window = Duration::from_secs(30);
        let previous = RequestCounter {
            count: 7,
            window_start: start,
        };
        // The request may have begun waiting before its window expired.
        // The serialized update must use the later actual update time.
        let updated = advance_request_counter(
            Some(previous),
            start + window + Duration::from_secs(1),
            window,
        );
        assert_eq!(updated.count, 1);
        assert_eq!(
            updated.window_start,
            start + window + Duration::from_secs(1)
        );
    }
}


#[cfg(test)]
mod window_bounds_tests {
    use super::*;

    #[test]
    fn active_rate_limit_windows_are_bounded_and_disabled_windows_stay_disabled() {
        let zero = normalize_rate_limit_config(RateLimitConfig {
            max_requests: 1,
            window: Duration::ZERO,
            enabled: true,
        });
        assert_eq!(zero.window, MIN_RATE_LIMIT_WINDOW);

        let subsecond = normalize_rate_limit_config(RateLimitConfig {
            max_requests: 1,
            window: Duration::from_millis(500),
            enabled: true,
        });
        assert_eq!(subsecond.window, MIN_RATE_LIMIT_WINDOW);

        let huge = normalize_rate_limit_config(RateLimitConfig {
            max_requests: 1,
            window: Duration::MAX,
            enabled: true,
        });
        assert_eq!(huge.window, MAX_RATE_LIMIT_WINDOW);

        assert_eq!(
            normalize_rate_limit_config(RateLimitConfig::disabled()).window,
            Duration::ZERO
        );
    }

    #[test]
    fn fractional_windows_share_the_same_memory_and_redis_precision() {
        let config = RateLimitConfig {
            max_requests: 1,
            window: Duration::from_nanos(1_500_000_001),
            enabled: true,
        };
        let memory = RateLimiter::new(config.clone());
        let redis = RateLimiter::with_redis(
            config,
            redis::Client::open("redis://127.0.0.1/").unwrap(),
            "rate-limit-test".to_string(),
        );

        assert_eq!(memory.config.window, Duration::from_millis(1501));
        assert_eq!(memory.config.window, redis.config.window);
        assert_eq!(bounded_redis_window_millis(redis.config.window), 1501);
        assert_eq!(memory.window_secs(), 2);
        assert_eq!(redis.window_secs(), 2);
    }

    #[tokio::test]
    async fn zero_length_memory_window_cannot_reset_the_counter_on_every_request() {
        let limiter = RateLimiter::new(RateLimitConfig::new(1, 0));
        assert_eq!(limiter.window_secs(), 1);
        assert!(limiter.check_rate_limit("client").await.is_ok());
        assert!(matches!(
            limiter.check_rate_limit("client").await,
            Err(RateLimitCheckError::Exceeded(_))
        ));
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;

    #[test]
    fn redis_rate_limit_script_repairs_counters_without_expiry() {
        assert!(RATE_LIMIT_REDIS_SCRIPT_SOURCE.contains("if ttl == -1 then"));
        assert!(RATE_LIMIT_REDIS_SCRIPT_SOURCE.contains("redis.call('PEXPIRE'"));
        assert!(RATE_LIMIT_REDIS_SCRIPT_SOURCE.contains("redis.call('PTTL'"));
        assert!(RATE_LIMIT_REDIS_SCRIPT_SOURCE.contains("if ttl < 0 then"));
        assert!(RATE_LIMIT_REDIS_SCRIPT_SOURCE.contains("redis.error_reply"));
    }

    #[test]
    fn redis_limiter_clones_share_one_reconnecting_connection_slot() {
        let client = redis::Client::open("redis://127.0.0.1:6379/0").expect("redis client");
        let limiter = RateLimiter::with_redis(
            RateLimitConfig::new(10, 60),
            client,
            "rate-limit:test".to_string(),
        );
        let clone = limiter.clone();
        let RateLimiterBackend::Redis { connection, .. } = &limiter.backend else {
            panic!("expected redis backend");
        };
        let RateLimiterBackend::Redis {
            connection: clone_connection,
            ..
        } = &clone.backend
        else {
            panic!("expected redis backend");
        };
        assert!(Arc::ptr_eq(connection, clone_connection));
        assert!(limiter.is_distributed());
    }

    #[test]
    fn rate_limit_retry_after_rounds_up_instead_of_inviting_early_retries() {
        assert_eq!(rate_limit_reset_after(Duration::ZERO), 0);
        assert_eq!(rate_limit_retry_after(Duration::ZERO), 1);
        assert_eq!(rate_limit_reset_after(Duration::from_secs(1)), 1);
        assert_eq!(rate_limit_retry_after(Duration::from_secs(1)), 1);
        assert_eq!(rate_limit_retry_after(Duration::from_millis(1_001)), 2);
        assert_eq!(rate_limit_retry_after(Duration::from_millis(1_999)), 2);
    }

    #[test]
    fn rate_limit_key_diagnostic_fingerprint_does_not_contain_raw_identity() {
        let raw = "ip:192.0.2.10|tenant:7f4f3e6e-3f1f-4f99-8d4b-3cc2c3b7a2d1|oauth_app:0e9adf5a-2b66-4d58-a5f7-4b5d1b8f7d12";
        let fingerprint = hex::encode(Sha256::digest(raw.as_bytes()))[..16].to_string();

        assert_ne!(fingerprint, raw);
        assert_eq!(fingerprint.len(), 16);
        assert!(!fingerprint.contains("192.0.2.10"));
        assert!(!fingerprint.contains("7f4f3e6e-3f1f-4f99-8d4b-3cc2c3b7a2d1"));
    }
}

#[cfg(test)]
#[path = "rate_limit_tests.rs"]
mod tests;
