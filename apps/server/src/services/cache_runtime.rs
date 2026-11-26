use rustok_cache::CacheService;

use crate::services::server_runtime_context::ServerRuntimeContext;

/// Return the cache service owned by this `ServerRuntimeContext`.
///
/// Clones of one context share the same local invalidation bus and Redis
/// client; independent contexts retain separate cache-service ownership.
pub fn ensure_cache_service(ctx: &ServerRuntimeContext) -> CacheService {
    ctx.shared_get_or_insert_with(|| {
        CacheService::from_url(ctx.settings().cache.redis_url.as_deref())
    })
}

#[cfg(test)]
mod tests {
    use super::ensure_cache_service;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use rustok_cache::CacheService;
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_initialization_uses_only_the_registered_cache_bus() {
        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            RustokSettings::default(),
        );
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(16));
        let mut tasks = Vec::new();
        for _ in 0..16 {
            let ctx = ctx.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                ensure_cache_service(&ctx)
            }));
        }
        let mut services = Vec::new();
        for task in tasks {
            services.push(task.await.expect("cache initializer joins"));
        }
        let registered = ctx.shared_get::<CacheService>().expect("registered cache");
        let receiver = registered.invalidations().subscribe_local();
        for service in services {
            assert!(receiver.same_channel(&service.invalidations().subscribe_local()));
        }

        let other_ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            RustokSettings::default(),
        );
        let other = ensure_cache_service(&other_ctx);
        assert!(!receiver.same_channel(&other.invalidations().subscribe_local()));
    }

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
