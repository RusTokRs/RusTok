#[cfg(all(test, feature = "redis-cache"))]
mod redis_circuit_breaker_tests {
    use super::super::*;
    use crate::context::CacheBackend;
    use crate::resilience::CircuitBreakerConfig;
    use std::time::Duration;

    #[tokio::test]
    async fn test_redis_cache_with_circuit_breaker() {
        // This test requires Redis to be running
        // Skip if Redis is not available
        let cache_result = RedisCacheBackend::with_circuit_breaker(
            "redis://127.0.0.1:6379",
            "test",
            Duration::from_secs(300),
            CircuitBreakerConfig {
                failure_threshold: 3,
                success_threshold: 2,
                timeout: Duration::from_secs(5),
                half_open_max_requests: Some(2),
            },
        )
        .await;

        if cache_result.is_err() {
            println!("Skipping test: Redis not available");
            return;
        }

        let cache = cache_result.unwrap();

        // Test successful operations
        let key = "test_key";
        let value = b"test_value".to_vec();

        // Should work if Redis is available
        if cache.set(key.to_string(), value.clone()).await.is_ok() {
            let retrieved = cache.get(key).await.unwrap();
            assert_eq!(retrieved, Some(value));
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_redis_failure() {
        // Use invalid Redis URL to trigger failures
        let cache = RedisCacheBackend::with_circuit_breaker(
            "redis://invalid-host:9999",
            "test",
            Duration::from_secs(300),
            CircuitBreakerConfig {
                failure_threshold: 2,
                success_threshold: 2,
                timeout: Duration::from_millis(100),
                half_open_max_requests: Some(1),
            },
        )
        .await
        .unwrap();

        // First failure
        let result1 = cache.get("key1").await;
        assert!(result1.is_err());

        // Second failure - should trip the breaker
        let result2 = cache.get("key2").await;
        assert!(result2.is_err());

        // Check circuit breaker state
        let breaker = cache.circuit_breaker();
        assert_eq!(breaker.stats().await.failure_count, 2);

        // Third attempt might be rejected by open circuit
        let result3 = cache.get("key3").await;
        assert!(result3.is_err());
    }

    #[tokio::test]
    async fn test_circuit_breaker_state_exposed() {
        let cache = RedisCacheBackend::with_circuit_breaker(
            "redis://invalid:9999",
            "test",
            Duration::from_secs(300),
            CircuitBreakerConfig::default(),
        )
        .await
        .unwrap();

        let breaker = cache.circuit_breaker();

        // Initially closed
        let _state = breaker.get_state().await;
        // State is not public, but we can check failure count
        assert_eq!(breaker.stats().await.failure_count, 0);
    }
}

#[cfg(test)]
mod in_memory_cache_tests {
    use crate::context::CacheBackend;
    use crate::InMemoryCacheBackend;
    use std::time::Duration;

    #[tokio::test]
    async fn test_in_memory_cache_basic_operations() {
        let cache = InMemoryCacheBackend::new(Duration::from_secs(300), 100);

        let key = "test_key";
        let value = b"test_value".to_vec();

        // Set
        cache.set(key.to_string(), value.clone()).await.unwrap();

        // Get
        let retrieved = cache.get(key).await.unwrap();
        assert_eq!(retrieved, Some(value));

        // Invalidate
        cache.invalidate(key).await.unwrap();
        let after_invalidate = cache.get(key).await.unwrap();
        assert_eq!(after_invalidate, None);
    }

    #[tokio::test]
    async fn test_in_memory_cache_health() {
        let cache = InMemoryCacheBackend::new(Duration::from_secs(300), 100);
        assert!(cache.health().await.is_ok());
    }

    #[tokio::test]
    async fn test_in_memory_cache_stats() {
        let cache = InMemoryCacheBackend::new(Duration::from_secs(300), 100);

        cache
            .set("key1".to_string(), b"value1".to_vec())
            .await
            .unwrap();
        cache
            .set("key2".to_string(), b"value2".to_vec())
            .await
            .unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entries, 2);
    }

    #[tokio::test]
    async fn test_in_memory_cache_expiration() {
        let cache = InMemoryCacheBackend::new(Duration::from_millis(100), 100);

        cache
            .set("key".to_string(), b"value".to_vec())
            .await
            .unwrap();

        // Immediate get should work
        let immediate = cache.get("key").await.unwrap();
        assert!(immediate.is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        let expired = cache.get("key").await.unwrap();
        assert_eq!(expired, None);
    }

    #[tokio::test]
    async fn test_in_memory_cache_respects_per_entry_ttl() {
        let cache = InMemoryCacheBackend::new(Duration::from_secs(5), 100);

        cache
            .set_with_ttl(
                "short".to_string(),
                b"value".to_vec(),
                Duration::from_millis(50),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(90)).await;

        let expired = cache.get("short").await.unwrap();
        assert_eq!(expired, None);
    }

    #[tokio::test]
    async fn test_in_memory_cache_set_uses_default_ttl() {
        let cache = InMemoryCacheBackend::new(Duration::from_millis(50), 100);

        cache
            .set("default".to_string(), b"value".to_vec())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(90)).await;

        let expired = cache.get("default").await.unwrap();
        assert_eq!(expired, None);
    }
}
