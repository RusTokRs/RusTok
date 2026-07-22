use std::collections::BTreeSet;

use rustok_api::{TenantRbacCatalog, TenantRbacPermission, TenantRbacRole};
use rustok_core::{Rbac, UserRole};
use uuid::Uuid;

/// Platform RBAC provider for the generic tenant catalog contract.
#[derive(Clone, Default)]
pub struct BuiltinTenantRbacCatalog;

const ROLES: &[UserRole] = &[
    UserRole::SuperAdmin,
    UserRole::Admin,
    UserRole::Manager,
    UserRole::Customer,
];

impl TenantRbacCatalog for BuiltinTenantRbacCatalog {
    fn roles(&self, _tenant_id: Uuid) -> Vec<TenantRbacRole> {
        ROLES
            .iter()
            .map(|role| {
                let mut permission_slugs = Rbac::permissions_for_role(role)
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                permission_slugs.sort();
                TenantRbacRole {
                    slug: role.to_string(),
                    display_name: role_display_name(role.clone()).to_string(),
                    permission_slugs,
                }
            })
            .collect()
    }

    fn permissions(&self, tenant_id: Uuid) -> Vec<TenantRbacPermission> {
        let permission_slugs = self
            .roles(tenant_id)
            .into_iter()
            .flat_map(|role| role.permission_slugs)
            .collect::<BTreeSet<_>>();
        permission_slugs
            .into_iter()
            .map(|slug| TenantRbacPermission {
                display_name: slug.replace(':', " / "),
                slug,
            })
            .collect()
    }
}

fn role_display_name(role: UserRole) -> &'static str {
    match role {
        UserRole::SuperAdmin => "Super Admin",
        UserRole::Admin => "Admin",
        UserRole::Manager => "Manager",
        UserRole::Customer => "Customer",
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::TenantRbacCatalog;
    use uuid::Uuid;

    use super::BuiltinTenantRbacCatalog;

    #[test]
    fn catalog_exposes_roles_and_validates_only_published_assignment_values() {
        let catalog = BuiltinTenantRbacCatalog;
        let tenant_id = Uuid::new_v4();
        let roles = catalog.roles(tenant_id);
        let permissions = catalog.permissions(tenant_id);

        assert!(roles.iter().any(|role| role.slug == "admin"));
        assert!(
            permissions
                .iter()
                .any(|permission| permission.slug == "ai:providers:manage")
        );
        assert!(
            catalog
                .validate_assignment(
                    tenant_id,
                    &["admin".to_string()],
                    &["ai:providers:manage".to_string()],
                )
                .is_ok()
        );
        assert!(
            catalog
                .validate_assignment(tenant_id, &["unknown".to_string()], &[])
                .is_err()
        );
    }
}
