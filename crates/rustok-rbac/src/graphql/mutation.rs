use async_graphql::{Context, FieldError, InputObject, Object, Result};
use rustok_api::{
    graphql::GraphQLError, has_effective_permission, AuthContext, Permission, TenantContext,
};
use rustok_core::{infer_user_role_from_permissions, UserRole};
use uuid::Uuid;

use super::types::{AssignUserRolePayload, RbacGraphqlUserRole};
use super::RbacGraphqlRoleWriterHandle;

#[derive(InputObject)]
pub struct AssignUserRoleInput {
    pub user_id: Uuid,
    pub role: RbacGraphqlUserRole,
}

#[derive(Default)]
pub struct RbacMutation;

fn actor_can_assign_role(auth: &AuthContext, target_role: &UserRole) -> bool {
    infer_user_role_from_permissions(&auth.permissions).can_assign_role(target_role)
}

#[Object]
impl RbacMutation {
    /// Assign a role to a user (replaces the current role).
    /// Requires `users:manage` permission and enforces role hierarchy.
    async fn assign_user_role(
        &self,
        ctx: &Context<'_>,
        input: AssignUserRoleInput,
    ) -> Result<AssignUserRolePayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        if !has_effective_permission(&auth.permissions, &Permission::USERS_MANAGE) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "users:manage required to assign roles",
            ));
        }

        let user_role: UserRole = input.role.into();
        if !actor_can_assign_role(auth, &user_role) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "cannot assign a peer or higher-privileged role",
            ));
        }

        let role = user_role.to_string();
        let writer = ctx.data::<RbacGraphqlRoleWriterHandle>()?;

        writer
            .0
            .replace_user_role(&tenant.id, &input.user_id, user_role)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err))?;

        Ok(AssignUserRolePayload {
            success: true,
            user_id: input.user_id.to_string(),
            role,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::actor_can_assign_role;
    use rustok_api::AuthContext;
    use rustok_core::{Rbac, UserRole};
    use uuid::Uuid;

    fn auth_for(role: UserRole) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: Rbac::permissions_for_role(&role).to_vec(),
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    #[test]
    fn admin_can_assign_manager_but_not_admin_or_super_admin() {
        let auth = auth_for(UserRole::Admin);

        assert!(actor_can_assign_role(&auth, &UserRole::Manager));
        assert!(!actor_can_assign_role(&auth, &UserRole::Admin));
        assert!(!actor_can_assign_role(&auth, &UserRole::SuperAdmin));
    }

    #[test]
    fn super_admin_can_assign_super_admin() {
        let auth = auth_for(UserRole::SuperAdmin);
        assert!(actor_can_assign_role(&auth, &UserRole::SuperAdmin));
    }
}
