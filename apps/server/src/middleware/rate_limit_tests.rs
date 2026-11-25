use super::*;
use crate::auth::{AuthConfig, encode_access_token, encode_oauth_access_token};
use crate::common::settings::ForwardedHeadersMode;
use axum::body::Body;
use axum::http::HeaderValue;
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
fn build_rate_limit_key_accepts_case_insensitive_bearer_scheme() {
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
        format!("BEARER {token}").parse().unwrap(),
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
fn retry_after_never_reports_zero_for_an_active_window() {
    assert_eq!(rate_limit_retry_after(Duration::from_millis(999)), 1);
    assert_eq!(rate_limit_retry_after(Duration::from_secs(2)), 2);
    assert_eq!(rate_limit_retry_after(Duration::ZERO), 1);
}

#[test]
fn diagnostic_key_fingerprint_is_stable_and_redacted() {
    let raw = "ip:192.0.2.10|tenant:550e8400-e29b-41d4-a716-446655440000";
    let fingerprint = rate_limit_key_fingerprint(raw);

    assert_eq!(fingerprint.len(), 16);
    assert_ne!(fingerprint, raw);
    assert!(!fingerprint.contains("192.0.2.10"));
    assert!(!fingerprint.contains("550e8400"));
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
fn build_rate_limit_key_cannot_be_changed_by_spoofing_a_leftmost_proxy_hop() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        "198.51.100.66, 203.0.113.8".parse().unwrap(),
    );
    let request = request_with_peer_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10)));

    let key = build_rate_limit_key(
        &headers,
        &request,
        Some(&test_auth_config()),
        false,
        &trusted_request_trust(),
    );

    assert_eq!(key, "ip:203.0.113.8");
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

#[tokio::test]
async fn redis_connection_timeout_includes_waiting_for_initializer() {
    let client = redis::Client::open("redis://127.0.0.1:6379/").expect("Redis client");
    let limiter = RateLimiter::with_redis(
        RateLimitConfig::new(5, 60),
        client.clone(),
        "rate-limit-initializer-test".to_string(),
    );
    let RateLimiterBackend::Redis { connection, .. } = &limiter.backend else {
        panic!("expected Redis backend");
    };

    let held_slot = Arc::clone(connection);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let initializer = tokio::spawn(async move {
        held_slot
            .get_or_try_init(|| async move {
                started_tx.send(()).expect("initializer started");
                std::future::pending::<Result<redis::aio::ConnectionManager, String>>().await
            })
            .await
            .map(|_| ())
    });
    started_rx.await.expect("initialization slot acquired");

    let result = tokio::time::timeout(
        RATE_LIMIT_REDIS_OPERATION_TIMEOUT + Duration::from_secs(2),
        limiter.redis_connection(&client, connection),
    )
    .await;
    initializer.abort();
    assert!(initializer.await.expect_err("initializer cancelled").is_cancelled());

    let Err(error) = result.expect("waiting for initializer must also be bounded") else {
        panic!("stalled initialization cannot succeed");
    };
    assert!(error.contains("timed out"));
    assert!(connection.get().is_none());
}

#[test]
fn redis_window_millis_saturate_at_i64_max() {
    assert_eq!(bounded_redis_window_millis(Duration::MAX), i64::MAX);
    assert_eq!(bounded_redis_window_millis(Duration::ZERO), 1);
    assert_eq!(bounded_redis_window_millis(Duration::from_millis(1500)), 1500);
}

#[test]
fn redis_rate_limit_result_rejects_invalid_counters_and_expiry() {
    for (count, ttl) in [(-1, 1000), (0, 1000), (1, -1), (1, -2)] {
        assert!(matches!(
            decode_redis_rate_limit_result(count, ttl),
            Err(RateLimitCheckError::BackendUnavailable(_))
        ));
    }
    assert_eq!(decode_redis_rate_limit_result(1, 0).unwrap(), (1, 0));
    assert_eq!(decode_redis_rate_limit_result(2, 1501).unwrap(), (2, 2));
}

