//! Host adapter that binds the RBAC owner's module-availability contract to
//! the platform module registry and the tenant's effective module policy.
//!
//! Ownership (which module declares which permission) is derived once from
//! the compiled module registry and shared through the runtime context; the
//! enabled set is resolved per tenant through the revision-bound effective
//! policy cache, so disabling a module takes effect without a restart.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rustok_core::ModuleRegistry;
use rustok_modules::ModuleControlPlane;
use rustok_rbac::PermissionAvailability;

use super::effective_module_policy::{EffectiveModulePolicyService, ModulePolicyAuthorityRevision};
use super::server_runtime_context::ServerRuntimeContext;
use crate::error::{Error, Result};

#[derive(Clone)]
pub struct RbacPermissionOwnership(pub Arc<BTreeMap<String, String>>);

fn build_permission_ownership(
    db: &sea_orm::DatabaseConnection,
    registry: &ModuleRegistry,
) -> Result<BTreeMap<String, String>> {
    // The definition catalog is an owner service: obtain it through the
    // control plane instead of constructing it in the host adapter.
    let catalog = ModuleControlPlane::new(db.clone())
        .catalog(registry)
        .map_err(|error| {
            Error::Message(format!("RBAC permission ownership catalog failed: {error}"))
        })?;
    let mut ownership: BTreeMap<String, String> = BTreeMap::new();
    for definition in catalog.definitions() {
        for permission in &definition.permissions {
            // First registration wins: ownership must be stable and must not
            // depend on registry iteration order.
            ownership
                .entry(permission.key.clone())
                .or_insert_with(|| definition.slug.clone());
        }
    }
    Ok(ownership)
}

pub(super) fn permission_ownership(
    ctx: &ServerRuntimeContext,
    registry: &ModuleRegistry,
) -> Result<Arc<BTreeMap<String, String>>> {
    if let Some(cached) = ctx.shared_get::<RbacPermissionOwnership>() {
        return Ok(cached.0);
    }
    let ownership = Arc::new(build_permission_ownership(ctx.db(), registry)?);
    ctx.shared_insert(RbacPermissionOwnership(ownership.clone()));
    Ok(ownership)
}

/// Resolve the availability snapshot for a tenant. Returns `Ok(None)` when
/// no module registry is composed into this runtime (all permissions are
/// platform-core in that case). Resolution failures are surfaced: callers on
/// the authorization path must fail closed rather than silently treating a
/// disabled module's permissions as active.
pub async fn permission_availability_for(
    ctx: &ServerRuntimeContext,
    tenant_id: uuid::Uuid,
) -> Result<Option<PermissionAvailability>> {
    let Some(registry) = ctx.shared_get::<ModuleRegistry>() else {
        return Ok(None);
    };
    let ownership = permission_ownership(ctx, &registry)?;
    resolve_permission_availability(
        ctx.db(),
        &registry,
        &ctx.effective_policy_cache(),
        &ownership,
        tenant_id,
    )
    .await
    .map(Some)
}

/// Immutable observation of the Modules owner's static policy. Registry
/// inputs are trusted host composition, not permission authority. Equality
/// compares the tenant-bound evidence, not the registry's pointer identity.
#[derive(Clone)]
pub(crate) struct RequestModulePolicyAuthority {
    registry: ModuleRegistry,
    ownership: Arc<BTreeMap<String, String>>,
    revision: ModulePolicyAuthorityRevision,
    identity: rustok_modules::EffectivePolicyCacheIdentity,
    availability: PermissionAvailability,
}

impl std::fmt::Debug for RequestModulePolicyAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RequestModulePolicyAuthority")
            .field("revision", &self.revision)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RequestModulePolicyAuthority {
    fn eq(&self, other: &Self) -> bool {
        self.revision == other.revision
            && self.identity == other.identity
            && self.availability == other.availability
    }
}

impl Eq for RequestModulePolicyAuthority {}

