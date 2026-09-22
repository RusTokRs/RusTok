use std::time::Duration;

use rustok_cache::{CacheInvalidationMessage, CacheService, VersionedCacheInvalidation};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};

use super::cache_runtime::ensure_cache_service;
use super::channel_cache_invalidation::{
    CHANNEL_RESOLUTION_INVALIDATION_CHANNEL, ChannelCacheInvalidationListenerHandle,
    start_channel_cache_invalidation_listener,
};
use super::server_runtime_context::ServerRuntimeContext;
use crate::common::settings::RustokSettings;

const CHANNEL_RESOLUTION_INVALIDATION_KEY: &str = "*";

async fn install_generation_state(db: &DatabaseConnection, generation: u64) {
    db.execute_unprepared(
        "CREATE TABLE channel_resolution_invalidation_state (scope TEXT PRIMARY KEY, generation BIGINT NOT NULL)",
    )
    .await
    .unwrap();
    db.execute_unprepared(&format!(
        "INSERT INTO channel_resolution_invalidation_state (scope, generation) VALUES ('resolution', {generation})"
    ))
    .await
    .unwrap();
}

fn invalidation_message(generation: u64) -> CacheInvalidationMessage {
    VersionedCacheInvalidation::new(
        CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
        CHANNEL_RESOLUTION_INVALIDATION_KEY,
        generation,
        generation,
    )
    .unwrap()
    .to_message()
    .unwrap()
}

fn settings_with_redis(url: &str) -> RustokSettings {
    let mut settings = RustokSettings::default();
    settings.cache.redis_url = Some(url.to_string());
    settings
}

async fn wait_for_readiness(handle: &ChannelCacheInvalidationListenerHandle, expected: bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while handle.is_ready() != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("channel invalidation readiness did not converge");
}

async fn publish_local(cache: &CacheService, generation: u64) {
    let outcome = cache
        .publish_invalidation(invalidation_message(generation))
        .await;
    assert_eq!(outcome.local_subscribers, 1);
}

async fn publish_redis_until_readiness(
    cache: &CacheService,
    remote: &ChannelCacheInvalidationListenerHandle,
    expected: bool,
    generation: u64,
) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let outcome = cache
                .publish_invalidation(invalidation_message(generation))
                .await;
            assert!(outcome.redis_published);
            if remote.is_ready() == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("remote replica did not consume the Redis invalidation before periodic reconciliation");
}

#[tokio::test]
async fn independent_replicas_fail_closed_and_recover_without_redis() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    install_generation_state(&db, 1).await;

    let ctx_a = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let ctx_b = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let cache_a = ensure_cache_service(&ctx_a);
    let cache_b = ensure_cache_service(&ctx_b);

    start_channel_cache_invalidation_listener(&ctx_a, cache_a.clone())
        .await
        .unwrap();
    start_channel_cache_invalidation_listener(&ctx_b, cache_b.clone())
        .await
        .unwrap();

    let handle_a = ctx_a
        .shared_get::<ChannelCacheInvalidationListenerHandle>()
        .expect("first replica listener handle");
    let handle_b = ctx_b
        .shared_get::<ChannelCacheInvalidationListenerHandle>()
        .expect("second replica listener handle");
    assert!(handle_a.is_ready());
    assert!(handle_b.is_ready());
    assert!(!cache_a.redis_configuration_present());
    assert!(!cache_b.redis_configuration_present());

    db.execute_unprepared("DROP TABLE channel_resolution_invalidation_state")
        .await
        .unwrap();
    publish_local(&cache_a, 2).await;
    publish_local(&cache_b, 2).await;
    wait_for_readiness(&handle_a, false).await;
    wait_for_readiness(&handle_b, false).await;

    install_generation_state(&db, 2).await;
    publish_local(&cache_a, 2).await;
    publish_local(&cache_b, 2).await;
    wait_for_readiness(&handle_a, true).await;
    wait_for_readiness(&handle_b, true).await;
}

#[tokio::test]
async fn local_listener_lag_fails_closed_and_recovers_from_durable_state() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    install_generation_state(&db, 1).await;
    let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let cache = ensure_cache_service(&ctx);
    start_channel_cache_invalidation_listener(&ctx, cache.clone())
        .await
        .unwrap();
    let handle = ctx
        .shared_get::<ChannelCacheInvalidationListenerHandle>()
        .expect("listener handle");
    assert!(handle.is_ready());

    let mut probe = cache
        .invalidations()
        .subscribe_local_channel(CHANNEL_RESOLUTION_INVALIDATION_CHANNEL);
    db.execute_unprepared("DROP TABLE channel_resolution_invalidation_state")
        .await
        .unwrap();

    // The local bus holds 256 messages. Publishing without Redis has no await
    // point, so neither subscription can drain until this burst completes.
    for _ in 0..300 {
        let outcome = cache.publish_invalidation(invalidation_message(2)).await;
        assert_eq!(outcome.local_subscribers, 2);
    }
    match probe.recv().await {
        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
            assert!(skipped >= 44);
        }
        other => panic!("expected deterministic local invalidation lag, got {other:?}"),
    }
    drop(probe);

    wait_for_readiness(&handle, false).await;
    install_generation_state(&db, 2).await;
    publish_local(&cache, 2).await;
    wait_for_readiness(&handle, true).await;
}

#[tokio::test]
#[ignore = "requires an isolated Redis instance via RUSTOK_CACHE_REAL_REDIS_URL"]
async fn redis_publication_drives_remote_replica_readiness_recovery() {
    let url = std::env::var("RUSTOK_CACHE_REAL_REDIS_URL")
        .expect("RUSTOK_CACHE_REAL_REDIS_URL must point to an isolated Redis instance");
    let db = Database::connect("sqlite::memory:").await.unwrap();
    install_generation_state(&db, 1).await;

    let ctx_a = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let ctx_b = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let cache_a = ensure_cache_service(&ctx_a);
    let cache_b = ensure_cache_service(&ctx_b);
    assert!(cache_a.redis_client_initialized());
    assert!(cache_b.redis_client_initialized());

    start_channel_cache_invalidation_listener(&ctx_a, cache_a.clone())
        .await
        .unwrap();
    start_channel_cache_invalidation_listener(&ctx_b, cache_b)
        .await
        .unwrap();

    let degraded_remote = ctx_b
        .shared_get::<ChannelCacheInvalidationListenerHandle>()
        .expect("remote degradation listener handle");
    assert!(degraded_remote.is_ready());

    db.execute_unprepared("DROP TABLE channel_resolution_invalidation_state")
        .await
        .unwrap();
    publish_redis_until_readiness(&cache_a, &degraded_remote, false, 2).await;

    let ctx_c = ServerRuntimeContext::new(db.clone(), settings_with_redis(url.as_str()));
    let cache_c = ensure_cache_service(&ctx_c);
    assert!(cache_c.redis_client_initialized());
    start_channel_cache_invalidation_listener(&ctx_c, cache_c)
        .await
        .unwrap();
    let recovering_remote = ctx_c
        .shared_get::<ChannelCacheInvalidationListenerHandle>()
        .expect("remote recovery listener handle");
    assert!(!recovering_remote.is_ready());

    install_generation_state(&db, 2).await;
    publish_redis_until_readiness(&cache_a, &recovering_remote, true, 2).await;
}
