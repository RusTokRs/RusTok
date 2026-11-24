use rustok_cache::CacheService;

use crate::services::server_runtime_context::ServerRuntimeContext;

/// Return the cache service owned by this `ServerRuntimeContext`.
///
/// Clones of one context share the same local invalidation bus and Redis
/// client; independent contexts retain separate cache-service ownership.
pub fn ensure_cache_service(ctx: &ServerRuntimeContext) -> CacheService {
    if let Some(existing) = ctx.shared_get::<CacheService>() {
        return existing;
    }

    let candidate = CacheService::from_url(ctx.settings().cache.redis_url.as_deref());
    if ctx.shared_insert_if_absent(candidate.clone()) {
        candidate
    } else {
        ctx.shared_get::<CacheService>().unwrap_or(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_cache_service;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use rustok_cache::CacheService;
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;

    #[tokio::test]
    async fn repeated_initialization_reuses_shared_invalidation_bus() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let first = ensure_cache_service(&ctx);
        let mut subscriber = first
            .invalidations()
            .subscribe_local_channel("cache-runtime-test");
        let second = ensure_cache_service(&ctx);

        let outcome = second
            .publish_invalidation(rustok_cache::CacheInvalidationMessage::new(
                "cache-runtime-test",
                "cache-runtime-test-key",
            ))
            .await
            .expect("publish invalidation failed");
        assert_eq!(outcome.local_subscribers, 1);
        assert_eq!(
            subscriber.recv().await.unwrap().key,
            "cache-runtime-test-key"
        );
        assert!(ctx.shared_get::<CacheService>().is_some());
    }
}