impl RequestModulePolicyAuthority {
    async fn resolve(
        db: &sea_orm::DatabaseConnection,
        registry: ModuleRegistry,
        ownership: Arc<BTreeMap<String, String>>,
        tenant_id: uuid::Uuid,
        initialize: bool,
    ) -> Result<Self> {
        let (revision, snapshot) =
            EffectiveModulePolicyService::resolve_authority_snapshot(
                db, &registry, tenant_id, initialize,
            )
            .await
            .map_err(|error| Error::Message(format!("module policy resolution failed: {error}")))?;
        let enabled = snapshot.policy.into_enabled_modules().into_iter().collect();
        let availability = PermissionAvailability::new((*ownership).clone(), enabled);
        Ok(Self {
            registry,
            ownership,
            revision,
            identity: snapshot.cache_identity,
            availability,
        })
    }

    pub(crate) fn availability(&self) -> &PermissionAvailability {
        &self.availability
    }

    pub(crate) fn covers(&self, permissions: &[rustok_api::Permission]) -> bool {
        permissions.iter().all(|permission| self.availability.is_active(permission))
    }

    pub(crate) async fn is_current(
        &self,
        db: &sea_orm::DatabaseConnection,
        tenant_id: uuid::Uuid,
        permissions: &[rustok_api::Permission],
    ) -> Result<bool> {
        if self.identity.tenant_id() != tenant_id || !self.covers(permissions) {
            return Ok(false);
        }
        let live = Self::resolve(
            db, self.registry.clone(), Arc::clone(&self.ownership), tenant_id, false,
        )
        .await?;
        Ok(self == &live)
    }
}

/// Use the current owner policy, never the process-local TTL cache. Retain
/// the exact observation used to filter the admitted grants.
pub(crate) async fn authoritative_permission_availability_for(
    ctx: &ServerRuntimeContext,
    tenant_id: uuid::Uuid,
    initialize: bool,
) -> Result<Option<Arc<RequestModulePolicyAuthority>>> {
    let Some(registry) = ctx.shared_get::<ModuleRegistry>() else {
        return Ok(None);
    };
    let ownership = permission_ownership(ctx, &registry)?;
    RequestModulePolicyAuthority::resolve(ctx.db(), registry, ownership, tenant_id, initialize)
        .await
        .map(Arc::new)
        .map(Some)
}

pub(super) async fn resolve_permission_availability(
    db: &sea_orm::DatabaseConnection,
    registry: &ModuleRegistry,
    cache: &rustok_modules::ModuleEffectivePolicyCache,
    ownership: &BTreeMap<String, String>,
    tenant_id: uuid::Uuid,
) -> Result<PermissionAvailability> {
    let policy = EffectiveModulePolicyService::resolve_cached(db, registry, tenant_id, cache)
        .await
        .map_err(|error| Error::Message(format!("module policy resolution failed: {error}")))?;
    let enabled: BTreeSet<String> = policy.into_enabled_modules().into_iter().collect();
    Ok(PermissionAvailability::new(ownership.clone(), enabled))
}

/// Initialize composition before checking out the credential transaction.
pub(crate) async fn prepare_invite_module_policy(
    db: &sea_orm::DatabaseConnection,
    registry: Option<&ModuleRegistry>,
) -> Result<()> {
    if registry.is_some() {
        super::platform_composition::PlatformCompositionService::active_snapshot(db)
            .await
            .map_err(|error| Error::Message(format!("module composition unavailable: {error}")))?;
    }
    Ok(())
}

