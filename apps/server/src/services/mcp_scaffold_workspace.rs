use std::collections::HashSet;
use std::fs::OpenOptions;
use std::path::{Component, Path};

use uuid::Uuid;

use rustok_api::HostAuthority;

use crate::error::{Error, Result};

pub use rustok_mcp::MCP_SCAFFOLD_WORKSPACE_ROOT_ENV;

const MCP_SCAFFOLD_STAGING_PREFIX: &str = ".rustok-mcp-scaffold-staging-";
const MCP_SCAFFOLD_STAGING_MARKER: &str = ".rustok-mcp-scaffold-staging";
const MCP_SCAFFOLD_STAGING_MARKER_VERSION: &str = "rustok-mcp-scaffold-staging-v1";

/// An exclusive OS lock shared with the native xtask scaffold transaction.
/// Keep the file handle alive for the complete filesystem apply operation.
pub struct McpScaffoldWorkspaceLock {
    _file: std::fs::File,
}

/// Acquire the permanent workspace scaffold lock before claiming a persisted
/// draft. This coordinates MCP applies with native xtask scaffolding and
/// refuses to write while native scaffold recovery is pending.
pub async fn lock_mcp_scaffold_workspace(
    workspace_root: &str,
) -> Result<McpScaffoldWorkspaceLock> {
    let workspace_root = workspace_root.to_owned();
    tokio::task::spawn_blocking(move || {
        lock_mcp_scaffold_workspace_blocking(&workspace_root)
    })
    .await
    .map_err(|error| {
        tracing::error!(%error, "MCP scaffold workspace lock worker failed");
        Error::Message("MCP scaffold workspace lock failed".to_string())
    })?
}

fn lock_mcp_scaffold_workspace_blocking(
    workspace_root: &str,
) -> Result<McpScaffoldWorkspaceLock> {
    let root = std::fs::canonicalize(workspace_root).map_err(|error| {
        tracing::error!(%error, "Failed to canonicalize MCP scaffold workspace for locking");
        Error::InternalServerError
    })?;
    let control_dir = root.join(".rustok");
    ensure_control_directory(&control_dir, "MCP scaffold control directory")?;
    let state_dir = control_dir.join("scaffold");
    ensure_control_directory(&state_dir, "MCP scaffold state directory")?;

    let lock_path = state_dir.join("lock");
    match std::fs::symlink_metadata(&lock_path) {
        Ok(metadata)
            if metadata.is_file() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(Error::BadRequest(
                "Native scaffold lock path must be a regular file".to_string(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            tracing::error!(%error, path = %lock_path.display(), "Failed to inspect native scaffold lock");
            return Err(Error::InternalServerError);
        }
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| {
            tracing::error!(%error, path = %lock_path.display(), "Failed to open native scaffold lock");
            Error::InternalServerError
        })?;

    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            return Err(Error::BadRequest(
                "Scaffold workspace is busy; another MCP or native scaffold operation holds the workspace lock"
                    .to_string(),
            ));
        }
        Err(std::fs::TryLockError::Error(error)) => {
            tracing::error!(%error, path = %lock_path.display(), "Failed to acquire native scaffold lock");
            return Err(Error::InternalServerError);
        }
    }

    for marker in ["preparing", "active", "retired"] {
        let marker_path = state_dir.join(marker);
        match std::fs::symlink_metadata(&marker_path) {
            Ok(_) => {
                return Err(Error::BadRequest(format!(
                    "Native scaffold recovery is pending at {}; run cargo xtask scaffold --recover before applying an MCP scaffold",
                    state_dir.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::error!(
                    %error,
                    path = %marker_path.display(),
                    "Failed to inspect native scaffold recovery state"
                );
                return Err(Error::InternalServerError);
            }
        }
    }

    Ok(McpScaffoldWorkspaceLock { _file: file })
}

fn ensure_control_directory(path: &Path, label: &str) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(Error::BadRequest(format!("{label} cannot be a symbolic link")));
        }
        Ok(metadata) if metadata.is_dir() => return Ok(()),
        Ok(_) => return Err(Error::BadRequest(format!("{label} must be a directory"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::create_dir(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    tracing::error!(%error, path = %path.display(), "Failed to create MCP scaffold control directory");
                    return Err(Error::InternalServerError);
                }
            }
        }
        Err(error) => {
            tracing::error!(%error, path = %path.display(), "Failed to inspect MCP scaffold control directory");
            return Err(Error::InternalServerError);
        }
    }

    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        tracing::error!(%error, path = %path.display(), "Failed to re-inspect MCP scaffold control directory");
        Error::InternalServerError
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::BadRequest(format!(
            "{label} must be a real directory, not a symbolic link"
        )));
    }
    Ok(())
}

