//! Installer-only composition for the trusted static build executor.

use std::{path::PathBuf, sync::Arc};

use crate::services::release_activation_hook::ServerReleaseActivationHook;

/// Creates the shared build executor for a database opened by the installer.
///
/// Installer deployment owns this trusted static build operation and cannot
/// depend on a pre-existing HTTP host context. Server runtime workers never
/// invoke this executor.
pub fn build_execution_service_for_database(
    db: sea_orm::DatabaseConnection,
) -> rustok_build::BuildExecutionService {
    rustok_build::BuildExecutionService::new(
        db.clone(),
        Arc::new(rustok_build::NoopBuildEventPublisher),
        Arc::new(ServerReleaseActivationHook::new(db)),
        workspace_root(),
    )
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .expect("workspace root should be resolvable from apps/server")
}

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_root_points_to_repo_root() {
        assert!(super::workspace_root().join("modules.toml").exists());
    }
}
