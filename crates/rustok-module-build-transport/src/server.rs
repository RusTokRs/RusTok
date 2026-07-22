use std::sync::Arc;

use rustok_modules::{ModuleBuildRequest, ModuleBuildWorker, ModuleBuildWorkerReadiness};
use tonic::{Request, Response, Status};

use crate::module_build_proto::module_build_service_server::ModuleBuildService;
use crate::module_build_proto::{
    ExecuteBuildRequest, ExecuteBuildResponse, ReadinessRequest, ReadinessResponse,
};

/// Worker-side gRPC adapter. The deployment supplies the isolated executor;
/// this transport maps only the Rust-owned request/result protocol to mTLS RPC.
pub struct ModuleBuildGrpcService<W> {
    worker: Arc<W>,
}

impl<W> ModuleBuildGrpcService<W> {
    pub fn new(worker: Arc<W>) -> Self {
        Self { worker }
    }
}

#[tonic::async_trait]
impl<W> ModuleBuildService for ModuleBuildGrpcService<W>
where
    W: ModuleBuildWorker + ModuleBuildWorkerReadiness + 'static,
{
    async fn get_readiness(
        &self,
        _request: Request<ReadinessRequest>,
    ) -> Result<Response<ReadinessResponse>, Status> {
        Ok(Response::new(ReadinessResponse {
            ready: self.worker.is_ready(),
        }))
    }

    async fn execute_build(
        &self,
        request: Request<ExecuteBuildRequest>,
    ) -> Result<Response<ExecuteBuildResponse>, Status> {
        let request: ModuleBuildRequest =
            serde_json::from_slice(&request.into_inner().module_build_request_json)
                .map_err(|error| Status::invalid_argument(error.to_string()))?;
        let result = self
            .worker
            .execute_build(request)
            .await
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        let payload =
            serde_json::to_vec(&result).map_err(|error| Status::internal(error.to_string()))?;
        Ok(Response::new(ExecuteBuildResponse {
            module_build_result_json: payload,
        }))
    }
}
