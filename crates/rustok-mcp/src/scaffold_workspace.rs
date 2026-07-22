use std::path::{Path, PathBuf};

use alloy::storage::ScriptRegistry;

use crate::{
    AlloyMcpState, ApplyModuleScaffoldRequest, ApplyModuleScaffoldResponse,
    McpScaffoldDraftRuntimeContext,
};

pub const MCP_SCAFFOLD_WORKSPACE_ROOT_ENV: &str = "RUSTOK_MCP_SCAFFOLD_WORKSPACE_ROOT";

pub fn authorize_scaffold_workspace(requested_root: &str) -> Result<String, String> {
    let configured = std::env::var(MCP_SCAFFOLD_WORKSPACE_ROOT_ENV).map_err(|_| {
        format!(
            "MCP scaffold apply is disabled; host must configure {MCP_SCAFFOLD_WORKSPACE_ROOT_ENV}"
        )
    })?;
    authorize_scaffold_workspace_with_config(requested_root, &configured)
}

pub async fn apply_authorized_module_scaffold<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    context: Option<McpScaffoldDraftRuntimeContext>,
    mut request: ApplyModuleScaffoldRequest,
) -> Result<ApplyModuleScaffoldResponse, String> {
    request.workspace_root = authorize_scaffold_workspace(&request.workspace_root)?;
    crate::alloy_tools_unchecked::alloy_apply_module_scaffold(state, context, request).await
}

fn authorize_scaffold_workspace_with_config(
    requested_root: &str,
    configured_root: &str,
) -> Result<String, String> {
    let requested_root = requested_root.trim();
    let configured_root = configured_root.trim();
    if requested_root.is_empty() {
        return Err("workspace_root is required for MCP scaffold apply".to_string());
    }
    if configured_root.is_empty() {
        return Err(format!(
            "{MCP_SCAFFOLD_WORKSPACE_ROOT_ENV} must not be empty"
        ));
    }

    let configured = canonical_workspace_root(Path::new(configured_root), "configured")?;
    let requested = canonical_workspace_root(Path::new(requested_root), "requested")?;
    if requested != configured {
        return Err("Requested MCP scaffold workspace is not authorized by the host".to_string());
    }

    Ok(configured.to_string_lossy().into_owned())
}

fn canonical_workspace_root(path: &Path, source: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "{source} MCP scaffold workspace root must be absolute"
        ));
    }

    let canonical = std::fs::canonicalize(path).map_err(|error| {
        format!(
            "Failed to resolve {source} MCP scaffold workspace root `{}`: {error}",
            path.display()
        )
    })?;
    if !canonical.join("Cargo.toml").is_file() {
        return Err(format!(
            "{source} MCP scaffold workspace root `{}` is missing Cargo.toml",
            canonical.display()
        ));
    }

    let crates_path = canonical.join("crates");
    let canonical_crates = std::fs::canonicalize(&crates_path).map_err(|error| {
        format!(
            "Failed to resolve MCP scaffold crates directory `{}`: {error}",
            crates_path.display()
        )
    })?;
    if !canonical_crates.is_dir() || !canonical_crates.starts_with(&canonical) {
        return Err(
            "MCP scaffold crates directory must be contained by the configured workspace"
                .to_string(),
        );
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::authorize_scaffold_workspace_with_config;
    use std::path::PathBuf;

    fn workspace(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("rustok-mcp-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("crates")).expect("crates directory");
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("workspace manifest");
        root
    }

    #[test]
    fn only_configured_workspace_is_authorized() {
        let configured = workspace("configured");
        let other = workspace("other");

        let authorized = authorize_scaffold_workspace_with_config(
            configured.to_str().unwrap(),
            configured.to_str().unwrap(),
        )
        .expect("configured workspace");
        assert_eq!(
            PathBuf::from(authorized),
            std::fs::canonicalize(&configured).unwrap()
        );
        assert!(
            authorize_scaffold_workspace_with_config(
                other.to_str().unwrap(),
                configured.to_str().unwrap(),
            )
            .is_err()
        );

        let _ = std::fs::remove_dir_all(configured);
        let _ = std::fs::remove_dir_all(other);
    }

    #[test]
    fn relative_workspace_paths_are_rejected() {
        assert!(authorize_scaffold_workspace_with_config(".", ".").is_err());
    }
}
