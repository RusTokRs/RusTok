use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::GraphQLError, has_effective_permission,
};
use rustok_core::{Rbac, UserRole};

use super::control_plane::require_direct_control_plane_user;
use super::types::RoleInfo;

#[derive(Default)]
pub struct RbacQuery;

const ALL_ROLES: &[UserRole] = &[
    UserRole::SuperAdmin,
    UserRole::Admin,
    UserRole::Manager,
    UserRole::Customer,
];

fn display_name(role: &UserRole) -> &'static str {
    match role {
        UserRole::SuperAdmin => "Super Admin",
        UserRole::Admin => "Admin",
        UserRole::Manager => "Manager",
        UserRole::Customer => "Customer",
    }
}

#[Object]
impl RbacQuery {
    /// List all platform roles with their permission sets.
    /// Requires a direct, session-bound user principal with `settings:read`.
    async fn roles(&self, ctx: &Context<'_>) -> Result<Vec<RoleInfo>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_direct_control_plane_user(auth, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required to list roles",
            ));
        }

        let roles = ALL_ROLES
            .iter()
            .map(|role| {
                let mut perms: Vec<String> = Rbac::permissions_for_role(role)
                    .iter()
                    .map(|p| p.to_string())
                    .collect();
                perms.sort();
                RoleInfo {
                    slug: role.to_string(),
                    display_name: display_name(role).to_string(),
                    permissions: perms,
                }
            })
            .collect();

        Ok(roles)
    }
}
