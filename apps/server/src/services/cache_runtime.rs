use rustok_cache::CacheService;

use crate::services::server_runtime_context::ServerRuntimeContext;

/// Return the single process-wide cache service used by listeners, publishers,
/// middleware and rate limiters.
///
/// Startup has multiple phases and some security-critical initializers run
/// before the full application runtime is composed. Reusing one instance keeps
/// the local invalidation bus and Redis client identity stable across phases.
pub fn ensure_cache_service(ctx: &ServerRuntimeContext) -> CacheService {
    if let Some(existing) = ctx.shared_get::<CacheService>() {
        return existing;
    }

    let candidate = CacheService::from_url(ctx.settings().cache.redis_url.as_deref());
    let _ = ctx.shared_insert_if_absent(candidate.clone());
    ctx.shared_get::<CacheService>().unwrap_or(candidate)
}

#[cfg(test)]
mod tests {
    use super::ensure_cache_service;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use rustok_cache::CacheService;
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;

    #[tokio::test]
    async fn repeated_initialization_reuses_shared_invalidation_bus() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let first = ensure_cache_service(&ctx);
        let mut subscriber = first.invalidations().subscribe_local_channel("cache-runtime-test");
        let second = ensure_cache_service(&ctx);

        let outcome = second
            .publish_invalidation(rustok_cache::CacheInvalidationMessage {
                channel: "cache-runtime-test".to_string(),
                payload: "payload".to_string(),
            })
            .await;
        assert_eq!(outcome.local_subscribers, 1);
        assert_eq!(subscriber.recv().await.unwrap().payload, "payload");
        assert!(ctx.shared_get::<CacheService>().is_some());
    }
}