/// Caller holds SQLite's write fence or a PostgreSQL transaction. No cache
/// or additional pool checkout; locks remain held until commit.
pub(crate) async fn lock_invite_module_policy(
    db: &sea_orm::DatabaseConnection,
    tx: &sea_orm::DatabaseTransaction,
    registry: Option<&ModuleRegistry>,
    tenant_id: uuid::Uuid,
) -> Result<(String, Option<PermissionAvailability>)> {
    let Some(registry) = registry else {
        return Ok(("v1:uncomposed".to_string(), None));
    };
    let ownership = build_permission_ownership(db, registry)?;
    let owner_snapshot =
        rustok_modules::SeaOrmModuleCompositionService::lock_active_snapshot_in_transaction(tx)
            .await
            .map_err(|error| Error::Message(format!("module composition lock failed: {error}")))?;
    let snapshot = super::platform_composition::PlatformCompositionService::snapshot_from_owner(
        owner_snapshot,
    )
    .map_err(|error| Error::Message(format!("module composition decode failed: {error}")))?;
    let corequisites =
        crate::modules::ManifestManager::module_policy_corequisites(&snapshot.manifest)
            .map_err(|error| Error::Message(format!("module co-requisites unavailable: {error}")))?
            .into_iter()
            .flat_map(|(module_slug, required_modules)| {
                required_modules.into_iter().map(
                    move |(required_module_slug, version_requirement)| {
                        rustok_modules::ModuleEffectivePolicyCoRequisite {
                            module_slug: module_slug.clone(),
                            required_module_slug,
                            version_requirement,
                        }
                    },
                )
            });
    let (policy, state) = rustok_modules::SeaOrmModulePolicyRevisionConsumer::new(db.clone())
        .lock_and_resolve_static_policy_state_in_transaction(
            tx,
            tenant_id,
            "module.lifecycle",
            registry,
            snapshot.manifest.settings.default_enabled,
            corequisites,
        )
        .await
        .map_err(|error| Error::Message(format!("module policy lock failed: {error}")))?;
    let binding = format!(
        "v1:{}:{}:{}",
        snapshot.revision,
        state.current_sequence,
        policy.policy_revision()
    );
    let enabled = policy.into_enabled_modules().into_iter().collect();
    Ok((
        binding,
        Some(PermissionAvailability::new(ownership, enabled)),
    ))
}

/// GraphQL provider registered with the RBAC owner queries/mutations.
pub struct ServerRbacModuleAvailabilityProvider {
    ctx: ServerRuntimeContext,
}

impl ServerRbacModuleAvailabilityProvider {
    pub fn new(ctx: ServerRuntimeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait::async_trait]
impl rustok_rbac::graphql::RbacModuleAvailabilityProvider for ServerRbacModuleAvailabilityProvider {
    async fn permission_availability(
        &self,
        tenant_id: &uuid::Uuid,
    ) -> Option<PermissionAvailability> {
        match permission_availability_for(&self.ctx, *tenant_id).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                tracing::error!(
                    %error,
                    tenant_id = %tenant_id,
                    "RBAC module availability resolution failed; failing closed for module-owned permissions"
                );
                // The owner trait uses `None` for hosts with no module registry.
                // A resolution failure is different: keep ownership known and
                // mark every module-owned permission dormant instead of
                // silently presenting disabled-module grants as active.
                let registry = self.ctx.shared_get::<ModuleRegistry>()?;
                match permission_ownership(&self.ctx, &registry) {
                    Ok(ownership) => Some(fail_closed_availability(&ownership)),
                    Err(ownership_error) => {
                        tracing::error!(
                            %ownership_error,
                            tenant_id = %tenant_id,
                            "RBAC permission ownership is unavailable; denying all permissions"
                        );
                        Some(PermissionAvailability::all_disabled())
                    }
                }
            }
        }
    }
}

fn fail_closed_availability(ownership: &BTreeMap<String, String>) -> PermissionAvailability {
    PermissionAvailability::new(ownership.clone(), BTreeSet::new())
}

pub fn rbac_module_availability_handle(
    ctx: &ServerRuntimeContext,
) -> Option<rustok_rbac::graphql::RbacModuleAvailabilityHandle> {
    ctx.shared_get::<ModuleRegistry>()?;
    Some(rustok_rbac::graphql::RbacModuleAvailabilityHandle(
        Arc::new(ServerRbacModuleAvailabilityProvider::new(ctx.clone())),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::Permission;

    #[test]
    fn failed_availability_snapshot_keeps_module_permissions_dormant() {
        let ownership = RbacPermissionOwnership(Arc::new(BTreeMap::from([
            ("posts:manage".to_string(), "blog".to_string()),
            ("ai:providers:manage".to_string(), "ai".to_string()),
        ])));
        let snapshot = fail_closed_availability(&ownership.0);
        assert!(!snapshot.is_active(&Permission::POSTS_MANAGE));
        assert!(!snapshot.is_active(&Permission::AI_PROVIDERS_MANAGE));
        assert!(snapshot.is_active(&Permission::USERS_MANAGE));
        assert_eq!(
            snapshot.dormant_permissions(&[Permission::POSTS_MANAGE, Permission::USERS_MANAGE]),
            vec![Permission::POSTS_MANAGE]
        );
    }
}
