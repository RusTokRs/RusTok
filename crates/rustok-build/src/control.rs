//! Host-composed read-only build-history port for transports.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::PlatformBuildSnapshot;

#[async_trait]
pub trait BuildControl: Send + Sync {
    async fn active_build(&self) -> anyhow::Result<Option<PlatformBuildSnapshot>>;

    async fn list_builds_page(
        &self,
        limit: u64,
        offset: u64,
    ) -> anyhow::Result<Vec<PlatformBuildSnapshot>>;
}

#[derive(Clone)]
pub struct SharedBuildControl(pub Arc<dyn BuildControl>);
