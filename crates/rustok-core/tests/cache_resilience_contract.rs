use rustok_core::InMemoryCacheBackend;
use rustok_core::context::CacheBackend;
use rustok_core::resilience::{
    Bulkhead, BulkheadConfig, BulkheadError, CircuitBreaker, CircuitBreakerConfig,
    CircuitBreakerError, CircuitState, RetryPolicy, RetryStrategy, with_timeout,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[tokio::test]
async fn in_memory_cache_respects_entry_specific_ttl_and_invalidation() {
    let cache = InMemoryCacheBackend::new(Duration::from_secs(60), 128);

    cache
        .set_with_ttl(
            "short_lived".to_string(),
            b"transient".to_vec(),
            Duration::from_millis(25),
        )
        .await
        .unwrap();
    cache
        .set("default_ttl".to_string(), b"persistent".to_vec())
        .await
        .unwrap();

    assert_eq!(
        cache.get("short_lived").await.unwrap(),
        Some(b"transient".to_vec())
    );
    assert_eq!(
        cache.get("default_ttl").await.unwrap(),
        Some(b"persistent".to_vec())
    );

    tokio::time::sleep(Duration::from_millis(75)).await;

    assert_eq!(cache.get("short_lived").await.unwrap(), None);
    assert_eq!(
        cache.get("default_ttl").await.unwrap(),
        Some(b"persistent".to_vec())
    );

    cache.invalidate("default_ttl").await.unwrap();
    assert_eq!(cache.get("default_ttl").await.unwrap(), None);
    assert_eq!(cache.stats().hits, 0);
    assert_eq!(cache.stats().misses, 0);
}

#[test]
fn retry_strategy_delays_are_capped_at_configured_maximum() {
    let fixed = RetryStrategy::Fixed(Duration::from_millis(7));
    assert_eq!(fixed.delay(0), Duration::from_millis(7));
    assert_eq!(fixed.delay(25), Duration::from_millis(7));

    let exponential = RetryStrategy::Exponential {
        base: Duration::from_millis(100),
        max: Duration::from_millis(250),
    };
    assert_eq!(exponential.delay(0), Duration::from_millis(100));
    assert_eq!(exponential.delay(1), Duration::from_millis(200));
    assert_eq!(exponential.delay(2), Duration::from_millis(250));

    let linear = RetryStrategy::Linear {
        base: Duration::from_millis(90),
        max: Duration::from_millis(200),
    };
    assert_eq!(linear.delay(1), Duration::from_millis(90));
    assert_eq!(linear.delay(2), Duration::from_millis(180));
    assert_eq!(linear.delay(3), Duration::from_millis(200));
}

#[tokio::test]
async fn retry_policy_stops_when_error_is_not_retryable() {
    let attempts = Arc::new(AtomicU32::new(0));
    let policy = RetryPolicy {
        max_attempts: 5,
        strategy: RetryStrategy::Fixed(Duration::from_millis(1)),
        retryable_predicate: Some(|error| error.contains("retryable")),
    };

    let result = policy
        .execute({
            let attempts = Arc::clone(&attempts);
            move || {
                let attempts = Arc::clone(&attempts);
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>("fatal")
                }
            }
        })
        .await;

    assert_eq!(result, Err("fatal"));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn circuit_breaker_manual_controls_reset_state_and_counters() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_millis(10),
        half_open_max_requests: Some(1),
    });

    breaker.open().await;
    assert_eq!(breaker.get_state().await, CircuitState::Open);

    let rejected = breaker.call(|| async { Ok::<_, &str>(()) }).await;
    assert!(matches!(rejected, Err(CircuitBreakerError::Open)));
    assert_eq!(breaker.stats().await.total_rejected, 1);

    breaker.close().await;
    assert_eq!(breaker.get_state().await, CircuitState::Closed);
    assert_eq!(
        breaker.call(|| async { Ok::<_, &str>(42) }).await.unwrap(),
        42
    );

    breaker.reset().await;
    let stats = breaker.stats().await;
    assert_eq!(stats.state, CircuitState::Closed);
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.total_successes, 0);
    assert_eq!(stats.total_rejected, 0);
}

#[tokio::test]
async fn circuit_breaker_limits_half_open_probe_requests() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 2,
        timeout: Duration::from_millis(1),
        half_open_max_requests: Some(1),
    });

    let _ = breaker.call(|| async { Err::<(), _>("boom") }).await;
    assert_eq!(breaker.get_state().await, CircuitState::Open);

    tokio::time::sleep(Duration::from_millis(5)).await;

    let probe = breaker.call(|| async { Ok::<_, &str>(()) }).await;
    assert!(probe.is_ok());
    assert_eq!(breaker.get_state().await, CircuitState::HalfOpen);

    let limited = breaker.call(|| async { Ok::<_, &str>(()) }).await;
    assert!(matches!(limited, Err(CircuitBreakerError::Open)));
    assert_eq!(breaker.stats().await.total_rejected, 1);
}

#[tokio::test]
async fn bulkhead_tracks_failure_and_rejection_rates() {
    let bulkhead = Arc::new(Bulkhead::new(BulkheadConfig {
        max_concurrent_calls: 1,
        max_wait_duration: None,
    }));

    let upstream = bulkhead.call(|| async { Err::<(), _>("upstream") }).await;
    assert!(matches!(upstream, Err(BulkheadError::Upstream("upstream"))));

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let holder = {
        let bulkhead = Arc::clone(&bulkhead);
        tokio::spawn(async move {
            bulkhead
                .call(|| async move {
                    rx.await.ok();
                    Ok::<_, &str>(())
                })
                .await
        })
    };

    tokio::time::sleep(Duration::from_millis(20)).await;

    let rejected = bulkhead.call(|| async { Ok::<_, &str>(()) }).await;
    assert!(matches!(rejected, Err(BulkheadError::Full)));

    tx.send(()).ok();
    holder.await.unwrap().unwrap();

    let stats = bulkhead.stats();
    assert_eq!(stats.total_requests, 3);
    assert_eq!(stats.total_successes, 1);
    assert_eq!(stats.total_failures, 1);
    assert_eq!(stats.total_rejected, 1);
    assert_eq!(stats.rejection_rate(), 1.0 / 3.0);
}

#[tokio::test]
async fn timeout_error_preserves_configured_deadline() {
    let result = with_timeout(Duration::from_millis(5), || async {
        tokio::time::sleep(Duration::from_millis(25)).await;
        "finished"
    })
    .await;

    let error = result.unwrap_err();
    assert_eq!(error.duration, Duration::from_millis(5));
    assert_eq!(error.to_string(), "Operation timed out after 5ms");
}
