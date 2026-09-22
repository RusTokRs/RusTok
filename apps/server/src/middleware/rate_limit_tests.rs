use super::*;
use crate::auth::{AuthConfig, encode_access_token, encode_oauth_access_token};
use crate::common::settings::ForwardedHeadersMode;
use axum::body::Body;
use rustok_core::UserRole;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use uuid::Uuid;

fn test_auth_config() -> AuthConfig {
    AuthConfig::new("rate-limit-test-secret-with-sufficient-length".to_string())
        .with_expiration(3600, 3600)
        .with_issuer("rustok")
        .with_audience("rustok-admin")
}

fn trusted_request_trust() -> RequestTrustSettings {
    RequestTrustSettings {
        forwarded_headers_mode: ForwardedHeadersMode::TrustedOnly,
        trusted_proxy_cidrs: vec!["10.0.0.0/8".to_string()],
    }
}

fn request_with_peer_ip(peer_ip: IpAddr) -> Request {
    let mut request = Request::builder()
        .uri("/api/test")
        .body(Body::empty())
        .expect("request");
    request
        .extensions_mut()
        .insert(SocketAddr::from((peer_ip, 443)));
    request
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
        requests.run_pending_tasks().await;
        assert_eq!(requests.entry_count(), 3);
    }

    tokio::time::sleep(Duration::from_secs(2)).await;

    limiter.cleanup_expired().await;

    {
        let RateLimiterBackend::Memory { requests } = &limiter.backend else {
            panic!("expected in-memory limiter");
        };
        requests.run_pending_tasks().await;
        assert_eq!(requests.entry_count(), 0);
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
    let headers = HeaderMap::new();
    let request = request_with_peer_ip(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)));

    let key = build_rate_limit_key(
        &headers,
        &request,
        Some(&test_auth_config()),
        false,
        &RequestTrustSettings::default(),
    );
    assert_eq!(key, "ip:1.2.3.4");
}

#[test]
fn build_rate_limit_key_ignores_invalid_bearer_token() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
    headers.insert(header::AUTHORIZATION, "Bearer broken".parse().unwrap());
    let request = request_with_peer_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10)));

    let key = build_rate_limit_key(
        &headers,
        &request,
        Some(&test_auth_config()),
        true,
        &trusted_request_trust(),
    );
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
    let request = request_with_peer_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10)));

    let key = build_rate_limit_key(
        &headers,
        &request,
        Some(&config),
        true,
        &trusted_request_trust(),
    );
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
        UserRole::Customer,
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
    let request = request_with_peer_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10)));

    let key = build_rate_limit_key(
        &headers,
        &request,
        Some(&config),
        true,
        &trusted_request_trust(),
    );
    assert_eq!(
        key,
        format!("ip:1.2.3.4|tenant:{tenant_id}|oauth_app:{client_id}")
    );
}

#[test]
fn build_rate_limit_key_ignores_spoofed_forwarded_ip_for_untrusted_peer() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", "203.0.113.8".parse().unwrap());
    let request = request_with_peer_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50)));

    let key = build_rate_limit_key(
        &headers,
        &request,
        Some(&test_auth_config()),
        false,
        &trusted_request_trust(),
    );

    assert_eq!(key, "ip:192.168.1.50");
}

#[test]
fn redis_rate_limit_key_is_bounded_stable_and_redacts_identity() {
    let identity = "ip:1.2.3.4|tenant:550e8400-e29b-41d4-a716-446655440000";
    let first = redis_rate_limit_key("rustok:rate-limit:api", identity);
    let second = redis_rate_limit_key("rustok:rate-limit:api", identity);

    assert_eq!(first, second);
    assert!(first.starts_with("rustok:rate-limit:api:v1:sha256:"));
    assert!(!first.contains("1.2.3.4"));
    assert!(!first.contains("550e8400"));
    assert!(first.len() < 160);
}

#[tokio::test]
async fn redis_operation_timeout_bounds_stalled_backend_work() {
    let error = redis_with_timeout(
        Duration::from_millis(5),
        "test rate-limit operation",
        std::future::pending::<Result<(), std::io::Error>>(),
    )
    .await
    .unwrap_err();

    assert!(error.contains("timed out"));
}

#[test]
fn redis_window_seconds_saturate_at_i64_max() {
    assert_eq!(bounded_redis_window_seconds(Duration::MAX), i64::MAX);
    assert_eq!(bounded_redis_window_seconds(Duration::ZERO), 1);
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

#[test]
fn matching_path_policy_uses_first_matching_policy_order() {
    let oauth = PathRateLimitPolicy {
        limiter: Arc::new(RateLimiter::new_with_namespace(
            RateLimitConfig::new(1, 60),
            "oauth",
        )),
        prefixes: Arc::new(vec!["/api/oauth", "/api/auth"]),
    };
    let auth = PathRateLimitPolicy {
        limiter: Arc::new(RateLimiter::new_with_namespace(
            RateLimitConfig::new(1, 60),
            "auth",
        )),
        prefixes: Arc::new(vec!["/api/auth"]),
    };
    let api = PathRateLimitPolicy {
        limiter: Arc::new(RateLimiter::new_with_namespace(
            RateLimitConfig::new(1, 60),
            "api",
        )),
        prefixes: Arc::new(vec!["/api"]),
    };

    let policies = vec![oauth, auth, api];

    let selected = matching_path_policy(&policies, "/api/auth/login").expect("policy");

    assert_eq!(selected.limiter.namespace(), "oauth");
}

#[test]
fn matching_path_policy_returns_none_for_unmatched_path() {
    let policies = vec![PathRateLimitPolicy {
        limiter: Arc::new(RateLimiter::new_with_namespace(
            RateLimitConfig::new(1, 60),
            "api",
        )),
        prefixes: Arc::new(vec!["/api"]),
    }];

    assert!(matching_path_policy(&policies, "/health/ready").is_none());
}