#[tokio::test]
#[ignore = "requires an isolated Redis instance via RUSTOK_CACHE_REAL_REDIS_URL"]
async fn real_redis_rate_limit_preserves_precision_and_rejects_corrupt_counters() {
    let url = std::env::var("RUSTOK_CACHE_REAL_REDIS_URL")
        .expect("isolated Redis URL");
    let client = redis::Client::open(url).expect("Redis client");
    let mut connection = client.get_multiplexed_async_connection().await.unwrap();
    let prefix = format!("rate-limit-regression:{}", Uuid::new_v4());
    let key = redis_rate_limit_key(&prefix, "client");
    let script = Script::new(RATE_LIMIT_REDIS_SCRIPT_SOURCE);
    let window_ms = 60_501_i64;

    let (count, ttl): (i64, i64) = script
        .key(&key)
        .arg(window_ms)
        .invoke_async(&mut connection)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(ttl, window_ms);

    let _: () = redis::cmd("SET")
        .arg(&key)
        .arg(-1)
        .query_async(&mut connection)
        .await
        .unwrap();
    let limiter = RateLimiter::with_redis(
        RateLimitConfig::new(5, 60),
        client,
        prefix,
    );
    for _ in 0..2 {
        assert!(matches!(
            limiter.check_rate_limit("client").await,
            Err(RateLimitCheckError::BackendUnavailable(_))
        ));
    }
    let stored: i64 = redis::cmd("GET")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert_eq!(stored, -1);
    let ttl: i64 = redis::cmd("PTTL")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert!(ttl > 0 && ttl <= 60_000);

    // Overflow must not leave persistent counters blocked indefinitely.
    for invalid in ["9223372036854775807", "9223372036854775808"] {
        let _: () = redis::cmd("SET")
            .arg(&key)
            .arg(invalid)
            .query_async(&mut connection)
            .await
            .unwrap();
        for _ in 0..2 {
            assert!(matches!(
                limiter.check_rate_limit("client").await,
                Err(RateLimitCheckError::BackendUnavailable(_))
            ));
        }
        let stored: String = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut connection)
            .await
            .unwrap();
        assert_eq!(stored, invalid);
        let ttl: i64 = redis::cmd("PTTL")
            .arg(&key)
            .query_async(&mut connection)
            .await
            .unwrap();
        assert!(ttl > 0 && ttl <= 60_000);
    }

    // GET can also fail on a restored wrong-type key.
    let _: u64 = redis::cmd("DEL")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .unwrap();
    let _: u64 = redis::cmd("RPUSH")
        .arg(&key)
        .arg("invalid-counter")
        .query_async(&mut connection)
        .await
        .unwrap();
    assert!(matches!(
        limiter.check_rate_limit("client").await,
        Err(RateLimitCheckError::BackendUnavailable(_))
    ));
    let ttl: i64 = redis::cmd("PTTL")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert!(ttl > 0 && ttl <= 60_000);

    let deleted: u64 = redis::cmd("DEL")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert_eq!(deleted, 1);
}

#[tokio::test]
async fn rate_limited_response_includes_contract_headers() {
    let response = rate_limited_response(&RateLimitExceeded::new(20, 42));

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()["retry-after"], "42");
    assert_eq!(response.headers()["x-ratelimit-limit"], "20");
    assert_eq!(response.headers()["x-ratelimit-remaining"], "0");
    assert_eq!(response.headers()["x-ratelimit-reset"], "42");
    assert_eq!(response.headers()["content-type"], "application/json");
    let body = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body, serde_json::json!({
        "code": "rate_limit_exceeded",
        "message": "Rate limit exceeded"
    }));
}

#[test]
fn rate_limit_backend_unavailable_response_includes_contract_headers_and_status() {
    let response = rate_limit_backend_unavailable_response();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["retry-after"], "1");
    assert_eq!(response.headers()["content-type"], "application/json");
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
fn matching_path_policy_does_not_overmatch_unrelated_path_segments() {
    let policies = vec![PathRateLimitPolicy {
        limiter: Arc::new(RateLimiter::new_with_namespace(
            RateLimitConfig::new(1, 60),
            "auth",
        )),
        prefixes: Arc::new(vec!["/api/auth", "/health/ready"]),
    }];

    assert!(matching_path_policy(&policies, "/api/auth").is_some());
    assert!(matching_path_policy(&policies, "/api/auth/login").is_some());
    assert!(matching_path_policy(&policies, "/health/ready").is_some());
    assert!(matching_path_policy(&policies, "/api/authentication").is_none());
    assert!(matching_path_policy(&policies, "/api/auth2/login").is_none());
    assert!(matching_path_policy(&policies, "/health/readiness").is_none());
}

#[test]
fn matching_path_policy_respects_explicit_trailing_slash_prefixes() {
    let policies = vec![PathRateLimitPolicy {
        limiter: Arc::new(RateLimiter::new_with_namespace(
            RateLimitConfig::new(1, 60),
            "api",
        )),
        prefixes: Arc::new(vec!["/api/"]),
    }];

    assert!(matching_path_policy(&policies, "/api/graphql").is_some());
    assert!(matching_path_policy(&policies, "/api/").is_some());
    assert!(matching_path_policy(&policies, "/apiculture").is_none());
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

#[test]
fn extract_bearer_token_rejects_duplicate_authorization_headers() {
    let mut headers = HeaderMap::new();
    headers.append(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer token-one"),
    );
    headers.append(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer token-two"),
    );

    assert_eq!(extract_bearer_token(&headers), None);
}
