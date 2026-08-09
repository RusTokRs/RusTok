use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_blog::PublicCommentsSnapshotStore;
use rustok_cache::CacheService;
use rustok_core::{CacheBackend, ModuleRuntimeExtensions};
use tokio::sync::OnceCell;

use crate::services::{
    cache_runtime::ensure_cache_service, server_runtime_context::ServerRuntimeContext,
};

const CACHE_PREFIX: &str = "blog-public-comments-snapshot-v1";
const SNAPSHOT_TTL: Duration = Duration::from_secs(15 * 60);
const MAX_SNAPSHOT_KEYS: u64 = 10_000;

#[derive(Clone)]
struct ServerBlogPublicCommentsSnapshotStore {
    cache: CacheService,
    backend: Arc<OnceCell<Arc<dyn CacheBackend>>>,
}

impl ServerBlogPublicCommentsSnapshotStore {
    fn new(cache: CacheService) -> Self {
        Self {
            cache,
            backend: Arc::new(OnceCell::new()),
        }
    }

    async fn backend(&self) -> Arc<dyn CacheBackend> {
        self.backend
            .get_or_init(|| async {
                self.cache.backend(CACHE_PREFIX, SNAPSHOT_TTL, MAX_SNAPSHOT_KEYS).await
            })
            .await
            .clone()
    }
}

#[async_trait]
impl PublicCommentsSnapshotStore for ServerBlogPublicCommentsSnapshotStore {
    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        self.backend()
            .await
            .get(key)
            .await
            .map_err(|error| error.to_string())
    }

    async fn store(&self, key: String, value: Vec<u8>) -> Result<(), String> {
        self.backend()
            .await
            .set_with_ttl(key, value, SNAPSHOT_TTL)
            .await
            .map_err(|error| error.to_string())
    }
}

pub(super) fn register(
    extensions: &mut ModuleRuntimeExtensions,
    runtime_ctx: &ServerRuntimeContext,
) {
    let cache = ensure_cache_service(runtime_ctx);
    let store: Arc<dyn PublicCommentsSnapshotStore> =
        Arc::new(ServerBlogPublicCommentsSnapshotStore::new(cache));
    extensions.insert(store);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_contract_is_bounded_and_lazy() {
        assert_eq!(CACHE_PREFIX, "blog-public-comments-snapshot-v1");
        assert_eq!(SNAPSHOT_TTL, Duration::from_secs(900));
        assert_eq!(MAX_SNAPSHOT_KEYS, 10_000);
    }
}
