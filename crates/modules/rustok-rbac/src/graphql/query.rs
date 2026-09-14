use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    AuthContext, AuthPrincipalContext, Permission, RuntimeLocale, TenantContext,
    graphql::{GraphQLError, resolve_graphql_locale}, has_effective_permission,
};
use sea_orm::DatabaseConnection;

use crate::RbacPresentationService;

use super::control_plane::require_direct_control_plane_user;
use super::types::RoleInfo;

#[derive(Default)]
pub struct RbacQuery;

#[Object]
impl RbacQuery {
    /// List all platform roles with their permission sets and locale-aware owner presentation.
    /// Requires a direct, session-bound user principal with `settings:read`.
    async fn roles(&self, ctx: &Context<'_>) -> Result<Vec<RoleInfo>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let principal_context = ctx
            .data::<AuthPrincipalContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_direct_control_plane_user(auth, *principal_context, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:read required to list roles",
            ));
        }

        let db = ctx.data::<DatabaseConnection>()?;
        let locale = RuntimeLocale::new(resolve_graphql_locale(ctx, None)).map_err(|error| {
            <FieldError as GraphQLError>::bad_user_input(&format!(
                "invalid RBAC presentation locale: {error}"
            ))
        })?;
        let roles = RbacPresentationService::from_database(db.clone())
            .roles(tenant.id, &locale)
            .await
            .map_err(|error| {
                <FieldError as GraphQLError>::internal_error(&format!(
                    "failed to resolve RBAC role presentation: {error}"
                ))
            })?
            .into_iter()
            .map(|role| RoleInfo {
                slug: role.slug,
                display_name: role.display_name,
                permissions: role.permission_slugs,
            })
            .collect();

        Ok(roles)
    }
}
