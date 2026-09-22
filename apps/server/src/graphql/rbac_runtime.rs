use std::sync::Arc;

use async_trait::async_trait;
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, UpdateUserCommand, UserAdminMutationRuntime,
};
use rustok_core::{ModuleRuntimeExtensions, UserRole};
use rustok_rbac::graphql::{
    RbacGraphqlRoleWriteError, RbacGraphqlRoleWriter, RbacGraphqlRoleWriterHandle,
};
use uuid::Uuid;

use crate::services::server_runtime_context::ServerRuntimeContext;

struct ServerRbacGraphqlRoleWriter {
    runtime: UserAdminMutationRuntime,
}

#[async_trait]
impl RbacGraphqlRoleWriter for ServerRbacGraphqlRoleWriter {
    async fn replace_user_role(
        &self,
        tenant_id: &Uuid,
        actor_id: &Uuid,
        user_id: &Uuid,
        role: UserRole,
    ) -> Result<(), RbacGraphqlRoleWriteError> {
        self.runtime
            .port()
            .update_user(
                &AuthAdminMutationContext {
                    actor_id: *actor_id,
                    tenant_id: *tenant_id,
                    request_id: None,
                    locale: None,
                },
                UpdateUserCommand {
                    id: *user_id,
                    email: None,
                    password: None,
                    name: None,
                    role: Some(role.to_string()),
                    status: None,
                    custom_fields: None,
                },
            )
            .await
            .map(|_| ())
            .map_err(map_auth_admin_error)
    }
}

fn map_auth_admin_error(error: AuthAdminMutationError) -> RbacGraphqlRoleWriteError {
    match error {
        AuthAdminMutationError::Unauthorized => RbacGraphqlRoleWriteError::Forbidden(
            "authentication context is unavailable".to_string(),
        ),
        AuthAdminMutationError::Forbidden(message) => RbacGraphqlRoleWriteError::Forbidden(message),
        AuthAdminMutationError::NotFound(message) => RbacGraphqlRoleWriteError::NotFound(message),
        AuthAdminMutationError::Validation(message) | AuthAdminMutationError::Conflict(message) => {
            RbacGraphqlRoleWriteError::Conflict(message)
        }
        AuthAdminMutationError::CustomFieldsValidation(fields) => {
            RbacGraphqlRoleWriteError::Conflict(fields.to_string())
        }
        AuthAdminMutationError::Internal(message) => RbacGraphqlRoleWriteError::Internal(message),
    }
}

pub fn rbac_graphql_role_writer_from_context(
    ctx: &ServerRuntimeContext,
) -> RbacGraphqlRoleWriterHandle {
    let extensions = ctx
        .shared_get::<Arc<ModuleRuntimeExtensions>>()
        .expect("ModuleRuntimeExtensions must be initialized before GraphQL schema construction");
    let runtime = extensions
        .get::<UserAdminMutationRuntime>()
        .cloned()
        .expect("UserAdminMutationRuntime must be registered before GraphQL schema construction");

    RbacGraphqlRoleWriterHandle(Arc::new(ServerRbacGraphqlRoleWriter { runtime }))
}
