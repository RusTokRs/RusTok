use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use rustok_modules::{
    ModuleArtifactNodeAssignmentClaimCommand, ModuleArtifactNodeAssignmentHeartbeatCommand,
    ModuleArtifactNodeAssignmentReport, ModuleArtifactNodeReconciliationAuthorizer,
    ModuleArtifactNodeReconciliationError, ModuleArtifactNodeReconciliationRequest,
    ModuleArtifactNodeTopologyResolver, ModuleArtifactNodeTopologySnapshot, ModuleCommandContext,
    ModuleControlPlane,
};
use rustok_worker_transport::{
    PeerCertificateFingerprint, WorkerAdmission, peer_certificate_fingerprint,
};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::proto::artifact_node_reconciliation_service_server::{
    ArtifactNodeReconciliationService, ArtifactNodeReconciliationServiceServer,
};
use crate::{
    ARTIFACT_NODE_RECONCILIATION_MAX_MESSAGE_SIZE, owner_status,
    proto::{ReconcileTopologyRequest, ReconcileTopologyResponse},
};

const MAX_OPERATOR_NODE_IDS: usize = 1024;

/// Deployment-owned principal for a topology-authoring mTLS certificate.
/// `actor_id` is recorded in the owner audit/outbox contract, while
/// `allowed_node_ids` prevents a certificate from targeting another
/// deployment's execution nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleArtifactNodeReconciliationIdentity {
    pub actor_id: Uuid,
    allowed_node_ids: BTreeSet<Uuid>,
}

impl ModuleArtifactNodeReconciliationIdentity {
    pub fn new(
        actor_id: Uuid,
        allowed_node_ids: impl IntoIterator<Item = Uuid>,
    ) -> Result<Self, String> {
        if actor_id.is_nil() {
            return Err(
                "artifact node reconciliation identity requires a non-nil actor UUID".to_string(),
            );
        }
        let allowed_node_ids = allowed_node_ids.into_iter().collect::<Vec<_>>();
        if allowed_node_ids.is_empty() || allowed_node_ids.len() > MAX_OPERATOR_NODE_IDS {
            return Err(format!(
                "artifact node reconciliation identity requires between 1 and {MAX_OPERATOR_NODE_IDS} allowed node UUIDs"
            ));
        }
        if allowed_node_ids.contains(&Uuid::nil()) {
            return Err(
                "artifact node reconciliation identity cannot authorize a nil node UUID"
                    .to_string(),
            );
        }
        let configured_node_count = allowed_node_ids.len();
        let allowed_node_ids = allowed_node_ids.into_iter().collect::<BTreeSet<_>>();
        if allowed_node_ids.len() != configured_node_count {
            return Err(
                "artifact node reconciliation identity has duplicate node UUIDs".to_string(),
            );
        }
        Ok(Self {
            actor_id,
            allowed_node_ids,
        })
    }

    /// Returns the immutable deployment-owned node scope for configuration
    /// inspection. Callers cannot mutate the identity through this view.
    pub fn allowed_node_ids(&self) -> &BTreeSet<Uuid> {
        &self.allowed_node_ids
    }

    fn authorize_topology(
        &self,
        topology: &ModuleArtifactNodeTopologySnapshot,
    ) -> Result<(), Status> {
        if topology
            .assignments
            .iter()
            .any(|assignment| !self.allowed_node_ids.contains(&assignment.node_id))
        {
            return Err(Status::permission_denied(
                "mTLS deployment operator is not authorized for one or more artifact nodes",
            ));
        }
        Ok(())
    }
}

/// Maps a verified deployment mTLS certificate to one topology-authoring
/// principal. This is an authentication seam only; it never resolves
/// topology, installation identity, or runtime policy.
pub trait ModuleArtifactNodeReconciliationAuthenticator: Send + Sync {
    fn authenticate(
        &self,
        fingerprint: &PeerCertificateFingerprint,
    ) -> Result<ModuleArtifactNodeReconciliationIdentity, Status>;
}

/// Immutable certificate map supplied by deployment composition. A rotation
/// may map multiple fingerprints to the same operator identity, but one
/// fingerprint can never identify two operators.
pub struct StaticModuleArtifactNodeReconciliationAuthenticator {
    identities: BTreeMap<PeerCertificateFingerprint, ModuleArtifactNodeReconciliationIdentity>,
}

