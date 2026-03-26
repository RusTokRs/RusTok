use async_graphql::{Context, FieldError, InputObject, Object, Result};
use loco_rs::app::AppContext;
use rustok_core::Permission;
use uuid::Uuid;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::graphql::types::GqlUserRole;
use crate::services::rbac_service::RbacService;

use super::types::AssignUserRolePayload;

#[derive(InputObject)]
pub struct AssignUserRoleInput {
    pub user_id: Uuid,
    pub role: GqlUserRole,
}

#[derive(Default)]
pub struct RbacMutation;

#[Object]
impl RbacMutation {
    /// Assign a role to a user (replaces the current role).
    /// Requires `users:manage` permission.
    async fn assign_user_role(
        &self,
        ctx: &Context<'_>,
        input: AssignUserRoleInput,
    ) -> Result<AssignUserRolePayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let can_manage = RbacService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &Permission::USERS_MANAGE,
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !can_manage {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "users:manage required to assign roles",
            ));
        }

        let role_str = format!("{:?}", input.role);
        let user_role = input.role.into();

        RbacService::replace_user_role(&app_ctx.db, &input.user_id, &tenant.id, user_role)
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        Ok(AssignUserRolePayload {
            success: true,
            user_id: input.user_id.to_string(),
            role: role_str,
        })
    }
}
