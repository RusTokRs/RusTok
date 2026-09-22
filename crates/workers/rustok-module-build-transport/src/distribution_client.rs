use async_trait::async_trait;
use rustok_modules::{
    ModuleStaticDistributionCompletionOutcome, ModuleStaticDistributionExecutor,
    ModuleStaticDistributionExecutorError, ModuleStaticDistributionWorkItem,
};
use tokio::sync::Mutex;
use tonic::Request;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};

use crate::static_distribution_proto::static_distribution_build_service_client::StaticDistributionBuildServiceClient;
use crate::static_distribution_proto::{ExecuteBuildRequest, ReadinessRequest};

/// Owner-side adapter for a separately deployed static-distribution CI
/// executor. It has no in-process compilation fallback.
pub struct GrpcStaticDistributionExecutor {
    client: Mutex<StaticDistributionBuildServiceClient<Channel>>,
}

impl GrpcStaticDistributionExecutor {
    fn from_channel(channel: Channel) -> Self {
        Self {
            client: Mutex::new(StaticDistributionBuildServiceClient::new(channel)),
        }
    }

    /// Connects with deployment-provided mTLS identity, trust root, and worker
    /// domain. This is the only connection constructor.
    pub async fn connect_with_tls(
        endpoint: Endpoint,
        tls_config: ClientTlsConfig,
    ) -> Result<Self, String> {
        let channel = endpoint
            .tls_config(tls_config)
            .map_err(|error| error.to_string())?
            .connect()
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self::from_channel(channel))
    }

    /// Probes the authenticated listener after executor startup validation.
    pub async fn check_readiness(&self) -> Result<(), String> {
        let response = self
            .client
            .lock()
            .await
            .get_readiness(Request::new(ReadinessRequest {}))
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        if response.ready {
            Ok(())
        } else {
            Err("static distribution executor reported not ready".to_string())
        }
    }
}

#[async_trait]
impl ModuleStaticDistributionExecutor for GrpcStaticDistributionExecutor {
    async fn execute(
        &self,
        work_item: ModuleStaticDistributionWorkItem,
    ) -> Result<ModuleStaticDistributionCompletionOutcome, ModuleStaticDistributionExecutorError>
    {
        let payload = serde_json::to_vec(&work_item)
            .map_err(|error| ModuleStaticDistributionExecutorError::Transport(error.to_string()))?;
        let response = self
            .client
            .lock()
            .await
            .execute_build(Request::new(ExecuteBuildRequest {
                work_item_json: payload,
            }))
            .await
            .map_err(|error| ModuleStaticDistributionExecutorError::Transport(error.to_string()))?
            .into_inner();
        serde_json::from_slice(&response.outcome_json)
            .map_err(|error| ModuleStaticDistributionExecutorError::Transport(error.to_string()))
    }
}
