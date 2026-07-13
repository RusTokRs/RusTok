use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub const MCP_SCAFFOLD_WORKSPACE_ROOT_ENV: &str = "RUSTOK_MCP_SCAFFOLD_WORKSPACE_ROOT";

/// Resolve the only host-authorized workspace root for MCP scaffold writes.
///
/// Apply is disabled unless the host explicitly configures a workspace. The
/// caller-supplied path is treated only as a confirmation value and must
/// canonicalize to the configured root. Callers must pass the returned
/// canonical path to the filesystem write boundary.
pub fn authorize_mcp_scaffold_workspace(requested_root: &str) -> Result<String> {
    let requested_root = requested_root.trim();
    if requested_root.is_empty() {
        return Err(Error::BadRequest(
            "workspace_root is required for MCP scaffold apply".to_string(),
        ));
    }

    let configured = std::env::var(MCP_SCAFFOLD_WORKSPACE_ROOT_ENV).map_err(|_| {
        Error::BadRequest(format!(
            "MCP scaffold apply is disabled; host must configure {MCP_SCAFFOLD_WORKSPACE_ROOT_ENV}"
        ))
    })?;
    let configured = canonical_workspace_root(Path::new(configured.trim()), "configured")?;
    let requested = canonical_workspace_root(Path::new(requested_root), "requested")?;

    if requested != configured {
        return Err(Error::Unauthorized(
            "Requested MCP scaffold workspace is not authorized by the host".to_string(),
        ));
    }

    Ok(configured.to_string_lossy().into_owned())
}

fn canonical_workspace_root(path: &Path, source: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::BadRequest(format!(
            "{source} MCP scaffold workspace root must be absolute"
        )));
    }

    let canonical = std::fs::canonicalize(path).map_err(|error| {
        Error::BadRequest(format!(
            "Failed to resolve {source} MCP scaffold workspace root `{}`: {error}",
            path.display()
        ))
    })?;
    if !canonical.join("Cargo.toml").is_file() {
        return Err(Error::BadRequest(format!(
            "{source} MCP scaffold workspace root `{}` is missing Cargo.toml",
            canonical.display()
        )));
    }

    let crates_path = canonical.join("crates");
    let canonical_crates = std::fs::canonicalize(&crates_path).map_err(|error| {
        Error::BadRequest(format!(
            "Failed to resolve MCP scaffold crates directory `{}`: {error}",
            crates_path.display()
        ))
    })?;
    if !canonical_crates.is_dir() || !canonical_crates.starts_with(&canonical) {
        return Err(Error::BadRequest(
            "MCP scaffold crates directory must be a directory contained by the configured workspace"
                .to_string(),
        ));
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::{authorize_mcp_scaffold_workspace, MCP_SCAFFOLD_WORKSPACE_ROOT_ENV};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn workspace(name: &str) -> tempfile::TempDir {
        let root = tempfile::Builder::new()
            .prefix(name)
            .tempdir()
            .expect("temporary workspace");
        std::fs::write(root.path().join("Cargo.toml"), "[workspace]\n")
            .expect("workspace manifest");
        std::fs::create_dir(root.path().join("crates")).expect("crates directory");
        root
    }

    #[test]
    fn scaffold_apply_is_disabled_without_host_configuration() {
        let _guard = ENV_LOCK.lock().expect("environment lock");
        std::env::remove_var(MCP_SCAFFOLD_WORKSPACE_ROOT_ENV);
        let root = workspace("mcp-unconfigured");
        assert!(authorize_mcp_scaffold_workspace(root.path().to_str().unwrap()).is_err());
    }

    #[test]
    fn requested_workspace_must_match_configured_root() {
        let _guard = ENV_LOCK.lock().expect("environment lock");
        let configured = workspace("mcp-configured");
        let other = workspace("mcp-other");
        std::env::set_var(MCP_SCAFFOLD_WORKSPACE_ROOT_ENV, configured.path());

        assert!(authorize_mcp_scaffold_workspace(other.path().to_str().unwrap()).is_err());
        let authorized = authorize_mcp_scaffold_workspace(configured.path().to_str().unwrap())
            .expect("configured workspace should be allowed");
        assert_eq!(
            std::path::PathBuf::from(authorized),
            std::fs::canonicalize(configured.path()).unwrap()
        );

        std::env::remove_var(MCP_SCAFFOLD_WORKSPACE_ROOT_ENV);
    }
}