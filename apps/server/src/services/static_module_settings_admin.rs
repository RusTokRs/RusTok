use std::fmt::Display;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    AuthContext, Permission, PortError, SharedStaticModuleSettingsAdminReader,
    StaticModuleSettingsAdminReader, StaticTenantModuleView, TenantContext,
};
use rustok_core::ModuleRegistry;
use rustok_modules::{ModuleControlPlane, ModuleLifecycleDbWriterError};
use sea_orm::DatabaseConnection;

use crate::modules::ManifestManager;
use crate::services::platform_composition::PlatformCompositionService;
use crate::services::rbac_service::RbacService;

pub(crate) struct ServerStaticModuleSettingsAdminReader {
    db: DatabaseConnection,
    registry: ModuleRegistry,
}

impl ServerStaticModuleSettingsAdminReader {
    pub(crate) fn new(db: DatabaseConnection, registry: ModuleRegistry) -> Self {
        Self { db, registry }
    }

    pub(crate) fn shared(
        db: DatabaseConnection,
        registry: ModuleRegistry,
    ) -> SharedStaticModuleSettingsAdminReader {
        SharedStaticModuleSettingsAdminReader(Arc::new(Self::new(db, registry)))
    }
}

#[async_trait]
impl StaticModuleSettingsAdminReader for ServerStaticModuleSettingsAdminReader {
    async fn read(
        &self,
        tenant: &TenantContext,
        auth: &AuthContext,
        module_slug: &str,
    ) -> Result<StaticTenantModuleView, PortError> {
        if !tenant.is_active
            || tenant.id.is_nil()
            || auth.user_id.is_nil()
            || auth.tenant_id != tenant.id
            || !RbacService::has_permission(
                &self.db,
                &tenant.id,
                &auth.user_id,
                &Permission::MODULES_MANAGE,
            )
            .await
            .map_err(unavailable)?
        {
            return Err(PortError::forbidden(
                "PERMISSION_DENIED",
                "Module settings require modules:manage",
            ));
        }
        if !rustok_api::is_valid_module_slug(module_slug) {
            return Err(PortError::validation(
                "BAD_USER_INPUT",
                "Invalid module slug",
            ));
        }

        let manifest = PlatformCompositionService::active_manifest(&self.db)
            .await
            .map_err(unavailable)?;
        let co_requisites =
            ManifestManager::module_policy_corequisites(&manifest).map_err(unavailable)?;
        ModuleControlPlane::new(self.db.clone())
            .lifecycle(&self.registry, manifest.settings.default_enabled)
            .with_corequisites(co_requisites)
            .static_tenant_module_view(tenant.id, module_slug)
            .await
            .map_err(|error| match error {
                ModuleLifecycleDbWriterError::UnknownModule(_)
                | ModuleLifecycleDbWriterError::ArtifactSettings { .. } => {
                    PortError::not_found("NOT_FOUND", "Static module not found")
                }
                other => unavailable(other),
            })
    }
}

fn unavailable(error: impl Display) -> PortError {
    tracing::error!(error = %error, "Static module operator settings read failed");
    PortError::unavailable("INTERNAL_ERROR", "Module settings are unavailable")
}
