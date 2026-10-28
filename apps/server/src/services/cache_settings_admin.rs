//! Host implementation of the cache admin persistence seam.
//!
//! `rustok_cache` cannot import the host, so [`rustok_cache::CacheSettingsAdminPort`] is the only
//! way the cache admin server functions can read or persist the platform `cache` settings category.
//! The host publishes [`ServerCacheSettingsAdmin`] into the server-fn runtime context
//! (`app_router.rs`) next to the runtime `CacheService`; the admin adapters then report real
//! persisted state instead of hardcoded values.

use async_trait::async_trait;
use rustok_cache::{CacheSettingsAdminPort, SharedCacheSettingsAdmin};
use uuid::Uuid;

use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::settings_service::{SettingsService, ValidatorRegistry, category};

/// Adapter from the platform settings service to the cache admin port.
#[derive(Clone)]
pub struct ServerCacheSettingsAdmin {
    ctx: ServerRuntimeContext,
}

impl ServerCacheSettingsAdmin {
    pub fn new(ctx: ServerRuntimeContext) -> Self {
        Self { ctx }
    }

    /// Handle published to server functions.
    pub fn shared(ctx: &ServerRuntimeContext) -> SharedCacheSettingsAdmin {
        SharedCacheSettingsAdmin::new(std::sync::Arc::new(Self::new(ctx.clone())))
    }
}

#[async_trait]
impl CacheSettingsAdminPort for ServerCacheSettingsAdmin {
    async fn read(&self, tenant_id: Uuid) -> Result<serde_json::Value, String> {
        SettingsService::get(&self.ctx, tenant_id, category::CACHE)
            .await
            .map_err(|error| error.to_string())
    }

    async fn update(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        settings: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let event_bus = crate::services::event_bus::transactional_event_bus_from_context(&self.ctx);
        let validators = ValidatorRegistry::default();
        SettingsService::update(
            &self.ctx,
            &event_bus,
            tenant_id,
            category::CACHE,
            settings,
            actor_id,
            &validators,
        )
        .await
        .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::settings::RustokSettings;
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use serde_json::json;

    #[tokio::test]
    async fn cache_settings_round_trip_through_the_host_port() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let port = ServerCacheSettingsAdmin::shared(&ctx);
        let tenant_id = Uuid::new_v4();

        // The cache target is deployment-owned, so the environment decides what the document
        // contains; the test asserts the round trip, not a specific deployment.
        let current = port
            .port()
            .read(tenant_id)
            .await
            .expect("cache settings read");
        let written = port
            .port()
            .update(tenant_id, None, current.clone())
            .await
            .expect("cache settings persist");
        assert_eq!(written["mode"], current["mode"]);

        let read = port
            .port()
            .read(tenant_id)
            .await
            .expect("cache settings read");
        assert_eq!(read["redis_port"], current["redis_port"]);
    }

    #[tokio::test]
    async fn invalid_cache_settings_are_rejected_by_the_port() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let port = ServerCacheSettingsAdmin::shared(&ctx);

        let error = port
            .port()
            .update(Uuid::new_v4(), None, json!({ "mode": "memcached" }))
            .await
            .expect_err("unknown mode must not be persisted");
        assert!(error.contains("mode"), "unexpected error: {error}");
    }
}
