//! Server boundary for owner-controlled distribution deployment.
//!
//! The former adapter compiled and activated one independent release per role.
//! That split the production release head and could not provide atomic bundle
//! rollback. The server now fails closed until the `rustok-modules` rollout
//! owner and external deployment controller are composed here.

use rustok_installer::{
    InstallDeploymentPort, InstallDistributionDeployment, InstallDistributionDeploymentRequest,
    InstallExecutionError,
};
use sea_orm::DatabaseConnection;

#[derive(Clone, Default)]
pub struct ServerInstallerDeploymentAdapter;

#[async_trait::async_trait]
impl InstallDeploymentPort<DatabaseConnection> for ServerInstallerDeploymentAdapter {
    fn supports_distribution_deployment(&self) -> bool {
        false
    }

    async fn deploy_distribution(
        &self,
        _runtime: &DatabaseConnection,
        _request: InstallDistributionDeploymentRequest,
    ) -> Result<InstallDistributionDeployment, InstallExecutionError> {
        Err(InstallExecutionError::new(
            "installer apply requires the owner-controlled distribution rollout adapter",
        ))
    }
}

#[cfg(test)]
mod tests {
    use rustok_installer::InstallDeploymentPort;

    use super::ServerInstallerDeploymentAdapter;

    #[test]
    fn unsafe_per_role_build_deployment_is_not_advertised() {
        assert!(
            !InstallDeploymentPort::<sea_orm::DatabaseConnection>::supports_distribution_deployment(
                &ServerInstallerDeploymentAdapter,
            )
        );
    }
}