impl StaticModuleArtifactNodeReconciliationAuthenticator {
    pub fn new(
        entries: impl IntoIterator<
            Item = (
                PeerCertificateFingerprint,
                ModuleArtifactNodeReconciliationIdentity,
            ),
        >,
    ) -> Result<Self, String> {
        let mut identities = BTreeMap::new();
        for (fingerprint, identity) in entries {
            if identities.insert(fingerprint, identity).is_some() {
                return Err(
                    "artifact node reconciliation certificate fingerprint is duplicated"
                        .to_string(),
                );
            }
        }
        if identities.is_empty() {
            return Err(
                "artifact node reconciliation certificate map must not be empty".to_string(),
            );
        }
        Ok(Self { identities })
    }
}

impl ModuleArtifactNodeReconciliationAuthenticator
    for StaticModuleArtifactNodeReconciliationAuthenticator
{
    fn authenticate(
        &self,
        fingerprint: &PeerCertificateFingerprint,
    ) -> Result<ModuleArtifactNodeReconciliationIdentity, Status> {
        self.identities.get(fingerprint).cloned().ok_or_else(|| {
            Status::permission_denied(
                "mTLS peer is not an authorized artifact-node deployment operator",
            )
        })
    }
}

/// mTLS gRPC adapter for one authenticated desired-topology request. It owns
/// no topology source: its request snapshot is bounded by the certificate's
/// explicit node scope, then the `rustok-modules` owner reloads every admitted
/// installation identity in its transaction before persisting assignments.
pub struct ArtifactNodeReconciliationGrpcService<A> {
    db: DatabaseConnection,
    authenticator: A,
    admission: WorkerAdmission,
}

impl<A> ArtifactNodeReconciliationGrpcService<A> {
    pub fn new(db: DatabaseConnection, authenticator: A, admission: WorkerAdmission) -> Self {
        Self {
            db,
            authenticator,
            admission,
        }
    }

    /// Returns the bounded tonic service wrapper for independent reconciler
    /// composition. It is deliberately not served by the node-agent controller.
    pub fn into_tonic_service(self) -> ArtifactNodeReconciliationServiceServer<Self> {
        ArtifactNodeReconciliationServiceServer::new(self)
            .max_decoding_message_size(ARTIFACT_NODE_RECONCILIATION_MAX_MESSAGE_SIZE)
            .max_encoding_message_size(ARTIFACT_NODE_RECONCILIATION_MAX_MESSAGE_SIZE)
    }

    fn authenticate<T>(
        &self,
        request: &Request<T>,
    ) -> Result<ModuleArtifactNodeReconciliationIdentity, Status>
    where
        A: ModuleArtifactNodeReconciliationAuthenticator,
    {
        let fingerprint = peer_certificate_fingerprint(request)?;
        self.authenticator.authenticate(&fingerprint)
    }
}

#[async_trait]
impl<A> ArtifactNodeReconciliationService for ArtifactNodeReconciliationGrpcService<A>
where
    A: ModuleArtifactNodeReconciliationAuthenticator + 'static,
{
    async fn reconcile_topology(
        &self,
        request: Request<ReconcileTopologyRequest>,
    ) -> Result<Response<ReconcileTopologyResponse>, Status> {
        let identity = self.authenticate(&request)?;
        let (command, topology) = parse_request(request.into_inner(), identity.actor_id)?;
        identity.authorize_topology(&topology)?;
        let _permit = self.admission.acquire().await?;
        let owner = ModuleControlPlane::new(self.db.clone()).artifact_node_reconciliation(
            CertificateBoundReconciliationAuthorizer {
                actor_id: identity.actor_id,
            },
            SubmittedTopologyResolver {
                policy_revision: command.policy_revision.clone(),
                topology,
            },
        );
        let receipt = owner.request(command).await.map_err(owner_status)?;
        let receipt_json = serde_json::to_vec(&receipt).map_err(|_| {
            Status::internal("could not encode artifact node reconciliation receipt")
        })?;
        Ok(Response::new(ReconcileTopologyResponse { receipt_json }))
    }
}

fn parse_request(
    request: ReconcileTopologyRequest,
    actor_id: Uuid,
) -> Result<
    (
        ModuleArtifactNodeReconciliationRequest,
        ModuleArtifactNodeTopologySnapshot,
    ),
    Status,
