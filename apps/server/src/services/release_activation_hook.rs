//! Server-owned side effects after a release is activated.

use async_trait::async_trait;
use sea_orm::{sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::models::platform_state::{Column as PlatformStateColumn, Entity as PlatformStateEntity};
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
        let _ = PlatformStateEntity::update_many()
            .filter(PlatformStateColumn::Id.eq("active"))
            .col_expr(
                PlatformStateColumn::ActiveReleaseId,
                Expr::value(Some(release.id.clone())),
            )
            .col_expr(
                PlatformStateColumn::UpdatedAt,
                Expr::value(chrono::Utc::now()),
            )
            .exec(&self.db)
            .await;
        Ok(())
    }
}
