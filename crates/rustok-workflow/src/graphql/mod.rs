mod mutation;
mod query;
mod types;

use async_graphql::{Context, FieldError, Result};
use rustok_api::Permission;
use rustok_api::{AuthContext, graphql::GraphQLError, has_any_effective_permission};

pub use mutation::WorkflowMutation;
pub use query::WorkflowQuery;
pub use types::*;

pub(crate) const MODULE_SLUG: &str = "workflow";

pub(crate) fn require_workflow_permission(
    ctx: &Context<'_>,
    permissions: &[Permission],
    message: &str,
) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();

    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
    }

    Ok(auth)
}