/// Publish a complete scaffold directory with one same-filesystem rename.
/// All temporary writes stay in a uniquely named hidden sibling; the final
/// crate path is never exposed with partially written files.
pub fn apply_mcp_scaffold_atomically(
    draft: &rustok_mcp::StagedModuleScaffold,
    workspace_root: &str,
    _workspace_lock: McpScaffoldWorkspaceLock,
) -> std::result::Result<
    rustok_mcp::ApplyModuleScaffoldResponse,
    rustok_mcp::AlloyScaffoldError,
> {
    let workspace_root = Path::new(workspace_root);
    let modules_root = workspace_root.join("crates/modules");
    ensure_apply_directory(&modules_root)?;
    cleanup_stale_mcp_scaffold_staging_dirs(&modules_root)?;

    let final_target = workspace_root.join(&draft.preview.crate_path);
    if path_exists(&final_target)? {
        return Err(rustok_mcp::AlloyScaffoldError::TargetAlreadyExists(
            final_target.display().to_string(),
        ));
    }

    let staging_id = Uuid::new_v4().to_string();
    let staging_root = modules_root.join(format!("{MCP_SCAFFOLD_STAGING_PREFIX}{staging_id}"));
    std::fs::create_dir(&staging_root).map_err(|error| {
        rustok_mcp::AlloyScaffoldError::Workspace(format!(
            "Failed to create private MCP scaffold staging directory {}: {error}",
            staging_root.display()
        ))
    })?;

    let marker_path = staging_root.join(MCP_SCAFFOLD_STAGING_MARKER);
    let mut published = false;
    let result = (|| {
        let marker_content = format!("{MCP_SCAFFOLD_STAGING_MARKER_VERSION}\\n{staging_id}\\n");
        std::fs::write(&marker_path, marker_content).map_err(|error| {
            rustok_mcp::AlloyScaffoldError::Workspace(format!(
                "Failed to mark private MCP scaffold staging directory: {error}"
            ))
        })?;
        std::fs::write(staging_root.join("Cargo.toml"), "[workspace]\\n").map_err(|error| {
            rustok_mcp::AlloyScaffoldError::Workspace(format!(
                "Failed to initialize private MCP scaffold workspace: {error}"
            ))
        })?;
        std::fs::create_dir(staging_root.join("crates")).map_err(|error| {
            rustok_mcp::AlloyScaffoldError::Workspace(format!(
                "Failed to initialize private MCP scaffold crate root: {error}"
            ))
        })?;

        let staging_root_string = staging_root.to_string_lossy().into_owned();
        let response = rustok_mcp::apply_staged_scaffold(draft, &staging_root_string)?;
        let staged_target = staging_root.join(&draft.preview.crate_path);
        let staged_metadata = std::fs::symlink_metadata(&staged_target).map_err(|error| {
            rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                "Staged MCP scaffold target is missing or inaccessible: {error}"
            ))
        })?;
        if staged_metadata.file_type().is_symlink() || !staged_metadata.is_dir() {
            return Err(rustok_mcp::AlloyScaffoldError::FileWrite(
                "Staged MCP scaffold target is not a real directory".to_string(),
            ));
        }

        if path_exists(&final_target)? {
            return Err(rustok_mcp::AlloyScaffoldError::TargetAlreadyExists(
                final_target.display().to_string(),
            ));
        }
        if let Err(error) = std::fs::rename(&staged_target, &final_target) {
            if path_exists(&final_target)? {
                return Err(rustok_mcp::AlloyScaffoldError::TargetAlreadyExists(
                    final_target.display().to_string(),
                ));
            }
            return Err(rustok_mcp::AlloyScaffoldError::Workspace(format!(
                "Atomic MCP scaffold publish failed before the final crate directory was created: {error}"
            )));
        }
        published = true;

        std::fs::remove_dir_all(&staging_root).map_err(|error| {
            rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                "MCP scaffold was published at {}, but private staging cleanup failed at {}: {error}",
                final_target.display(),
                staging_root.display()
            ))
        })?;

        Ok(response)
    })();

    match result {
        Ok(response) => Ok(response),
        Err(error) if published => Err(error),
        Err(error) => match cleanup_mcp_scaffold_staging_dir(&staging_root) {
            Ok(()) => match error {
                rustok_mcp::AlloyScaffoldError::TargetAlreadyExists(_) => Err(error),
                other => Err(rustok_mcp::AlloyScaffoldError::Workspace(format!(
                    "MCP scaffold was not published; the private staging directory was removed: {other}"
                ))),
            },
            Err(cleanup_error) => Err(rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                "MCP scaffold was not published ({error}), and private staging cleanup failed ({cleanup_error})"
            ))),
        },
    }
}

