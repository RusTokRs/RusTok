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
use rustok_modules::ModuleDefinitionCatalog;
use rustok_rbac::PermissionAvailability;

use super::effective_module_policy::EffectiveModulePolicyService;
use super::server_runtime_context::ServerRuntimeContext;
use crate::error::{Error, Result};

#[derive(Clone)]
pub struct RbacPermissionOwnership(pub Arc<BTreeMap<String, String>>);

fn build_permission_ownership(registry: &ModuleRegistry) -> BTreeMap<String, String> {
    let mut ownership: BTreeMap<String, String> = BTreeMap::new();
    match ModuleDefinitionCatalog::from_static_registry(registry) {
        Ok(catalog) => {
            for definition in catalog.definitions() {
                for permission in &definition.permissions {
                    // First registration wins: ownership must be stable and
                    // must not depend on registry iteration order.
                    ownership
                        .entry(permission.key.clone())
                        .or_insert_with(|| definition.slug.clone());
                }
            }
        }
        Err(error) => {
            tracing::error!(
                %error,
                "failed to build RBAC permission ownership from the module registry; treating all permissions as platform-core"
            );
        }
    }
    ownership
}

fn permission_ownership(
    ctx: &ServerRuntimeContext,
    registry: &ModuleRegistry,
) -> Arc<BTreeMap<String, String>> {
    if let Some(cached) = ctx.shared_get::<RbacPermissionOwnership>() {
        return cached.0;
    }
    let ownership = Arc::new(build_permission_ownership(registry));
    ctx.shared_insert(RbacPermissionOwnership(ownership.clone()));
    ownership
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
    let ownership = permission_ownership(ctx, &registry);
    let snapshot = EffectiveModulePolicyService::resolve_snapshot_cached(
        ctx.db(),
        &registry,
        tenant_id,
        &ctx.effective_policy_cache(),
    )
    .await
    .map_err(|error| Error::Message(format!("module policy resolution failed: {error}")))?;
    let enabled: BTreeSet<String> = snapshot.policy.into_enabled_modules().into_iter().collect();
    Ok(Some(PermissionAvailability::new(
        (*ownership).clone(),
        enabled,
    )))
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
                let ownership = permission_ownership(&self.ctx, &registry);
                Some(fail_closed_availability(&ownership))
            }
        }
    }
}

fn fail_closed_availability(ownership: &RbacPermissionOwnership) -> PermissionAvailability {
    PermissionAvailability::new((*ownership.0).clone(), BTreeSet::new())
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
    use rustok_core::Permission;

    #[test]
    fn failed_availability_snapshot_keeps_module_permissions_dormant() {
        let ownership = RbacPermissionOwnership(Arc::new(BTreeMap::from([
            ("posts:manage".to_string(), "blog".to_string()),
            ("ai:providers:manage".to_string(), "ai".to_string()),
        ])));
        let snapshot = fail_closed_availability(&ownership);
        assert!(!snapshot.is_active(&Permission::POSTS_MANAGE));
        assert!(!snapshot.is_active(&Permission::AI_PROVIDERS_MANAGE));
        assert!(snapshot.is_active(&Permission::USERS_MANAGE));
        assert_eq!(
            snapshot.dormant_permissions(&[Permission::POSTS_MANAGE, Permission::USERS_MANAGE]),
            vec![Permission::POSTS_MANAGE]
        );
    }
}
