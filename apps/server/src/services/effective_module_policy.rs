use crate::services::platform_composition::{PlatformCompositionError, PlatformCompositionService};
use rustok_core::ModuleRegistry;
use rustok_modules::{ModuleControlPlane, ModuleLifecycleDbWriterError};
use sea_orm::{DatabaseConnection, DbErr};

pub struct EffectiveModulePolicyService;

impl EffectiveModulePolicyService {
    pub async fn resolve_enabled(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
    ) -> Result<std::collections::HashSet<String>, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        ModuleControlPlane::new(db.clone())
            .effective_policy(registry, manifest.settings.default_enabled)
            .resolve_enabled(tenant_id)
            .await
            .map_err(map_effective_policy_error)
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
        Ok(Self::resolve_enabled(db, registry, tenant_id)
            .await?
            .contains(module_slug))
    }
}

fn map_effective_policy_error(error: ModuleLifecycleDbWriterError) -> PlatformCompositionError {
    match error {
        ModuleLifecycleDbWriterError::Definition(error) => {
            PlatformCompositionError::Definition(error)
        }
        ModuleLifecycleDbWriterError::Database(error) => {
            PlatformCompositionError::Database(DbErr::Custom(error))
        }
        error => PlatformCompositionError::EffectivePolicy(error.to_string()),
    }
}
