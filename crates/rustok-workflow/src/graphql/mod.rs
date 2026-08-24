mod mutation;
mod query;
mod types;

use async_graphql::{Context, Result};
use rustok_api::{AuthContext, Permission};

pub use mutation::WorkflowMutation;
pub use query::WorkflowQuery;
pub use types::*;

pub(crate) const MODULE_SLUG: &str = "workflow";

pub(crate) fn require_workflow_permission<'a>(
    ctx: &'a Context<'_>,
    permissions: &[Permission],
    message: &str,
) -> Result<&'a AuthContext> {
    rustok_api::graphql::require_graphql_auth(ctx, permissions, message)
}
