use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;

use crate::cache::{CacheCompareAndSetOutcome, CacheStats};
use crate::events::EventTransport;
use crate::{Error, Result};

#[async_trait]
pub trait CacheBackend: Send + Sync {
    async fn health(&self) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: String, value: Vec<u8>) -> Result<()>;
    async fn set_with_ttl(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<()>;

    /// Atomically replace a cache entry only when its current bytes equal `expected`.
    ///
    /// `ttl = None` preserves the backend's default write policy. A zero TTL performs a
    /// conditional invalidation. Backends that cannot provide a real atomic primitive fail
    /// closed rather than emulating compare-and-set with a racy GET followed by SET.
    async fn compare_and_set(
        &self,
        _key: &str,
        _expected: &[u8],
        _value: Vec<u8>,
        _ttl: Option<Duration>,
    ) -> Result<CacheCompareAndSetOutcome> {
        Err(Error::Cache(
            "atomic cache compare-and-set is not supported by this backend".to_string(),
        ))
    }

    async fn invalidate(&self, key: &str) -> Result<()>;
    fn stats(&self) -> CacheStats;
}

#[async_trait]
pub trait SearchBackend: Send + Sync {
    async fn health(&self) -> Result<()>;
}

pub struct AppContext {
    pub db: Arc<DatabaseConnection>,
    pub events: Arc<dyn EventTransport>,
    pub cache: Arc<dyn CacheBackend>,
    pub search: Arc<dyn SearchBackend>,
}

impl AppContext {
    pub async fn new(
        db: DatabaseConnection,
        events: Arc<dyn EventTransport>,
        cache: Arc<dyn CacheBackend>,
        search: Arc<dyn SearchBackend>,
    ) -> Result<Self> {
        let db = Arc::new(db);

        Ok(Self {
            db,
            events,
            cache,
            search,
        })
    }
}
