use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PlatformBuildSnapshot, PlatformReleaseSnapshot};
use rustok_build::{
    BuildControl, BuildRollbackCommand, BuildService, SharedBuildControl, build_snapshot,
    release_snapshot,
};

use crate::services::build_event_hub::{
    BuildEventHubPublisher, CompositeBuildEventPublisher, build_event_hub_from_context,
};
use crate::services::event_bus::event_bus_from_context;
use crate::services::server_runtime_context::ServerRuntimeContext;

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

    fn rollback_service(&self, tenant_id: uuid::Uuid) -> BuildService {
        BuildService::with_event_publisher(
            self.runtime.db_clone(),
            Arc::new(CompositeBuildEventPublisher::new(vec![
                Arc::new(BuildEventHubPublisher::new(build_event_hub_from_context(
                    &self.runtime,
                ))),
                Arc::new(rustok_build::EventBusBuildEventPublisher::new(
                    event_bus_from_context(&self.runtime),
                    tenant_id,
                )),
            ])),
        )
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

    async fn active_release(&self) -> anyhow::Result<Option<PlatformReleaseSnapshot>> {
        Ok(self
            .read_service()
            .active_release()
            .await?
            .as_ref()
            .map(release_snapshot))
    }

    async fn list_releases_page(
        &self,
        limit: u64,
        offset: u64,
    ) -> anyhow::Result<Vec<PlatformReleaseSnapshot>> {
        Ok(self
            .read_service()
            .list_releases_page(limit, offset)
            .await?
            .iter()
            .map(release_snapshot)
            .collect())
    }

    async fn rollback_build(
        &self,
        command: BuildRollbackCommand,
    ) -> anyhow::Result<PlatformBuildSnapshot> {
        tracing::info!(
            build_id = %command.build_id,
            tenant_id = %command.tenant_id,
            actor_id = %command.actor_id,
            "rolling back platform build through host build control"
        );
        let build = self
            .rollback_service(command.tenant_id)
            .rollback_build(command)
            .await?;
        Ok(build_snapshot(&build))
    }
}
