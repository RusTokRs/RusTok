use async_graphql::{Context, FieldError, Object, Result};
use loco_rs::app::AppContext;
use rustok_core::{Permission, Rbac, UserRole};

use crate::context::{AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::services::rbac_service::RbacService;

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
    /// Requires `settings:read` permission.
    async fn roles(&self, ctx: &Context<'_>) -> Result<Vec<RoleInfo>> {
        let app_ctx = ctx.data::<AppContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let can_read = RbacService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &Permission::SETTINGS_READ,
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !can_read {
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
