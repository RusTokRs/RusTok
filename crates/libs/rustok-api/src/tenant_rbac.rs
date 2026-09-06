use std::collections::BTreeSet;
use std::sync::Arc;

use thiserror::Error;
use uuid::Uuid;

/// Stable, tenant-scoped role metadata suitable for an owner-owned selection UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenantRbacRole {
    pub slug: String,
    pub display_name: String,
    pub permission_slugs: Vec<String>,
}

/// Stable, tenant-scoped permission metadata suitable for an owner-owned selection UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenantRbacPermission {
    pub slug: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TenantRbacCatalogError {
    #[error("role `{slug}` is not available for tenant `{tenant_id}")]
    UnknownRole { tenant_id: Uuid, slug: String },
    #[error("permission `{slug}` is not available for tenant `{tenant_id}")]
    UnknownPermission { tenant_id: Uuid, slug: String },
}

/// Read-only, tenant-scoped RBAC vocabulary published by the platform.
///
/// Consumers select only values returned by this catalog. They must never
/// accept arbitrary role or permission strings as a substitute for it.
pub trait TenantRbacCatalog: Send + Sync {
    fn roles(&self, tenant_id: Uuid) -> Vec<TenantRbacRole>;

    fn permissions(&self, tenant_id: Uuid) -> Vec<TenantRbacPermission>;

    fn validate_assignment(
        &self,
        tenant_id: Uuid,
        role_slugs: &[String],
        permission_slugs: &[String],
    ) -> Result<(), TenantRbacCatalogError> {
        let roles = self
            .roles(tenant_id)
            .into_iter()
            .map(|role| role.slug)
            .collect::<BTreeSet<_>>();
        for slug in role_slugs {
            if !roles.contains(slug) {
                return Err(TenantRbacCatalogError::UnknownRole {
                    tenant_id,
                    slug: slug.clone(),
                });
            }
        }

        let permissions = self
            .permissions(tenant_id)
            .into_iter()
            .map(|permission| permission.slug)
            .collect::<BTreeSet<_>>();
        for slug in permission_slugs {
            if !permissions.contains(slug) {
                return Err(TenantRbacCatalogError::UnknownPermission {
                    tenant_id,
                    slug: slug.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Cloneable generic runtime-extension value for the platform RBAC catalog.
#[derive(Clone)]
pub struct SharedTenantRbacCatalog(pub Arc<dyn TenantRbacCatalog>);
