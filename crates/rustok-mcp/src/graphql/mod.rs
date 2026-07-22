pub mod mutation;
pub mod query;
pub mod types;

use std::sync::Arc;

use async_graphql::{Context, FieldError, Result};
use rustok_api::{AuthContext, Permission, graphql::GraphQLError, has_effective_permission};
use rustok_core::ModuleRuntimeExtensions;

use crate::{McpManagementContext, McpManagementMutationError, McpManagementRuntime};

pub use mutation::McpMutation;
pub use query::McpQuery;
pub use types::*;

fn require_auth_context<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    ctx.data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())
}

fn ensure_permission(auth: &AuthContext, permission: Permission, message: &str) -> Result<()> {
    if has_effective_permission(&auth.permissions, &permission) {
        Ok(())
    } else {
        Err(<FieldError as GraphQLError>::permission_denied(message))
    }
}

fn runtime(ctx: &Context<'_>) -> Result<McpManagementRuntime> {
    ctx.data::<Arc<ModuleRuntimeExtensions>>()?
        .get::<McpManagementRuntime>()
        .cloned()
        .ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error(
                "McpManagementRuntime is not registered; initialize the server provider",
            )
        })
}

fn management_context(auth: &AuthContext) -> McpManagementContext {
    McpManagementContext {
        actor_id: auth.user_id,
        tenant_id: auth.tenant_id,
    }
}

fn map_error(error: McpManagementMutationError) -> FieldError {
    match error {
        McpManagementMutationError::Validation(message)
        | McpManagementMutationError::Conflict(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        McpManagementMutationError::NotFound(message) => {
            <FieldError as GraphQLError>::not_found(&message)
        }
        McpManagementMutationError::Internal(message) => {
            <FieldError as GraphQLError>::internal_error(&message)
        }
    }
}
