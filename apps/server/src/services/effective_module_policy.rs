use crate::services::platform_composition::{PlatformCompositionError, PlatformCompositionService};
use rustok_core::ModuleRegistry;
use rustok_modules::{
    ModuleControlPlane, ModuleEffectivePolicy, ModuleEffectivePolicyChannelInput,
    ModuleEffectivePolicyMaintenanceInput, ModuleEffectivePolicyNodeReadinessInput,
    ModuleLifecycleDbWriterError, TenantModuleOverrideSnapshot,
};
use sea_orm::{DatabaseConnection, DbErr};

pub struct EffectiveModulePolicyService;

impl EffectiveModulePolicyService {
    pub async fn resolve(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
    ) -> Result<ModuleEffectivePolicy, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        ModuleControlPlane::new(db.clone())
            .effective_policy(registry, manifest.settings.default_enabled)
            .resolve(tenant_id)
            .await
            .map_err(map_effective_policy_error)
    }

    pub async fn resolve_enabled(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
    ) -> Result<std::collections::HashSet<String>, PlatformCompositionError> {
        Self::resolve(db, registry, tenant_id)
            .await
            .map(ModuleEffectivePolicy::into_enabled_modules)
    }

    /// Resolves module availability from a channel-owner snapshot. Channel
    /// resolution remains in `rustok-channel`; this adapter only forwards its
    /// validated neutral contract to the module owner.
    pub async fn resolve_for_channel(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        channel: ModuleEffectivePolicyChannelInput,
    ) -> Result<ModuleEffectivePolicy, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        ModuleControlPlane::new(db.clone())
            .effective_policy(registry, manifest.settings.default_enabled)
            .resolve_for_channel(tenant_id, channel)
            .await
            .map_err(map_effective_policy_error)
    }

    /// Forwards all host-owned policy snapshots to the single module owner
    /// decision. Channel and maintenance resolution stay outside this adapter.
    pub async fn resolve_for_context(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        channel: Option<ModuleEffectivePolicyChannelInput>,
        maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
        node_readiness: Option<ModuleEffectivePolicyNodeReadinessInput>,
    ) -> Result<ModuleEffectivePolicy, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        ModuleControlPlane::new(db.clone())
            .effective_policy(registry, manifest.settings.default_enabled)
            .resolve_for_context(tenant_id, channel, maintenance, node_readiness)
            .await
            .map_err(map_effective_policy_error)
    }

    pub async fn resolve_for_node_readiness(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        node_readiness: ModuleEffectivePolicyNodeReadinessInput,
    ) -> Result<ModuleEffectivePolicy, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        ModuleControlPlane::new(db.clone())
            .effective_policy(registry, manifest.settings.default_enabled)
            .resolve_for_node_readiness(tenant_id, node_readiness)
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

    pub async fn tenant_override_snapshots(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        limit: u32,
    ) -> Result<Vec<TenantModuleOverrideSnapshot>, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        ModuleControlPlane::new(db.clone())
            .effective_policy(registry, manifest.settings.default_enabled)
            .tenant_override_snapshots(tenant_id, limit)
            .await
            .map_err(map_effective_policy_error)
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
