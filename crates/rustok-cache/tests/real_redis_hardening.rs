use std::time::Duration;

use rustok_cache::{CacheGenerationSource, CacheLeaseOptions, CacheLeaseOutcome, CacheService};

fn real_redis_url() -> String {
    std::env::var("RUSTOK_CACHE_REAL_REDIS_URL")
        .expect("RUSTOK_CACHE_REAL_REDIS_URL must point to an isolated Redis instance")
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
async fn distributed_lease_enforces_token_ownership_and_reacquisition() {
    let url = real_redis_url();
    let first_service = CacheService::from_url(Some(&url));
    let second_service = CacheService::from_url(Some(&url));
    let key = format!("real-lease-{}", uuid::Uuid::new_v4());
    let options = CacheLeaseOptions::new(
        Duration::from_secs(2),
        Duration::from_millis(500),
    )
    .unwrap();

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
