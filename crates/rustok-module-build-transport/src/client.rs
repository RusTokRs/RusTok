use async_trait::async_trait;
use rustok_modules::{
    ModuleBuildProtocolError, ModuleBuildRequest, ModuleBuildResult, ModuleBuildWorker,
};
use tokio::sync::Mutex;
use tonic::Request;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};

use crate::module_build_proto::module_build_service_client::ModuleBuildServiceClient;
use crate::module_build_proto::{ExecuteBuildRequest, ReadinessRequest};

/// Owner-side adapter for the separately deployed build worker. Connection,
/// deadline, protocol, and worker errors are returned to the caller; this
/// adapter never falls back to in-process Cargo execution.
pub struct GrpcModuleBuildWorker {
    client: Mutex<ModuleBuildServiceClient<Channel>>,
}

impl GrpcModuleBuildWorker {
    fn from_channel(channel: Channel) -> Self {
        Self {
            client: Mutex::new(ModuleBuildServiceClient::new(channel)),
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

    /// Probes the authenticated worker listener after startup validation.
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
            Err("module build worker reported not ready".to_string())
        }
    }
}

#[async_trait]
impl ModuleBuildWorker for GrpcModuleBuildWorker {
    async fn execute_build(
        &self,
        request: ModuleBuildRequest,
    ) -> Result<ModuleBuildResult, ModuleBuildProtocolError> {
        let payload = serde_json::to_vec(&request)
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        let response = self
            .client
            .lock()
            .await
            .execute_build(Request::new(ExecuteBuildRequest {
                module_build_request_json: payload,
            }))
            .await
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?
            .into_inner();
        serde_json::from_slice(&response.module_build_result_json)
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))
    }
}
