//! Host-composed read-only build-history port for transports.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PlatformBuildProgressSnapshot, PlatformBuildSnapshot};
use uuid::Uuid;

#[async_trait]
pub trait BuildControl: Send + Sync {
    async fn active_build(&self) -> anyhow::Result<Option<PlatformBuildSnapshot>>;

    async fn list_builds_page(
        &self,
        limit: u64,
        offset: u64,
    ) -> anyhow::Result<Vec<PlatformBuildSnapshot>>;

    /// Current global watermark for durable build-state snapshots.
    async fn current_progress_sequence(&self) -> anyhow::Result<i64>;

    /// Current queued/running snapshots for a newly connected subscriber.
    async fn active_build_progress(
        &self,
    ) -> anyhow::Result<Vec<PlatformBuildProgressSnapshot>>;

    /// Snapshot for one build, including terminal builds.
    async fn build_progress(
        &self,
        build_id: Uuid,
    ) -> anyhow::Result<Option<PlatformBuildProgressSnapshot>>;

    /// Latest state snapshots for builds changed after a global cursor.
    /// Intermediate updates to one build may be coalesced to its newest row.
    async fn build_progress_after(
        &self,
        sequence: i64,
        limit: u64,
    ) -> anyhow::Result<Vec<PlatformBuildProgressSnapshot>>;
}

#[derive(Clone)]
pub struct SharedBuildControl(pub Arc<dyn BuildControl>);

impl std::fmt::Debug for SharedBuildControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedBuildControl").finish()
    }
}