> {
    let idempotency_key = Uuid::parse_str(&request.idempotency_key).map_err(|_| {
        Status::invalid_argument("artifact node reconciliation idempotency key must be a UUID")
    })?;
    let correlation_id = Uuid::parse_str(&request.correlation_id).map_err(|_| {
        Status::invalid_argument("artifact node reconciliation correlation ID must be a UUID")
    })?;
    let topology: ModuleArtifactNodeTopologySnapshot =
        serde_json::from_slice(&request.topology_json).map_err(|_| {
            Status::invalid_argument("artifact node reconciliation topology is invalid")
        })?;
    Ok((
        ModuleArtifactNodeReconciliationRequest {
            expected_reconciliation_state_revision: request.expected_reconciliation_state_revision,
            policy_revision: request.policy_revision,
            topology_digest: topology.topology_digest.clone(),
            context: ModuleCommandContext {
                actor_id,
                tenant_id: None,
                trace_id: request.trace_id,
                correlation_id,
                idempotency_key,
            },
        },
        topology,
    ))
}

struct CertificateBoundReconciliationAuthorizer {
    actor_id: Uuid,
}

#[async_trait]
impl ModuleArtifactNodeReconciliationAuthorizer for CertificateBoundReconciliationAuthorizer {
    async fn authorize_request(
        &self,
        command: &ModuleArtifactNodeReconciliationRequest,
    ) -> Result<(), ModuleArtifactNodeReconciliationError> {
        if command.context.actor_id == self.actor_id {
            Ok(())
        } else {
            Err(ModuleArtifactNodeReconciliationError::AuthorizationDenied(
                "artifact node reconciliation actor does not match the mTLS deployment principal"
                    .to_string(),
            ))
        }
    }

    async fn authorize_assignment_claim(
        &self,
        _command: &ModuleArtifactNodeAssignmentClaimCommand,
    ) -> Result<(), ModuleArtifactNodeReconciliationError> {
        Err(agent_port_denial())
    }

    async fn authorize_assignment_heartbeat(
        &self,
        _command: &ModuleArtifactNodeAssignmentHeartbeatCommand,
    ) -> Result<(), ModuleArtifactNodeReconciliationError> {
        Err(agent_port_denial())
    }

    async fn authorize_report(
        &self,
        _command: &ModuleArtifactNodeAssignmentReport,
    ) -> Result<(), ModuleArtifactNodeReconciliationError> {
        Err(agent_port_denial())
    }
}

fn agent_port_denial() -> ModuleArtifactNodeReconciliationError {
    ModuleArtifactNodeReconciliationError::AuthorizationDenied(
        "artifact node topology service cannot use the node-agent port".to_string(),
    )
}

struct SubmittedTopologyResolver {
    policy_revision: String,
    topology: ModuleArtifactNodeTopologySnapshot,
}

