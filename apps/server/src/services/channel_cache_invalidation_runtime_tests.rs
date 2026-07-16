use std::time::Duration;

use rustok_cache::{CacheInvalidationMessage, CacheService, VersionedCacheInvalidation};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};

use super::cache_runtime::ensure_cache_service;
use super::channel_cache_invalidation::{
    start_channel_cache_invalidation_listener, ChannelCacheInvalidationListenerHandle,
    CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
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
