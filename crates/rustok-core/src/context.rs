use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;

use crate::cache::CacheStats;
use crate::events::EventTransport;
use crate::Result;

#[async_trait]
pub trait CacheBackend: Send + Sync {
    async fn health(&self) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: String, value: Vec<u8>) -> Result<()>;
    async fn set_with_ttl(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<()>;
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
