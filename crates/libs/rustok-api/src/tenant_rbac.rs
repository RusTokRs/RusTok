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
    #[error("role `{slug}` is not available for tenant `{tenant_id}`")]
    UnknownRole { tenant_id: Uuid, slug: String },
    #[error("permission `{slug}` is not available for tenant `{tenant_id}`")]
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

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCatalog;

    impl TenantRbacCatalog for MockCatalog {
        fn roles(&self, _tenant_id: Uuid) -> Vec<TenantRbacRole> {
            vec![TenantRbacRole {
                slug: "admin".to_string(),
                display_name: "Administrator".to_string(),
                permission_slugs: vec!["products:read".to_string()],
            }]
        }

        fn permissions(&self, _tenant_id: Uuid) -> Vec<TenantRbacPermission> {
            vec![TenantRbacPermission {
                slug: "products:read".to_string(),
                display_name: "Read Products".to_string(),
            }]
        }
    }

    #[test]
    fn validate_assignment_accepts_catalog_roles_and_permissions() {
        let catalog = MockCatalog;
        let tenant_id = Uuid::new_v4();

        let result = catalog.validate_assignment(
            tenant_id,
            &["admin".to_string()],
            &["products:read".to_string()],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn validate_assignment_rejects_unknown_role() {
        let catalog = MockCatalog;
        let tenant_id = Uuid::new_v4();

        let result = catalog.validate_assignment(
            tenant_id,
            &["superadmin".to_string()],
            &["products:read".to_string()],
        );
        assert_eq!(
            result,
            Err(TenantRbacCatalogError::UnknownRole {
                tenant_id,
                slug: "superadmin".to_string(),
            })
        );
    }

    #[test]
    fn validate_assignment_rejects_unknown_permission() {
        let catalog = MockCatalog;
        let tenant_id = Uuid::new_v4();

        let result = catalog.validate_assignment(
            tenant_id,
            &["admin".to_string()],
            &["unknown:manage".to_string()],
        );
        assert_eq!(
            result,
            Err(TenantRbacCatalogError::UnknownPermission {
                tenant_id,
                slug: "unknown:manage".to_string(),
            })
        );
    }

    #[test]
    fn error_display_formats_balanced_backticks() {
        let tenant_id = Uuid::nil();
        let role_err = TenantRbacCatalogError::UnknownRole {
            tenant_id,
            slug: "operator".to_string(),
        };
        assert_eq!(
            role_err.to_string(),
            format!("role `operator` is not available for tenant `{tenant_id}`")
        );

        let perm_err = TenantRbacCatalogError::UnknownPermission {
            tenant_id,
            slug: "orders:delete".to_string(),
        };
        assert_eq!(
            perm_err.to_string(),
            format!("permission `orders:delete` is not available for tenant `{tenant_id}`")
        );
    }
}
