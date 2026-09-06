use rustok_cache::CacheService;

#[cfg(feature = "redis-cache")]
#[tokio::test]
async fn configured_invalid_redis_url_is_degraded_in_legacy_health_report() {
    let service = CacheService::from_url(Some("://invalid-redis-url"));

    let report = service.health().await;

    assert!(report.redis_configured);
    assert!(!report.redis_healthy);
    assert!(!report.is_healthy());
    assert!(
        report
            .redis_error
            .as_deref()
            .unwrap_or_default()
            .contains("could not be initialized")
    );
}
