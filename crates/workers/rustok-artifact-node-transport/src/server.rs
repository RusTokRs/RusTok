use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use rustok_modules::{
    ModuleArtifactNodeAgentPort, ModuleArtifactNodeAssignmentClaimCommand,
    ModuleArtifactNodeAssignmentHeartbeatCommand, ModuleArtifactNodeAssignmentReport,
};
use rustok_worker_transport::{
    PeerCertificateFingerprint, WorkerAdmission, peer_certificate_fingerprint,
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::proto::artifact_node_service_server::{ArtifactNodeService, ArtifactNodeServiceServer};
use crate::proto::{
    ClaimAssignmentRequest, ClaimAssignmentResponse, HeartbeatAssignmentRequest,
    HeartbeatAssignmentResponse, NoAssignment, ReportAssignmentRequest, ReportAssignmentResponse,
    claim_assignment_response,
};
use crate::{ARTIFACT_NODE_AGENT_PROTOCOL_REVISION, owner_status};

const MAX_AGENT_ID_BYTES: usize = 128;

/// Deployment-owned node-agent principal. It is resolved from a verified mTLS
/// peer certificate, never from an RPC field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleArtifactNodeAgentIdentity {
    pub node_id: Uuid,
    pub agent_id: String,
}

impl ModuleArtifactNodeAgentIdentity {
    pub fn new(node_id: Uuid, agent_id: impl Into<String>) -> Result<Self, String> {
        let agent_id = agent_id.into();
        if node_id.is_nil() {
            return Err("artifact node agent identity requires a non-nil node UUID".to_string());
        }
        if agent_id.is_empty() || agent_id.trim() != agent_id || agent_id.len() > MAX_AGENT_ID_BYTES
        {
            return Err("artifact node agent identity is invalid".to_string());
        }
        Ok(Self { node_id, agent_id })
    }
}

/// Resolves a verified mTLS deployment principal to one fixed node/agent
/// identity. Implementations are deployment adapters, not control-plane
/// topology or artifact-policy owners.
pub trait ModuleArtifactNodeAgentAuthenticator: Send + Sync {
    fn authenticate(
        &self,
        fingerprint: &PeerCertificateFingerprint,
    ) -> Result<ModuleArtifactNodeAgentIdentity, Status>;
}

/// In-memory representation of an immutable deployment-owned certificate map.
/// A composition layer may reload the complete map atomically, but this adapter
/// does not read configuration files, databases, or request metadata itself.
pub struct StaticModuleArtifactNodeAgentAuthenticator {
    identities: BTreeMap<PeerCertificateFingerprint, ModuleArtifactNodeAgentIdentity>,
}

impl StaticModuleArtifactNodeAgentAuthenticator {
    pub fn new(
        entries: impl IntoIterator<Item = (PeerCertificateFingerprint, ModuleArtifactNodeAgentIdentity)>,
    ) -> Result<Self, String> {
        let mut identities = BTreeMap::new();
        for (fingerprint, identity) in entries {
            if identities.insert(fingerprint, identity).is_some() {
                return Err("artifact node agent certificate fingerprint is duplicated".to_string());
            }
        }
        if identities.is_empty() {
            return Err("artifact node agent certificate map must not be empty".to_string());
        }
        Ok(Self { identities })
    }
}

impl ModuleArtifactNodeAgentAuthenticator for StaticModuleArtifactNodeAgentAuthenticator {
    fn authenticate(
        &self,
        fingerprint: &PeerCertificateFingerprint,
    ) -> Result<ModuleArtifactNodeAgentIdentity, Status> {
        self.identities
            .get(fingerprint)
            .cloned()
            .ok_or_else(|| Status::permission_denied("mTLS peer is not an authorized node agent"))
    }
}

/// gRPC adapter for the owner-issued claim/heartbeat/report port. It has no
/// artifact materialization, CAS, sandbox, database, topology, or policy
/// implementation.
pub struct ArtifactNodeGrpcService<P, A> {
    port: Arc<P>,
    authenticator: A,
    admission: WorkerAdmission,
}

impl<P, A> ArtifactNodeGrpcService<P, A> {
    pub fn new(port: Arc<P>, authenticator: A, admission: WorkerAdmission) -> Self {
        Self {
            port,
            authenticator,
            admission,
        }
    }

