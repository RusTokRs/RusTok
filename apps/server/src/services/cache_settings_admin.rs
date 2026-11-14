//! Host implementation of the cache admin persistence seam.
//!
//! `rustok_cache` cannot import the host, so [`rustok_cache::CacheSettingsAdminPort`] is the only
//! way the cache admin server functions can read or persist the platform `cache` settings category.
//! The host publishes [`ServerCacheSettingsAdmin`] into the server-fn runtime context
//! (`app_router.rs`) next to the runtime `CacheService`; the admin adapters then report real
//! persisted state instead of hardcoded values.

use async_trait::async_trait;
use rustok_cache::{CacheSettingsAdminPort, CacheSettingsSnapshot, SharedCacheSettingsAdmin};
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
    async fn read(&self, tenant_id: Uuid) -> Result<CacheSettingsSnapshot, String> {
        let snapshot = SettingsService::get_snapshot(&self.ctx, tenant_id, category::CACHE)
            .await
            .map_err(|error| error.to_string())?;
        Ok(CacheSettingsSnapshot {
            settings: snapshot.settings,
            schema_version: snapshot.schema_version,
        })
    }

    async fn update(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        settings: serde_json::Value,
        expected_schema_version: i32,
    ) -> Result<CacheSettingsSnapshot, String> {
        let event_bus = crate::services::event_bus::transactional_event_bus_from_context(&self.ctx);
        let validators = ValidatorRegistry::default();
        let snapshot = SettingsService::update_if_current(
            &self.ctx,
            &event_bus,
            tenant_id,
            category::CACHE,
            settings,
            actor_id,
            &validators,
            expected_schema_version,
        )
        .await
        .map_err(|error| error.to_string())?;
        Ok(CacheSettingsSnapshot {
            settings: snapshot.settings,
            schema_version: snapshot.schema_version,
        })
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
        use sea_orm::ActiveModelTrait;

        let db = setup_test_db_with_migrations::<Migrator>().await;
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let port = ServerCacheSettingsAdmin::shared(&ctx);
        let tenant = crate::models::tenants::ActiveModel::new("Cache Tenant", "cache-tenant")
            .insert(ctx.db())
            .await
            .expect("insert test tenant");
        let tenant_id = tenant.id;

        // The cache target is deployment-owned, so the environment decides what the document
        // contains; the test asserts the round trip, not a specific deployment.
        let current = port
            .port()
            .read(tenant_id)
            .await
            .expect("cache settings read");
        let actor_id = Some(Uuid::new_v4());
        let written = port
            .port()
            .update(
                tenant_id,
                actor_id,
                current.settings.clone(),
                current.schema_version,
            )
            .await
            .expect("cache settings persist");
        assert_eq!(written.settings["mode"], current.settings["mode"]);
        assert_eq!(written.schema_version, current.schema_version + 1);

        let read = port
            .port()
            .read(tenant_id)
            .await
            .expect("cache settings read");
        assert_eq!(read.settings["redis_port"], current.settings["redis_port"]);
        assert_eq!(read.schema_version, written.schema_version);
    }

    #[tokio::test]
    async fn invalid_cache_settings_are_rejected_by_the_port() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let port = ServerCacheSettingsAdmin::shared(&ctx);

        let error = port
            .port()
            .update(
                Uuid::new_v4(),
                Some(Uuid::new_v4()),
                json!({ "mode": "memcached" }),
                0,
            )
            .await
            .expect_err("unknown mode must not be persisted");
        assert!(error.contains("mode"), "unexpected error: {error}");
    }

    #[tokio::test]
    async fn anonymous_cache_settings_write_is_rejected_before_persisting() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let port = ServerCacheSettingsAdmin::shared(&ctx);
        let tenant_id = Uuid::new_v4();

        let before = port
            .port()
            .read(tenant_id)
            .await
            .expect("cache settings read");
        let error = port
            .port()
            .update(
                tenant_id,
                None,
                json!({ "mode": "redis", "redis_url": "redis://localhost:6379" }),
                before.schema_version,
            )
            .await
            .expect_err("a settings write without an actor must be rejected");
        assert!(
            error.contains("requires an authenticated actor"),
            "unexpected error: {error}"
        );

        // Nothing was written: the rejection happens before the transaction.
        let after = port
            .port()
            .read(tenant_id)
            .await
            .expect("cache settings read");
        assert_eq!(after.schema_version, before.schema_version);
    }
}
