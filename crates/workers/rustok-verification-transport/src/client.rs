use async_trait::async_trait;
use rustok_modules::{TrustVerificationDecision, TrustVerificationRequest, TrustVerifier};
use tokio::sync::Mutex;
use tonic::Request;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};

use crate::proto::verification_service_client::VerificationServiceClient;
use crate::proto::{ReadinessRequest, VerifyRequest};

/// Owner-side adapter. Any connection, deadline, protocol, or worker error is
/// returned to `ModuleInstaller`, which rejects admission without a fallback.
pub struct GrpcTrustVerifier {
    client: Mutex<VerificationServiceClient<Channel>>,
}

impl GrpcTrustVerifier {
    pub fn from_channel(channel: Channel) -> Self {
        Self {
            client: Mutex::new(VerificationServiceClient::new(channel)),
        }
    }

    pub async fn connect(endpoint: Endpoint) -> Result<Self, String> {
        let channel = endpoint
            .connect()
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self::from_channel(channel))
    }

    /// Connect to a worker that requires mutual TLS. The deployment host owns
    /// the client identity, trust root, and expected server domain.
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

    /// Performs the worker's mTLS-protected readiness probe. Deployment
    /// supervisors can use it to wait for a fully validated worker before
    /// sending admission traffic.
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
            Err("verification worker reported not ready".to_string())
        }
    }
}

#[async_trait]
impl TrustVerifier for GrpcTrustVerifier {
    async fn verify(
        &self,
        request: TrustVerificationRequest,
    ) -> Result<TrustVerificationDecision, String> {
        let payload = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
        let response = self
            .client
            .lock()
            .await
            .verify(Request::new(VerifyRequest {
                trust_verification_request_json: payload,
            }))
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        serde_json::from_slice(&response.trust_verification_decision_json)
            .map_err(|error| error.to_string())
    }
}
