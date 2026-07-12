//! Host integration ports for build/release lifecycle side effects.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::release::Model as Release;

/// Deployment backend selected by an executable host or operational CLI.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentBackend {
    #[default]
    RecordOnly,
    Filesystem,
    Http,
    Container,
}

/// Serializable release-publication settings shared by executable hosts.
///
/// Secret resolution and host-specific process execution remain outside this
/// contract. A host passes the already resolved HTTP bearer token when needed.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeploymentSettings {
    #[serde(default)]
    pub backend: DeploymentBackend,
    #[serde(default = "default_filesystem_root_dir")]
    pub filesystem_root_dir: String,
    #[serde(default)]
    pub public_base_url: Option<String>,
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default)]
    pub bearer_token: Option<String>,
    #[serde(default = "default_docker_bin")]
    pub docker_bin: String,
    #[serde(default)]
    pub image_repository: Option<String>,
    #[serde(default)]
    pub rollout_command: Option<String>,
}

impl Default for DeploymentSettings {
    fn default() -> Self {
        Self {
            backend: DeploymentBackend::RecordOnly,
            filesystem_root_dir: default_filesystem_root_dir(),
            public_base_url: None,
            endpoint_url: None,
            bearer_token: None,
            docker_bin: default_docker_bin(),
            image_repository: None,
            rollout_command: None,
        }
    }
}

fn default_filesystem_root_dir() -> String {
    "artifacts/releases".to_string()
}

fn default_docker_bin() -> String {
    "docker".to_string()
}

/// Filesystem locations supplied by an executable host for release publication.
///
/// Build and release persistence must not infer a repository layout from the
/// crate that composes a publisher. Container publication receives its runtime
/// assets explicitly for the same reason.
#[derive(Debug, Clone)]
pub struct DeploymentWorkspace {
    root: PathBuf,
    migration_dir: Option<PathBuf>,
    config_dir: Option<PathBuf>,
}

impl DeploymentWorkspace {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            migration_dir: None,
            config_dir: None,
        }
    }

    pub fn with_runtime_assets(
        mut self,
        migration_dir: impl Into<PathBuf>,
        config_dir: impl Into<PathBuf>,
    ) -> Self {
        self.migration_dir = Some(migration_dir.into());
        self.config_dir = Some(config_dir.into());
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn migration_dir(&self) -> Option<&Path> {
        self.migration_dir.as_deref()
    }

    pub fn config_dir(&self) -> Option<&Path> {
        self.config_dir.as_deref()
    }
}

/// Typed request for a host-owned release publication operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasePublishRequest {
    pub release_id: String,
    pub activate: bool,
}

/// Host adapter boundary for release artifact publication and deployment.
///
/// `rustok-build` owns release persistence; the host decides whether a release
/// is recorded, copied to a filesystem, sent to a remote endpoint, or rolled
/// out as a container. Installer and CLI orchestration must consume this port
/// rather than run deployment commands themselves.
#[async_trait]
pub trait ReleasePublisherPort: Send + Sync {
    async fn publish_release(&self, request: ReleasePublishRequest) -> anyhow::Result<Release>;
}

/// Executes host-owned work after a release becomes active.
///
/// Build persistence and state transitions remain in `rustok-build`; host
/// integration such as OAuth connection synchronization or active-release
/// projection is supplied explicitly by the runtime.
#[async_trait]
pub trait ReleaseActivationHook: Send + Sync {
    async fn after_release_activated(&self, release: &Release) -> anyhow::Result<()>;
}

#[derive(Default)]
pub struct NoopReleaseActivationHook;

#[async_trait]
impl ReleaseActivationHook for NoopReleaseActivationHook {
    async fn after_release_activated(&self, _release: &Release) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{DeploymentBackend, DeploymentSettings, DeploymentWorkspace};

    #[test]
    fn deployment_settings_default_to_record_only() {
        let settings = DeploymentSettings::default();

        assert_eq!(settings.backend, DeploymentBackend::RecordOnly);
        assert_eq!(settings.filesystem_root_dir, "artifacts/releases");
        assert_eq!(settings.docker_bin, "docker");
    }

    #[test]
    fn deployment_workspace_requires_host_supplied_paths() {
        let workspace = DeploymentWorkspace::new("C:/workspace").with_runtime_assets(
            "C:/workspace/apps/server/migration",
            "C:/workspace/apps/server/config",
        );

        assert_eq!(workspace.root(), Path::new("C:/workspace"));
        assert_eq!(
            workspace.migration_dir(),
            Some(Path::new("C:/workspace/apps/server/migration"))
        );
        assert_eq!(
            workspace.config_dir(),
            Some(Path::new("C:/workspace/apps/server/config"))
        );
    }
}