    /// Returns the bounded tonic service wrapper for deployment composition.
    pub fn into_tonic_service(self) -> ArtifactNodeServiceServer<Self> {
        ArtifactNodeServiceServer::new(self)
            .max_decoding_message_size(crate::ARTIFACT_NODE_AGENT_MAX_MESSAGE_SIZE)
            .max_encoding_message_size(crate::ARTIFACT_NODE_AGENT_MAX_MESSAGE_SIZE)
    }

    fn authenticate<T>(
        &self,
        request: &Request<T>,
    ) -> Result<ModuleArtifactNodeAgentIdentity, Status>
    where
        A: ModuleArtifactNodeAgentAuthenticator,
    {
        let fingerprint = peer_certificate_fingerprint(request)?;
        self.authenticator.authenticate(&fingerprint)
    }
}

#[async_trait]
impl<P, A> ArtifactNodeService for ArtifactNodeGrpcService<P, A>
where
    P: ModuleArtifactNodeAgentPort + 'static,
    A: ModuleArtifactNodeAgentAuthenticator + 'static,
{
    async fn claim_assignment(
        &self,
        request: Request<ClaimAssignmentRequest>,
    ) -> Result<Response<ClaimAssignmentResponse>, Status> {
        let principal = self.authenticate(&request)?;
        validate_protocol_revision(request.get_ref().protocol_revision)?;
        let _permit = self.admission.acquire().await?;
        let work = self
            .port
            .claim_assignment(ModuleArtifactNodeAssignmentClaimCommand {
                node_id: principal.node_id,
                agent_id: principal.agent_id,
            })
            .await
            .map_err(owner_status)?;
        let result = match work {
            Some(work) => claim_assignment_response::Result::WorkItemJson(
                serde_json::to_vec(&work)
                    .map_err(|_| Status::internal("could not encode artifact node assignment"))?,
            ),
            None => claim_assignment_response::Result::NoAssignment(NoAssignment {}),
        };
        Ok(Response::new(ClaimAssignmentResponse {
            protocol_revision: ARTIFACT_NODE_AGENT_PROTOCOL_REVISION,
            result: Some(result),
        }))
    }

    async fn heartbeat_assignment(
        &self,
        request: Request<HeartbeatAssignmentRequest>,
    ) -> Result<Response<HeartbeatAssignmentResponse>, Status> {
        let principal = self.authenticate(&request)?;
        let request = request.into_inner();
        validate_protocol_revision(request.protocol_revision)?;
        let claim_id = Uuid::parse_str(&request.claim_id)
            .map_err(|_| Status::invalid_argument("artifact node claim ID must be a UUID"))?;
        let receipt = self
            .port
            .heartbeat_assignment(ModuleArtifactNodeAssignmentHeartbeatCommand {
                claim_id,
                agent_id: principal.agent_id,
            })
            .await
            .map_err(owner_status)?;
        Ok(Response::new(HeartbeatAssignmentResponse {
            protocol_revision: ARTIFACT_NODE_AGENT_PROTOCOL_REVISION,
            receipt_json: serde_json::to_vec(&receipt).map_err(|_| {
                Status::internal("could not encode artifact node heartbeat receipt")
            })?,
        }))
    }

    async fn report_assignment(
        &self,
        request: Request<ReportAssignmentRequest>,
    ) -> Result<Response<ReportAssignmentResponse>, Status> {
        let principal = self.authenticate(&request)?;
        let request = request.into_inner();
        validate_protocol_revision(request.protocol_revision)?;
        let report: ModuleArtifactNodeAssignmentReport =
            serde_json::from_slice(&request.report_json)
                .map_err(|_| Status::invalid_argument("artifact node report is invalid"))?;
        validate_report_principal(&report, &principal)?;
        let _permit = self.admission.acquire().await?;
        let receipt = self
            .port
            .report_assignment(report)
            .await
            .map_err(owner_status)?;
        Ok(Response::new(ReportAssignmentResponse {
            protocol_revision: ARTIFACT_NODE_AGENT_PROTOCOL_REVISION,
            receipt_json: serde_json::to_vec(&receipt)
                .map_err(|_| Status::internal("could not encode artifact node report receipt"))?,
        }))
    }
}

