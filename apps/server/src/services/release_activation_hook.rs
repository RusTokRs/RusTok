//! Server-owned side effects after a release is activated.

use async_trait::async_trait;
use rustok_modules::ModuleControlPlane;
use sea_orm::DatabaseConnection;

use crate::services::oauth_app::sync_manifest_managed_apps_for_all_tenants;

pub struct ServerReleaseActivationHook {
    db: DatabaseConnection,
}

impl ServerReleaseActivationHook {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl rustok_build::ReleaseActivationHook for ServerReleaseActivationHook {
    async fn after_release_activated(
        &self,
        release: &rustok_build::release::Model,
    ) -> anyhow::Result<()> {
        let manifest = serde_json::from_value(release.manifest_snapshot.clone())?;
        sync_manifest_managed_apps_for_all_tenants(&self.db, &manifest).await?;
        ModuleControlPlane::new(self.db.clone())
            .composition()
            .set_active_release(&release.id)
            .await
            .map_err(anyhow::Error::msg)?;
        Ok(())
    }
}
