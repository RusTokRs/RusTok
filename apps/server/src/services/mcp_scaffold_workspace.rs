use rustok_api::HostAuthority;

use crate::error::{Error, Result};

pub use rustok_mcp::MCP_SCAFFOLD_WORKSPACE_ROOT_ENV;

/// Resolve the only host-authorized workspace root for MCP scaffold writes.
///
/// The canonical path policy lives in `rustok-mcp`; the server adds a
/// host-global authority boundary because the configured workspace is shared
/// by all tenants.
fn require_host_scaffold_manage_authority() -> Result<()> {
    match crate::host_authority::current_host_authority() {
        Some(authority) if authority.allows(HostAuthority::Manage) => Ok(()),
        _ => Err(Error::Forbidden(
            "Host-global manage authority is required to apply MCP scaffolds".to_string(),
        )),
    }
}

/// Resolve the configured workspace on the blocking pool. The owner crate
/// performs canonicalization and containment checks; no synchronous filesystem
/// work should run on an async request worker.
pub async fn authorize_mcp_scaffold_workspace(requested_root: &str) -> Result<String> {
    require_host_scaffold_manage_authority()?;

    let requested_root = requested_root.to_owned();
    tokio::task::spawn_blocking(move || {
        rustok_mcp::authorize_scaffold_workspace(&requested_root)
    })
    .await
    .map_err(|error| {
        Error::Message(format!(
            "MCP scaffold workspace authorization worker failed: {error}"
        ))
    })?
    .map_err(|error| Error::BadRequest(error.to_string()))
}

#[cfg(test)]
mod tests {
    use rustok_api::{HostAuthority, HostAuthorityContext};
    use uuid::Uuid;

    use crate::error::Error;

    #[tokio::test]
    async fn scaffold_apply_requires_host_global_manage_authority() {
        assert!(matches!(
            super::require_host_scaffold_manage_authority(),
            Err(Error::Forbidden(_))
        ));

        let read_authority = HostAuthorityContext::for_actor(
            HostAuthority::Read,
            Uuid::new_v4(),
        )
        .expect("read host authority");
        let read_result = crate::host_authority::with_host_authority_scope(
            Some(read_authority),
            async { super::require_host_scaffold_manage_authority() },
        )
        .await;
        assert!(matches!(read_result, Err(Error::Forbidden(_))));

        let manage_authority =
            HostAuthorityContext::manage(Uuid::new_v4()).expect("manage host authority");
        let manage_result = crate::host_authority::with_host_authority_scope(
            Some(manage_authority),
            async { super::require_host_scaffold_manage_authority() },
        )
        .await;
        assert!(manage_result.is_ok());
    }
}
