use std::collections::HashSet;
use std::path::{Component, Path};

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
/// performs root canonicalization and containment checks; no synchronous
/// filesystem work should run on an async request worker.
pub async fn authorize_mcp_scaffold_workspace(requested_root: &str) -> Result<String> {
    require_host_scaffold_manage_authority()?;

    let requested_root = requested_root.to_owned();
    tokio::task::spawn_blocking(move || {
        rustok_mcp::authorize_scaffold_workspace(&requested_root)
    })
    .await
    .map_err(|error| {
        tracing::error!(
            %error,
            "MCP scaffold workspace authorization worker failed"
        );
        Error::Message("MCP scaffold workspace authorization failed".to_string())
    })?
    .map_err(|error| Error::BadRequest(error.to_string()))
}

/// Validate the persisted scaffold target before any files are written.
///
/// The underlying scaffold crate receives a relative crate path and writes it
/// below the workspace root. Reject unexpected layouts and every existing
/// symlink component (including dangling target symlinks) so that joining the
/// path cannot escape the host-authorized workspace.
pub async fn validate_mcp_scaffold_target_path(
    workspace_root: &str,
    crate_name: &str,
    crate_path: &str,
    file_paths: &[String],
) -> Result<()> {
    let workspace_root = workspace_root.to_owned();
    let crate_name = crate_name.to_owned();
    let crate_path = crate_path.to_owned();
    let file_paths = file_paths.to_vec();

    tokio::task::spawn_blocking(move || {
        validate_mcp_scaffold_target_path_blocking(
            &workspace_root,
            &crate_name,
            &crate_path,
            &file_paths,
        )
    })
    .await
    .map_err(|error| {
        tracing::error!(
            %error,
            "MCP scaffold target path validation worker failed"
        );
        Error::Message("MCP scaffold target path validation failed".to_string())
    })?
}

fn validate_mcp_scaffold_target_path_blocking(
    workspace_root: &str,
    crate_name: &str,
    crate_path: &str,
    file_paths: &[String],
) -> Result<()> {
    let name_path = Path::new(crate_name);
    let mut name_components = name_path.components();
    if !matches!(name_components.next(), Some(Component::Normal(_)))
        || name_components.next().is_some()
    {
        return Err(Error::BadRequest(
            "MCP scaffold crate name must be a single path component".to_string(),
        ));
    }

    let expected_relative_path = Path::new("crates").join("modules").join(crate_name);
    if Path::new(crate_path) != expected_relative_path.as_path() {
        return Err(Error::BadRequest(
            "MCP scaffold target must be crates/modules/<crate_name> within the authorized workspace"
                .to_string(),
        ));
    }

    if file_paths.is_empty() {
        return Err(Error::BadRequest(
            "MCP scaffold preview must contain at least one file".to_string(),
        ));
    }
    let mut seen_file_paths = HashSet::with_capacity(file_paths.len());
    for file_path in file_paths {
        let path = Path::new(file_path);
        if path.as_os_str().is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(Error::BadRequest(
                "MCP scaffold file paths must be relative and cannot traverse parent directories"
                    .to_string(),
            ));
        }
        if !seen_file_paths.insert(file_path.as_str()) {
            return Err(Error::BadRequest(
                "MCP scaffold preview contains duplicate file paths".to_string(),
            ));
        }
    }

    let canonical_root = std::fs::canonicalize(workspace_root).map_err(|error| {
        tracing::error!(%error, "Failed to canonicalize authorized MCP scaffold workspace");
        Error::InternalServerError
    })?;
    let components: Vec<_> = expected_relative_path.components().collect();
    let mut current = canonical_root.clone();

    for (index, component) in components.iter().enumerate() {
        let Component::Normal(segment) = *component else {
            return Err(Error::BadRequest(
                "MCP scaffold target contains an unsafe path component".to_string(),
            ));
        };
        current.push(segment);

        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                tracing::error!(
                    %error,
                    path = %current.display(),
                    "Failed to inspect MCP scaffold target path"
                );
                return Err(Error::InternalServerError);
            }
        };

        if metadata.file_type().is_symlink() {
            return Err(Error::BadRequest(
                "MCP scaffold target cannot traverse symbolic links".to_string(),
            ));
        }
        if index + 1 < components.len() && !metadata.is_dir() {
            return Err(Error::BadRequest(
                "MCP scaffold target parent components must be directories".to_string(),
            ));
        }

        let canonical_component = std::fs::canonicalize(&current).map_err(|error| {
            tracing::error!(
                %error,
                path = %current.display(),
                "Failed to canonicalize MCP scaffold target component"
            );
            Error::InternalServerError
        })?;
        if !canonical_component.starts_with(&canonical_root) {
            return Err(Error::BadRequest(
                "MCP scaffold target escapes the authorized workspace".to_string(),
            ));
        }
        current = canonical_component;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rustok_api::{HostAuthority, HostAuthorityContext};
    use std::path::PathBuf;
    use uuid::Uuid;

    use crate::error::Error;

    fn workspace(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rustok-mcp-scaffold-{label}-{}",
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(root.join("crates")).expect("crates directory");
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("workspace manifest");
        root
    }

    #[test]
    fn scaffold_target_rejects_path_traversal_and_unexpected_layout() {
        let root = workspace("path");
        let root_string = root.to_string_lossy().into_owned();

        assert!(super::validate_mcp_scaffold_target_path_blocking(
            &root_string,
            "rustok-example",
            "crates/modules/rustok-example",
            &["Cargo.toml".to_string(), "src/lib.rs".to_string()],
        )
        .is_ok());
        assert!(matches!(
            super::validate_mcp_scaffold_target_path_blocking(
                &root_string,
                "../outside",
                "crates/modules/../outside",
                &["Cargo.toml".to_string()],
            ),
            Err(Error::BadRequest(_))
        ));
        assert!(matches!(
            super::validate_mcp_scaffold_target_path_blocking(
                &root_string,
                "rustok-example",
                "../outside",
                &["Cargo.toml".to_string()],
            ),
            Err(Error::BadRequest(_))
        ));

        assert!(matches!(
            super::validate_mcp_scaffold_target_path_blocking(
                &root_string,
                "rustok-example",
                "crates/modules/rustok-example",
                &["Cargo.toml".to_string(), "Cargo.toml".to_string()],
            ),
            Err(Error::BadRequest(_))
        ));

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn scaffold_target_rejects_symlinked_and_dangling_target_components() {
        let root = workspace("symlink");
        let root_string = root.to_string_lossy().into_owned();
        let modules = root.join("crates/modules");
        let outside = std::env::temp_dir().join(format!(
            "rustok-mcp-scaffold-outside-{}",
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(&outside).expect("outside directory");

        std::os::unix::fs::symlink(&outside, &modules).expect("modules symlink");
        assert!(matches!(
            super::validate_mcp_scaffold_target_path_blocking(
                &root_string,
                "rustok-example",
                "crates/modules/rustok-example",
                &["Cargo.toml".to_string()],
            ),
            Err(Error::BadRequest(_))
        ));

        std::fs::remove_file(&modules).expect("remove modules symlink");
        std::fs::create_dir_all(&modules).expect("modules directory");
        std::os::unix::fs::symlink(
            root.join("missing-target"),
            modules.join("rustok-example"),
        )
        .expect("dangling target symlink");
        assert!(matches!(
            super::validate_mcp_scaffold_target_path_blocking(
                &root_string,
                "rustok-example",
                "crates/modules/rustok-example",
                &["Cargo.toml".to_string()],
            ),
            Err(Error::BadRequest(_))
        ));

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside);
    }

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