fn validate_protocol_revision(revision: u32) -> Result<(), Status> {
    if revision == ARTIFACT_NODE_AGENT_PROTOCOL_REVISION {
        Ok(())
    } else {
        Err(Status::failed_precondition(
            "artifact node agent protocol revision does not match",
        ))
    }
}

fn validate_report_principal(
    report: &ModuleArtifactNodeAssignmentReport,
    principal: &ModuleArtifactNodeAgentIdentity,
) -> Result<(), Status> {
    if report.node_id == principal.node_id && report.agent_id == principal.agent_id {
        Ok(())
    } else {
        Err(Status::permission_denied(
            "artifact node report principal does not match the mTLS peer",
        ))
    }
}

#[cfg(test)]
mod tests {
    use rustok_modules::{
        ArtifactPayloadKind, ModuleArtifactNodeInstallationScope, ModuleReconciliationPhase,
    };

    use super::*;

    fn fingerprint(character: char) -> PeerCertificateFingerprint {
        PeerCertificateFingerprint::parse(format!("sha256:{}", character.to_string().repeat(64)))
            .expect("fingerprint")
    }

    #[test]
    fn static_authenticator_requires_one_unique_mtls_fingerprint() {
        let identity =
            ModuleArtifactNodeAgentIdentity::new(Uuid::new_v4(), "node-agent-a").expect("identity");
        assert!(StaticModuleArtifactNodeAgentAuthenticator::new([]).is_err());
        assert!(
            StaticModuleArtifactNodeAgentAuthenticator::new([
                (fingerprint('a'), identity.clone()),
                (fingerprint('a'), identity),
            ])
            .is_err()
        );
    }

    #[test]
    fn node_agent_identity_rejects_nil_or_unbounded_agent_values() {
        assert!(ModuleArtifactNodeAgentIdentity::new(Uuid::nil(), "node-agent-a").is_err());
        assert!(ModuleArtifactNodeAgentIdentity::new(Uuid::new_v4(), " node-agent-a").is_err());
        assert!(ModuleArtifactNodeAgentIdentity::new(Uuid::new_v4(), "a".repeat(129)).is_err());
    }

    #[test]
    fn protocol_revision_fails_closed() {
        assert!(validate_protocol_revision(ARTIFACT_NODE_AGENT_PROTOCOL_REVISION).is_ok());
        assert_eq!(
            validate_protocol_revision(ARTIFACT_NODE_AGENT_PROTOCOL_REVISION + 1)
                .expect_err("revision mismatch")
                .code(),
            tonic::Code::FailedPrecondition
        );
    }

    #[test]
    fn unknown_fingerprint_and_report_principal_mismatch_are_denied() {
        let node_id = Uuid::new_v4();
        let principal =
            ModuleArtifactNodeAgentIdentity::new(node_id, "node-agent-a").expect("principal");
        let authenticator = StaticModuleArtifactNodeAgentAuthenticator::new([(
            fingerprint('a'),
            principal.clone(),
        )])
        .expect("authenticator");
        assert_eq!(
            authenticator
                .authenticate(&fingerprint('b'))
                .expect_err("unknown fingerprint")
                .code(),
            tonic::Code::PermissionDenied
        );
        let report = ModuleArtifactNodeAssignmentReport {
            claim_id: Uuid::new_v4(),
            reconciliation_id: Uuid::new_v4(),
            node_id,
            installation_id: Uuid::new_v4(),
            expected_observation_revision: 0,
            phase: ModuleReconciliationPhase::Prepared,
            installation_scope: ModuleArtifactNodeInstallationScope::Platform,
            release_digest: format!("sha256:{}", "a".repeat(64)),
            payload_digest: format!("sha256:{}", "b".repeat(64)),
            payload_kind: ArtifactPayloadKind::Rhai,
            payload_media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
            admission_revision: 1,
            dependency_graph_revision: 1,
            dependency_graph_digest: format!("sha256:{}", "c".repeat(64)),
            capability_grant_revision: 1,
            executor_abi: "rustok:module/runtime@1".to_string(),
            policy_revision: format!("sha256:{}", "d".repeat(64)),
            health_evidence: None,
            failure: None,
            agent_id: "another-agent".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        assert_eq!(
            validate_report_principal(&report, &principal)
                .expect_err("mismatched report identity")
                .code(),
            tonic::Code::PermissionDenied
        );
    }
}
