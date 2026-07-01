pub mod mutation;
pub mod query;
pub mod types;

use async_graphql::{FieldError, Result};

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use rustok_api::Permission;
use rustok_rbac::has_effective_permission_in_set;

pub(super) fn ensure_mcp_read(auth: &AuthContext) -> Result<()> {
    if has_effective_permission_in_set(&auth.permissions, &Permission::MCP_READ) {
        Ok(())
    } else {
        Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: mcp:read required",
        ))
    }
}

pub(super) fn ensure_mcp_manage(auth: &AuthContext) -> Result<()> {
    if has_effective_permission_in_set(&auth.permissions, &Permission::MCP_MANAGE) {
        Ok(())
    } else {
        Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: mcp:manage required",
        ))
    }
}

pub use mutation::McpMutation;
pub use query::McpQuery;
pub use types::*;
