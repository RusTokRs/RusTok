//! Host integration ports for build/release lifecycle side effects.

use async_trait::async_trait;

use crate::release::Model as Release;

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
