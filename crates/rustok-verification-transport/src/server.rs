use std::sync::Arc;

use rustok_modules::{TrustVerificationRequest, TrustVerifier};
use tonic::{Request, Response, Status};

use crate::proto::verification_service_server::VerificationService;
use crate::proto::{ReadinessRequest, ReadinessResponse, VerifyRequest, VerifyResponse};

/// Worker-side service adapter. The worker implementation retains ownership of
/// signature, provenance, and SBOM verification; this adapter only maps the
/// typed owner port onto gRPC.
pub struct VerificationGrpcService<V> {
    verifier: Arc<V>,
}

impl<V> VerificationGrpcService<V> {
    pub fn new(verifier: Arc<V>) -> Self {
        Self { verifier }
    }
}

#[tonic::async_trait]
impl<V> VerificationService for VerificationGrpcService<V>
where
    V: TrustVerifier + 'static,
{
    async fn get_readiness(
        &self,
        _request: Request<ReadinessRequest>,
    ) -> Result<Response<ReadinessResponse>, Status> {
        // Reaching this handler means listener TLS, policy parsing, and worker
        // construction have all completed. Startup failures exit the process
        // instead of exposing a degraded verifier.
        Ok(Response::new(ReadinessResponse { ready: true }))
    }

    async fn verify(
        &self,
        request: Request<VerifyRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: TrustVerificationRequest =
            serde_json::from_slice(&request.into_inner().trust_verification_request_json)
                .map_err(|error| Status::invalid_argument(error.to_string()))?;
        let decision = self
            .verifier
            .verify(request)
            .await
            .map_err(Status::failed_precondition)?;
        let payload =
            serde_json::to_vec(&decision).map_err(|error| Status::internal(error.to_string()))?;
        Ok(Response::new(VerifyResponse {
            trust_verification_decision_json: payload,
        }))
    }
}
