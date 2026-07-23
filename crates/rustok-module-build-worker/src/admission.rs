use std::sync::Arc;

use rustok_worker_transport::WorkerAdmission;
use tonic::{Request, Response, Status};

use crate::RunnerGrpcService;
use crate::proto::runner_service_server::RunnerService;
use crate::proto::{
    GetReadinessRequest, GetReadinessResponse, StartBuildRequest, StartBuildResponse,
};

#[derive(Clone)]
pub struct AdmissionRunnerGrpcService {
    inner: Arc<RunnerGrpcService>,
    admission: WorkerAdmission,
}

impl AdmissionRunnerGrpcService {
    pub fn new(inner: RunnerGrpcService, admission: WorkerAdmission) -> Self {
        Self {
            inner: Arc::new(inner),
            admission,
        }
    }
}

#[tonic::async_trait]
impl RunnerService for AdmissionRunnerGrpcService {
    async fn start_build(
        &self,
        request: Request<StartBuildRequest>,
    ) -> Result<Response<StartBuildResponse>, Status> {
        let _permit = self.admission.acquire().await?;
        RunnerService::start_build(self.inner.as_ref(), request).await
    }

    async fn get_readiness(
        &self,
        request: Request<GetReadinessRequest>,
    ) -> Result<Response<GetReadinessResponse>, Status> {
        RunnerService::get_readiness(self.inner.as_ref(), request).await
    }
}
