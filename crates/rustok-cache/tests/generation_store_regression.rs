use rustok_cache::CacheService;

#[tokio::test]
async fn cache_service_generation_handles_share_trusted_local_snapshots() {
    // An invalid configured URL guarantees that no Redis client can mask the local snapshot path.
    let service = CacheService::from_url(Some("://invalid-redis-url"));
    let first = service.namespace_generations();
    let second = service.namespace_generations();

    first.seed_local("tenant-generation-regression", 7).unwrap();

    let observed = second.read("tenant-generation-regression").await.unwrap();
    assert_eq!(observed.value(), 7);
    assert_eq!(second.local_snapshot_count(), 1);
}

#[tokio::test]
async fn distinct_cache_services_do_not_share_generation_snapshots() {
    let first_service = CacheService::from_url(Some("://invalid-first-redis-url"));
    let second_service = CacheService::from_url(Some("://invalid-second-redis-url"));

    first_service
        .namespace_generations()
        .seed_local("tenant-generation-isolation", 11)
        .unwrap();

    let second = second_service
        .namespace_generations()
        .read("tenant-generation-isolation")
        .await
        .unwrap();
    assert_eq!(second.value(), 0);
    assert_eq!(second.local_snapshot_count(), 0);
}