fn ensure_apply_directory(path: &Path) -> std::result::Result<(), rustok_mcp::AlloyScaffoldError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(rustok_mcp::AlloyScaffoldError::Workspace(format!(
                "MCP scaffold parent cannot be a symbolic link: {}",
                path.display()
            )));
        }
        Ok(metadata) if metadata.is_dir() => return Ok(()),
        Ok(_) => {
            return Err(rustok_mcp::AlloyScaffoldError::Workspace(format!(
                "MCP scaffold parent is not a directory: {}",
                path.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::create_dir(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(rustok_mcp::AlloyScaffoldError::Workspace(format!(
                        "Failed to create MCP scaffold parent {}: {error}",
                        path.display()
                    )));
                }
            }
        }
        Err(error) => {
            return Err(rustok_mcp::AlloyScaffoldError::Workspace(format!(
                "Failed to inspect MCP scaffold parent {}: {error}",
                path.display()
            )));
        }
    }

    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        rustok_mcp::AlloyScaffoldError::Workspace(format!(
            "Failed to re-inspect MCP scaffold parent {}: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(rustok_mcp::AlloyScaffoldError::Workspace(format!(
            "MCP scaffold parent must be a real directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn path_exists(path: &Path) -> std::result::Result<bool, rustok_mcp::AlloyScaffoldError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(rustok_mcp::AlloyScaffoldError::Workspace(format!(
            "Failed to inspect MCP scaffold target {}: {error}",
            path.display()
        ))),
    }
}

fn cleanup_stale_mcp_scaffold_staging_dirs(
    modules_root: &Path,
) -> std::result::Result<(), rustok_mcp::AlloyScaffoldError> {
    let entries = std::fs::read_dir(modules_root).map_err(|error| {
        rustok_mcp::AlloyScaffoldError::Workspace(format!(
            "Failed to inspect MCP scaffold staging directory {}: {error}",
            modules_root.display()
        ))
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            rustok_mcp::AlloyScaffoldError::Workspace(format!(
                "Failed to read MCP scaffold staging entry: {error}"
            ))
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(staging_id) = name.strip_prefix(MCP_SCAFFOLD_STAGING_PREFIX) else {
            continue;
        };
        if Uuid::parse_str(staging_id).is_err() {
            return Err(rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                "Unrecognized private MCP scaffold staging path {}; preserve it for manual reconciliation",
                entry.path().display()
            )));
        }

        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                "Failed to inspect stale MCP scaffold staging path {}: {error}",
                path.display()
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                "Stale MCP scaffold staging path is not a real directory: {}",
                path.display()
            )));
        }

        let marker_path = path.join(MCP_SCAFFOLD_STAGING_MARKER);
        match std::fs::symlink_metadata(&marker_path) {
            Ok(marker_metadata)
                if marker_metadata.is_file() && !marker_metadata.file_type().is_symlink() =>
            {
                let marker = std::fs::read_to_string(&marker_path).map_err(|error| {
                    rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                        "Failed to read MCP scaffold staging marker {}: {error}",
                        marker_path.display()
                    ))
                })?;
                let expected = format!("{MCP_SCAFFOLD_STAGING_MARKER_VERSION}\\n{staging_id}\\n");
                if marker != expected {
                    return Err(rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                        "MCP scaffold staging marker is invalid at {}; preserve it for manual reconciliation",
                        path.display()
                    )));
                }
                cleanup_mcp_scaffold_staging_dir(&path)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut children = std::fs::read_dir(&path).map_err(|read_error| {
                    rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                        "Failed to inspect unmarked MCP scaffold staging directory {}: {read_error}",
                        path.display()
                    ))
                })?;
                if children.next().is_none() {
                    std::fs::remove_dir(&path).map_err(|remove_error| {
                        rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                            "Failed to remove empty MCP scaffold staging directory {}: {remove_error}",
                            path.display()
                        ))
                    })?;
                } else {
                    return Err(rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                        "Unmarked MCP scaffold staging directory is not empty at {}; preserve it for manual reconciliation",
                        path.display()
                    )));
                }
            }
            _ => {
                return Err(rustok_mcp::AlloyScaffoldError::FileWrite(format!(
                    "MCP scaffold staging marker is unsafe at {}; preserve it for manual reconciliation",
                    marker_path.display()
                )));
            }
        }
    }

    Ok(())
}

fn cleanup_mcp_scaffold_staging_dir(
    path: &Path,
) -> std::result::Result<(), rustok_mcp::AlloyScaffoldError> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(rustok_mcp::AlloyScaffoldError::FileWrite(format!(
            "Failed to clean private MCP scaffold staging directory {}: {error}",
            path.display()
        ))),
    }
}

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
