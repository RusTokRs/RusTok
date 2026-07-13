use crate::error::{Error, Result};

pub use rustok_mcp::MCP_SCAFFOLD_WORKSPACE_ROOT_ENV;

/// Resolve the only host-authorized workspace root for MCP scaffold writes.
///
/// The canonical policy lives in `rustok-mcp` so HTTP, GraphQL, stdio and
/// in-process AI transports cannot drift apart.
pub fn authorize_mcp_scaffold_workspace(requested_root: &str) -> Result<String> {
    rustok_mcp::authorize_scaffold_workspace(requested_root).map_err(Error::BadRequest)
}