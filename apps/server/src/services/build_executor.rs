//! Server composition for the host-independent build executor.

use std::{path::PathBuf, sync::Arc};

use crate::services::{
    build_event_hub::{build_event_hub_from_context, BuildEventHubPublisher},
    release_activation_hook::ServerReleaseActivationHook,
    server_runtime_context::ServerRuntimeContext,
};

pub fn build_execution_service(ctx: &ServerRuntimeContext) -> rustok_build::BuildExecutionService {
    build_execution_service_with_event_publisher(
        ctx,
        Arc::new(BuildEventHubPublisher::new(build_event_hub_from_context(
            ctx,
        ))),
    )
}

/// Creates the shared build executor for a database opened by the installer.
///
/// Installer deployment uses the same execution and release-activation path as
/// the runtime worker, but cannot depend on a pre-existing HTTP host context.
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

pub fn build_execution_service_with_event_publisher(
    ctx: &ServerRuntimeContext,
    event_publisher: Arc<dyn rustok_build::BuildEventPublisher>,
) -> rustok_build::BuildExecutionService {
    rustok_build::BuildExecutionService::new(
        ctx.db_clone(),
        event_publisher,
        Arc::new(ServerReleaseActivationHook::new(ctx.db_clone())),
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
