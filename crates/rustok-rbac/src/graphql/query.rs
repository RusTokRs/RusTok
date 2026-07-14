use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    graphql::GraphQLError, has_effective_permission, AuthContext, Permission, TenantContext,
};
use rustok_core::{Rbac, UserRole};
use uuid::Uuid;

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

fn ensure_direct_user_principal(auth: &AuthContext) -> Result<()> {
    if auth.client_id.is_some() || auth.grant_type != "direct" || auth.session_id.is_nil() {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "RBAC control-plane queries require a direct, session-bound user principal",
        ));
    }

    Ok(())
}

fn ensure_tenant_binding(auth: &AuthContext, tenant_id: Uuid) -> Result<()> {
    if auth.tenant_id != tenant_id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "authenticated principal belongs to another tenant",
        ));
    }

    Ok(())
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

        ensure_direct_user_principal(auth)?;
        ensure_tenant_binding(auth, tenant.id)?;

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

#[cfg(test)]
mod tests {
    use super::{ensure_direct_user_principal, ensure_tenant_binding};
    use rustok_api::AuthContext;
    use uuid::Uuid;

    fn auth_context(
        tenant_id: Uuid,
        session_id: Uuid,
        client_id: Option<Uuid>,
        grant_type: &str,
    ) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id,
            tenant_id,
            permissions: Vec::new(),
            client_id,
            scopes: Vec::new(),
            grant_type: grant_type.to_string(),
        }
    }

    #[test]
    fn direct_session_bound_user_principal_is_allowed() {
        let tenant_id = Uuid::new_v4();
        let auth = auth_context(tenant_id, Uuid::new_v4(), None, "direct");

        assert!(ensure_direct_user_principal(&auth).is_ok());
        assert!(ensure_tenant_binding(&auth, tenant_id).is_ok());
    }

    #[test]
    fn oauth_principals_are_denied() {
        for grant_type in ["authorization_code", "client_credentials"] {
            let auth = auth_context(
                Uuid::new_v4(),
                Uuid::nil(),
                Some(Uuid::new_v4()),
                grant_type,
            );

            assert!(ensure_direct_user_principal(&auth).is_err());
        }
    }

    #[test]
    fn malformed_direct_principal_without_session_is_denied() {
        let auth = auth_context(Uuid::new_v4(), Uuid::nil(), None, "direct");

        assert!(ensure_direct_user_principal(&auth).is_err());
    }

    #[test]
    fn cross_tenant_context_is_denied() {
        let auth = auth_context(Uuid::new_v4(), Uuid::new_v4(), None, "direct");

        assert!(ensure_tenant_binding(&auth, Uuid::new_v4()).is_err());
    }
}
