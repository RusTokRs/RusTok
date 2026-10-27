//! Host seam for the cache admin surface.
//!
//! The cache admin UI shows operator diagnostics (Redis health) and the persisted `cache` settings
//! category. Both live in the host: the runtime `CacheService` is host-owned and the platform
//! settings store belongs to the host settings service. The module cannot import the host, so the
//! host publishes an implementation of [`CacheSettingsAdminPort`] into the server-fn runtime
//! context and the admin server functions call it.
//!
//! This replaces the previous "native" adapters that reported a hardcoded in-memory backend and
//! acknowledged writes without persisting anything.

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

/// Host-owned persistence for the platform `cache` settings category.
#[async_trait]
pub trait CacheSettingsAdminPort: Send + Sync {
    /// Read the effective cache settings for `tenant_id`, with secrets redacted.
    async fn read(&self, tenant_id: Uuid) -> Result<serde_json::Value, String>;

    /// Persist validated cache settings and return the stored, redacted document.
    async fn update(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        settings: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}

/// Newtype so the host can publish the port in a type-keyed runtime context.
#[derive(Clone)]
pub struct SharedCacheSettingsAdmin(Arc<dyn CacheSettingsAdminPort>);

impl SharedCacheSettingsAdmin {
    pub fn new(port: Arc<dyn CacheSettingsAdminPort>) -> Self {
        Self(port)
    }

    pub fn port(&self) -> &Arc<dyn CacheSettingsAdminPort> {
        &self.0
    }
}

impl std::fmt::Debug for SharedCacheSettingsAdmin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SharedCacheSettingsAdmin(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecordingPort {
        stored: std::sync::Mutex<serde_json::Value>,
    }

    #[async_trait]
    impl CacheSettingsAdminPort for RecordingPort {
        async fn read(&self, _tenant_id: Uuid) -> Result<serde_json::Value, String> {
            Ok(self.stored.lock().expect("lock").clone())
        }

        async fn update(
            &self,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            settings: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            *self.stored.lock().expect("lock") = settings.clone();
            Ok(settings)
        }
    }

    #[tokio::test]
    async fn port_round_trips_through_the_shared_handle() {
        let port = SharedCacheSettingsAdmin::new(Arc::new(RecordingPort {
            stored: std::sync::Mutex::new(serde_json::json!({})),
        }));
        let tenant_id = Uuid::new_v4();
        let updated = port
            .port()
            .update(
                tenant_id,
                None,
                serde_json::json!({ "mode": "redis", "redis_url": "redis://cache:6379" }),
            )
            .await
            .expect("update succeeds");
        assert_eq!(updated["mode"], "redis");
        let read = port.port().read(tenant_id).await.expect("read succeeds");
        assert_eq!(read, updated);
    }
}
