#[test]
fn rate_limit_cleanup_worker_does_not_own_the_cache_forever() {
    let wrapper = include_str!("../src/middleware/rate_limit.rs");
    let base = include_str!("../src/middleware/rate_limit_base.rs");
    let bootstrap = include_str!("../src/services/app_runtime.rs");

    assert!(wrapper.contains("#[path = \"rate_limit_base.rs\"]"));
    assert!(wrapper.contains("fn cleanup_task_has_external_owners"));
    assert!(wrapper.contains("Arc::strong_count(limiter) > 1"));
    assert!(wrapper.contains("if !cleanup_task_has_external_owners(&limiter)"));
    assert!(wrapper.contains("return;"));
    assert!(wrapper.contains("limiter.cleanup_expired().await"));

    assert!(base.contains("pub struct RateLimiter"));
    assert!(base.contains(".max_capacity(MEMORY_BACKEND_MAX_ENTRIES)"));
    assert!(base.contains(".time_to_idle(config.window)"));
    assert!(base.contains("pub async fn cleanup_expired(&self)"));

    assert!(bootstrap.contains("cleanup_task(limiter_for_cleanup).await"));
    assert!(!bootstrap.contains("rate_limit_base::cleanup_task"));
}
