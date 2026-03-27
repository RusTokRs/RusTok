/// Rate Limiting Middleware for RusToK
///
/// Implements a sliding window rate limiter to protect endpoints from abuse.
/// Supports per-IP rate limiting with configurable limits.
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use once_cell::sync::Lazy;
use redis::Script;
use rustok_telemetry::metrics::{
    record_rate_limit_backend_unavailable, record_rate_limit_exceeded, update_rate_limit_runtime,
};
use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use rustok_cache::CacheService;

use crate::auth::{decode_access_token, AuthConfig};
use crate::common::settings::RateLimitBackendKind;

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

#[derive(Clone)]
enum RateLimiterBackend {
    Memory {
        requests: Arc<RwLock<HashMap<String, RequestCounter>>>,
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
        Self {
            backend: RateLimiterBackend::Memory {
                requests: Arc::new(RwLock::new(HashMap::new())),
            },
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
        requests: &Arc<RwLock<HashMap<String, RequestCounter>>>,
        key: &str,
    ) -> Result<RateLimitInfo, RateLimitCheckError> {
        let now = Instant::now();

        {
            let requests = requests.read().await;
            if let Some(counter) = requests.get(key).filter(|counter| {
                now.duration_since(counter.window_start) <= self.config.window
                    && counter.count >= self.config.max_requests
            }) {
                let retry_after = self
                    .config
                    .window
                    .saturating_sub(now.duration_since(counter.window_start))
                    .as_secs();

                warn!(
                    key = %key,
                    count = counter.count,
                    limit = self.config.max_requests,
                    retry_after = retry_after,
                    "Rate limit exceeded"
                );

                return Err(RateLimitCheckError::Exceeded(RateLimitExceeded::new(
                    self.config.max_requests,
                    retry_after,
                )));
            }
        }

        let mut requests = requests.write().await;
        let counter = requests
            .entry(key.to_string())
            .or_insert_with(|| RequestCounter {
                count: 0,
                window_start: now,
            });

        if now.duration_since(counter.window_start) > self.config.window {
            counter.count = 0;
            counter.window_start = now;
        }

        if counter.count >= self.config.max_requests {
            let retry_after = self
                .config
                .window
                .saturating_sub(now.duration_since(counter.window_start))
                .as_secs();

            warn!(
                key = %key,
                count = counter.count,
                limit = self.config.max_requests,
                retry_after = retry_after,
                "Rate limit exceeded (race condition check)"
            );

            return Err(RateLimitCheckError::Exceeded(RateLimitExceeded::new(
                self.config.max_requests,
                retry_after,
            )));
        }

        counter.count += 1;

        let reset_at = counter.window_start + self.config.window;
        let reset_secs = reset_at.saturating_duration_since(now).as_secs();

        Ok(RateLimitInfo {
            limit: self.config.max_requests,
            remaining: self.config.max_requests.saturating_sub(counter.count),
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

        let redis_key = format!("{key_prefix}:{key}");
        let window_secs = self.window_secs().max(1);
        let mut connection = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| {
                RateLimitCheckError::BackendUnavailable(format!(
                    "failed to connect to redis rate-limit backend: {error}"
                ))
            })?;

        let (current, ttl): (i64, i64) = RATE_LIMIT_REDIS_SCRIPT
            .key(redis_key.as_str())
            .arg(window_secs as i64)
            .invoke_async(&mut connection)
            .await
            .map_err(|error| {
                RateLimitCheckError::BackendUnavailable(format!(
                    "failed to execute redis rate-limit script: {error}"
                ))
            })?;

        let current = current.max(0) as usize;
        let retry_after = ttl.max(1) as u64;

        if current > self.config.max_requests {
            warn!(
                key = %key,
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
        let mut requests = requests.write().await;
        let now = Instant::now();

        requests
            .retain(|_, counter| now.duration_since(counter.window_start) <= self.config.window);

        debug!(
            retained = requests.len(),
            "Cleaned up expired rate limit entries"
        );
    }

    pub async fn check_backend_health(&self) -> Result<(), String> {
        match &self.backend {
            RateLimiterBackend::Memory { .. } => Ok(()),
            RateLimiterBackend::Redis { client, .. } => {
                let mut connection =
                    client
                        .get_multiplexed_async_connection()
                        .await
                        .map_err(|error| {
                            format!("failed to connect to redis rate-limit backend: {error}")
                        })?;

                let response: String = redis::cmd("PING")
                    .query_async(&mut connection)
                    .await
                    .map_err(|error| format!("failed to ping redis rate-limit backend: {error}"))?;

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
                let requests = requests.read().await;
                RateLimitStats {
                    active_clients: requests.len(),
                    total_entries: requests.len(),
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
    pub limiter: Arc<RateLimiter>,
    pub prefixes: Arc<Vec<&'static str>>,
    pub auth_config: Option<AuthConfig>,
    pub trusted_auth_dimensions: bool,
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
/// such as X-User-ID.  Any client can set an arbitrary value, which would allow
/// them to exhaust another user's rate-limit bucket or bypass their own.
/// User-scoped rate limiting must be implemented after JWT verification using
/// the verified claims from a trusted middleware layer.
fn extract_client_id(headers: &HeaderMap) -> String {
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(first_ip) = forwarded_str.split(',').next() {
                let ip = first_ip.trim();
                if IpAddr::from_str(ip).is_ok() {
                    return format!("ip:{}", ip);
                }
            }
        }
    }

    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            if IpAddr::from_str(ip_str).is_ok() {
                return format!("ip:{}", ip_str);
            }
        }
    }

    "ip:unknown".to_string()
}

pub fn extract_client_id_pub(headers: &HeaderMap) -> String {
    extract_client_id(headers)
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
    auth_config: Option<&AuthConfig>,
    trusted_auth_dimensions: bool,
) -> String {
    let mut key = extract_client_id(headers);

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
        state.auth_config.as_ref(),
        state.trusted_auth_dimensions,
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
    let should_limit = state.prefixes.iter().any(|prefix| path.starts_with(prefix));

    if !should_limit {
        return Ok(next.run(request).await);
    }

    let rate_limit_key = build_rate_limit_key(
        &headers,
        state.auth_config.as_ref(),
        state.trusted_auth_dimensions,
    );

    debug!(rate_limit_key = %rate_limit_key, path = %path, "Checking rate limit for auth path");

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

pub async fn cleanup_task(limiter: Arc<RateLimiter>) {
    let mut interval = tokio::time::interval(Duration::from_secs(300));

    loop {
        interval.tick().await;
        limiter.cleanup_expired().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{encode_access_token, encode_oauth_access_token, AuthConfig};
    use rustok_core::UserRole;
    use uuid::Uuid;

    fn test_auth_config() -> AuthConfig {
        AuthConfig::new("rate-limit-test-secret-with-sufficient-length".to_string())
            .with_expiration(3600, 3600)
            .with_issuer("rustok")
            .with_audience("rustok-admin")
    }

    #[tokio::test]
    async fn test_rate_limit_allows_requests_within_limit() {
        let config = RateLimitConfig::new(5, 60);
        let limiter = RateLimiter::new(config);

        for i in 1..=5 {
            let result = limiter.check_rate_limit("test-client").await;
            assert!(result.is_ok(), "Request {} should be allowed", i);

            let info = result.unwrap();
            assert_eq!(info.remaining, 5 - i);
        }
    }

    #[tokio::test]
    async fn test_rate_limit_blocks_excess_requests() {
        let config = RateLimitConfig::new(3, 60);
        let limiter = RateLimiter::new(config);

        for _ in 0..3 {
            assert!(limiter.check_rate_limit("test-client").await.is_ok());
        }

        let result = limiter.check_rate_limit("test-client").await;
        assert!(result.is_err());
        let RateLimitCheckError::Exceeded(exceeded) = result.unwrap_err() else {
            panic!("expected exceeded error");
        };
        assert_eq!(exceeded.limit, 3);
        assert!(exceeded.retry_after > 0);
    }

    #[tokio::test]
    async fn test_rate_limit_resets_after_window() {
        let config = RateLimitConfig::new(2, 1);
        let limiter = RateLimiter::new(config);

        assert!(limiter.check_rate_limit("test-client").await.is_ok());
        assert!(limiter.check_rate_limit("test-client").await.is_ok());
        assert!(limiter.check_rate_limit("test-client").await.is_err());

        tokio::time::sleep(Duration::from_secs(2)).await;

        assert!(limiter.check_rate_limit("test-client").await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limit_separate_clients() {
        let config = RateLimitConfig::new(2, 60);
        let limiter = RateLimiter::new(config);

        assert!(limiter.check_rate_limit("client-a").await.is_ok());
        assert!(limiter.check_rate_limit("client-a").await.is_ok());
        assert!(limiter.check_rate_limit("client-a").await.is_err());

        assert!(limiter.check_rate_limit("client-b").await.is_ok());
        assert!(limiter.check_rate_limit("client-b").await.is_ok());
    }

    #[tokio::test]
    async fn test_disabled_rate_limiter() {
        let config = RateLimitConfig::disabled();
        let limiter = RateLimiter::new(config);

        for _ in 0..1000 {
            assert!(limiter.check_rate_limit("test-client").await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let config = RateLimitConfig::new(10, 1);
        let limiter = RateLimiter::new(config);

        limiter.check_rate_limit("client-1").await.ok();
        limiter.check_rate_limit("client-2").await.ok();
        limiter.check_rate_limit("client-3").await.ok();

        {
            let RateLimiterBackend::Memory { requests } = &limiter.backend else {
                panic!("expected in-memory limiter");
            };
            let requests = requests.read().await;
            assert_eq!(requests.len(), 3);
        }

        tokio::time::sleep(Duration::from_secs(2)).await;

        limiter.cleanup_expired().await;

        {
            let RateLimiterBackend::Memory { requests } = &limiter.backend else {
                panic!("expected in-memory limiter");
            };
            let requests = requests.read().await;
            assert_eq!(requests.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        use tokio::task::JoinSet;

        let config = RateLimitConfig::new(100, 60);
        let limiter = Arc::new(RateLimiter::new(config));

        let mut tasks = JoinSet::new();

        for i in 0..50 {
            let limiter = limiter.clone();
            tasks.spawn(async move { limiter.check_rate_limit(&format!("client-{}", i)).await });
        }

        while let Some(result) = tasks.join_next().await {
            assert!(result.unwrap().is_ok());
        }
    }

    #[test]
    fn extract_client_id_does_not_use_x_user_id() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-user-id",
            "550e8400-e29b-41d4-a716-446655440000".parse().unwrap(),
        );
        headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());

        let id = extract_client_id(&headers);
        assert_eq!(id, "ip:1.2.3.4", "must use IP, not x-user-id");
    }

    #[test]
    fn extract_client_id_falls_back_to_unknown() {
        let headers = HeaderMap::new();
        let id = extract_client_id(&headers);
        assert_eq!(id, "ip:unknown");
    }

    #[test]
    fn build_rate_limit_key_uses_ip_only_without_trusted_dimensions() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());

        let key = build_rate_limit_key(&headers, Some(&test_auth_config()), false);
        assert_eq!(key, "ip:1.2.3.4");
    }

    #[test]
    fn build_rate_limit_key_ignores_invalid_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        headers.insert(header::AUTHORIZATION, "Bearer broken".parse().unwrap());

        let key = build_rate_limit_key(&headers, Some(&test_auth_config()), true);
        assert_eq!(key, "ip:1.2.3.4");
    }

    #[test]
    fn build_rate_limit_key_adds_trusted_tenant_dimension_for_direct_token() {
        let config = test_auth_config();
        let tenant_id = Uuid::new_v4();
        let token = encode_access_token(
            &config,
            Uuid::new_v4(),
            tenant_id,
            UserRole::Admin,
            Uuid::new_v4(),
        )
        .expect("token");

        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );

        let key = build_rate_limit_key(&headers, Some(&config), true);
        assert_eq!(key, format!("ip:1.2.3.4|tenant:{tenant_id}"));
    }

    #[test]
    fn build_rate_limit_key_adds_oauth_app_dimension_for_oauth_token() {
        let config = test_auth_config();
        let tenant_id = Uuid::new_v4();
        let client_id = Uuid::new_v4();
        let token = encode_oauth_access_token(
            &config,
            Uuid::new_v4(),
            tenant_id,
            client_id,
            &["catalog:read".to_string()],
            "client_credentials",
            3600,
        )
        .expect("token");

        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );

        let key = build_rate_limit_key(&headers, Some(&config), true);
        assert_eq!(
            key,
            format!("ip:1.2.3.4|tenant:{tenant_id}|oauth_app:{client_id}")
        );
    }

    #[test]
    fn rate_limited_response_includes_contract_headers() {
        let response = rate_limited_response(&RateLimitExceeded::new(20, 42));

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()["retry-after"], "42");
        assert_eq!(response.headers()["x-ratelimit-limit"], "20");
        assert_eq!(response.headers()["x-ratelimit-remaining"], "0");
        assert_eq!(response.headers()["x-ratelimit-reset"], "42");
    }
}
