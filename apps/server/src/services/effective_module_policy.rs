use rustok_core::ModuleRegistry;
use rustok_modules::{resolve_effective_modules, TenantModuleOverride};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::models::_entities::tenant_modules::{
    Column as TenantModulesColumn, Entity as TenantModulesEntity,
};
use crate::services::platform_composition::{PlatformCompositionError, PlatformCompositionService};

pub struct EffectiveModulePolicyService;

impl EffectiveModulePolicyService {
    pub async fn resolve_enabled(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
    ) -> Result<std::collections::HashSet<String>, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        let overrides = TenantModulesEntity::find()
            .filter(TenantModulesColumn::TenantId.eq(tenant_id))
            .all(db)
            .await?;

        Ok(resolve_effective_modules(
            registry,
            manifest.settings.default_enabled,
            overrides.into_iter().map(|module| TenantModuleOverride {
                module_slug: module.module_slug,
                enabled: module.enabled,
            }),
        ))
    }

    pub async fn list_enabled(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
    ) -> Result<Vec<String>, PlatformCompositionError> {
        let mut modules = Self::resolve_enabled(db, registry, tenant_id)
            .await?
            .into_iter()
            .collect::<Vec<_>>();
        modules.sort();
        Ok(modules)
    }

    pub async fn is_enabled(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        module_slug: &str,
    ) -> Result<bool, PlatformCompositionError> {
        if registry.is_core(module_slug) {
            return Ok(true);
        }
        Ok(Self::resolve_enabled(db, registry, tenant_id)
            .await?
            .contains(module_slug))
    }
}
