use std::sync::Arc;

use crate::services::server_runtime_context::ServerRuntimeContext;
use async_trait::async_trait;
use rustok_api::{PlatformBuildProgressSnapshot, PlatformBuildSnapshot};
use rustok_build::{
    BuildControl, BuildService, SharedBuildControl, build_progress_snapshot, build_snapshot,
};

#[derive(Clone)]
pub struct ServerBuildControl {
    db: sea_orm::DatabaseConnection,
}

impl ServerBuildControl {
    pub fn new(runtime: ServerRuntimeContext) -> Self {
        Self {
            db: runtime.db_clone(),
        }
    }

    pub fn shared(runtime: ServerRuntimeContext) -> SharedBuildControl {
        SharedBuildControl(Arc::new(Self::new(runtime)))
    }

    fn read_service(&self) -> BuildService {
        BuildService::new(self.db.clone())
    }
}

/// Retrieve the shared read-only build control port, installing the server
/// adapter once when a runtime context is assembled outside normal bootstrap.
pub fn build_control_from_context(ctx: &ServerRuntimeContext) -> SharedBuildControl {
    ctx.shared_get_or_insert_with(|| ServerBuildControl::shared(ctx.clone()))
}

#[async_trait]
impl BuildControl for ServerBuildControl {
    async fn active_build(&self) -> anyhow::Result<Option<PlatformBuildSnapshot>> {
        Ok(self
            .read_service()
            .active_build()
            .await?
            .as_ref()
            .map(build_snapshot))
    }

    async fn list_builds_page(
        &self,
        limit: u64,
        offset: u64,
    ) -> anyhow::Result<Vec<PlatformBuildSnapshot>> {
        Ok(self
            .read_service()
            .list_builds_page(limit, offset)
            .await?
            .iter()
            .map(build_snapshot)
            .collect())
    }

    async fn current_progress_sequence(&self) -> anyhow::Result<i64> {
        self.read_service().current_progress_sequence().await
    }

    async fn active_build_progress(
        &self,
    ) -> anyhow::Result<Vec<PlatformBuildProgressSnapshot>> {
        Ok(self
            .read_service()
            .active_build_progress()
            .await?
            .iter()
            .map(build_progress_snapshot)
            .collect())
    }

    async fn build_progress(
        &self,
        build_id: uuid::Uuid,
    ) -> anyhow::Result<Option<PlatformBuildProgressSnapshot>> {
        Ok(self
            .read_service()
            .build_progress(build_id)
            .await?
            .as_ref()
            .map(build_progress_snapshot))
    }

    async fn build_progress_after(
        &self,
        sequence: i64,
        limit: u64,
    ) -> anyhow::Result<Vec<PlatformBuildProgressSnapshot>> {
        Ok(self
            .read_service()
            .build_progress_after(sequence, limit)
            .await?
            .iter()
            .map(build_progress_snapshot)
            .collect())
    }
}
