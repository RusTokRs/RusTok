use leptos::prelude::*;

use crate::model::RbacAdminBootstrap;
#[cfg(feature = "ssr")]
use crate::model::{RbacModulePermissionGroup, RbacRoleInfo};

#[cfg(feature = "ssr")]
const RBAC_ADMIN_BOUNDARY: &str = "rbac_admin_native_transport";

#[cfg(feature = "ssr")]
fn rbac_admin_scope_matches<T: PartialEq>(auth_tenant_id: &T, resolved_tenant_id: &T) -> bool {
    auth_tenant_id == resolved_tenant_id
}

#[cfg(feature = "ssr")]
fn require_rbac_admin_tenant_scope<T>(
    auth_tenant_id: &T,
    resolved_tenant_id: &T,
) -> Result<(), ServerFnError>
where
    T: PartialEq + std::fmt::Display,
{
    if rbac_admin_scope_matches(auth_tenant_id, resolved_tenant_id) {
        return Ok(());
    }

    tracing::warn!(
        auth_tenant_id = %auth_tenant_id,
        resolved_tenant_id = %resolved_tenant_id,
        code = "rbac.admin_tenant_scope_mismatch",
        boundary = RBAC_ADMIN_BOUNDARY,
        "RBAC admin permissions cannot cross the resolved tenant boundary"
    );
    Err(ServerFnError::new("RBAC admin access is denied"))
}

#[cfg(feature = "ssr")]
fn rbac_admin_context_error<E: std::fmt::Debug>(
    error: E,
    context_kind: &'static str,
    public_message: &'static str,
) -> ServerFnError {
    tracing::error!(
        error = ?error,
        context_kind,
        code = "rbac.admin_context_unavailable",
        boundary = RBAC_ADMIN_BOUNDARY,
        "RBAC admin request context extraction failed"
    );
    ServerFnError::new(public_message)
}

#[server(prefix = "/api/fn", endpoint = "rbac/bootstrap")]
pub async fn fetch_bootstrap_native() -> Result<RbacAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{AuthContext, Permission, TenantContext, has_effective_permission};
        use rustok_core::{ModuleRegistry, Rbac, UserRole, infer_user_role_from_permissions};

        let registry = expect_context::<ModuleRegistry>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|error| {
                rbac_admin_context_error(
                    error,
                    "auth",
                    "RBAC authentication context is temporarily unavailable",
                )
            })?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|error| {
                rbac_admin_context_error(
                    error,
                    "tenant",
                    "RBAC tenant context is temporarily unavailable",
                )
            })?;
        require_rbac_admin_tenant_scope(&auth.tenant_id, &tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new(
                "settings:read required to load RBAC administration bootstrap",
            ));
        }

        let mut module_permissions = registry
            .list()
            .into_iter()
            .filter_map(|module| {
                let mut permissions = module
                    .permissions()
                    .into_iter()
                    .map(|permission| permission.to_string())
                    .collect::<Vec<_>>();
                permissions.sort();
                permissions.dedup();
                if permissions.is_empty() {
                    None
                } else {
                    Some(RbacModulePermissionGroup {
                        module_slug: module.slug().to_string(),
                        permissions,
                    })
                }
            })
            .collect::<Vec<_>>();
        module_permissions.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));

        let mut granted_permissions = auth
            .permissions
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        granted_permissions.sort();
        granted_permissions.dedup();

        let roles = [
            UserRole::SuperAdmin,
            UserRole::Admin,
            UserRole::Manager,
            UserRole::Customer,
        ]
        .into_iter()
        .map(|role| {
            let mut permissions = Rbac::permissions_for_role(&role)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            permissions.sort();
            RbacRoleInfo {
                slug: role.to_string(),
                display_name: match role {
                    UserRole::SuperAdmin => "Super Admin",
                    UserRole::Admin => "Admin",
                    UserRole::Manager => "Manager",
                    UserRole::Customer => "Customer",
                }
                .to_string(),
                permissions,
            }
        })
        .collect();

        Ok(RbacAdminBootstrap {
            tenant_slug: tenant.slug,
            current_user_id: auth.user_id.to_string(),
            inferred_role: format!("{:?}", infer_user_role_from_permissions(&auth.permissions)),
            granted_permissions,
            module_permissions,
            roles,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "rustok-rbac-admin requires the `ssr` feature for native bootstrap",
        ))
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::{rbac_admin_scope_matches, require_rbac_admin_tenant_scope};

    #[test]
    fn rbac_admin_scope_requires_matching_tenant() {
        assert!(rbac_admin_scope_matches(&"tenant-a", &"tenant-a"));
        assert!(!rbac_admin_scope_matches(&"tenant-a", &"tenant-b"));
        assert!(require_rbac_admin_tenant_scope(&"tenant-a", &"tenant-a").is_ok());
        assert!(require_rbac_admin_tenant_scope(&"tenant-a", &"tenant-b").is_err());
    }
}
