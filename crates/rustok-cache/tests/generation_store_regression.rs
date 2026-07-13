use rustok_cache::CacheService;

#[tokio::test]
async fn cache_service_generation_handles_share_trusted_local_snapshots() {
    // An invalid configured URL guarantees that no Redis client can mask the local snapshot path.
    let service = CacheService::from_url(Some("://invalid-redis-url"));
    let first = service.namespace_generations();
    let second = service.namespace_generations();

    first.seed_local("tenant-generation-regression", 7).unwrap();

    let observed = second
        .read("tenant-generation-regression")
        .await
        .unwrap();
    assert_eq!(observed.value(), 7);
    assert_eq!(second.local_snapshot_count(), 1);
}
