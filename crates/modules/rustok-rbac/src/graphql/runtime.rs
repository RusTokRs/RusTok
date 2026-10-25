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
            Ok(role) => self.replace_user_role(tenant_id, actor_id, user_id, role).await,
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
