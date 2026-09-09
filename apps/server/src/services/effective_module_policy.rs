use crate::modules::ManifestManager;
use crate::services::platform_composition::{PlatformCompositionError, PlatformCompositionService};
use async_trait::async_trait;
use rustok_api::ModuleEffectivePolicyView;
use rustok_core::ModuleRegistry;
use rustok_modules::{
    EffectivePolicyCacheIdentity, ModuleControlPlane, ModuleEffectivePolicy,
    ModuleEffectivePolicyCache, ModuleEffectivePolicyChannelInput,
    ModuleEffectivePolicyMaintenanceInput, ModuleEffectivePolicyReader,
    ModuleEffectivePolicyReaderError, ModuleLifecycleDbWriterError,
    SharedModuleEffectivePolicyReader, SharedStaticModuleLifecycleReader,
    StaticModuleLifecycleReader, StaticModuleLifecycleReaderError, TenantModuleOverrideSnapshot,
};
use sea_orm::{DatabaseConnection, DbErr};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug)]
pub struct EffectiveModulePolicySnapshot {
    pub policy: ModuleEffectivePolicy,
    pub cache_identity: EffectivePolicyCacheIdentity,
    pub default_enabled_modules: Vec<String>,
}

pub struct EffectiveModulePolicyService;

/// Server host adapter for the owner-issued effective-policy view.
///
/// This is shared with Leptos native functions so they receive the same
/// active-composition inputs as GraphQL rather than rebuilding availability
/// from a partial manifest projection.
#[derive(Clone)]
pub struct ServerEffectiveModulePolicyReader {
    db: DatabaseConnection,
    registry: ModuleRegistry,
}

impl ServerEffectiveModulePolicyReader {
    pub fn shared(
        db: DatabaseConnection,
        registry: ModuleRegistry,
    ) -> SharedModuleEffectivePolicyReader {
        SharedModuleEffectivePolicyReader(Arc::new(Self { db, registry }))
    }
}

#[async_trait]
impl ModuleEffectivePolicyReader for ServerEffectiveModulePolicyReader {
    async fn resolve(
        &self,
        tenant_id: uuid::Uuid,
    ) -> Result<ModuleEffectivePolicyView, ModuleEffectivePolicyReaderError> {
        EffectiveModulePolicyService::resolve_view(&self.db, &self.registry, tenant_id)
            .await
            .map_err(|error| {
                tracing::error!(
                    tenant_id = %tenant_id,
                    error = %error,
                    "failed to resolve the owner-issued effective module policy"
                );
                ModuleEffectivePolicyReaderError::Unavailable
            })
    }
}

/// Server adapter for active-composition-aware static lifecycle reads.
///
/// Native Admin functions use this port instead of deserializing the durable
/// composition manifest to obtain lifecycle defaults or revisions.
#[derive(Clone)]
pub struct ServerStaticModuleLifecycleReader {
    db: DatabaseConnection,
    registry: ModuleRegistry,
}

impl ServerStaticModuleLifecycleReader {
    pub fn shared(
        db: DatabaseConnection,
        registry: ModuleRegistry,
    ) -> SharedStaticModuleLifecycleReader {
        SharedStaticModuleLifecycleReader(Arc::new(Self { db, registry }))
    }
}

#[async_trait]
impl StaticModuleLifecycleReader for ServerStaticModuleLifecycleReader {
    async fn static_tenant_module_views(
        &self,
        tenant_id: uuid::Uuid,
        limit: u32,
    ) -> Result<Vec<rustok_api::StaticTenantModuleView>, StaticModuleLifecycleReaderError> {
        EffectiveModulePolicyService::static_tenant_module_views(
            &self.db,
            &self.registry,
            tenant_id,
            limit,
        )
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                error = %error,
                "failed to resolve the static tenant lifecycle projection"
            );
            StaticModuleLifecycleReaderError::Unavailable
        })
    }

    async fn static_lifecycle_revisions(
        &self,
        tenant_id: uuid::Uuid,
        module_slugs: Vec<String>,
    ) -> Result<BTreeMap<String, i64>, StaticModuleLifecycleReaderError> {
        let snapshots = EffectiveModulePolicyService::static_lifecycle_snapshots(
            &self.db,
            &self.registry,
            tenant_id,
            module_slugs,
        )
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                error = %error,
                "failed to resolve static module lifecycle revisions"
            );
            StaticModuleLifecycleReaderError::Unavailable
        })?;

        project_static_lifecycle_revisions(snapshots).map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                "static module lifecycle revision is outside the browser-safe range"
            );
            error
        })
    }
}

