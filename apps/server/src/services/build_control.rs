use std::sync::Arc;

use crate::services::server_runtime_context::ServerRuntimeContext;
use async_trait::async_trait;
use rustok_api::PlatformBuildSnapshot;
use rustok_build::{BuildControl, BuildService, SharedBuildControl, build_snapshot};

#[derive(Clone)]
pub struct ServerBuildControl {
    runtime: ServerRuntimeContext,
}

impl ServerBuildControl {
    pub fn new(runtime: ServerRuntimeContext) -> Self {
        Self { runtime }
    }

    pub fn shared(runtime: ServerRuntimeContext) -> SharedBuildControl {
        SharedBuildControl(Arc::new(Self::new(runtime)))
    }

    fn read_service(&self) -> BuildService {
        BuildService::new(self.runtime.db_clone())
    }
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
}
