use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rustok_cache::{
    CacheBackendOptions, CacheGenerationSource, CacheLeaseOptions, CacheLeaseOutcome, CacheService,
    DurableCacheInvalidationRecord, VersionedCacheInvalidation,
};
use rustok_core::CircuitBreakerConfig;

fn real_redis_url() -> String {
    std::env::var("RUSTOK_CACHE_REAL_REDIS_URL")
        .expect("RUSTOK_CACHE_REAL_REDIS_URL must point to an isolated Redis instance")
}

async fn pause_redis_all(url: &str, duration: Duration) {
    let client = redis::Client::open(url).expect("Redis URL should be valid");
    let mut connection = client
        .get_multiplexed_async_connection()
        .await
        .expect("Redis control connection should open");
    let reply = redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .arg("ALL")
        .query_async::<String>(&mut connection)
        .await
        .expect("Redis CLIENT PAUSE should succeed");
    assert_eq!(reply, "OK");
}

#[tokio::test]
#[ignore = "requires an isolated Redis instance via RUSTOK_CACHE_REAL_REDIS_URL"]
async fn namespace_generation_is_shared_and_monotonic_across_services() {
    let url = real_redis_url();
    let first_service = CacheService::from_url(Some(&url));
    let second_service = CacheService::from_url(Some(&url));
    let namespace = format!("real-generation-{}", uuid::Uuid::new_v4());

    let first_store = first_service.namespace_generations();
    let second_store = second_service.namespace_generations();

    let initial = first_store.read(&namespace).await.unwrap();
    assert_eq!(initial.value(), 0);
    assert_eq!(initial.source(), CacheGenerationSource::SharedRedis);

    let first = first_store.bump(&namespace).await.unwrap();
    assert_eq!(first.value(), 1);
    assert!(first.is_shared());

    let observed = second_store.read(&namespace).await.unwrap();
    assert_eq!(observed.value(), 1);
    assert_eq!(observed.source(), CacheGenerationSource::SharedRedis);

    let second = second_store.bump(&namespace).await.unwrap();
    assert_eq!(second.value(), 2);
    assert_eq!(first_store.read(&namespace).await.unwrap().value(), 2);
}

#[tokio::test]
#[ignore = "requires an isolated Redis instance via RUSTOK_CACHE_REAL_REDIS_URL"]
async fn durable_invalidation_reaches_a_ready_redis_subscriber() {
    let url = real_redis_url();
    let publisher = CacheService::from_url(Some(&url));
    let subscriber = CacheService::from_url(Some(&url));
    let namespace = format!("real-invalidation-generation-{}", uuid::Uuid::new_v4());
    let channel = format!("real.cache.invalidation.{}", uuid::Uuid::new_v4());

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (message_tx, message_rx) = tokio::sync::oneshot::channel();
    let message_tx = Arc::new(Mutex::new(Some(message_tx)));
    let invalidations = subscriber.invalidations();
    let subscribe_channel = channel.clone();
    let task = tokio::spawn(async move {
        invalidations
            .consume_subscription_with_ready(
                &subscribe_channel,
                move || async move {
                    let _ = ready_tx.send(());
                },
                move |message| {
                    let message_tx = Arc::clone(&message_tx);
                    async move {
                        if let Some(sender) = message_tx
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .take()
                        {
                            let _ = sender.send(message);
                        }
                    }
                },
            )
            .await
    });

    tokio::time::timeout(Duration::from_secs(5), ready_rx)
        .await
        .expect("Redis invalidation subscriber did not become ready")
        .expect("Redis invalidation ready signal was dropped");

    let generation = publisher
        .bump_cache_backend_generation(&namespace)
        .await
        .unwrap();
    let record = DurableCacheInvalidationRecord::new(
        uuid::Uuid::new_v4(),
        Some(uuid::Uuid::new_v4()),
        channel,
        "tenant:42",
        generation.generation,
        1_000,
        "tenant.updated",
        Some("live-redis-test".to_string()),
    )
    .unwrap();
    let outcome = publisher
        .invalidations()
        .publish_durable(&record)
        .await
        .unwrap();
    assert!(outcome.redis_published);

    let message = tokio::time::timeout(Duration::from_secs(5), message_rx)
        .await
        .expect("Redis invalidation message was not received")
        .expect("Redis invalidation message sender was dropped");
    let decoded = VersionedCacheInvalidation::from_message(&message).unwrap();
    assert_eq!(decoded.generation, generation.generation);
    assert_eq!(decoded.key, "tenant:42");

    task.abort();
}