#[async_trait]
impl ModuleArtifactNodeTopologyResolver for SubmittedTopologyResolver {
    async fn resolve(
        &self,
        policy_revision: &str,
    ) -> Result<ModuleArtifactNodeTopologySnapshot, String> {
        if policy_revision != self.policy_revision {
            return Err(
                "artifact node reconciliation policy revision changed during topology resolution"
                    .to_string(),
            );
        }
        Ok(self.topology.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_modules::ModuleArtifactNodeAssignmentTarget;

    fn fingerprint(character: char) -> PeerCertificateFingerprint {
        PeerCertificateFingerprint::parse(format!("sha256:{}", character.to_string().repeat(64)))
            .expect("fingerprint")
    }

    fn topology(node_id: Uuid) -> ModuleArtifactNodeTopologySnapshot {
        ModuleArtifactNodeTopologySnapshot {
            topology_reference: "deployment:integration-a".to_string(),
            topology_digest: format!("sha256:{}", "a".repeat(64)),
            assignments: vec![ModuleArtifactNodeAssignmentTarget {
                node_id,
                installation_id: Uuid::new_v4(),
            }],
        }
    }

    #[test]
    fn reconciliation_identity_requires_bounded_non_nil_node_scope() {
        let actor_id = Uuid::new_v4();
        assert!(
            ModuleArtifactNodeReconciliationIdentity::new(Uuid::nil(), [Uuid::new_v4()]).is_err()
        );
        assert!(ModuleArtifactNodeReconciliationIdentity::new(actor_id, []).is_err());
        assert!(ModuleArtifactNodeReconciliationIdentity::new(actor_id, [Uuid::nil()]).is_err());
        let duplicate_node_id = Uuid::new_v4();
        assert!(
            ModuleArtifactNodeReconciliationIdentity::new(
                actor_id,
                [duplicate_node_id, duplicate_node_id],
            )
            .is_err()
        );
        let identity = ModuleArtifactNodeReconciliationIdentity::new(actor_id, [Uuid::new_v4()])
            .expect("identity");
        assert_eq!(identity.allowed_node_ids().len(), 1);
    }

    #[test]
    fn certificate_map_rejects_unknown_and_duplicate_operator_principals() {
        let identity =
            ModuleArtifactNodeReconciliationIdentity::new(Uuid::new_v4(), [Uuid::new_v4()])
                .expect("identity");
        assert!(StaticModuleArtifactNodeReconciliationAuthenticator::new([]).is_err());
        assert!(
            StaticModuleArtifactNodeReconciliationAuthenticator::new([
                (fingerprint('a'), identity.clone()),
                (fingerprint('a'), identity.clone()),
            ])
            .is_err()
        );
        let authenticator = StaticModuleArtifactNodeReconciliationAuthenticator::new([(
            fingerprint('a'),
            identity,
        )])
        .expect("authenticator");
        assert_eq!(
            authenticator
                .authenticate(&fingerprint('b'))
                .expect_err("unknown operator")
                .code(),
            tonic::Code::PermissionDenied
        );
    }

    #[test]
    fn request_uses_certificate_actor_and_never_accepts_one_from_the_body() {
        let actor_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();
        let submitted = topology(node_id);
        let (command, parsed_topology) = parse_request(
            ReconcileTopologyRequest {
                expected_reconciliation_state_revision: 4,
                policy_revision: format!("sha256:{}", "b".repeat(64)),
                idempotency_key: Uuid::new_v4().to_string(),
                topology_json: serde_json::to_vec(&submitted).expect("topology JSON"),
                trace_id: "test:artifact-node-reconciliation".to_string(),
                correlation_id: Uuid::new_v4().to_string(),
            },
            actor_id,
        )
        .expect("request");
        assert_eq!(command.context.actor_id, actor_id);
        assert_eq!(command.context.tenant_id, None);
        assert_eq!(command.expected_reconciliation_state_revision, 4);
        assert_eq!(command.topology_digest, submitted.topology_digest);
        assert_eq!(parsed_topology, submitted);
    }

    #[test]
    fn identity_scope_rejects_another_nodes_topology() {
        let identity =
            ModuleArtifactNodeReconciliationIdentity::new(Uuid::new_v4(), [Uuid::new_v4()])
                .expect("identity");
        assert_eq!(
            identity
                .authorize_topology(&topology(Uuid::new_v4()))
                .expect_err("foreign node")
                .code(),
            tonic::Code::PermissionDenied
        );
    }

    #[tokio::test]
    async fn certificate_authorizer_denies_agent_operations_and_actor_mismatch() {
        let authorizer = CertificateBoundReconciliationAuthorizer {
            actor_id: Uuid::new_v4(),
        };
        let request = ModuleArtifactNodeReconciliationRequest {
            expected_reconciliation_state_revision: 0,
            policy_revision: format!("sha256:{}", "c".repeat(64)),
            topology_digest: format!("sha256:{}", "d".repeat(64)),
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: None,
                trace_id: "test:certificate-authorizer".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
        };
        assert!(authorizer.authorize_request(&request).await.is_err());
        assert!(
            authorizer
                .authorize_assignment_claim(&ModuleArtifactNodeAssignmentClaimCommand {
                    node_id: Uuid::new_v4(),
                    agent_id: "node-agent".to_string(),
                })
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn submitted_topology_resolver_fences_policy_revision() {
        let resolver = SubmittedTopologyResolver {
            policy_revision: "sha256:policy".to_string(),
            topology: topology(Uuid::new_v4()),
        };
        assert!(resolver.resolve("sha256:policy").await.is_ok());
        assert!(resolver.resolve("sha256:other").await.is_err());
    }
}
