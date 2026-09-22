use std::sync::Arc;

use rustok_modules::{
    ModuleStaticDistributionExecutor, ModuleStaticDistributionExecutorError,
    ModuleStaticDistributionExecutorReadiness, ModuleStaticDistributionWorkItem,
};
use tonic::{Request, Response, Status};

use crate::static_distribution_proto::static_distribution_build_service_server::StaticDistributionBuildService;
use crate::static_distribution_proto::{
    ExecuteBuildRequest, ExecuteBuildResponse, ReadinessRequest, ReadinessResponse,
};

/// Worker-side adapter for a deployment-provided static-distribution CI
/// executor. This service does not own the queue, lease, or result persistence.
pub struct StaticDistributionGrpcService<E> {
    executor: Arc<E>,
}

impl<E> StaticDistributionGrpcService<E> {
    pub fn new(executor: Arc<E>) -> Self {
        Self { executor }
    }
}

#[tonic::async_trait]
impl<E> StaticDistributionBuildService for StaticDistributionGrpcService<E>
where
    E: ModuleStaticDistributionExecutor + ModuleStaticDistributionExecutorReadiness + 'static,
{
    async fn get_readiness(
        &self,
        _request: Request<ReadinessRequest>,
    ) -> Result<Response<ReadinessResponse>, Status> {
        Ok(Response::new(ReadinessResponse {
            ready: self.executor.is_ready(),
        }))
    }

    async fn execute_build(
        &self,
        request: Request<ExecuteBuildRequest>,
    ) -> Result<Response<ExecuteBuildResponse>, Status> {
        let work_item: ModuleStaticDistributionWorkItem =
            serde_json::from_slice(&request.into_inner().work_item_json)
                .map_err(|error| Status::invalid_argument(error.to_string()))?;
        work_item
            .validate()
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        let outcome = self
            .executor
            .execute(work_item)
            .await
            .map_err(executor_status)?;
        let payload =
            serde_json::to_vec(&outcome).map_err(|error| Status::internal(error.to_string()))?;
        Ok(Response::new(ExecuteBuildResponse {
            outcome_json: payload,
        }))
    }
}

fn executor_status(error: ModuleStaticDistributionExecutorError) -> Status {
    match error {
        ModuleStaticDistributionExecutorError::Transport(detail) => Status::unavailable(detail),
        ModuleStaticDistributionExecutorError::Rejected(detail) => {
            Status::failed_precondition(detail)
        }
    }
}