#[tokio::test]
#[ignore = "requires an isolated Redis instance via RUSTOK_CACHE_REAL_REDIS_URL"]
async fn distributed_lease_enforces_token_ownership_and_reacquisition() {
    let url = real_redis_url();
    let first_service = CacheService::from_url(Some(&url));
    let second_service = CacheService::from_url(Some(&url));
    let key = format!("real-lease-{}", uuid::Uuid::new_v4());
    let options =
        CacheLeaseOptions::new(Duration::from_secs(2), Duration::from_millis(500)).unwrap();

    let first = match first_service
        .try_acquire_distributed_lease("hardening", &key, options)
        .await
        .unwrap()
    {
        CacheLeaseOutcome::Acquired(lease) => lease,
        CacheLeaseOutcome::Contended => panic!("first lease acquisition must succeed"),
    };

    assert!(matches!(
        second_service
            .try_acquire_distributed_lease("hardening", &key, options)
            .await
            .unwrap(),
        CacheLeaseOutcome::Contended
    ));

    assert!(first.release().await.unwrap());

    let second = match second_service
        .try_acquire_distributed_lease("hardening", &key, options)
        .await
        .unwrap()
    {
        CacheLeaseOutcome::Acquired(lease) => lease,
        CacheLeaseOutcome::Contended => panic!("lease must be acquirable after owner release"),
    };
    assert!(second.release().await.unwrap());
}

#[tokio::test]
#[ignore = "requires an isolated Redis instance via RUSTOK_CACHE_REAL_REDIS_URL"]
async fn shared_client_weighted_backend_honors_subsecond_ttl_and_invalidation() {
    let url = real_redis_url();
    let service = CacheService::from_url(Some(&url));
    let prefix = format!("real-weighted-{}", uuid::Uuid::new_v4());
    let backend = service
        .backend_weighted(&prefix, Duration::from_secs(10), 1024 * 1024)
        .await;

    backend
        .set_with_ttl(
            "short".to_string(),
            b"value".to_vec(),
            Duration::from_millis(80),
        )
        .await
        .unwrap();
    assert_eq!(backend.get("short").await.unwrap(), Some(b"value".to_vec()));

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(backend.get("short").await.unwrap(), None);

    backend
        .set("invalidate".to_string(), b"value".to_vec())
        .await
        .unwrap();
    backend.invalidate("invalidate").await.unwrap();
    assert_eq!(backend.get("invalidate").await.unwrap(), None);
}

#[tokio::test]
#[ignore = "requires Redis 7 CLIENT PAUSE via RUSTOK_CACHE_REAL_REDIS_URL"]
async fn shared_backend_times_out_opens_circuit_and_recovers_after_latency() {
    let url = real_redis_url();
    let service = CacheService::from_url(Some(&url));
    let mut options = CacheBackendOptions::default();
    options.redis_circuit_breaker = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_millis(200),
        half_open_max_requests: Some(1),
    };
    let backend = service
        .backend_shared_client_with_options(
            &format!("real-latency-{}", uuid::Uuid::new_v4()),
            Duration::from_secs(30),
            128,
            options,
        )
        .await;
    backend
        .health()
        .await
        .expect("shared Redis backend should start healthy");

    pause_redis_all(&url, Duration::from_millis(2_600)).await;
    tokio::time::sleep(Duration::from_millis(25)).await;

    let timed_out_at = Instant::now();
    let timeout_error = backend
        .health()
        .await
        .expect_err("paused Redis health must hit the bounded operation timeout");
    let timeout_elapsed = timed_out_at.elapsed();
    assert!(
        timeout_error
            .to_string()
            .contains("shared Redis cache operation timed out after 2000 ms")
    );
    assert!(timeout_elapsed < Duration::from_millis(2_500));

    let rejected_at = Instant::now();
    let open_error = backend
        .health()
        .await
        .expect_err("opened Redis circuit must reject the next health probe");
    assert!(
        open_error
            .to_string()
            .contains("Redis unavailable (circuit breaker open)")
    );
    assert!(rejected_at.elapsed() < Duration::from_millis(250));

    tokio::time::sleep(Duration::from_millis(800)).await;
    backend
        .health()
        .await
        .expect("half-open Redis probe should recover after CLIENT PAUSE expires");
    backend
        .health()
        .await
        .expect("closed Redis circuit should remain healthy after recovery");
}
