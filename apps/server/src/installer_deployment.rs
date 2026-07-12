//! Server-owned adapter from installer role requests to build and release execution.

use std::sync::Arc;

use rustok_build::{
    BuildRequest, BuildRuntimeMode, BuildService, BuildStatus, NoopBuildEventPublisher,
    ReleaseStatus,
};
use rustok_installer::{
    InstallDeploymentPort, InstallExecutionError, InstallRole, InstallRoleDeployment,
    InstallRoleDeploymentRequest,
};
use sea_orm::DatabaseConnection;

use crate::common::settings::BuildRuntimeSettings;
use crate::modules::ManifestManager;
use crate::services::build_executor::build_execution_service_for_database;
use crate::services::platform_composition::PlatformCompositionService;
use crate::services::release_activation_hook::ServerReleaseActivationHook;
use crate::services::release_backend::ReleaseDeploymentService;

/// Host adapter that turns a distributed installer role into one active release.
#[derive(Clone)]
pub struct ServerInstallerDeploymentAdapter {
    build_settings: BuildRuntimeSettings,
}

impl ServerInstallerDeploymentAdapter {
    pub fn new(build_settings: BuildRuntimeSettings) -> Self {
        Self { build_settings }
    }
}

#[async_trait::async_trait]
impl InstallDeploymentPort<DatabaseConnection> for ServerInstallerDeploymentAdapter {
    fn supports_distributed_deployment(&self) -> bool {
        self.build_settings.enabled
    }

    async fn deploy_role(
        &self,
        runtime: &DatabaseConnection,
        request: InstallRoleDeploymentRequest,
    ) -> Result<InstallRoleDeployment, InstallExecutionError> {
        if !self.build_settings.enabled {
            return Err(InstallExecutionError::new(
                "distributed installer deployment requires rustok.build.enabled",
            ));
        }
        let runtime_mode = runtime_mode_for_role(request.role)?;
        let snapshot = PlatformCompositionService::active_snapshot(runtime)
            .await
            .map_err(execution_error)?;
        let role_plan = ManifestManager::role_build_plan(&snapshot.manifest, runtime_mode);
        let manifest_snapshot =
            PlatformCompositionService::manifest_snapshot_json(&snapshot.manifest)
                .map_err(execution_error)?;
        let build_service = BuildService::with_runtime(
            runtime.clone(),
            Arc::new(NoopBuildEventPublisher),
            Arc::new(ServerReleaseActivationHook::new(runtime.clone())),
        );
        let build = build_service
            .request_build(BuildRequest {
                manifest_ref: format!(
                    "installer:{}:{}:{}",
                    request.composition.hash,
                    snapshot.revision,
                    request.role.as_str(),
                ),
                manifest_revision: snapshot.revision,
                manifest_snapshot,
                artifact_identity: request.composition.hash.clone(),
                requested_by: format!("installer:session:{}", request.session_id),
                reason: Some(format!(
                    "distributed installer deployment for {} composition {}",
                    request.role.as_str(),
                    request.composition.revision,
                )),
                modules_delta: format!(
                    "installer distributed role {} composition {}",
                    request.role.as_str(),
                    request.composition.hash,
                ),
                modules: ManifestManager::build_modules(&snapshot.manifest),
                profile: role_plan.profile,
                execution_plan: role_plan.execution_plan,
            })
            .await
            .map_err(execution_error)?;
        let executor = build_execution_service_for_database(runtime.clone());
        if build.status != BuildStatus::Success {
            executor
                .execute_build(build.id, false)
                .await
                .map_err(execution_error)?;
        }
        let release = executor
            .ensure_release_for_build(build.id, request.environment.as_str(), false)
            .await
            .map_err(execution_error)?;
        let release = ReleaseDeploymentService::with_database(
            runtime.clone(),
            self.build_settings.deployment.clone(),
        )
        .publish_release(&release.id, true)
        .await
        .map_err(execution_error)?;
        if release.status != ReleaseStatus::Active {
            return Err(InstallExecutionError::new(format!(
                "deployment for role `{}` is not active yet",
                request.role.as_str(),
            )));
        }

        Ok(InstallRoleDeployment {
            role: request.role,
            composition: request.composition,
            build_id: build.id.to_string(),
            release_id: release.id.clone(),
            deployment_reference: format!("{}:{}", release.environment, runtime_mode.as_str(),),
        })
    }
}

fn runtime_mode_for_role(role: InstallRole) -> Result<BuildRuntimeMode, InstallExecutionError> {
    match role {
        InstallRole::Api => Ok(BuildRuntimeMode::Api),
        InstallRole::AdminSsr => Ok(BuildRuntimeMode::AdminSsr),
        InstallRole::StorefrontSsr => Ok(BuildRuntimeMode::StorefrontSsr),
        InstallRole::Worker => Ok(BuildRuntimeMode::Worker),
        InstallRole::Registry => Ok(BuildRuntimeMode::RegistryOnly),
        InstallRole::Monolith => Err(InstallExecutionError::new(
            "monolith role must not use the distributed deployment adapter",
        )),
    }
}

fn execution_error(error: impl std::fmt::Display) -> InstallExecutionError {
    InstallExecutionError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use rustok_build::BuildRuntimeMode;
    use rustok_installer::InstallRole;

    use super::runtime_mode_for_role;

    #[test]
    fn installer_roles_map_to_distinct_runtime_modes() {
        assert_eq!(
            runtime_mode_for_role(InstallRole::Api).unwrap(),
            BuildRuntimeMode::Api
        );
        assert_eq!(
            runtime_mode_for_role(InstallRole::Worker).unwrap(),
            BuildRuntimeMode::Worker
        );
        assert!(runtime_mode_for_role(InstallRole::Monolith).is_err());
    }
}
