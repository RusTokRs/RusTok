//! Host integration ports for build/release lifecycle side effects.

use async_trait::async_trait;

use crate::release::Model as Release;

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
