use async_graphql::{Context, FieldError, InputObject, Object, Result};
use rustok_api::{
    graphql::GraphQLError, has_effective_permission, AuthContext, Permission, TenantContext,
};
use rustok_core::UserRole;
use uuid::Uuid;

use super::types::{AssignUserRolePayload, RbacGraphqlUserRole};
use super::{RbacGraphqlRoleWriteError, RbacGraphqlRoleWriterHandle};

#[derive(InputObject)]
pub struct AssignUserRoleInput {
    pub user_id: Uuid,
    pub role: RbacGraphqlUserRole,
}

#[derive(Default)]
pub struct RbacMutation;

fn map_role_write_error(error: RbacGraphqlRoleWriteError) -> FieldError {
    match error {
        RbacGraphqlRoleWriteError::Forbidden(message) => {
            <FieldError as GraphQLError>::permission_denied(&message)
        }
        RbacGraphqlRoleWriteError::NotFound(message) => {
            <FieldError as GraphQLError>::not_found(&message)
        }
        RbacGraphqlRoleWriteError::Conflict(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        RbacGraphqlRoleWriteError::Internal(message) => {
            <FieldError as GraphQLError>::internal_error(&message)
        }
    }
}

fn ensure_direct_user_principal(auth: &AuthContext) -> Result<()> {
    if auth.client_id.is_some() || auth.grant_type != "direct" || auth.session_id.is_nil() {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "RBAC control-plane mutations require a direct, session-bound user principal",
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
impl RbacMutation {
    /// Assign a role to a user (replaces the current role).
    /// Requires a direct, session-bound user principal with `users:manage`;
    /// hierarchy, target and continuity rules are enforced transactionally by
    /// the host role writer.
    async fn assign_user_role(
        &self,
        ctx: &Context<'_>,
        input: AssignUserRoleInput,
    ) -> Result<AssignUserRolePayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        ensure_direct_user_principal(auth)?;
        ensure_tenant_binding(auth, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::USERS_MANAGE) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "users:manage required to assign roles",
            ));
        }

        let user_role: UserRole = input.role.into();
        let role = user_role.to_string();
        let writer = ctx.data::<RbacGraphqlRoleWriterHandle>()?;

        writer
            .0
            .replace_user_role(&tenant.id, &auth.user_id, &input.user_id, user_role)
            .await
            .map_err(map_role_write_error)?;

        Ok(AssignUserRolePayload {
            success: true,
            user_id: input.user_id.to_string(),
            role,
        })
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
    fn client_credentials_principal_is_denied() {
        let auth = auth_context(
            Uuid::new_v4(),
            Uuid::nil(),
            Some(Uuid::new_v4()),
            "client_credentials",
        );

        assert!(ensure_direct_user_principal(&auth).is_err());
    }

    #[test]
    fn authorization_code_oauth_principal_is_denied_by_default() {
        let auth = auth_context(
            Uuid::new_v4(),
            Uuid::nil(),
            Some(Uuid::new_v4()),
            "authorization_code",
        );

        assert!(ensure_direct_user_principal(&auth).is_err());
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
