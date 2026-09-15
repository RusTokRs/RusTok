use leptos::prelude::*;

use crate::model::RbacAdminBootstrap;
#[cfg(feature = "ssr")]
use crate::model::{RbacModulePermissionGroup, RbacRoleInfo};

#[cfg(feature = "ssr")]
const RBAC_ADMIN_BOUNDARY: &str = "rbac_admin_native_transport";

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
        use rustok_api::{
            AuthContext, AuthPrincipalContext, HostRuntimeContext, PLATFORM_FALLBACK_LOCALE,
            Permission, RequestContext, RuntimeLocale, TenantContext, has_effective_permission,
        };
        use rustok_core::{ModuleRegistry, infer_user_role_from_permissions};
        use rustok_rbac::{
            RbacControlPlanePrincipal, RbacLocalizedCatalogReader, require_direct_control_plane_user,
        };

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
        let principal_context = leptos_axum::extract::<AuthPrincipalContext>()
            .await
            .map_err(|error| {
                rbac_admin_context_error(
                    error,
                    "principal_kind",
                    "RBAC principal context is temporarily unavailable",
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
        let principal = RbacControlPlanePrincipal {
            tenant_id: auth.tenant_id,
            principal_kind: principal_context.kind,
        };
        require_direct_control_plane_user(principal, tenant.id).map_err(|error| {
            tracing::warn!(
                reason = ?error,
                auth_tenant_id = %auth.tenant_id,
                resolved_tenant_id = %tenant.id,
                code = "rbac.admin_control_plane_denied",
                boundary = RBAC_ADMIN_BOUNDARY,
                "RBAC admin bootstrap principal is not eligible for control-plane access"
            );
            ServerFnError::new("RBAC admin access is denied")
        })?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new(
                "settings:read required to load RBAC administration bootstrap",
            ));
        }

        let request_context = leptos_axum::extract::<RequestContext>().await.ok();
        let locale = request_context
            .as_ref()
            .and_then(|request| RuntimeLocale::new(&request.locale).ok())
            .or_else(|| RuntimeLocale::new(&tenant.default_locale).ok())
            .or_else(|| RuntimeLocale::new(PLATFORM_FALLBACK_LOCALE).ok())
            .ok_or_else(|| ServerFnError::new("RBAC request locale is unavailable"))?;
        let host = expect_context::<HostRuntimeContext>();
        let roles = RbacLocalizedCatalogReader::new(host.db_clone())
            .roles(tenant.id, &locale)
            .await
            .map_err(|error| {
                tracing::error!(
                    error_type = std::any::type_name_of_val(&error),
                    tenant_id = %tenant.id,
                    code = "rbac.admin_presentation_unavailable",
                    boundary = RBAC_ADMIN_BOUNDARY,
                    "RBAC admin localized presentation read failed"
                );
                ServerFnError::new("RBAC role presentation is temporarily unavailable")
            })?
            .into_iter()
            .map(|role| RbacRoleInfo {
                slug: role.slug,
                display_name: role.display_name,
                permissions: role.permission_slugs,
            })
            .collect();

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
