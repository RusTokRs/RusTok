use rustok_cache::{CacheInvalidationMessage, CacheService};

#[tokio::test]
async fn unavailable_redis_counts_one_publish_failure_without_losing_local_delivery() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind an isolated unavailable Redis endpoint");
    let address = listener.local_addr().expect("read isolated endpoint");
    drop(listener);

    let service = CacheService::from_url(Some(&format!("redis://{address}/")));
    let invalidations = service.invalidations();
    let mut local = invalidations.subscribe_local_channel("cache.failure.metrics");

    let outcome = invalidations
        .publish(CacheInvalidationMessage::new(
            "cache.failure.metrics",
            "product:42",
        ))
        .await;

    let delivered = tokio::time::timeout(std::time::Duration::from_secs(1), local.recv())
        .await
        .expect("local invalidation should not wait for Redis recovery")
        .expect("local invalidation channel should remain open");
    assert_eq!(delivered.key, "product:42");
    assert_eq!(outcome.local_subscribers, 1);
    assert!(!outcome.redis_published);

    let stats = invalidations.stats();
    assert_eq!(stats.local_published_total, 1);
    assert_eq!(stats.redis_publish_success_total, 0);
    assert_eq!(stats.redis_publish_failure_total, 1);
    assert_eq!(stats.rejected_total, 0);
}
