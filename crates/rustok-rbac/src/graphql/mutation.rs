use async_graphql::{Context, FieldError, InputObject, Object, Result};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::GraphQLError, has_effective_permission,
};
use rustok_core::UserRole;
use uuid::Uuid;

use super::control_plane::require_direct_control_plane_user;
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

        require_direct_control_plane_user(auth, tenant.id)?;

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