fn project_static_lifecycle_revisions(
    snapshots: BTreeMap<String, rustok_modules::StaticTenantLifecycleSnapshot>,
) -> Result<BTreeMap<String, i64>, StaticModuleLifecycleReaderError> {
    snapshots
        .into_values()
        .map(|snapshot| {
            let revision = i64::try_from(snapshot.revision)
                .map_err(|_| StaticModuleLifecycleReaderError::Unavailable)?;
            Ok((snapshot.module_slug, revision))
        })
        .collect()
}

impl EffectiveModulePolicyService {
    pub async fn resolve_snapshot(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
    ) -> Result<EffectiveModulePolicySnapshot, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        let co_requisites = ManifestManager::module_policy_corequisites(&manifest)?;
        let default_enabled_modules = manifest.settings.default_enabled;
        let policy = ModuleControlPlane::new(db.clone())
            .lifecycle(registry, default_enabled_modules.clone())
            .with_corequisites(co_requisites)
            .effective_policy(tenant_id)
            .await
            .map_err(map_effective_policy_error)?;
        let cache_identity = policy
            .cache_identity(tenant_id)
            .map_err(|error| PlatformCompositionError::EffectivePolicy(error.to_string()))?;
        Ok(EffectiveModulePolicySnapshot {
            policy,
            cache_identity,
            default_enabled_modules,
        })
    }

    pub async fn resolve(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
    ) -> Result<ModuleEffectivePolicy, PlatformCompositionError> {
        Self::resolve_snapshot(db, registry, tenant_id)
            .await
            .map(|snapshot| snapshot.policy)
    }

    /// Returns the redacted operator projection of the exact owner decision.
    pub async fn resolve_view(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
    ) -> Result<ModuleEffectivePolicyView, PlatformCompositionError> {
        Self::resolve(db, registry, tenant_id).await.map(Into::into)
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

    /// Resolves the effective policy snapshot through the revision-bound cache.
    /// If a fresh cache entry exists, it is returned immediately; otherwise, the
    /// canonical policy is computed from the database, inserted into the cache, and returned.
    pub async fn resolve_snapshot_cached(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        cache: &ModuleEffectivePolicyCache,
    ) -> Result<EffectiveModulePolicySnapshot, PlatformCompositionError> {
        if let Some((policy, cache_identity)) = cache.get_latest(tenant_id) {
            let manifest = PlatformCompositionService::active_manifest(db).await?;
            return Ok(EffectiveModulePolicySnapshot {
                policy,
                cache_identity,
                default_enabled_modules: manifest.settings.default_enabled,
            });
        }
        let snapshot = Self::resolve_snapshot(db, registry, tenant_id).await?;
        let _ = cache.insert(tenant_id, snapshot.policy.clone());
        Ok(snapshot)
    }

    /// Resolves the effective policy through the revision-bound cache.
    pub async fn resolve_cached(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        cache: &ModuleEffectivePolicyCache,
    ) -> Result<ModuleEffectivePolicy, PlatformCompositionError> {
        Self::resolve_snapshot_cached(db, registry, tenant_id, cache)
            .await
            .map(|snapshot| snapshot.policy)
    }

    /// Explicitly invalidates the cached policy for a tenant.
    pub fn invalidate_tenant(cache: &ModuleEffectivePolicyCache, tenant_id: uuid::Uuid) -> bool {
        cache.invalidate_tenant(tenant_id)
    }

    /// Resolves module availability from a channel-owner snapshot. Channel
    /// resolution remains in `rustok-channel`; the active package co-requisite
    /// contract is supplied to the canonical modules-owner decision.
    pub async fn resolve_for_channel(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        channel: ModuleEffectivePolicyChannelInput,
    ) -> Result<ModuleEffectivePolicy, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        let co_requisites = ManifestManager::module_policy_corequisites(&manifest)?;
        ModuleControlPlane::new(db.clone())
            .lifecycle(registry, manifest.settings.default_enabled)
            .with_corequisites(co_requisites)
            .effective_policy_for_channel(tenant_id, channel)
            .await
            .map_err(map_effective_policy_error)
    }

    /// Forwards channel and maintenance owner inputs plus the active package
    /// co-requisite contract into one canonical modules-owner decision.
    pub async fn resolve_for_context(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        channel: Option<ModuleEffectivePolicyChannelInput>,
        maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
    ) -> Result<ModuleEffectivePolicy, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        let co_requisites = ManifestManager::module_policy_corequisites(&manifest)?;
        ModuleControlPlane::new(db.clone())
            .lifecycle(registry, manifest.settings.default_enabled)
            .with_corequisites(co_requisites)
            .effective_policy_for_context(tenant_id, channel, maintenance)
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
            .lifecycle(registry, manifest.settings.default_enabled)
            .tenant_override_snapshots(tenant_id, limit)
            .await
            .map_err(map_effective_policy_error)
    }

    /// Returns the owner-issued static lifecycle projection instead of making
    /// GraphQL reconstruct a response from override and aggregate tables.
    pub async fn static_tenant_module_views(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        limit: u32,
    ) -> Result<Vec<rustok_api::StaticTenantModuleView>, PlatformCompositionError> {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        let co_requisites = ManifestManager::module_policy_corequisites(&manifest)?;
        ModuleControlPlane::new(db.clone())
            .lifecycle(registry, manifest.settings.default_enabled)
            .with_corequisites(co_requisites)
            .static_tenant_module_views(tenant_id, limit)
            .await
            .map_err(map_effective_policy_error)
    }

    /// Returns owner-issued static lifecycle revisions for the compiled
    /// registry in one query. The read leaves inherited/default state
    /// unmaterialized and reports revision zero for it.
    pub async fn static_lifecycle_snapshots(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        module_slugs: impl IntoIterator<Item = String>,
    ) -> Result<
        BTreeMap<String, rustok_modules::StaticTenantLifecycleSnapshot>,
        PlatformCompositionError,
    > {
        let manifest = PlatformCompositionService::active_manifest(db).await?;
        let co_requisites = ManifestManager::module_policy_corequisites(&manifest)?;
        ModuleControlPlane::new(db.clone())
            .lifecycle(registry, manifest.settings.default_enabled)
            .with_corequisites(co_requisites)
            .static_lifecycle_snapshots(tenant_id, module_slugs)
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_modules::StaticTenantLifecycleSnapshot;

    #[test]
    fn static_lifecycle_revision_projection_uses_owner_module_slug_and_browser_safe_revision() {
        let tenant_id = uuid::Uuid::nil();
        let revisions = project_static_lifecycle_revisions(BTreeMap::from([(
            "owner-map-key-is-not-the-contract".to_string(),
            StaticTenantLifecycleSnapshot {
                tenant_id,
                module_slug: "catalog".to_string(),
                revision: 42,
                active_idempotency_key: None,
            },
        )]))
        .expect("a browser-safe owner revision must project");

        assert_eq!(revisions, BTreeMap::from([("catalog".to_string(), 42)]));
    }

    #[test]
    fn static_lifecycle_revision_projection_fails_closed_for_out_of_range_revision() {
        let result = project_static_lifecycle_revisions(BTreeMap::from([(
            "catalog".to_string(),
            StaticTenantLifecycleSnapshot {
                tenant_id: uuid::Uuid::nil(),
                module_slug: "catalog".to_string(),
                revision: u64::MAX,
                active_idempotency_key: None,
            },
        )]));

        assert_eq!(result, Err(StaticModuleLifecycleReaderError::Unavailable));
    }
}
