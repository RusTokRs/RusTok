use std::sync::Arc;

use rustok_modules::{TrustVerificationRequest, TrustVerifier};
use rustok_worker_transport::WorkerAdmission;
use tonic::{Request, Response, Status};

use crate::proto::verification_service_server::VerificationService;
use crate::proto::{ReadinessRequest, ReadinessResponse, VerifyRequest, VerifyResponse};

/// Worker-side service adapter. The worker implementation retains ownership of
/// signature, provenance, and SBOM verification; this adapter only maps the
/// typed owner port onto gRPC.
pub struct VerificationGrpcService<V> {
    verifier: Arc<V>,
    admission: WorkerAdmission,
}

impl<V> VerificationGrpcService<V> {
    pub fn new(verifier: Arc<V>, admission: WorkerAdmission) -> Self {
        Self {
            verifier,
            admission,
        }
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
        let _permit = self.admission.acquire().await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_modules::{
        ModuleArtifactDescriptor, OciArtifactReference, TrustEvidenceKind,
        TrustEvidenceReference, TrustVerificationDecision, TrustVerificationRequest,
        TrustVerifier,
    };

    struct MockVerifier {
        admitted: bool,
    }

    #[async_trait::async_trait]
    impl TrustVerifier for MockVerifier {
        async fn verify(
            &self,
            request: TrustVerificationRequest,
        ) -> std::result::Result<TrustVerificationDecision, String> {
            if self.admitted {
                Ok(TrustVerificationDecision {
                    signer_identity: "test-signer".to_string(),
                    trust_policy_revision: request.trust_policy_revision,
                    capability_policy_revision: request.capability_policy_revision,
                    signature_verified: true,
                    provenance_verified: true,
                    sbom_verified: true,
                    license_policy_verified: true,
                    vulnerability_policy_verified: true,
                    evidence: vec![TrustEvidenceReference {
                        kind: TrustEvidenceKind::Signature,
                        reference: "oci://test/sig".to_string(),
                        digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                            .to_string(),
                    }],
                })
            } else {
                Err("trust verification failed".to_string())
            }
        }
    }

    fn test_descriptor() -> ModuleArtifactDescriptor {
        serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "slug": "test-pkg",
            "version": "1.0.0",
            "payload_kind": "wasm_component",
            "module_kind": "optional",
            "runtime_abi": "wasm32-wasip2",
            "platform_compatibility": "any",
            "entrypoint": "test.wasm",
            "artifact_digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        }))
        .expect("test descriptor JSON should deserialize")
    }

    #[tokio::test]
    async fn test_readiness_probe() {
        let verifier = Arc::new(MockVerifier { admitted: true });
        let admission = WorkerAdmission::new(10, std::time::Duration::from_secs(5)).unwrap();
        let service = VerificationGrpcService::new(verifier, admission);

        let res = service
            .get_readiness(Request::new(ReadinessRequest {}))
            .await
            .expect("readiness probe should succeed")
            .into_inner();
        assert!(res.ready);
    }

    #[tokio::test]
    async fn test_verify_success() {
        let verifier = Arc::new(MockVerifier { admitted: true });
        let admission = WorkerAdmission::new(10, std::time::Duration::from_secs(5)).unwrap();
        let service = VerificationGrpcService::new(verifier, admission);

        let request = TrustVerificationRequest {
            reference: OciArtifactReference {
                registry: "ghcr.io".to_string(),
                repository: "rustok/test".to_string(),
                digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
            },
            descriptor: test_descriptor(),
            trust_policy_revision: 1,
            capability_policy_revision: 2,
            expected_alloy_workspace_provenance: None,
        };

        let request_payload = serde_json::to_vec(&request).unwrap();
        let response = service
            .verify(Request::new(VerifyRequest {
                trust_verification_request_json: request_payload,
            }))
            .await
            .expect("verification should succeed")
            .into_inner();

        let decision: TrustVerificationDecision =
            serde_json::from_slice(&response.trust_verification_decision_json).unwrap();
        assert_eq!(decision.signer_identity, "test-signer");
        assert_eq!(decision.trust_policy_revision, 1);
    }

    #[tokio::test]
    async fn test_verify_rejection() {
        let verifier = Arc::new(MockVerifier { admitted: false });
        let admission = WorkerAdmission::new(10, std::time::Duration::from_secs(5)).unwrap();
        let service = VerificationGrpcService::new(verifier, admission);

        let request = TrustVerificationRequest {
            reference: OciArtifactReference {
                registry: "ghcr.io".to_string(),
                repository: "rustok/test".to_string(),
                digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
            },
            descriptor: test_descriptor(),
            trust_policy_revision: 1,
            capability_policy_revision: 2,
            expected_alloy_workspace_provenance: None,
        };

        let request_payload = serde_json::to_vec(&request).unwrap();
        let status = service
            .verify(Request::new(VerifyRequest {
                trust_verification_request_json: request_payload,
            }))
            .await
            .expect_err("should return error status");

        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    }
}
