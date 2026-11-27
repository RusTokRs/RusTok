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
use std::{collections::BTreeMap, future::Future, sync::Arc};

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
    cache: Option<ModuleEffectivePolicyCache>,
}

impl ServerEffectiveModulePolicyReader {
    pub fn shared(
        db: DatabaseConnection,
        registry: ModuleRegistry,
    ) -> SharedModuleEffectivePolicyReader {
        SharedModuleEffectivePolicyReader(Arc::new(Self {
            db,
            registry,
            cache: None,
        }))
    }

    pub fn shared_cached(
        db: DatabaseConnection,
        registry: ModuleRegistry,
        cache: ModuleEffectivePolicyCache,
    ) -> SharedModuleEffectivePolicyReader {
        SharedModuleEffectivePolicyReader(Arc::new(Self {
            db,
            registry,
            cache: Some(cache),
        }))
    }
}

#[async_trait]
impl ModuleEffectivePolicyReader for ServerEffectiveModulePolicyReader {
    async fn resolve(
        &self,
        tenant_id: uuid::Uuid,
    ) -> Result<ModuleEffectivePolicyView, ModuleEffectivePolicyReaderError> {
        let result = if let Some(cache) = &self.cache {
            EffectiveModulePolicyService::resolve_cached(&self.db, &self.registry, tenant_id, cache)
                .await
                .map(Into::into)
        } else {
            EffectiveModulePolicyService::resolve_view(&self.db, &self.registry, tenant_id).await
        };
        result.map_err(|error| {
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

        project_static_lifecycle_revisions(snapshots).inspect_err(|_error| {
            tracing::error!(
                tenant_id = %tenant_id,
                "static module lifecycle revision is outside the browser-safe range"
            );
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
        let composition = PlatformCompositionService::active_snapshot(db).await?;
        Self::resolve_snapshot_from_composition(db, registry, tenant_id, composition).await
    }

    /// Retain the composition revision that actually supplied the policy
    /// inputs. Revalidation must not recreate a missing active composition.
    pub(super) async fn resolve_authority_snapshot(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        initialize: bool,
    ) -> Result<(i64, EffectiveModulePolicySnapshot), PlatformCompositionError> {
        let composition = if initialize {
            PlatformCompositionService::active_snapshot(db).await?
        } else {
            let owner = ModuleControlPlane::new(db.clone()).composition();
            PlatformCompositionService::snapshot_from_owner(owner.active_snapshot().await?)?
        };
        let revision = composition.revision;
        let snapshot =
            Self::resolve_snapshot_from_composition(db, registry, tenant_id, composition).await?;
        Ok((revision, snapshot))
    }

    async fn resolve_snapshot_from_composition(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        composition: super::platform_composition::PlatformCompositionSnapshot,
    ) -> Result<EffectiveModulePolicySnapshot, PlatformCompositionError> {
        let manifest = composition.manifest;
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

    /// Uncached artifact-serving policy. Keep the static authorization
    /// projection independent from sandbox availability; both use the active
    /// manifest and the same Modules owner policy evaluator.
    pub(crate) async fn resolve_artifact_serving(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        supported_payload_kinds: &[rustok_modules::ArtifactPayloadKind],
        channel: Option<ModuleEffectivePolicyChannelInput>,
        maintenance: ModuleEffectivePolicyMaintenanceInput,
    ) -> Result<
        (
            ModuleEffectivePolicy,
            rustok_modules::ModuleCompositionIdentity,
        ),
        PlatformCompositionError,
    > {
        let owner_snapshot = PlatformCompositionService::active_owner_snapshot(db).await?;
        let composition = owner_snapshot.identity();
        let manifest = PlatformCompositionService::snapshot_from_owner(owner_snapshot)?.manifest;
        let co_requisites = ManifestManager::module_policy_corequisites(&manifest)?;
        let policy = ModuleControlPlane::new(db.clone())
            .lifecycle(registry, manifest.settings.default_enabled)
            .with_corequisites(co_requisites)
            .artifact_serving_policy_for_context(
                tenant_id,
                supported_payload_kinds,
                channel,
                Some(maintenance),
            )
            .await
            .map_err(map_effective_policy_error)?;
        Ok((policy, composition))
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

    /// Resolves only the policy through the locally invalidated TTL cache.
    /// Callers needing defaults must use `resolve_snapshot`, not combine a
    /// cached policy with a separately read manifest. A cached hit has no
    /// independently verified current owner revision; this is not a
    /// transactional or distributed freshness check.
    pub async fn resolve_cached(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        cache: &ModuleEffectivePolicyCache,
    ) -> Result<ModuleEffectivePolicy, PlatformCompositionError> {
        resolve_policy_with_cache(tenant_id, cache, || Self::resolve(db, registry, tenant_id)).await
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

    pub async fn is_enabled_cached(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        cache: &ModuleEffectivePolicyCache,
    ) -> Result<bool, PlatformCompositionError> {
        Ok(Self::resolve_cached(db, registry, tenant_id, cache)
            .await?
            .contains(module_slug))
    }
}

// Keeping the loader injectable makes the invalidation/retry behavior testable
// without a database; production supplies the canonical owner resolve above.
async fn resolve_policy_with_cache<F, Fut>(
    tenant_id: uuid::Uuid,
    cache: &ModuleEffectivePolicyCache,
    mut load: F,
) -> Result<ModuleEffectivePolicy, PlatformCompositionError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<ModuleEffectivePolicy, PlatformCompositionError>>,
{
    // Avoid an unbounded loop if invalidations or competing fills are busy.
    for _ in 0..3 {
        let stamp = cache
            .begin_read()
            .map_err(|error| PlatformCompositionError::EffectivePolicy(error.to_string()))?;
        if let Some((policy, _identity)) = cache.get_latest(tenant_id) {
            let current = cache
                .is_current_hit(&stamp, tenant_id, &policy)
                .map_err(|error| PlatformCompositionError::EffectivePolicy(error.to_string()))?;
            if current {
                return Ok(policy);
            }
            continue;
        }
        let policy = load().await?;
        let inserted = cache
            .insert_if_current(&stamp, tenant_id, policy.clone())
            .map_err(|error| PlatformCompositionError::EffectivePolicy(error.to_string()))?;
        if inserted {
            return Ok(policy);
        }
    }
    Err(PlatformCompositionError::EffectivePolicy(
        "effective policy changed during resolution; retry".to_string(),
    ))
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
    use std::cell::Cell;

    fn sample_policy(digest_char: char) -> ModuleEffectivePolicy {
        serde_json::from_value(serde_json::json!({
            "policy_revision": format!("sha256:{}", digest_char.to_string().repeat(64)),
            "enabled_modules": [],
            "decisions": {}
        }))
        .expect("valid policy fixture")
    }

    #[tokio::test]
    async fn policy_cache_hit_does_not_load_owner_again() {
        let cache = ModuleEffectivePolicyCache::new();
        let tenant = uuid::Uuid::new_v4();
        let expected = sample_policy('a');
        let stamp = cache.begin_read().unwrap();
        assert!(
            cache
                .insert_if_current(&stamp, tenant, expected.clone())
                .unwrap()
        );
        let calls = Cell::new(0);
        let actual = resolve_policy_with_cache(tenant, &cache, || {
            calls.set(calls.get() + 1);
            async { Ok(sample_policy('b')) }
        })
        .await
        .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(calls.get(), 0);
    }

    #[tokio::test]
    async fn invalidated_in_flight_policy_is_discarded_and_reloaded() {
        let cache = ModuleEffectivePolicyCache::new();
        let tenant = uuid::Uuid::new_v4();
        let calls = Cell::new(0);
        let cache_ref = &cache;
        let actual = resolve_policy_with_cache(tenant, &cache, || {
            let attempt = calls.get();
            calls.set(attempt + 1);
            let cache = cache_ref;
            async move {
                if attempt == 0 {
                    cache.invalidate_tenant(tenant);
                    Ok(sample_policy('a'))
                } else {
                    Ok(sample_policy('b'))
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(calls.get(), 2);
        assert_eq!(actual, sample_policy('b'));
        assert_eq!(cache.get_latest(tenant).unwrap().0, actual);
    }

    #[tokio::test]
    async fn invalidation_while_owner_read_is_suspended_fences_completion() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::sync::Barrier;

        let cache = ModuleEffectivePolicyCache::new();
        let tenant = uuid::Uuid::new_v4();
        let started = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        let calls = Arc::new(AtomicUsize::new(0));
        let task_cache = cache.clone();
        let task_started = Arc::clone(&started);
        let task_resume = Arc::clone(&resume);
        let task_calls = Arc::clone(&calls);
        let task = tokio::spawn(async move {
            resolve_policy_with_cache(tenant, &task_cache, || {
                let attempt = task_calls.fetch_add(1, Ordering::SeqCst);
                let started = Arc::clone(&task_started);
                let resume = Arc::clone(&task_resume);
                async move {
                    if attempt == 0 {
                        started.wait().await;
                        resume.wait().await;
                        Ok(sample_policy('a'))
                    } else {
                        Ok(sample_policy('b'))
                    }
                }
            })
            .await
        });
        started.wait().await;
        assert!(!cache.invalidate_tenant(tenant));
        resume.wait().await;
        let actual = task.await.unwrap().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(actual, sample_policy('b'));
        assert_eq!(cache.get_latest(tenant).unwrap().0, actual);
    }

    #[tokio::test]
    async fn competing_fill_is_read_instead_of_overwritten() {
        let cache = ModuleEffectivePolicyCache::new();
        let tenant = uuid::Uuid::new_v4();
        let calls = Cell::new(0);
        let cache_ref = &cache;
        let actual = resolve_policy_with_cache(tenant, &cache, || {
            calls.set(calls.get() + 1);
            let cache = cache_ref;
            async move {
                let competing_stamp = cache.begin_read().unwrap();
                assert!(
                    cache
                        .insert_if_current(&competing_stamp, tenant, sample_policy('b'))
                        .unwrap()
                );
                Ok(sample_policy('a'))
            }
        })
        .await
        .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(actual, sample_policy('b'));
        assert_eq!(cache.get_latest(tenant).unwrap().0, actual);
    }

    #[tokio::test]
    async fn repeated_invalidation_returns_error_without_caching_stale_results() {
        let cache = ModuleEffectivePolicyCache::new();
        let tenant = uuid::Uuid::new_v4();
        let calls = Cell::new(0);
        let cache_ref = &cache;
        let result = resolve_policy_with_cache(tenant, &cache, || {
            calls.set(calls.get() + 1);
            let cache = cache_ref;
            async move {
                cache.invalidate_tenant(tenant);
                Ok(sample_policy('a'))
            }
        })
        .await;
        assert!(matches!(
            result,
            Err(PlatformCompositionError::EffectivePolicy(_))
        ));
        assert_eq!(calls.get(), 3);
        assert!(cache.get_latest(tenant).is_none());
    }

    #[tokio::test]
    async fn owner_failure_is_propagated_without_caching() {
        let cache = ModuleEffectivePolicyCache::new();
        let tenant = uuid::Uuid::new_v4();
        let result = resolve_policy_with_cache(tenant, &cache, || async {
            Err(PlatformCompositionError::EffectivePolicy(
                "owner failed".into(),
            ))
        })
        .await;
        assert!(matches!(
            result,
            Err(PlatformCompositionError::EffectivePolicy(message)) if message == "owner failed"
        ));
        assert!(cache.get_latest(tenant).is_none());
    }

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
