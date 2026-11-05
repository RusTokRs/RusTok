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

/// Redacted cache settings document plus the CAS revision the client read.
#[derive(Debug, Clone)]
pub struct CacheSettingsSnapshot {
    pub settings: serde_json::Value,
    pub schema_version: i32,
}

/// Host-owned persistence for the platform `cache` settings category.
#[async_trait]
pub trait CacheSettingsAdminPort: Send + Sync {
    /// Read the effective cache settings for `tenant_id`, with secrets redacted,
    /// together with the revision required for a compare-and-set write.
    async fn read(&self, tenant_id: Uuid) -> Result<CacheSettingsSnapshot, String>;

    /// Persist validated cache settings only while the category still has the
    /// expected revision, and return the stored, redacted snapshot.
    async fn update(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        settings: serde_json::Value,
        expected_schema_version: i32,
    ) -> Result<CacheSettingsSnapshot, String>;
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
        stored: std::sync::Mutex<CacheSettingsSnapshot>,
    }

    #[async_trait]
    impl CacheSettingsAdminPort for RecordingPort {
        async fn read(&self, _tenant_id: Uuid) -> Result<CacheSettingsSnapshot, String> {
            Ok(self.stored.lock().expect("lock").clone())
        }

        async fn update(
            &self,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            settings: serde_json::Value,
            expected_schema_version: i32,
        ) -> Result<CacheSettingsSnapshot, String> {
            let mut stored = self.stored.lock().expect("lock");
            if stored.schema_version != expected_schema_version {
                return Err("settings changed concurrently".to_string());
            }
            stored.settings = settings.clone();
            stored.schema_version += 1;
            Ok(stored.clone())
        }
    }

    #[tokio::test]
    async fn port_round_trips_through_the_shared_handle() {
        let port = SharedCacheSettingsAdmin::new(Arc::new(RecordingPort {
            stored: std::sync::Mutex::new(CacheSettingsSnapshot {
                settings: serde_json::json!({}),
                schema_version: 0,
            }),
        }));
        let tenant_id = Uuid::new_v4();
        let updated = port
            .port()
            .update(
                tenant_id,
                None,
                serde_json::json!({ "mode": "redis", "redis_url": "redis://cache:6379" }),
                0,
            )
            .await
            .expect("update succeeds");
        assert_eq!(updated.settings["mode"], "redis");
        assert_eq!(updated.schema_version, 1);
        let read = port.port().read(tenant_id).await.expect("read succeeds");
        assert_eq!(read.settings, updated.settings);
        assert_eq!(read.schema_version, updated.schema_version);
        let stale = port
            .port()
            .update(
                tenant_id,
                None,
                serde_json::json!({ "mode": "in-memory" }),
                0,
            )
            .await
            .expect_err("stale revision must conflict");
        assert!(stale.contains("concurrently"));
    }
}
