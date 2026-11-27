use async_trait::async_trait;
use rustok_core::UserRole;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RbacGraphqlRoleWriteError {
    #[error("permission denied: {0}")]
    Forbidden(String),
    #[error("role assignment target not found: {0}")]
    NotFound(String),
    #[error("role assignment conflict: {0}")]
    Conflict(String),
    #[error("role assignment failed: {0}")]
    Internal(String),
}

#[async_trait]
pub trait RbacGraphqlRoleWriter: Send + Sync {
    async fn replace_user_role(
        &self,
        tenant_id: &Uuid,
        actor_id: &Uuid,
        user_id: &Uuid,
        role: UserRole,
    ) -> Result<(), RbacGraphqlRoleWriteError>;

    /// Assign any tenant-persisted role by slug (custom roles included).
    /// Hosts without a slug-aware writer only support the canonical built-in
    /// roles and reject custom slugs.
    async fn replace_user_role_by_slug(
        &self,
        tenant_id: &Uuid,
        actor_id: &Uuid,
        user_id: &Uuid,
        role_slug: &str,
    ) -> Result<(), RbacGraphqlRoleWriteError> {
        match role_slug.trim().to_lowercase().parse::<UserRole>() {
            Ok(role) => {
                self.replace_user_role(tenant_id, actor_id, user_id, role)
                    .await
            }
            Err(_) => Err(RbacGraphqlRoleWriteError::Forbidden(format!(
                "custom role '{role_slug}' cannot be assigned by this host"
            ))),
        }
    }

    async fn invalidate_role_users(
        &self,
        tenant_id: &Uuid,
        role_id: &Uuid,
    ) -> Result<(), RbacGraphqlRoleWriteError> {
        let _ = (tenant_id, role_id);
        Ok(())
    }

    /// Notify the host after a role-grant replacement and its durable generation commit.
    ///
    /// The generation is a global recovery watermark, not a per-role event.
    /// The compatibility default invokes the older membership hook; hosts with
    /// distributed caches should also reconcile or publish the committed marker.
    async fn role_permissions_committed(
        &self,
        tenant_id: &Uuid,
        role_id: &Uuid,
        _generation: u64,
    ) -> Result<(), RbacGraphqlRoleWriteError> {
        self.invalidate_role_users(tenant_id, role_id).await
    }
}

#[derive(Clone)]
pub struct RbacGraphqlRoleWriterHandle(pub Arc<dyn RbacGraphqlRoleWriter>);

/// Host-provided snapshot of permission ownership and tenant module
/// enablement. When no provider is registered, every permission is treated
/// as platform-core (always active) and role grants are never dormant.
#[async_trait]
pub trait RbacModuleAvailabilityProvider: Send + Sync {
    async fn permission_availability(
        &self,
        tenant_id: &Uuid,
    ) -> Option<crate::PermissionAvailability>;
}

#[derive(Clone)]
pub struct RbacModuleAvailabilityHandle(pub Arc<dyn RbacModuleAvailabilityProvider>);

/// Host-authenticated transaction authority for RBAC control-plane writes.
/// Implementations acquire credential, module policy and ordered role fences
/// on the supplied owner transaction, not on another pool connection.
#[derive(Clone)]
pub struct RbacLockedMutationAuthority {
    pub permissions: Vec<rustok_api::Permission>,
    pub availability: Option<crate::PermissionAvailability>,
    pub deadline: chrono::DateTime<chrono::Utc>,
}

impl RbacLockedMutationAuthority {
    /// Restrict live authority to the original admitted effective grants.
    /// A manage grant covers concrete actions on its resource, not the reverse.
    pub fn restrict_to(&mut self, admitted: &[rustok_api::Permission]) {
        let live = &self.permissions;
        let mut common: Vec<_> = live
            .iter()
            .chain(admitted)
            .copied()
            .filter(|permission| {
                rustok_api::has_effective_permission(live, permission)
                    && rustok_api::has_effective_permission(admitted, permission)
            })
            .collect();
        common.sort_by_cached_key(ToString::to_string);
        common.dedup();
        self.permissions = common;
    }
}

/// The returned authority must be bounded by both the admitted request
/// and current effective grants, not merely satisfy each gate independently.
#[async_trait]
pub trait RbacMutationAuthority: Send + Sync {
    async fn prepare(&self) -> Result<(), RbacGraphqlRoleWriteError>;
    async fn lock(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        target_role_id: Option<Uuid>,
        required: &[rustok_api::Permission],
    ) -> Result<RbacLockedMutationAuthority, RbacGraphqlRoleWriteError>;
    async fn ensure_deadline(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        authority: &RbacLockedMutationAuthority,
    ) -> Result<(), RbacGraphqlRoleWriteError>;
}

#[derive(Clone)]
pub struct RbacMutationAuthorityHandle(pub Arc<dyn RbacMutationAuthority>);

#[cfg(test)]
mod tests {
    use super::RbacLockedMutationAuthority;
    use rustok_api::Permission;

    #[test]
    fn locked_authority_intersects_effective_grants_without_expanding_manage() {
        for (live, admitted) in [
            (
                vec![Permission::USERS_MANAGE, Permission::SETTINGS_MANAGE],
                vec![Permission::USERS_READ, Permission::POSTS_MANAGE],
            ),
            (
                vec![Permission::USERS_READ, Permission::POSTS_MANAGE],
                vec![Permission::USERS_MANAGE, Permission::SETTINGS_MANAGE],
            ),
        ] {
            let mut authority = RbacLockedMutationAuthority {
                permissions: live,
                availability: None,
                deadline: chrono::Utc::now(),
            };
            authority.restrict_to(&admitted);
            assert_eq!(authority.permissions, vec![Permission::USERS_READ]);
            authority.restrict_to(&admitted);
            assert_eq!(authority.permissions, vec![Permission::USERS_READ]);
            authority.restrict_to(&[]);
            assert!(authority.permissions.is_empty());
        }
    }
}
