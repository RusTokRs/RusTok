//! Durable owner reconciliation for dynamic artifact and sandbox assignments.
//!
//! The module control plane selects a complete desired set. Authenticated node
//! agents can only claim and report their own exact assignment; the owner
//! persists every observed identity, fences stale lifecycle changes, and alone
//! advances the durable observed head.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::DomainEvent;

use crate::{
    ArtifactPayloadKind, ControlPlaneInfrastructure, ModuleCommandContext,
    data::{now_expression, placeholder, uuid_from_row, uuid_value},
    installation::{InstalledModuleArtifact, ModuleInstallationScope},
    promotion::{digest_json, valid_digest, valid_reference},
    reconciliation::{
        ModuleDesiredObservedState, ModuleReconciliationEvidence, ModuleReconciliationFailure,
        ModuleReconciliationPhase,
    },
};

const RECONCILIATION_STATE_ID: &str = "current";
const TOPOLOGY_DIGEST_CONTRACT: &str = "rustok.artifact_node_reconciliation.topology";
const MAX_TARGET_ASSIGNMENTS: usize = 1024;
const MAX_REFERENCE_BYTES: usize = 512;
const MAX_EXECUTOR_ABI_BYTES: usize = 128;
const MAX_PAYLOAD_MEDIA_TYPE_BYTES: usize = 256;
const MAX_AGENT_ID_BYTES: usize = 128;
const MAX_FAILURE_CODE_BYTES: usize = 128;
const MAX_FAILURE_DETAIL_BYTES: usize = 2_000;
/// Owner-issued node assignment leases are fixed at five minutes. Deployment
/// agents use this public contract to keep their bounded heartbeat interval
/// safely below the owner fence without reimplementing the value.
pub const MODULE_ARTIFACT_NODE_ASSIGNMENT_LEASE_SECONDS: i64 = 300;

/// The immutable scope class an agent needs to enforce its artifact boundary.
/// The tenant identity remains owner-local and is never included in a node
/// work item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleArtifactNodeInstallationScope {
    Platform,
    Tenant,
}

impl ModuleArtifactNodeInstallationScope {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Tenant => "tenant",
        }
    }

    fn parse(value: &str) -> Result<Self, ModuleArtifactNodeReconciliationError> {
        match value {
            "platform" => Ok(Self::Platform),
            "tenant" => Ok(Self::Tenant),
            _ => Err(ModuleArtifactNodeReconciliationError::Store(
                "artifact node assignment installation scope is invalid".to_string(),
            )),
        }
    }
}

/// Aggregate lifecycle state. Only the owner moves an assignment from healthy
/// to active, and only a converged aggregate becomes the observed head.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleArtifactNodeReconciliationStatus {
    Preparing,
    Activating,
    Converged,
    Failed,
    Degraded,
    Superseded,
}

impl ModuleArtifactNodeReconciliationStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Activating => "activating",
            Self::Converged => "converged",
            Self::Failed => "failed",
            Self::Degraded => "degraded",
            Self::Superseded => "superseded",
        }
    }

    fn parse(value: &str) -> Result<Self, ModuleArtifactNodeReconciliationError> {
        match value {
            "preparing" => Ok(Self::Preparing),
            "activating" => Ok(Self::Activating),
            "converged" => Ok(Self::Converged),
            "failed" => Ok(Self::Failed),
            "degraded" => Ok(Self::Degraded),
            "superseded" => Ok(Self::Superseded),
            _ => Err(ModuleArtifactNodeReconciliationError::Store(
                "artifact node reconciliation status is invalid".to_string(),
            )),
        }
    }

    const fn is_claimable(self) -> bool {
        matches!(
            self,
            Self::Preparing | Self::Activating | Self::Converged | Self::Degraded
        )
    }
}

/// One owner-selected node/installation pair. The resolver deliberately
/// carries no release, capability, or artifact values: those are loaded from
/// the control-plane-owned admitted installation record inside the request
/// transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeAssignmentTarget {
    pub node_id: Uuid,
    pub installation_id: Uuid,
}

/// A deterministic owner topology snapshot. Its digest binds the reference and
/// sorted node/installation target set before persistent assignments are made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeTopologySnapshot {
    pub topology_reference: String,
    pub topology_digest: String,
    pub assignments: Vec<ModuleArtifactNodeAssignmentTarget>,
}

/// Trusted owner seam for topology selection. The resolver may use deployment
/// metadata, but it cannot inject artifact identity because the owner reloads
/// every installation under its own transaction and locks it before writing.
#[async_trait]
pub trait ModuleArtifactNodeTopologyResolver: Send + Sync {
    async fn resolve(
        &self,
        policy_revision: &str,
    ) -> Result<ModuleArtifactNodeTopologySnapshot, String>;
}

/// Requests a fresh desired set after a policy, topology, or admitted artifact
/// identity change. The expected state revision prevents two operators from
/// silently replacing each other's target set. `topology_digest` binds the
/// resolver output into the idempotency identity so a replay cannot substitute
/// another deployment target set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeReconciliationRequest {
    pub expected_reconciliation_state_revision: u64,
    pub policy_revision: String,
    pub topology_digest: String,
    pub context: ModuleCommandContext,
}

/// Authenticated pull request from one node agent. The authorizer binds the
/// agent principal to `node_id`; this command never selects an installation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeAssignmentClaimCommand {
    pub node_id: Uuid,
    pub agent_id: String,
}

/// Extends one exact assignment lease while materialization or health checking
/// is still in progress.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeAssignmentHeartbeatCommand {
    pub claim_id: Uuid,
    pub agent_id: String,
}

/// Complete immutable identity expected by an agent for one selected artifact.
/// Tenant identity, secret references, grants, and database access are not
/// exposed through this transport-neutral owner contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeAssignment {
    pub node_id: Uuid,
    pub installation_id: Uuid,
    pub installation_scope: ModuleArtifactNodeInstallationScope,
    pub release_digest: String,
    pub payload_digest: String,
    pub payload_kind: ArtifactPayloadKind,
    pub payload_media_type: String,
    pub admission_revision: u64,
    pub dependency_graph_revision: u64,
    pub dependency_graph_digest: String,
    pub capability_grant_revision: u64,
    pub executor_abi: String,
    pub policy_revision: String,
    pub ordinal: u16,
    pub observation_revision: u64,
    pub phase: ModuleReconciliationPhase,
    pub health_evidence: Option<ModuleReconciliationEvidence>,
    pub failure: Option<ModuleReconciliationFailure>,
    pub reported_by: Option<String>,
    pub last_report_digest: Option<String>,
    pub active_claim_id: Option<Uuid>,
    pub claimed_by_agent: Option<String>,
    pub claim_expires_at: Option<DateTime<Utc>>,
}

/// Immutable aggregate identity carried with every node work item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeReconciliationWorkIdentity {
    pub reconciliation_id: Uuid,
    pub reconciliation_revision: u64,
    pub topology_reference: String,
    pub topology_digest: String,
    pub policy_revision: String,
}

/// One owner-issued work lease. It intentionally contains one assignment only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeAssignmentWorkItem {
    pub claim_id: Uuid,
    pub lease_expires_at: DateTime<Utc>,
    pub expected_observation_revision: u64,
    pub reconciliation: ModuleArtifactNodeReconciliationWorkIdentity,
    pub assignment: ModuleArtifactNodeAssignment,
}

/// A node report must echo the full owner-issued identity. This turns an agent
/// response into evidence for a particular admitted artifact rather than a
/// generic readiness signal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeAssignmentReport {
    pub claim_id: Uuid,
    pub reconciliation_id: Uuid,
    pub node_id: Uuid,
    pub installation_id: Uuid,
    pub expected_observation_revision: u64,
    pub phase: ModuleReconciliationPhase,
    pub installation_scope: ModuleArtifactNodeInstallationScope,
    pub release_digest: String,
    pub payload_digest: String,
    pub payload_kind: ArtifactPayloadKind,
    pub payload_media_type: String,
    pub admission_revision: u64,
    pub dependency_graph_revision: u64,
    pub dependency_graph_digest: String,
    pub capability_grant_revision: u64,
    pub executor_abi: String,
    pub policy_revision: String,
    pub health_evidence: Option<ModuleReconciliationEvidence>,
    pub failure: Option<ModuleReconciliationFailure>,
    pub agent_id: String,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeAssignmentHeartbeatReceipt {
    pub claim_id: Uuid,
    pub lease_expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeReconciliation {
    pub reconciliation_id: Uuid,
    pub predecessor_reconciliation_id: Option<Uuid>,
    pub reconciliation_revision: u64,
    pub topology_reference: String,
    pub topology_digest: String,
    pub policy_revision: String,
    pub target_assignment_count: u16,
    pub status: ModuleArtifactNodeReconciliationStatus,
    pub requested_by: Uuid,
    pub failure: Option<ModuleReconciliationFailure>,
    pub assignments: Vec<ModuleArtifactNodeAssignment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeReconciliationReceipt {
    pub reconciliation_id: Uuid,
    pub reconciliation_revision: u64,
    pub reconciliation_state_revision: u64,
    pub status: ModuleArtifactNodeReconciliationStatus,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactNodeAssignmentReportReceipt {
    pub reconciliation_id: Uuid,
    pub reconciliation_revision: u64,
    pub reconciliation_state_revision: u64,
    pub reconciliation_status: ModuleArtifactNodeReconciliationStatus,
    pub node_id: Uuid,
    pub installation_id: Uuid,
    pub observation_revision: u64,
    pub phase: ModuleReconciliationPhase,
    pub created: bool,
}

#[async_trait]
pub trait ModuleArtifactNodeReconciliationAuthorizer: Send + Sync {
    async fn authorize_request(
        &self,
        command: &ModuleArtifactNodeReconciliationRequest,
    ) -> Result<(), ModuleArtifactNodeReconciliationError>;

    async fn authorize_assignment_claim(
        &self,
        command: &ModuleArtifactNodeAssignmentClaimCommand,
    ) -> Result<(), ModuleArtifactNodeReconciliationError>;

    async fn authorize_assignment_heartbeat(
        &self,
        command: &ModuleArtifactNodeAssignmentHeartbeatCommand,
    ) -> Result<(), ModuleArtifactNodeReconciliationError>;

    async fn authorize_report(
        &self,
        command: &ModuleArtifactNodeAssignmentReport,
    ) -> Result<(), ModuleArtifactNodeReconciliationError>;
}

/// Narrow authenticated node-agent boundary over the owner reconciliation
/// aggregate. Transport adapters authenticate a deployment principal before
/// constructing these commands; the port never accepts topology or artifact
/// selection from an agent.
#[async_trait]
pub trait ModuleArtifactNodeAgentPort: Send + Sync {
    async fn claim_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentClaimCommand,
    ) -> Result<Option<ModuleArtifactNodeAssignmentWorkItem>, ModuleArtifactNodeReconciliationError>;

    async fn heartbeat_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentHeartbeatCommand,
    ) -> Result<ModuleArtifactNodeAssignmentHeartbeatReceipt, ModuleArtifactNodeReconciliationError>;

    async fn report_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentReport,
    ) -> Result<ModuleArtifactNodeAssignmentReportReceipt, ModuleArtifactNodeReconciliationError>;
}

/// SeaORM implementation of the dynamic artifact/sandbox node-reconciliation
/// owner. It is deliberately independent of `rustok-sandbox` and deployment
/// transports: those components call this owner through authenticated adapters.
#[derive(Clone)]
pub struct SeaOrmModuleArtifactNodeReconciliationService<A, T> {
    db: DatabaseConnection,
    authorizer: A,
    topology: T,
    infrastructure: ControlPlaneInfrastructure,
}

/// Owner-only projection of the authenticated node-agent port. It deliberately
/// exposes no desired-topology request, state projection, or reconciliation
/// lookup. Deployment transports authenticate the node principal before they
/// call this projection; the durable owner remains the sole authority for
/// claims, lease renewal, and observed assignment reports.
#[derive(Clone)]
pub struct SeaOrmModuleArtifactNodeAgentService {
    inner: SeaOrmModuleArtifactNodeReconciliationService<
        AgentPortOnlyAuthorizer,
        AgentPortOnlyTopologyResolver,
    >,
}

#[derive(Clone, Copy)]
struct AgentPortOnlyAuthorizer;

#[derive(Clone, Copy)]
struct AgentPortOnlyTopologyResolver;

#[async_trait]
impl ModuleArtifactNodeReconciliationAuthorizer for AgentPortOnlyAuthorizer {
    async fn authorize_request(
        &self,
        _command: &ModuleArtifactNodeReconciliationRequest,
    ) -> Result<(), ModuleArtifactNodeReconciliationError> {
        Err(ModuleArtifactNodeReconciliationError::AuthorizationDenied(
            "the artifact node agent port cannot request desired topology".to_string(),
        ))
    }

    async fn authorize_assignment_claim(
        &self,
        _command: &ModuleArtifactNodeAssignmentClaimCommand,
    ) -> Result<(), ModuleArtifactNodeReconciliationError> {
        Ok(())
    }

    async fn authorize_assignment_heartbeat(
        &self,
        _command: &ModuleArtifactNodeAssignmentHeartbeatCommand,
    ) -> Result<(), ModuleArtifactNodeReconciliationError> {
        Ok(())
    }

    async fn authorize_report(
        &self,
        _command: &ModuleArtifactNodeAssignmentReport,
    ) -> Result<(), ModuleArtifactNodeReconciliationError> {
        Ok(())
    }
}

#[async_trait]
impl ModuleArtifactNodeTopologyResolver for AgentPortOnlyTopologyResolver {
    async fn resolve(
        &self,
        _policy_revision: &str,
    ) -> Result<ModuleArtifactNodeTopologySnapshot, String> {
        Err("the artifact node agent port cannot resolve deployment topology".to_string())
    }
}

impl SeaOrmModuleArtifactNodeAgentService {
    pub(crate) fn with_infrastructure(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            inner: SeaOrmModuleArtifactNodeReconciliationService::with_infrastructure(
                db,
                AgentPortOnlyAuthorizer,
                AgentPortOnlyTopologyResolver,
                infrastructure,
            ),
        }
    }
}

/// Read-only execution gate for one server node. It proves that the exact
/// installation selected by the runtime still belongs to the durable observed
/// reconciliation head; neither a node-local cache nor a worker readiness
/// response can substitute for that proof.
#[derive(Clone)]
pub struct SeaOrmArtifactNodeReadiness {
    db: DatabaseConnection,
    node_id: Uuid,
}

impl SeaOrmArtifactNodeReadiness {
    pub fn new(
        db: DatabaseConnection,
        node_id: Uuid,
    ) -> Result<Self, ModuleArtifactNodeReconciliationError> {
        if node_id.is_nil() {
            return Err(ModuleArtifactNodeReconciliationError::InvalidNodeIdentity);
        }
        Ok(Self { db, node_id })
    }

    /// Requires the complete durable assignment identity and current canonical
    /// effective-policy revision for the admitted installation selected by the
    /// runtime. The method deliberately does not accept a slug, release tag,
    /// cache generation, or agent report.
    pub async fn require_active(
        &self,
        artifact: &InstalledModuleArtifact,
        policy_revision: &str,
    ) -> Result<(), ModuleArtifactNodeReconciliationError> {
        let identity = InstallationIdentity {
            installation_scope: match artifact.scope {
                ModuleInstallationScope::Platform => ModuleArtifactNodeInstallationScope::Platform,
                ModuleInstallationScope::Tenant { .. } => {
                    ModuleArtifactNodeInstallationScope::Tenant
                }
            },
            release_digest: artifact.reference.digest.clone(),
            payload_digest: artifact.descriptor.artifact_digest.clone(),
            payload_kind: artifact.descriptor.payload_kind,
            payload_media_type: artifact.payload_media_type.clone(),
            admission_revision: 0,
            dependency_graph_revision: artifact.dependency_lock.graph_revision,
            dependency_graph_digest: artifact.dependency_lock.graph_digest.clone(),
            capability_grant_revision: artifact.capability_grant_revision,
            executor_abi: artifact.descriptor.runtime_abi.clone(),
        };
        if artifact.installation_id.is_nil()
            || !valid_digest(policy_revision)
            || !valid_digest(&identity.release_digest)
            || !valid_digest(&identity.payload_digest)
            || !valid_text(&identity.payload_media_type, MAX_PAYLOAD_MEDIA_TYPE_BYTES)
            || identity.dependency_graph_revision == 0
            || !valid_digest(&identity.dependency_graph_digest)
            || identity.capability_grant_revision == 0
            || !valid_text(&identity.executor_abi, MAX_EXECUTOR_ABI_BYTES)
        {
            return Err(ModuleArtifactNodeReconciliationError::InvalidRuntimeIdentity);
        }
        self.require_active_identity(artifact.installation_id, &identity, policy_revision)
            .await
    }

    async fn require_active_identity(
        &self,
        installation_id: Uuid,
        identity: &InstallationIdentity,
        policy_revision: &str,
    ) -> Result<(), ModuleArtifactNodeReconciliationError> {
        let backend = self.db.get_database_backend();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT reconciliation.reconciliation_id \
                     FROM module_artifact_node_reconciliation_state state \
                     JOIN module_artifact_node_reconciliations reconciliation \
                       ON reconciliation.reconciliation_id = state.observed_reconciliation_id \
                     JOIN module_artifact_node_reconciliation_assignments assignment \
                       ON assignment.reconciliation_id = reconciliation.reconciliation_id \
                     JOIN module_artifact_installations installation \
                       ON installation.installation_id = assignment.installation_id \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     WHERE state.state_id = {} \
                       AND reconciliation.status = 'converged' \
                       AND reconciliation.policy_revision = {} \
                       AND assignment.node_id = {} \
                       AND assignment.installation_id = {} \
                       AND assignment.phase = 'active' \
                       AND assignment.installation_scope = {} \
                       AND assignment.release_digest = {} \
                       AND assignment.payload_digest = {} \
                       AND assignment.payload_kind = {} \
                       AND assignment.payload_media_type = {} \
                       AND assignment.dependency_graph_revision = {} \
                       AND assignment.dependency_graph_digest = {} \
                       AND assignment.capability_grant_revision = {} \
                       AND assignment.executor_abi = {} \
                       AND assignment.policy_revision = reconciliation.policy_revision \
                       AND assignment.observed_installation_scope = assignment.installation_scope \
                       AND assignment.observed_release_digest = assignment.release_digest \
                       AND assignment.observed_payload_digest = assignment.payload_digest \
                       AND assignment.observed_payload_kind = assignment.payload_kind \
                       AND assignment.observed_payload_media_type = assignment.payload_media_type \
                       AND assignment.observed_admission_revision = assignment.admission_revision \
                       AND assignment.observed_dependency_graph_revision = assignment.dependency_graph_revision \
                       AND assignment.observed_dependency_graph_digest = assignment.dependency_graph_digest \
                       AND assignment.observed_capability_grant_revision = assignment.capability_grant_revision \
                       AND assignment.observed_executor_abi = assignment.executor_abi \
                       AND assignment.observed_policy_revision = assignment.policy_revision \
                       AND installation.scope_kind = assignment.installation_scope \
                       AND installation.manifest_digest = assignment.release_digest \
                       AND installation.payload_digest = assignment.payload_digest \
                       AND installation.payload_kind = assignment.payload_kind \
                       AND installation.dependency_graph_revision = assignment.dependency_graph_revision \
                       AND installation.dependency_graph_digest = assignment.dependency_graph_digest \
                       AND installation.capability_grant_revision = assignment.capability_grant_revision \
                       AND installation.runtime_abi = assignment.executor_abi \
                       AND admission.revision = assignment.admission_revision \
                       AND admission.media_type = assignment.payload_media_type \
                       AND admission.status = 'active' \
                       AND NOT EXISTS ( \
                           SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                           WHERE uninstall.installation_id = installation.installation_id \
                       ) \
                     LIMIT 1",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    placeholder(backend, 10),
                    placeholder(backend, 11),
                    placeholder(backend, 12),
                    placeholder(backend, 13),
                ),
                vec![
                    RECONCILIATION_STATE_ID.into(),
                    policy_revision.to_string().into(),
                    uuid_value(self.node_id, backend),
                    uuid_value(installation_id, backend),
                    identity.installation_scope.as_str().into(),
                    identity.release_digest.clone().into(),
                    identity.payload_digest.clone().into(),
                    identity.payload_kind.as_str().into(),
                    identity.payload_media_type.clone().into(),
                    revision_value(identity.dependency_graph_revision)?,
                    identity.dependency_graph_digest.clone().into(),
                    revision_value(identity.capability_grant_revision)?,
                    identity.executor_abi.clone().into(),
                ],
            ))
            .await
            .map_err(store_error)?;
        if row.is_some() {
            Ok(())
        } else {
            Err(ModuleArtifactNodeReconciliationError::AssignmentUnavailable)
        }
    }
}

impl<A, T> SeaOrmModuleArtifactNodeReconciliationService<A, T>
where
    A: ModuleArtifactNodeReconciliationAuthorizer,
    T: ModuleArtifactNodeTopologyResolver,
{
    pub(crate) fn with_infrastructure(
        db: DatabaseConnection,
        authorizer: A,
        topology: T,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            authorizer,
            topology,
            infrastructure,
        }
    }

    /// Persists a complete owner-selected desired assignment set and its
    /// transactional outbox event. Artifact metadata is read again under the
    /// transaction lock so a lifecycle change cannot race the desired record.
    pub async fn request(
        &self,
        command: ModuleArtifactNodeReconciliationRequest,
    ) -> Result<ModuleArtifactNodeReconciliationReceipt, ModuleArtifactNodeReconciliationError>
    {
        validate_request(&command)?;
        self.authorizer.authorize_request(&command).await?;
        let request_digest = digest_json(&command).map_err(digest_error)?;
        let principal_id = command.context.actor_id.to_string();
        if let Some(operation) = load_operation(
            &self.db,
            command.context.idempotency_key,
            "request",
            &request_digest,
            &principal_id,
            Some(&command.context),
        )
        .await?
        {
            return replay_request(&operation);
        }

        let topology = self
            .topology
            .resolve(&command.policy_revision)
            .await
            .map_err(ModuleArtifactNodeReconciliationError::TopologyResolution)?;
        validate_topology(&topology)?;
        if topology.topology_digest != command.topology_digest {
            return Err(ModuleArtifactNodeReconciliationError::TopologyDigestMismatch);
        }
        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(operation) = reserve_operation(
            &transaction,
            command.context.idempotency_key,
            "request",
            &request_digest,
            &principal_id,
            Some(&command.context),
        )
        .await?
        {
            transaction.commit().await.map_err(store_error)?;
            return replay_request(&operation);
        }
        let state = load_state(&transaction, true).await?;
        if state.revision != command.expected_reconciliation_state_revision {
            return Err(ModuleArtifactNodeReconciliationError::RevisionConflict {
                expected: command.expected_reconciliation_state_revision,
                current: state.revision,
            });
        }
        let prior_desired = match state.desired_id {
            Some(reconciliation_id) => {
                Some(load_reconciliation(&transaction, reconciliation_id, true).await?)
            }
            None => None,
        };
        if prior_desired.as_ref().is_some_and(|reconciliation| {
            matches!(
                reconciliation.status,
                ModuleArtifactNodeReconciliationStatus::Preparing
                    | ModuleArtifactNodeReconciliationStatus::Activating
            )
        }) {
            return Err(ModuleArtifactNodeReconciliationError::ReconciliationInProgress);
        }
        let resolved_assignments =
            resolve_assignment_identities(&transaction, &topology, true).await?;
        let predecessor = match state.observed_id {
            Some(reconciliation_id) => {
                Some(load_reconciliation(&transaction, reconciliation_id, true).await?)
            }
            None => None,
        };
        if predecessor.as_ref().is_some_and(|reconciliation| {
            reconciliation.status == ModuleArtifactNodeReconciliationStatus::Converged
                && reconciliation.topology_digest == topology.topology_digest
                && reconciliation.policy_revision == command.policy_revision
                && reconciliation_matches_identities(reconciliation, &resolved_assignments)
        }) {
            return Err(ModuleArtifactNodeReconciliationError::NoReconciliationChange);
        }
        let reconciliation_revision =
            prior_desired.as_ref().map_or(Ok(1_u64), |reconciliation| {
                reconciliation
                    .reconciliation_revision
                    .checked_add(1)
                    .ok_or(ModuleArtifactNodeReconciliationError::RevisionOverflow)
            })?;
        let reconciliation_state_revision = state
            .revision
            .checked_add(1)
            .ok_or(ModuleArtifactNodeReconciliationError::RevisionOverflow)?;
        let reconciliation_id = self.infrastructure.new_id();
        insert_reconciliation(
            &transaction,
            reconciliation_id,
            predecessor
                .as_ref()
                .map(|reconciliation| reconciliation.reconciliation_id),
            reconciliation_revision,
            &topology,
            &command,
        )
        .await?;
        insert_assignments(
            &transaction,
            reconciliation_id,
            &command.policy_revision,
            &resolved_assignments,
        )
        .await?;
        advance_state(
            &transaction,
            state.revision,
            reconciliation_state_revision,
            Some(reconciliation_id),
            state.observed_id,
        )
        .await?;
        let receipt = ModuleArtifactNodeReconciliationReceipt {
            reconciliation_id,
            reconciliation_revision,
            reconciliation_state_revision,
            status: ModuleArtifactNodeReconciliationStatus::Preparing,
            created: true,
        };
        complete_request_operation(&transaction, command.context.idempotency_key, &receipt).await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &command.context,
                    DomainEvent::ModuleArtifactNodeReconciliationRequested {
                        reconciliation_id,
                        predecessor_reconciliation_id: predecessor
                            .as_ref()
                            .map(|reconciliation| reconciliation.reconciliation_id),
                        reconciliation_revision,
                        reconciliation_state_revision,
                        topology_digest: topology.topology_digest,
                        policy_revision: command.policy_revision,
                        target_assignments: u32::try_from(resolved_assignments.len())
                            .map_err(|_| ModuleArtifactNodeReconciliationError::InvalidTopology)?,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(receipt)
    }

    /// Claims one exact owner-issued assignment. A stale lease may be
    /// reclaimed by the same or a replacement authenticated agent.
    pub async fn claim_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentClaimCommand,
    ) -> Result<Option<ModuleArtifactNodeAssignmentWorkItem>, ModuleArtifactNodeReconciliationError>
    {
        validate_assignment_claim(&command)?;
        self.authorizer.authorize_assignment_claim(&command).await?;
        let transaction = self.db.begin().await.map_err(store_error)?;
        let state = load_state(&transaction, true).await?;
        let Some(reconciliation_id) = state.desired_id else {
            transaction.commit().await.map_err(store_error)?;
            return Ok(None);
        };
        let reconciliation = load_reconciliation(&transaction, reconciliation_id, true).await?;
        if !reconciliation.status.is_claimable() {
            transaction.commit().await.map_err(store_error)?;
            return Ok(None);
        }
        validate_reconciliation_installation_identities(&transaction, &reconciliation).await?;
        let now = self.infrastructure.now();
        let Some(assignment) = load_next_assignment_for_node(
            &transaction,
            reconciliation_id,
            command.node_id,
            &command.agent_id,
            &now,
        )
        .await?
        else {
            transaction.commit().await.map_err(store_error)?;
            return Ok(None);
        };
        if let (Some(claim_id), Some(claimed_by_agent), Some(lease_expires_at)) = (
            assignment.active_claim_id,
            assignment.claimed_by_agent.as_deref(),
            assignment.claim_expires_at,
        ) && claimed_by_agent == command.agent_id.as_str()
        {
            transaction.commit().await.map_err(store_error)?;
            return Ok(Some(ModuleArtifactNodeAssignmentWorkItem {
                claim_id,
                lease_expires_at,
                expected_observation_revision: assignment.observation_revision,
                reconciliation: work_identity(&reconciliation),
                assignment,
            }));
        }
        let lease_expires_at = now
            .checked_add_signed(Duration::seconds(
                MODULE_ARTIFACT_NODE_ASSIGNMENT_LEASE_SECONDS,
            ))
            .ok_or(ModuleArtifactNodeReconciliationError::LeaseOverflow)?;
        let claim_id = self.infrastructure.new_id();
        claim_assignment(
            &transaction,
            reconciliation_id,
            &assignment,
            claim_id,
            &command.agent_id,
            &lease_expires_at,
            &now,
        )
        .await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(Some(ModuleArtifactNodeAssignmentWorkItem {
            claim_id,
            lease_expires_at,
            expected_observation_revision: assignment.observation_revision,
            reconciliation: work_identity(&reconciliation),
            assignment,
        }))
    }

    /// Renews a still-current claim without mutating desired or observed state.
    pub async fn heartbeat_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentHeartbeatCommand,
    ) -> Result<ModuleArtifactNodeAssignmentHeartbeatReceipt, ModuleArtifactNodeReconciliationError>
    {
        validate_assignment_heartbeat(&command)?;
        self.authorizer
            .authorize_assignment_heartbeat(&command)
            .await?;
        let now = self.infrastructure.now();
        let lease_expires_at = now
            .checked_add_signed(Duration::seconds(
                MODULE_ARTIFACT_NODE_ASSIGNMENT_LEASE_SECONDS,
            ))
            .ok_or(ModuleArtifactNodeReconciliationError::LeaseOverflow)?;
        let transaction = self.db.begin().await.map_err(store_error)?;
        heartbeat_assignment(&transaction, &command, &lease_expires_at, &now).await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleArtifactNodeAssignmentHeartbeatReceipt {
            claim_id: command.claim_id,
            lease_expires_at,
        })
    }

    /// Records one fenced node observation. An agent cannot report `active`:
    /// after every required assignment is healthy, this owner atomically marks
    /// the complete set active and advances the observed reconciliation head.
    pub async fn report(
        &self,
        command: ModuleArtifactNodeAssignmentReport,
    ) -> Result<ModuleArtifactNodeAssignmentReportReceipt, ModuleArtifactNodeReconciliationError>
    {
        validate_report(&command)?;
        self.authorizer.authorize_report(&command).await?;
        let request_digest = digest_json(&command).map_err(digest_error)?;
        if let Some(operation) = load_operation(
            &self.db,
            command.idempotency_key,
            "report",
            &request_digest,
            &command.agent_id,
            None,
        )
        .await?
        {
            return replay_report(&operation);
        }
        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(operation) = reserve_operation(
            &transaction,
            command.idempotency_key,
            "report",
            &request_digest,
            &command.agent_id,
            None,
        )
        .await?
        {
            transaction.commit().await.map_err(store_error)?;
            return replay_report(&operation);
        }
        let state = load_state(&transaction, true).await?;
        if state.desired_id != Some(command.reconciliation_id) {
            return Err(ModuleArtifactNodeReconciliationError::StaleReconciliation);
        }
        let reconciliation =
            load_reconciliation(&transaction, command.reconciliation_id, true).await?;
        if matches!(
            reconciliation.status,
            ModuleArtifactNodeReconciliationStatus::Failed
                | ModuleArtifactNodeReconciliationStatus::Superseded
        ) {
            return Err(ModuleArtifactNodeReconciliationError::TerminalReconciliation);
        }
        let assignment = load_assignment(
            &transaction,
            command.reconciliation_id,
            command.node_id,
            command.installation_id,
            true,
        )
        .await?;
        validate_report_identity(&command, &assignment)?;
        validate_current_assignment_identity(&transaction, &assignment, true).await?;
        let now = self.infrastructure.now();
        if assignment.active_claim_id != Some(command.claim_id)
            || assignment.claimed_by_agent.as_deref() != Some(command.agent_id.as_str())
            || assignment
                .claim_expires_at
                .is_none_or(|expires_at| expires_at < now)
        {
            return Err(ModuleArtifactNodeReconciliationError::ClaimConflict);
        }
        if assignment.observation_revision != command.expected_observation_revision {
            return Err(
                ModuleArtifactNodeReconciliationError::ObservationRevisionConflict {
                    expected: command.expected_observation_revision,
                    current: assignment.observation_revision,
                },
            );
        }
        validate_transition(assignment.phase, command.phase)?;
        let observation_revision = assignment
            .observation_revision
            .checked_add(1)
            .ok_or(ModuleArtifactNodeReconciliationError::RevisionOverflow)?;
        update_assignment(
            &transaction,
            &command,
            observation_revision,
            &request_digest,
            &now,
        )
        .await?;

        let phase_counts = load_phase_counts(&transaction, command.reconciliation_id).await?;
        let target_assignments = usize::from(reconciliation.target_assignment_count);
        let mut next_status = reconciliation.status;
        let mut reconciliation_state_revision = state.revision;
        let mut observed_reconciliation_id = state.observed_id;
        let mut status_failure = None;
        if command.phase == ModuleReconciliationPhase::Failed {
            status_failure = command.failure.clone();
            next_status = if reconciliation.status
                == ModuleArtifactNodeReconciliationStatus::Converged
            {
                observed_reconciliation_id = None;
                ModuleArtifactNodeReconciliationStatus::Degraded
            } else if reconciliation.status == ModuleArtifactNodeReconciliationStatus::Degraded {
                ModuleArtifactNodeReconciliationStatus::Degraded
            } else {
                ModuleArtifactNodeReconciliationStatus::Failed
            };
        } else if reconciliation.status == ModuleArtifactNodeReconciliationStatus::Preparing
            && phase_counts.prepared + phase_counts.healthy == target_assignments
        {
            if phase_counts.healthy == target_assignments {
                activate_healthy_assignments(&transaction, command.reconciliation_id).await?;
                next_status = ModuleArtifactNodeReconciliationStatus::Converged;
                observed_reconciliation_id = Some(reconciliation.reconciliation_id);
            } else {
                next_status = ModuleArtifactNodeReconciliationStatus::Activating;
            }
        } else if reconciliation.status == ModuleArtifactNodeReconciliationStatus::Activating
            && phase_counts.healthy == target_assignments
        {
            activate_healthy_assignments(&transaction, command.reconciliation_id).await?;
            next_status = ModuleArtifactNodeReconciliationStatus::Converged;
            observed_reconciliation_id = Some(reconciliation.reconciliation_id);
        }
        if next_status != reconciliation.status {
            reconciliation_state_revision = state
                .revision
                .checked_add(1)
                .ok_or(ModuleArtifactNodeReconciliationError::RevisionOverflow)?;
            update_reconciliation_status(
                &transaction,
                reconciliation.reconciliation_id,
                reconciliation.status,
                next_status,
                status_failure.as_ref(),
            )
            .await?;
            if next_status == ModuleArtifactNodeReconciliationStatus::Converged
                && let Some(previous_observed) = state.observed_id.filter(|reconciliation_id| {
                    *reconciliation_id != reconciliation.reconciliation_id
                })
            {
                supersede_reconciliation(&transaction, previous_observed).await?;
            }
            advance_state(
                &transaction,
                state.revision,
                reconciliation_state_revision,
                state.desired_id,
                observed_reconciliation_id,
            )
            .await?;
        }
        let receipt = ModuleArtifactNodeAssignmentReportReceipt {
            reconciliation_id: reconciliation.reconciliation_id,
            reconciliation_revision: reconciliation.reconciliation_revision,
            reconciliation_state_revision,
            reconciliation_status: next_status,
            node_id: command.node_id,
            installation_id: command.installation_id,
            observation_revision,
            phase: command.phase,
            created: true,
        };
        complete_report_operation(&transaction, command.idempotency_key, &receipt).await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    None,
                    DomainEvent::ModuleArtifactNodeAssignmentObserved {
                        reconciliation_id: reconciliation.reconciliation_id,
                        node_id: command.node_id,
                        installation_id: command.installation_id,
                        release_digest: command.release_digest,
                        reporter_id: command.agent_id,
                        observation_revision,
                        phase: command.phase.as_str().to_string(),
                        report_digest: request_digest,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        if next_status != reconciliation.status {
            self.infrastructure
                .write_event(
                    &transaction,
                    self.infrastructure.event_envelope(
                        None,
                        None,
                        DomainEvent::ModuleArtifactNodeReconciliationStatusChanged {
                            reconciliation_id: reconciliation.reconciliation_id,
                            reconciliation_revision: reconciliation.reconciliation_revision,
                            reconciliation_state_revision,
                            status: next_status.as_str().to_string(),
                            observed_reconciliation_id,
                            failure_code: status_failure.map(|failure| failure.code),
                        },
                    ),
                )
                .await
                .map_err(store_error)?;
        }
        transaction.commit().await.map_err(store_error)?;
        Ok(receipt)
    }

    pub async fn state(
        &self,
    ) -> Result<ModuleDesiredObservedState, ModuleArtifactNodeReconciliationError> {
        load_state(&self.db, false).await
    }

    pub async fn get(
        &self,
        reconciliation_id: Uuid,
    ) -> Result<ModuleArtifactNodeReconciliation, ModuleArtifactNodeReconciliationError> {
        if reconciliation_id.is_nil() {
            return Err(ModuleArtifactNodeReconciliationError::InvalidCommand);
        }
        load_reconciliation(&self.db, reconciliation_id, false).await
    }
}

#[async_trait]
impl<A, T> ModuleArtifactNodeAgentPort for SeaOrmModuleArtifactNodeReconciliationService<A, T>
where
    A: ModuleArtifactNodeReconciliationAuthorizer,
    T: ModuleArtifactNodeTopologyResolver,
{
    async fn claim_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentClaimCommand,
    ) -> Result<Option<ModuleArtifactNodeAssignmentWorkItem>, ModuleArtifactNodeReconciliationError>
    {
        Self::claim_assignment(self, command).await
    }

    async fn heartbeat_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentHeartbeatCommand,
    ) -> Result<ModuleArtifactNodeAssignmentHeartbeatReceipt, ModuleArtifactNodeReconciliationError>
    {
        Self::heartbeat_assignment(self, command).await
    }

    async fn report_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentReport,
    ) -> Result<ModuleArtifactNodeAssignmentReportReceipt, ModuleArtifactNodeReconciliationError>
    {
        Self::report(self, command).await
    }
}

#[async_trait]
impl ModuleArtifactNodeAgentPort for SeaOrmModuleArtifactNodeAgentService {
    async fn claim_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentClaimCommand,
    ) -> Result<Option<ModuleArtifactNodeAssignmentWorkItem>, ModuleArtifactNodeReconciliationError>
    {
        self.inner.claim_assignment(command).await
    }

    async fn heartbeat_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentHeartbeatCommand,
    ) -> Result<ModuleArtifactNodeAssignmentHeartbeatReceipt, ModuleArtifactNodeReconciliationError>
    {
        self.inner.heartbeat_assignment(command).await
    }

    async fn report_assignment(
        &self,
        command: ModuleArtifactNodeAssignmentReport,
    ) -> Result<ModuleArtifactNodeAssignmentReportReceipt, ModuleArtifactNodeReconciliationError>
    {
        self.inner.report(command).await
    }
}

#[derive(Serialize)]
struct TopologyDigestInput<'a> {
    contract: &'static str,
    topology_reference: &'a str,
    assignments: &'a [ModuleArtifactNodeAssignmentTarget],
}

/// Computes the canonical topology digest that guards target selection.
pub fn module_artifact_node_topology_digest(
    topology_reference: &str,
    assignments: &[ModuleArtifactNodeAssignmentTarget],
) -> Result<String, ModuleArtifactNodeReconciliationError> {
    digest_json(&TopologyDigestInput {
        contract: TOPOLOGY_DIGEST_CONTRACT,
        topology_reference,
        assignments,
    })
    .map_err(digest_error)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedAssignment {
    target: ModuleArtifactNodeAssignmentTarget,
    identity: InstallationIdentity,
}

/// Identity reloaded from the installation/admission owner tables. It does not
/// carry a node because node placement belongs to the topology resolver, never
/// to an installation record.
#[derive(Clone, Debug, PartialEq, Eq)]
struct InstallationIdentity {
    installation_scope: ModuleArtifactNodeInstallationScope,
    release_digest: String,
    payload_digest: String,
    payload_kind: ArtifactPayloadKind,
    payload_media_type: String,
    admission_revision: u64,
    dependency_graph_revision: u64,
    dependency_graph_digest: String,
    capability_grant_revision: u64,
    executor_abi: String,
}

impl InstallationIdentity {
    fn matches_assignment(&self, assignment: &ModuleArtifactNodeAssignment) -> bool {
        self.installation_scope == assignment.installation_scope
            && self.release_digest == assignment.release_digest
            && self.payload_digest == assignment.payload_digest
            && self.payload_kind == assignment.payload_kind
            && self.payload_media_type == assignment.payload_media_type
            && self.admission_revision == assignment.admission_revision
            && self.dependency_graph_revision == assignment.dependency_graph_revision
            && self.dependency_graph_digest == assignment.dependency_graph_digest
            && self.capability_grant_revision == assignment.capability_grant_revision
            && self.executor_abi == assignment.executor_abi
    }
}

#[derive(Clone, Debug)]
struct OperationRecord {
    operation_kind: String,
    request_digest: String,
    principal_id: String,
    trace_id: Option<String>,
    correlation_id: Option<Uuid>,
    reconciliation_id: Option<Uuid>,
    reconciliation_revision: Option<u64>,
    reconciliation_state_revision: Option<u64>,
    reconciliation_status: Option<ModuleArtifactNodeReconciliationStatus>,
    node_id: Option<Uuid>,
    installation_id: Option<Uuid>,
    observation_revision: Option<u64>,
    assignment_phase: Option<ModuleReconciliationPhase>,
    completed: bool,
}

struct OperationCompletion {
    reconciliation_id: Uuid,
    reconciliation_revision: u64,
    reconciliation_state_revision: u64,
    reconciliation_status: ModuleArtifactNodeReconciliationStatus,
    node_id: Option<Uuid>,
    installation_id: Option<Uuid>,
    observation_revision: Option<u64>,
    assignment_phase: Option<ModuleReconciliationPhase>,
}

#[derive(Default)]
struct PhaseCounts {
    prepared: usize,
    healthy: usize,
}

#[derive(Debug, Error)]
pub enum ModuleArtifactNodeReconciliationError {
    #[error("artifact node reconciliation command is invalid")]
    InvalidCommand,
    #[error("artifact node identity must be a non-nil UUID")]
    InvalidNodeIdentity,
    #[error("artifact runtime installation identity is invalid")]
    InvalidRuntimeIdentity,
    #[error("artifact node reconciliation topology is invalid")]
    InvalidTopology,
    #[error("artifact node reconciliation topology does not match the requested digest")]
    TopologyDigestMismatch,
    #[error("artifact node topology resolution failed: {0}")]
    TopologyResolution(String),
    #[error(
        "artifact node reconciliation state revision conflict: expected {expected}, current {current}"
    )]
    RevisionConflict { expected: u64, current: u64 },
    #[error(
        "artifact node assignment observation revision conflict: expected {expected}, current {current}"
    )]
    ObservationRevisionConflict { expected: u64, current: u64 },
    #[error("an artifact node reconciliation is already preparing or activating")]
    ReconciliationInProgress,
    #[error("artifact node reconciliation does not change topology, policy, or admitted identity")]
    NoReconciliationChange,
    #[error("artifact node reconciliation is stale")]
    StaleReconciliation,
    #[error("artifact node reconciliation is terminal")]
    TerminalReconciliation,
    #[error("artifact node reconciliation was not found")]
    ReconciliationNotFound,
    #[error("artifact node reconciliation assignment was not found")]
    AssignmentNotFound,
    #[error("artifact node has no current active observed assignment for this installation")]
    AssignmentUnavailable,
    #[error("artifact node assignment claim conflicts or has expired")]
    ClaimConflict,
    #[error("artifact node assignment lease overflow")]
    LeaseOverflow,
    #[error("artifact node assignment report identity does not match the desired assignment")]
    ObservationIdentityMismatch,
    #[error("artifact node assignment phase transition is invalid")]
    InvalidTransition,
    #[error("artifact node reconciliation idempotency key conflicts with another command")]
    IdempotencyConflict,
    #[error("artifact node reconciliation revision overflow")]
    RevisionOverflow,
    #[error("artifact node reconciliation authorization denied: {0}")]
    AuthorizationDenied(String),
    #[error("artifact node reconciliation store failed: {0}")]
    Store(String),
}

fn validate_request(
    command: &ModuleArtifactNodeReconciliationRequest,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    if !valid_digest(&command.policy_revision)
        || !valid_digest(&command.topology_digest)
        || !valid_platform_command_context(&command.context)
    {
        return Err(ModuleArtifactNodeReconciliationError::InvalidCommand);
    }
    Ok(())
}

fn valid_platform_command_context(context: &ModuleCommandContext) -> bool {
    context.tenant_id.is_none() && context.validate().is_ok()
}

fn validate_topology(
    topology: &ModuleArtifactNodeTopologySnapshot,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    if !valid_reference(&topology.topology_reference)
        || topology.topology_reference.len() > MAX_REFERENCE_BYTES
        || !valid_digest(&topology.topology_digest)
        || topology.assignments.is_empty()
        || topology.assignments.len() > MAX_TARGET_ASSIGNMENTS
        || topology
            .assignments
            .iter()
            .any(|assignment| assignment.node_id.is_nil() || assignment.installation_id.is_nil())
        || topology.assignments.windows(2).any(|assignments| {
            assignment_sort_key(&assignments[0]) >= assignment_sort_key(&assignments[1])
        })
    {
        return Err(ModuleArtifactNodeReconciliationError::InvalidTopology);
    }
    let expected =
        module_artifact_node_topology_digest(&topology.topology_reference, &topology.assignments)?;
    if topology.topology_digest != expected {
        return Err(ModuleArtifactNodeReconciliationError::InvalidTopology);
    }
    Ok(())
}

fn assignment_sort_key(assignment: &ModuleArtifactNodeAssignmentTarget) -> (Uuid, Uuid) {
    (assignment.node_id, assignment.installation_id)
}

fn validate_assignment_claim(
    command: &ModuleArtifactNodeAssignmentClaimCommand,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    if command.node_id.is_nil() || !valid_text(&command.agent_id, MAX_AGENT_ID_BYTES) {
        return Err(ModuleArtifactNodeReconciliationError::InvalidCommand);
    }
    Ok(())
}

fn validate_assignment_heartbeat(
    command: &ModuleArtifactNodeAssignmentHeartbeatCommand,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    if command.claim_id.is_nil() || !valid_text(&command.agent_id, MAX_AGENT_ID_BYTES) {
        return Err(ModuleArtifactNodeReconciliationError::InvalidCommand);
    }
    Ok(())
}

fn validate_report(
    command: &ModuleArtifactNodeAssignmentReport,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let health_evidence_valid = command.health_evidence.as_ref().is_none_or(|evidence| {
        valid_reference(&evidence.reference)
            && evidence.reference.len() <= MAX_REFERENCE_BYTES
            && valid_digest(&evidence.digest)
    });
    let failure_valid = command.failure.as_ref().is_none_or(|failure| {
        valid_text(&failure.code, MAX_FAILURE_CODE_BYTES)
            && valid_text(&failure.detail, MAX_FAILURE_DETAIL_BYTES)
    });
    let agent_reportable_phase = matches!(
        command.phase,
        ModuleReconciliationPhase::Prepared
            | ModuleReconciliationPhase::Healthy
            | ModuleReconciliationPhase::Failed
    );
    if command.claim_id.is_nil()
        || command.reconciliation_id.is_nil()
        || command.node_id.is_nil()
        || command.installation_id.is_nil()
        || !agent_reportable_phase
        || !command
            .phase
            .permits_report_payload(command.health_evidence.is_some(), command.failure.is_some())
        || !health_evidence_valid
        || !failure_valid
        || !valid_digest(&command.release_digest)
        || !valid_digest(&command.payload_digest)
        || !valid_text(&command.payload_media_type, MAX_PAYLOAD_MEDIA_TYPE_BYTES)
        || command.admission_revision == 0
        || command.dependency_graph_revision == 0
        || !valid_digest(&command.dependency_graph_digest)
        || command.capability_grant_revision == 0
        || !valid_text(&command.executor_abi, MAX_EXECUTOR_ABI_BYTES)
        || !valid_digest(&command.policy_revision)
        || !valid_text(&command.agent_id, MAX_AGENT_ID_BYTES)
        || command.idempotency_key.is_nil()
    {
        return Err(ModuleArtifactNodeReconciliationError::InvalidCommand);
    }
    Ok(())
}

fn validate_transition(
    current: ModuleReconciliationPhase,
    next: ModuleReconciliationPhase,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    if !current.allows_standard_transition_to(next) {
        return Err(ModuleArtifactNodeReconciliationError::InvalidTransition);
    }
    Ok(())
}

async fn resolve_assignment_identities(
    connection: &impl ConnectionTrait,
    topology: &ModuleArtifactNodeTopologySnapshot,
    lock_rows: bool,
) -> Result<Vec<ResolvedAssignment>, ModuleArtifactNodeReconciliationError> {
    let mut resolved = Vec::with_capacity(topology.assignments.len());
    for target in &topology.assignments {
        resolved.push(ResolvedAssignment {
            target: target.clone(),
            identity: load_current_installation_identity(
                connection,
                target.installation_id,
                lock_rows,
            )
            .await?,
        });
    }
    Ok(resolved)
}

async fn load_current_installation_identity(
    connection: &impl ConnectionTrait,
    installation_id: Uuid,
    lock_row: bool,
) -> Result<InstallationIdentity, ModuleArtifactNodeReconciliationError> {
    let backend = connection.get_database_backend();
    let lock = if lock_row && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT installation.scope_kind, installation.manifest_digest, installation.payload_digest, \
                        installation.payload_kind, admission.media_type AS payload_media_type, \
                        admission.revision AS admission_revision, \
                        installation.dependency_graph_revision, installation.dependency_graph_digest, \
                        installation.capability_grant_revision, installation.runtime_abi \
                 FROM module_artifact_installations installation \
                 JOIN module_artifact_admissions admission \
                   ON admission.installation_id = installation.installation_id \
                 WHERE installation.installation_id = {} \
                   AND admission.status IN ('admitted', 'installed', 'active') \
                   AND NOT EXISTS ( \
                       SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                       WHERE uninstall.installation_id = installation.installation_id \
                   ){lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(installation_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleArtifactNodeReconciliationError::StaleReconciliation)?;
    let scope: String = row.try_get("", "scope_kind").map_err(store_error)?;
    let release_digest: String = row.try_get("", "manifest_digest").map_err(store_error)?;
    let payload_digest: String = row.try_get("", "payload_digest").map_err(store_error)?;
    let payload_kind = parse_payload_kind(
        &row.try_get::<String>("", "payload_kind")
            .map_err(store_error)?,
    )?;
    let payload_media_type: String = row.try_get("", "payload_media_type").map_err(store_error)?;
    let dependency_graph_digest: String = row
        .try_get("", "dependency_graph_digest")
        .map_err(store_error)?;
    let executor_abi: String = row.try_get("", "runtime_abi").map_err(store_error)?;
    if !valid_digest(&release_digest)
        || !valid_digest(&payload_digest)
        || !valid_text(&payload_media_type, MAX_PAYLOAD_MEDIA_TYPE_BYTES)
        || !valid_digest(&dependency_graph_digest)
        || !valid_text(&executor_abi, MAX_EXECUTOR_ABI_BYTES)
    {
        return Err(ModuleArtifactNodeReconciliationError::StaleReconciliation);
    }
    Ok(InstallationIdentity {
        installation_scope: ModuleArtifactNodeInstallationScope::parse(&scope)?,
        release_digest,
        payload_digest,
        payload_kind,
        payload_media_type,
        admission_revision: revision_from_row(&row, "admission_revision", false)?,
        dependency_graph_revision: revision_from_row(&row, "dependency_graph_revision", false)?,
        dependency_graph_digest,
        capability_grant_revision: revision_from_row(&row, "capability_grant_revision", false)?,
        executor_abi,
    })
}

async fn insert_reconciliation(
    transaction: &DatabaseTransaction,
    reconciliation_id: Uuid,
    predecessor_reconciliation_id: Option<Uuid>,
    reconciliation_revision: u64,
    topology: &ModuleArtifactNodeTopologySnapshot,
    command: &ModuleArtifactNodeReconciliationRequest,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_node_reconciliations \
                 (reconciliation_id, predecessor_reconciliation_id, reconciliation_revision, \
                  topology_reference, topology_digest, policy_revision, target_assignment_count, \
                  status, requested_by, requested_at, status_changed_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}, 'preparing', {}, {}, {})",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                now_expression(backend),
                now_expression(backend),
            ),
            vec![
                uuid_value(reconciliation_id, backend),
                optional_uuid_value(predecessor_reconciliation_id, backend),
                revision_value(reconciliation_revision)?,
                topology.topology_reference.clone().into(),
                topology.topology_digest.clone().into(),
                command.policy_revision.clone().into(),
                i64::try_from(topology.assignments.len())
                    .map_err(|_| ModuleArtifactNodeReconciliationError::InvalidTopology)?
                    .into(),
                uuid_value(command.context.actor_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    Ok(())
}

async fn insert_assignments(
    transaction: &DatabaseTransaction,
    reconciliation_id: Uuid,
    policy_revision: &str,
    assignments: &[ResolvedAssignment],
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    for (ordinal, assignment) in assignments.iter().enumerate() {
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_node_reconciliation_assignments \
                     (reconciliation_id, node_id, installation_id, installation_scope, \
                      release_digest, payload_digest, payload_kind, payload_media_type, admission_revision, dependency_graph_revision, \
                      dependency_graph_digest, capability_grant_revision, executor_abi, policy_revision, \
                      ordinal, observation_revision, phase) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 0, 'pending')",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    placeholder(backend, 10),
                    placeholder(backend, 11),
                    placeholder(backend, 12),
                    placeholder(backend, 13),
                    placeholder(backend, 14),
                    placeholder(backend, 15),
                ),
                vec![
                    uuid_value(reconciliation_id, backend),
                    uuid_value(assignment.target.node_id, backend),
                    uuid_value(assignment.target.installation_id, backend),
                    assignment.identity.installation_scope.as_str().into(),
                    assignment.identity.release_digest.clone().into(),
                    assignment.identity.payload_digest.clone().into(),
                    assignment.identity.payload_kind.as_str().into(),
                    assignment.identity.payload_media_type.clone().into(),
                    revision_value(assignment.identity.admission_revision)?,
                    revision_value(assignment.identity.dependency_graph_revision)?,
                    assignment.identity.dependency_graph_digest.clone().into(),
                    revision_value(assignment.identity.capability_grant_revision)?,
                    assignment.identity.executor_abi.clone().into(),
                    policy_revision.to_owned().into(),
                    i64::try_from(ordinal)
                        .map_err(|_| ModuleArtifactNodeReconciliationError::InvalidTopology)?
                        .into(),
                ],
            ))
            .await
            .map_err(store_error)?;
    }
    Ok(())
}

async fn validate_reconciliation_installation_identities(
    transaction: &DatabaseTransaction,
    reconciliation: &ModuleArtifactNodeReconciliation,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    for assignment in &reconciliation.assignments {
        validate_current_assignment_identity(transaction, assignment, true).await?;
    }
    Ok(())
}

async fn validate_current_assignment_identity(
    connection: &impl ConnectionTrait,
    assignment: &ModuleArtifactNodeAssignment,
    lock_row: bool,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let current =
        load_current_installation_identity(connection, assignment.installation_id, lock_row)
            .await?;
    if !current.matches_assignment(assignment) {
        return Err(ModuleArtifactNodeReconciliationError::StaleReconciliation);
    }
    Ok(())
}

fn reconciliation_matches_identities(
    reconciliation: &ModuleArtifactNodeReconciliation,
    assignments: &[ResolvedAssignment],
) -> bool {
    reconciliation.assignments.len() == assignments.len()
        && reconciliation
            .assignments
            .iter()
            .zip(assignments)
            .all(|(stored, current)| {
                stored.node_id == current.target.node_id
                    && stored.installation_id == current.target.installation_id
                    && current.identity.matches_assignment(stored)
            })
}

async fn update_assignment(
    transaction: &DatabaseTransaction,
    command: &ModuleArtifactNodeAssignmentReport,
    observation_revision: u64,
    report_digest: &str,
    now: &DateTime<Utc>,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    let health_reference = command
        .health_evidence
        .as_ref()
        .map(|evidence| evidence.reference.clone());
    let health_digest = command
        .health_evidence
        .as_ref()
        .map(|evidence| evidence.digest.clone());
    let failure_code = command.failure.as_ref().map(|failure| failure.code.clone());
    let failure_detail = command
        .failure
        .as_ref()
        .map(|failure| failure.detail.clone());
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_node_reconciliation_assignments \
                 SET observation_revision = {}, phase = {}, observed_installation_scope = {}, \
                     observed_release_digest = {}, observed_payload_digest = {}, \
                     observed_payload_kind = {}, observed_payload_media_type = {}, \
                     observed_admission_revision = {}, observed_dependency_graph_revision = {}, \
                     observed_dependency_graph_digest = {}, observed_capability_grant_revision = {}, \
                     observed_executor_abi = {}, observed_policy_revision = {}, \
                     health_evidence_reference = {}, health_evidence_digest = {}, \
                     failure_code = {}, failure_detail = {}, reported_by = {}, last_report_digest = {}, \
                     first_reported_at = COALESCE(first_reported_at, {}), last_reported_at = {}, \
                     active_claim_id = NULL, claimed_by_agent = NULL, claim_expires_at = NULL \
                 WHERE reconciliation_id = {} AND node_id = {} AND installation_id = {} \
                   AND observation_revision = {} AND active_claim_id = {} \
                   AND claimed_by_agent = {} AND claim_expires_at >= {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                placeholder(backend, 9),
                placeholder(backend, 10),
                placeholder(backend, 11),
                placeholder(backend, 12),
                placeholder(backend, 13),
                placeholder(backend, 14),
                placeholder(backend, 15),
                placeholder(backend, 16),
                placeholder(backend, 17),
                placeholder(backend, 18),
                placeholder(backend, 19),
                placeholder(backend, 20),
                placeholder(backend, 21),
                placeholder(backend, 22),
                placeholder(backend, 23),
                placeholder(backend, 24),
                placeholder(backend, 25),
                placeholder(backend, 26),
                placeholder(backend, 27),
                placeholder(backend, 28),
            ),
            vec![
                revision_value(observation_revision)?,
                command.phase.as_str().into(),
                command.installation_scope.as_str().into(),
                command.release_digest.clone().into(),
                command.payload_digest.clone().into(),
                command.payload_kind.as_str().into(),
                command.payload_media_type.clone().into(),
                revision_value(command.admission_revision)?,
                revision_value(command.dependency_graph_revision)?,
                command.dependency_graph_digest.clone().into(),
                revision_value(command.capability_grant_revision)?,
                command.executor_abi.clone().into(),
                command.policy_revision.clone().into(),
                health_reference.into(),
                health_digest.into(),
                failure_code.into(),
                failure_detail.into(),
                command.agent_id.clone().into(),
                report_digest.to_owned().into(),
                now.to_owned().into(),
                now.to_owned().into(),
                uuid_value(command.reconciliation_id, backend),
                uuid_value(command.node_id, backend),
                uuid_value(command.installation_id, backend),
                revision_value_allow_zero(command.expected_observation_revision)?,
                uuid_value(command.claim_id, backend),
                command.agent_id.clone().into(),
                now.to_owned().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleArtifactNodeReconciliationError::ClaimConflict);
    }
    Ok(())
}

async fn load_phase_counts(
    connection: &impl ConnectionTrait,
    reconciliation_id: Uuid,
) -> Result<PhaseCounts, ModuleArtifactNodeReconciliationError> {
    let backend = connection.get_database_backend();
    let rows = connection
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT phase, COUNT(*) AS count \
                 FROM module_artifact_node_reconciliation_assignments \
                 WHERE reconciliation_id = {} GROUP BY phase",
                placeholder(backend, 1),
            ),
            vec![uuid_value(reconciliation_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    let mut counts = PhaseCounts::default();
    for row in rows {
        let phase = parse_phase(&row.try_get::<String>("", "phase").map_err(store_error)?)?;
        let count: i64 = row.try_get("", "count").map_err(store_error)?;
        let count = usize::try_from(count).map_err(|_| {
            ModuleArtifactNodeReconciliationError::Store(
                "artifact node assignment count is invalid".to_string(),
            )
        })?;
        match phase {
            ModuleReconciliationPhase::Prepared => counts.prepared = count,
            ModuleReconciliationPhase::Healthy => counts.healthy = count,
            ModuleReconciliationPhase::Pending
            | ModuleReconciliationPhase::Active
            | ModuleReconciliationPhase::Failed => {}
        }
    }
    Ok(counts)
}

async fn activate_healthy_assignments(
    transaction: &DatabaseTransaction,
    reconciliation_id: Uuid,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_node_reconciliation_assignments \
                 SET phase = 'active' \
                 WHERE reconciliation_id = {} AND phase = 'healthy'",
                placeholder(backend, 1),
            ),
            vec![uuid_value(reconciliation_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    Ok(())
}

async fn update_reconciliation_status(
    transaction: &DatabaseTransaction,
    reconciliation_id: Uuid,
    expected_status: ModuleArtifactNodeReconciliationStatus,
    next_status: ModuleArtifactNodeReconciliationStatus,
    failure: Option<&ModuleReconciliationFailure>,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    let (converged_at, failed_at) = match next_status {
        ModuleArtifactNodeReconciliationStatus::Converged => {
            (now_expression(backend).to_string(), "NULL".to_string())
        }
        ModuleArtifactNodeReconciliationStatus::Failed
        | ModuleArtifactNodeReconciliationStatus::Degraded => {
            ("NULL".to_string(), now_expression(backend).to_string())
        }
        _ => ("NULL".to_string(), "NULL".to_string()),
    };
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_node_reconciliations \
                 SET status = {}, status_changed_at = {}, converged_at = {converged_at}, \
                     failed_at = {failed_at}, failure_code = {}, failure_detail = {} \
                 WHERE reconciliation_id = {} AND status = {}",
                placeholder(backend, 1),
                now_expression(backend),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
            ),
            vec![
                next_status.as_str().into(),
                failure.map(|failure| failure.code.clone()).into(),
                failure.map(|failure| failure.detail.clone()).into(),
                uuid_value(reconciliation_id, backend),
                expected_status.as_str().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleArtifactNodeReconciliationError::StaleReconciliation);
    }
    Ok(())
}

async fn supersede_reconciliation(
    transaction: &DatabaseTransaction,
    reconciliation_id: Uuid,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_node_reconciliations \
                 SET status = 'superseded', status_changed_at = {} \
                 WHERE reconciliation_id = {} AND status = 'converged'",
                now_expression(backend),
                placeholder(backend, 1),
            ),
            vec![uuid_value(reconciliation_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleArtifactNodeReconciliationError::StaleReconciliation);
    }
    Ok(())
}

async fn load_state<C: ConnectionTrait>(
    connection: &C,
    lock_row: bool,
) -> Result<ModuleDesiredObservedState, ModuleArtifactNodeReconciliationError> {
    let backend = connection.get_database_backend();
    let lock = if lock_row && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT revision, desired_reconciliation_id, observed_reconciliation_id \
                 FROM module_artifact_node_reconciliation_state \
                 WHERE state_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![RECONCILIATION_STATE_ID.into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or_else(|| {
            ModuleArtifactNodeReconciliationError::Store(
                "artifact node reconciliation state is unavailable".to_string(),
            )
        })?;
    Ok(ModuleDesiredObservedState {
        revision: revision_from_row(&row, "revision", true)?,
        desired_id: optional_uuid_from_row(&row, "desired_reconciliation_id", backend)?,
        observed_id: optional_uuid_from_row(&row, "observed_reconciliation_id", backend)?,
    })
}

async fn advance_state(
    transaction: &DatabaseTransaction,
    expected_revision: u64,
    next_revision: u64,
    desired_reconciliation_id: Option<Uuid>,
    observed_reconciliation_id: Option<Uuid>,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_node_reconciliation_state \
                 SET revision = {}, desired_reconciliation_id = {}, observed_reconciliation_id = {}, \
                     updated_at = {} \
                 WHERE state_id = {} AND revision = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
                placeholder(backend, 4),
                placeholder(backend, 5),
            ),
            vec![
                revision_value(next_revision)?,
                optional_uuid_value(desired_reconciliation_id, backend),
                optional_uuid_value(observed_reconciliation_id, backend),
                RECONCILIATION_STATE_ID.into(),
                revision_value_allow_zero(expected_revision)?,
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        let current = load_state(transaction, false).await?;
        return Err(ModuleArtifactNodeReconciliationError::RevisionConflict {
            expected: expected_revision,
            current: current.revision,
        });
    }
    Ok(())
}

async fn load_reconciliation<C: ConnectionTrait>(
    connection: &C,
    reconciliation_id: Uuid,
    lock_row: bool,
) -> Result<ModuleArtifactNodeReconciliation, ModuleArtifactNodeReconciliationError> {
    let backend = connection.get_database_backend();
    let lock = if lock_row && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT reconciliation_id, predecessor_reconciliation_id, reconciliation_revision, \
                        topology_reference, topology_digest, policy_revision, target_assignment_count, \
                        status, requested_by, failure_code, failure_detail \
                 FROM module_artifact_node_reconciliations \
                 WHERE reconciliation_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(reconciliation_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleArtifactNodeReconciliationError::ReconciliationNotFound)?;
    let assignment_rows = connection
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT node_id, installation_id, installation_scope, release_digest, payload_digest, \
                        payload_kind, payload_media_type, \
                        admission_revision, dependency_graph_revision, dependency_graph_digest, \
                        capability_grant_revision, executor_abi, policy_revision, ordinal, \
                        observation_revision, phase, observed_installation_scope, observed_release_digest, \
                        observed_payload_digest, observed_payload_kind, observed_payload_media_type, \
                        observed_admission_revision, \
                        observed_dependency_graph_revision, observed_dependency_graph_digest, \
                        observed_capability_grant_revision, observed_executor_abi, observed_policy_revision, \
                        health_evidence_reference, health_evidence_digest, \
                        failure_code, failure_detail, reported_by, last_report_digest, \
                        active_claim_id, claimed_by_agent, claim_expires_at \
                 FROM module_artifact_node_reconciliation_assignments \
                 WHERE reconciliation_id = {} ORDER BY ordinal",
                placeholder(backend, 1),
            ),
            vec![uuid_value(reconciliation_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    let assignments = assignment_rows
        .iter()
        .map(|row| assignment_from_row(row, backend))
        .collect::<Result<Vec<_>, _>>()?;
    let target_assignment_count: i64 = row
        .try_get("", "target_assignment_count")
        .map_err(store_error)?;
    let target_assignment_count = u16::try_from(target_assignment_count).map_err(|_| {
        ModuleArtifactNodeReconciliationError::Store(
            "artifact node target-assignment count is invalid".to_string(),
        )
    })?;
    if usize::from(target_assignment_count) != assignments.len() {
        return Err(ModuleArtifactNodeReconciliationError::Store(
            "artifact node reconciliation topology is incomplete".to_string(),
        ));
    }
    let failure = failure_from_row(&row, "artifact node reconciliation")?;
    Ok(ModuleArtifactNodeReconciliation {
        reconciliation_id: uuid_from_row(&row, "reconciliation_id", backend)
            .map_err(store_error)?,
        predecessor_reconciliation_id: optional_uuid_from_row(
            &row,
            "predecessor_reconciliation_id",
            backend,
        )?,
        reconciliation_revision: revision_from_row(&row, "reconciliation_revision", false)?,
        topology_reference: row.try_get("", "topology_reference").map_err(store_error)?,
        topology_digest: row.try_get("", "topology_digest").map_err(store_error)?,
        policy_revision: row.try_get("", "policy_revision").map_err(store_error)?,
        target_assignment_count,
        status: ModuleArtifactNodeReconciliationStatus::parse(
            &row.try_get::<String>("", "status").map_err(store_error)?,
        )?,
        requested_by: uuid_from_row(&row, "requested_by", backend).map_err(store_error)?,
        failure,
        assignments,
    })
}

async fn load_assignment<C: ConnectionTrait>(
    connection: &C,
    reconciliation_id: Uuid,
    node_id: Uuid,
    installation_id: Uuid,
    lock_row: bool,
) -> Result<ModuleArtifactNodeAssignment, ModuleArtifactNodeReconciliationError> {
    let backend = connection.get_database_backend();
    let lock = if lock_row && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT node_id, installation_id, installation_scope, release_digest, payload_digest, \
                        payload_kind, payload_media_type, \
                        admission_revision, dependency_graph_revision, dependency_graph_digest, \
                        capability_grant_revision, executor_abi, policy_revision, ordinal, \
                        observation_revision, phase, observed_installation_scope, observed_release_digest, \
                        observed_payload_digest, observed_payload_kind, observed_payload_media_type, \
                        observed_admission_revision, \
                        observed_dependency_graph_revision, observed_dependency_graph_digest, \
                        observed_capability_grant_revision, observed_executor_abi, observed_policy_revision, \
                        health_evidence_reference, health_evidence_digest, \
                        failure_code, failure_detail, reported_by, last_report_digest, \
                        active_claim_id, claimed_by_agent, claim_expires_at \
                 FROM module_artifact_node_reconciliation_assignments \
                 WHERE reconciliation_id = {} AND node_id = {} AND installation_id = {}{lock}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(reconciliation_id, backend),
                uuid_value(node_id, backend),
                uuid_value(installation_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleArtifactNodeReconciliationError::AssignmentNotFound)?;
    assignment_from_row(&row, backend)
}

async fn load_next_assignment_for_node(
    transaction: &DatabaseTransaction,
    reconciliation_id: Uuid,
    node_id: Uuid,
    agent_id: &str,
    now: &DateTime<Utc>,
) -> Result<Option<ModuleArtifactNodeAssignment>, ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    let lock = if backend == DbBackend::Postgres {
        " FOR UPDATE SKIP LOCKED"
    } else {
        ""
    };
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT node_id, installation_id, installation_scope, release_digest, payload_digest, \
                        payload_kind, payload_media_type, \
                        admission_revision, dependency_graph_revision, dependency_graph_digest, \
                        capability_grant_revision, executor_abi, policy_revision, ordinal, \
                        observation_revision, phase, observed_installation_scope, observed_release_digest, \
                        observed_payload_digest, observed_payload_kind, observed_payload_media_type, \
                        observed_admission_revision, \
                        observed_dependency_graph_revision, observed_dependency_graph_digest, \
                        observed_capability_grant_revision, observed_executor_abi, observed_policy_revision, \
                        health_evidence_reference, health_evidence_digest, \
                        failure_code, failure_detail, reported_by, last_report_digest, \
                        active_claim_id, claimed_by_agent, claim_expires_at \
                 FROM module_artifact_node_reconciliation_assignments \
                 WHERE reconciliation_id = {} AND node_id = {} \
                   AND phase IN ('pending', 'prepared', 'healthy', 'active') \
                   AND (active_claim_id IS NULL OR claim_expires_at < {} \
                        OR (claimed_by_agent = {} AND claim_expires_at >= {})) \
                 ORDER BY CASE WHEN claimed_by_agent = {} AND claim_expires_at >= {} THEN 0 ELSE 1 END, \
                          ordinal \
                 LIMIT 1{lock}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
            ),
            vec![
                uuid_value(reconciliation_id, backend),
                uuid_value(node_id, backend),
                now.to_owned().into(),
                agent_id.to_owned().into(),
                now.to_owned().into(),
                agent_id.to_owned().into(),
                now.to_owned().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    row.map(|row| assignment_from_row(&row, backend))
        .transpose()
}

async fn claim_assignment(
    transaction: &DatabaseTransaction,
    reconciliation_id: Uuid,
    assignment: &ModuleArtifactNodeAssignment,
    claim_id: Uuid,
    agent_id: &str,
    lease_expires_at: &DateTime<Utc>,
    now: &DateTime<Utc>,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_node_reconciliation_assignments \
                 SET active_claim_id = {}, claimed_by_agent = {}, claim_expires_at = {} \
                 WHERE reconciliation_id = {} AND node_id = {} AND installation_id = {} \
                   AND phase IN ('pending', 'prepared', 'healthy', 'active') \
                   AND observation_revision = {} \
                   AND (active_claim_id IS NULL OR claim_expires_at < {})",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
            ),
            vec![
                uuid_value(claim_id, backend),
                agent_id.to_owned().into(),
                lease_expires_at.to_owned().into(),
                uuid_value(reconciliation_id, backend),
                uuid_value(assignment.node_id, backend),
                uuid_value(assignment.installation_id, backend),
                revision_value_allow_zero(assignment.observation_revision)?,
                now.to_owned().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleArtifactNodeReconciliationError::ClaimConflict);
    }
    Ok(())
}

async fn heartbeat_assignment(
    transaction: &DatabaseTransaction,
    command: &ModuleArtifactNodeAssignmentHeartbeatCommand,
    lease_expires_at: &DateTime<Utc>,
    now: &DateTime<Utc>,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_node_reconciliation_assignments \
                 SET claim_expires_at = {} \
                 WHERE active_claim_id = {} AND claimed_by_agent = {} AND claim_expires_at >= {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            vec![
                lease_expires_at.to_owned().into(),
                uuid_value(command.claim_id, backend),
                command.agent_id.clone().into(),
                now.to_owned().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleArtifactNodeReconciliationError::ClaimConflict);
    }
    Ok(())
}

fn assignment_from_row(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ModuleArtifactNodeAssignment, ModuleArtifactNodeReconciliationError> {
    let ordinal: i64 = row.try_get("", "ordinal").map_err(store_error)?;
    let health_evidence = evidence_from_row(row)?;
    let failure = failure_from_row(row, "artifact node assignment")?;
    let assignment = ModuleArtifactNodeAssignment {
        node_id: uuid_from_row(row, "node_id", backend).map_err(store_error)?,
        installation_id: uuid_from_row(row, "installation_id", backend).map_err(store_error)?,
        installation_scope: ModuleArtifactNodeInstallationScope::parse(
            &row.try_get::<String>("", "installation_scope")
                .map_err(store_error)?,
        )?,
        release_digest: row.try_get("", "release_digest").map_err(store_error)?,
        payload_digest: row.try_get("", "payload_digest").map_err(store_error)?,
        payload_kind: parse_payload_kind(
            &row.try_get::<String>("", "payload_kind")
                .map_err(store_error)?,
        )?,
        payload_media_type: row.try_get("", "payload_media_type").map_err(store_error)?,
        admission_revision: revision_from_row(row, "admission_revision", false)?,
        dependency_graph_revision: revision_from_row(row, "dependency_graph_revision", false)?,
        dependency_graph_digest: row
            .try_get("", "dependency_graph_digest")
            .map_err(store_error)?,
        capability_grant_revision: revision_from_row(row, "capability_grant_revision", false)?,
        executor_abi: row.try_get("", "executor_abi").map_err(store_error)?,
        policy_revision: row.try_get("", "policy_revision").map_err(store_error)?,
        ordinal: u16::try_from(ordinal).map_err(|_| {
            ModuleArtifactNodeReconciliationError::Store(
                "artifact node assignment ordinal is invalid".to_string(),
            )
        })?,
        observation_revision: revision_from_row(row, "observation_revision", true)?,
        phase: parse_phase(&row.try_get::<String>("", "phase").map_err(store_error)?)?,
        health_evidence,
        failure,
        reported_by: row.try_get("", "reported_by").map_err(store_error)?,
        last_report_digest: row.try_get("", "last_report_digest").map_err(store_error)?,
        active_claim_id: optional_uuid_from_row(row, "active_claim_id", backend)?,
        claimed_by_agent: row.try_get("", "claimed_by_agent").map_err(store_error)?,
        claim_expires_at: row
            .try_get::<Option<DateTime<Utc>>>("", "claim_expires_at")
            .map_err(store_error)?,
    };
    if !valid_text(&assignment.payload_media_type, MAX_PAYLOAD_MEDIA_TYPE_BYTES) {
        return Err(ModuleArtifactNodeReconciliationError::Store(
            "artifact node assignment payload media type is invalid".to_string(),
        ));
    }
    validate_observed_assignment_identity(row, &assignment)?;
    Ok(assignment)
}

fn evidence_from_row(
    row: &QueryResult,
) -> Result<Option<ModuleReconciliationEvidence>, ModuleArtifactNodeReconciliationError> {
    let reference: Option<String> = row
        .try_get("", "health_evidence_reference")
        .map_err(store_error)?;
    let digest: Option<String> = row
        .try_get("", "health_evidence_digest")
        .map_err(store_error)?;
    match (reference, digest) {
        (Some(reference), Some(digest)) => {
            Ok(Some(ModuleReconciliationEvidence { reference, digest }))
        }
        (None, None) => Ok(None),
        _ => Err(ModuleArtifactNodeReconciliationError::Store(
            "artifact node health evidence is incomplete".to_string(),
        )),
    }
}

fn validate_observed_assignment_identity(
    row: &QueryResult,
    assignment: &ModuleArtifactNodeAssignment,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let scope: Option<String> = row
        .try_get("", "observed_installation_scope")
        .map_err(store_error)?;
    let release_digest: Option<String> = row
        .try_get("", "observed_release_digest")
        .map_err(store_error)?;
    let payload_digest: Option<String> = row
        .try_get("", "observed_payload_digest")
        .map_err(store_error)?;
    let payload_kind: Option<String> = row
        .try_get("", "observed_payload_kind")
        .map_err(store_error)?;
    let payload_media_type: Option<String> = row
        .try_get("", "observed_payload_media_type")
        .map_err(store_error)?;
    let admission_revision = optional_revision_from_row(row, "observed_admission_revision")?;
    let dependency_graph_revision =
        optional_revision_from_row(row, "observed_dependency_graph_revision")?;
    let dependency_graph_digest: Option<String> = row
        .try_get("", "observed_dependency_graph_digest")
        .map_err(store_error)?;
    let capability_grant_revision =
        optional_revision_from_row(row, "observed_capability_grant_revision")?;
    let executor_abi: Option<String> = row
        .try_get("", "observed_executor_abi")
        .map_err(store_error)?;
    let policy_revision: Option<String> = row
        .try_get("", "observed_policy_revision")
        .map_err(store_error)?;
    let fields_present = scope.is_some()
        || release_digest.is_some()
        || payload_digest.is_some()
        || payload_kind.is_some()
        || payload_media_type.is_some()
        || admission_revision.is_some()
        || dependency_graph_revision.is_some()
        || dependency_graph_digest.is_some()
        || capability_grant_revision.is_some()
        || executor_abi.is_some()
        || policy_revision.is_some();
    if assignment.observation_revision == 0 {
        if !fields_present {
            return Ok(());
        }
    } else if scope.as_deref() == Some(assignment.installation_scope.as_str())
        && release_digest.as_deref() == Some(assignment.release_digest.as_str())
        && payload_digest.as_deref() == Some(assignment.payload_digest.as_str())
        && payload_kind.as_deref() == Some(assignment.payload_kind.as_str())
        && payload_media_type.as_deref() == Some(assignment.payload_media_type.as_str())
        && admission_revision == Some(assignment.admission_revision)
        && dependency_graph_revision == Some(assignment.dependency_graph_revision)
        && dependency_graph_digest.as_deref() == Some(assignment.dependency_graph_digest.as_str())
        && capability_grant_revision == Some(assignment.capability_grant_revision)
        && executor_abi.as_deref() == Some(assignment.executor_abi.as_str())
        && policy_revision.as_deref() == Some(assignment.policy_revision.as_str())
    {
        return Ok(());
    }
    Err(ModuleArtifactNodeReconciliationError::Store(
        "artifact node observed identity does not match its desired assignment".to_string(),
    ))
}

fn failure_from_row(
    row: &QueryResult,
    subject: &str,
) -> Result<Option<ModuleReconciliationFailure>, ModuleArtifactNodeReconciliationError> {
    let code: Option<String> = row.try_get("", "failure_code").map_err(store_error)?;
    let detail: Option<String> = row.try_get("", "failure_detail").map_err(store_error)?;
    match (code, detail) {
        (Some(code), Some(detail)) => Ok(Some(ModuleReconciliationFailure { code, detail })),
        (None, None) => Ok(None),
        _ => Err(ModuleArtifactNodeReconciliationError::Store(format!(
            "{subject} failure is incomplete"
        ))),
    }
}

fn parse_phase(
    value: &str,
) -> Result<ModuleReconciliationPhase, ModuleArtifactNodeReconciliationError> {
    ModuleReconciliationPhase::parse(value).ok_or_else(|| {
        ModuleArtifactNodeReconciliationError::Store(
            "artifact node assignment phase is invalid".to_string(),
        )
    })
}

fn parse_payload_kind(
    value: &str,
) -> Result<ArtifactPayloadKind, ModuleArtifactNodeReconciliationError> {
    match value {
        "rhai" => Ok(ArtifactPayloadKind::Rhai),
        "wasm_component" => Ok(ArtifactPayloadKind::WasmComponent),
        "static_promoted" => Ok(ArtifactPayloadKind::StaticPromoted),
        "sidecar" => Ok(ArtifactPayloadKind::Sidecar),
        _ => Err(ModuleArtifactNodeReconciliationError::Store(
            "artifact node assignment payload kind is invalid".to_string(),
        )),
    }
}

fn validate_report_identity(
    command: &ModuleArtifactNodeAssignmentReport,
    assignment: &ModuleArtifactNodeAssignment,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    if command.installation_scope != assignment.installation_scope
        || command.release_digest != assignment.release_digest
        || command.payload_digest != assignment.payload_digest
        || command.payload_kind != assignment.payload_kind
        || command.payload_media_type != assignment.payload_media_type
        || command.admission_revision != assignment.admission_revision
        || command.dependency_graph_revision != assignment.dependency_graph_revision
        || command.dependency_graph_digest != assignment.dependency_graph_digest
        || command.capability_grant_revision != assignment.capability_grant_revision
        || command.executor_abi != assignment.executor_abi
        || command.policy_revision != assignment.policy_revision
    {
        return Err(ModuleArtifactNodeReconciliationError::ObservationIdentityMismatch);
    }
    Ok(())
}

fn work_identity(
    reconciliation: &ModuleArtifactNodeReconciliation,
) -> ModuleArtifactNodeReconciliationWorkIdentity {
    ModuleArtifactNodeReconciliationWorkIdentity {
        reconciliation_id: reconciliation.reconciliation_id,
        reconciliation_revision: reconciliation.reconciliation_revision,
        topology_reference: reconciliation.topology_reference.clone(),
        topology_digest: reconciliation.topology_digest.clone(),
        policy_revision: reconciliation.policy_revision.clone(),
    }
}

async fn reserve_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    operation_kind: &str,
    request_digest: &str,
    principal_id: &str,
    context: Option<&ModuleCommandContext>,
) -> Result<Option<OperationRecord>, ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    let inserted = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_node_reconciliation_operations \
                 (idempotency_key, operation_kind, request_digest, principal_id, trace_id, correlation_id, created_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}) ON CONFLICT (idempotency_key) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                now_expression(backend),
            ),
            vec![
                uuid_value(idempotency_key, backend),
                operation_kind.to_owned().into(),
                request_digest.to_owned().into(),
                principal_id.to_owned().into(),
                context.map(|value| value.trace_id.clone()).into(),
                optional_uuid_value(context.map(|value| value.correlation_id), backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if inserted.rows_affected() == 1 {
        return Ok(None);
    }
    load_operation(
        transaction,
        idempotency_key,
        operation_kind,
        request_digest,
        principal_id,
        context,
    )
    .await
}

async fn load_operation<C: ConnectionTrait>(
    connection: &C,
    idempotency_key: Uuid,
    operation_kind: &str,
    request_digest: &str,
    principal_id: &str,
    context: Option<&ModuleCommandContext>,
) -> Result<Option<OperationRecord>, ModuleArtifactNodeReconciliationError> {
    let backend = connection.get_database_backend();
    let Some(row) = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_kind, request_digest, principal_id, trace_id, correlation_id, reconciliation_id, \
                        reconciliation_revision, reconciliation_state_revision, reconciliation_status, \
                        node_id, installation_id, observation_revision, assignment_phase, \
                        CASE WHEN completed_at IS NULL THEN 0 ELSE 1 END AS completed \
                 FROM module_artifact_node_reconciliation_operations \
                 WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(idempotency_key, backend)],
        ))
        .await
        .map_err(store_error)?
    else {
        return Ok(None);
    };
    let record = OperationRecord {
        operation_kind: row.try_get("", "operation_kind").map_err(store_error)?,
        request_digest: row.try_get("", "request_digest").map_err(store_error)?,
        principal_id: row.try_get("", "principal_id").map_err(store_error)?,
        trace_id: row.try_get("", "trace_id").map_err(store_error)?,
        correlation_id: optional_uuid_from_row(&row, "correlation_id", backend)?,
        reconciliation_id: optional_uuid_from_row(&row, "reconciliation_id", backend)?,
        reconciliation_revision: optional_revision_from_row(&row, "reconciliation_revision")?,
        reconciliation_state_revision: optional_revision_from_row(
            &row,
            "reconciliation_state_revision",
        )?,
        reconciliation_status: row
            .try_get::<Option<String>>("", "reconciliation_status")
            .map_err(store_error)?
            .as_deref()
            .map(ModuleArtifactNodeReconciliationStatus::parse)
            .transpose()?,
        node_id: optional_uuid_from_row(&row, "node_id", backend)?,
        installation_id: optional_uuid_from_row(&row, "installation_id", backend)?,
        observation_revision: optional_revision_from_row(&row, "observation_revision")?,
        assignment_phase: row
            .try_get::<Option<String>>("", "assignment_phase")
            .map_err(store_error)?
            .as_deref()
            .map(parse_phase)
            .transpose()?,
        completed: row.try_get::<i64>("", "completed").map_err(store_error)? == 1,
    };
    if record.operation_kind != operation_kind
        || record.request_digest != request_digest
        || record.principal_id != principal_id
        || record.trace_id.as_deref() != context.map(|value| value.trace_id.as_str())
        || record.correlation_id != context.map(|value| value.correlation_id)
    {
        return Err(ModuleArtifactNodeReconciliationError::IdempotencyConflict);
    }
    Ok(Some(record))
}

async fn complete_request_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    receipt: &ModuleArtifactNodeReconciliationReceipt,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    complete_operation(
        transaction,
        idempotency_key,
        OperationCompletion {
            reconciliation_id: receipt.reconciliation_id,
            reconciliation_revision: receipt.reconciliation_revision,
            reconciliation_state_revision: receipt.reconciliation_state_revision,
            reconciliation_status: receipt.status,
            node_id: None,
            installation_id: None,
            observation_revision: None,
            assignment_phase: None,
        },
    )
    .await
}

async fn complete_report_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    receipt: &ModuleArtifactNodeAssignmentReportReceipt,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    complete_operation(
        transaction,
        idempotency_key,
        OperationCompletion {
            reconciliation_id: receipt.reconciliation_id,
            reconciliation_revision: receipt.reconciliation_revision,
            reconciliation_state_revision: receipt.reconciliation_state_revision,
            reconciliation_status: receipt.reconciliation_status,
            node_id: Some(receipt.node_id),
            installation_id: Some(receipt.installation_id),
            observation_revision: Some(receipt.observation_revision),
            assignment_phase: Some(receipt.phase),
        },
    )
    .await
}

async fn complete_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    completion: OperationCompletion,
) -> Result<(), ModuleArtifactNodeReconciliationError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_node_reconciliation_operations \
                 SET reconciliation_id = {}, reconciliation_revision = {}, \
                     reconciliation_state_revision = {}, reconciliation_status = {}, \
                     node_id = {}, installation_id = {}, observation_revision = {}, \
                     assignment_phase = {}, completed_at = {} \
                 WHERE idempotency_key = {} AND completed_at IS NULL",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                now_expression(backend),
                placeholder(backend, 9),
            ),
            vec![
                uuid_value(completion.reconciliation_id, backend),
                revision_value(completion.reconciliation_revision)?,
                revision_value(completion.reconciliation_state_revision)?,
                completion.reconciliation_status.as_str().into(),
                optional_uuid_value(completion.node_id, backend),
                optional_uuid_value(completion.installation_id, backend),
                optional_revision_value(completion.observation_revision)?,
                completion
                    .assignment_phase
                    .map(|phase| phase.as_str().to_string())
                    .into(),
                uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleArtifactNodeReconciliationError::IdempotencyConflict);
    }
    Ok(())
}

fn replay_request(
    operation: &OperationRecord,
) -> Result<ModuleArtifactNodeReconciliationReceipt, ModuleArtifactNodeReconciliationError> {
    if !operation.completed
        || operation.operation_kind != "request"
        || operation.node_id.is_some()
        || operation.installation_id.is_some()
        || operation.observation_revision.is_some()
        || operation.assignment_phase.is_some()
    {
        return Err(ModuleArtifactNodeReconciliationError::IdempotencyConflict);
    }
    Ok(ModuleArtifactNodeReconciliationReceipt {
        reconciliation_id: operation
            .reconciliation_id
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        reconciliation_revision: operation
            .reconciliation_revision
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        reconciliation_state_revision: operation
            .reconciliation_state_revision
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        status: operation
            .reconciliation_status
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        created: false,
    })
}

fn replay_report(
    operation: &OperationRecord,
) -> Result<ModuleArtifactNodeAssignmentReportReceipt, ModuleArtifactNodeReconciliationError> {
    if !operation.completed || operation.operation_kind != "report" {
        return Err(ModuleArtifactNodeReconciliationError::IdempotencyConflict);
    }
    Ok(ModuleArtifactNodeAssignmentReportReceipt {
        reconciliation_id: operation
            .reconciliation_id
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        reconciliation_revision: operation
            .reconciliation_revision
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        reconciliation_state_revision: operation
            .reconciliation_state_revision
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        reconciliation_status: operation
            .reconciliation_status
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        node_id: operation
            .node_id
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        installation_id: operation
            .installation_id
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        observation_revision: operation
            .observation_revision
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        phase: operation
            .assignment_phase
            .ok_or(ModuleArtifactNodeReconciliationError::IdempotencyConflict)?,
        created: false,
    })
}

fn valid_text(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= maximum_bytes
        && !value.chars().any(char::is_control)
}

fn optional_uuid_value(value: Option<Uuid>, backend: DbBackend) -> sea_orm::Value {
    match (backend, value) {
        (DbBackend::Postgres, value) => sea_orm::Value::Uuid(value),
        (_, Some(value)) => value.to_string().into(),
        (_, None) => sea_orm::Value::String(None),
    }
}

fn optional_uuid_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Option<Uuid>, ModuleArtifactNodeReconciliationError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(store_error),
        _ => row
            .try_get::<Option<String>>("", column)
            .map_err(store_error)?
            .map(|value| Uuid::parse_str(&value).map_err(store_error))
            .transpose(),
    }
}

fn revision_value(value: u64) -> Result<sea_orm::Value, ModuleArtifactNodeReconciliationError> {
    i64::try_from(value)
        .map(Into::into)
        .map_err(|_| ModuleArtifactNodeReconciliationError::RevisionOverflow)
}

fn revision_value_allow_zero(
    value: u64,
) -> Result<sea_orm::Value, ModuleArtifactNodeReconciliationError> {
    revision_value(value)
}

fn optional_revision_value(
    value: Option<u64>,
) -> Result<sea_orm::Value, ModuleArtifactNodeReconciliationError> {
    match value {
        Some(value) => revision_value(value),
        None => Ok(sea_orm::Value::BigInt(None)),
    }
}

fn revision_from_row(
    row: &QueryResult,
    column: &str,
    allow_zero: bool,
) -> Result<u64, ModuleArtifactNodeReconciliationError> {
    let value: i64 = row.try_get("", column).map_err(store_error)?;
    if value < 0 || (!allow_zero && value == 0) {
        return Err(ModuleArtifactNodeReconciliationError::Store(format!(
            "artifact node reconciliation revision `{column}` is invalid"
        )));
    }
    u64::try_from(value).map_err(|_| ModuleArtifactNodeReconciliationError::RevisionOverflow)
}

fn optional_revision_from_row(
    row: &QueryResult,
    column: &str,
) -> Result<Option<u64>, ModuleArtifactNodeReconciliationError> {
    row.try_get::<Option<i64>>("", column)
        .map_err(store_error)?
        .map(|value| {
            if value <= 0 {
                Err(ModuleArtifactNodeReconciliationError::Store(format!(
                    "artifact node reconciliation revision `{column}` is invalid"
                )))
            } else {
                u64::try_from(value)
                    .map_err(|_| ModuleArtifactNodeReconciliationError::RevisionOverflow)
            }
        })
        .transpose()
}

fn digest_error(error: impl std::fmt::Display) -> ModuleArtifactNodeReconciliationError {
    ModuleArtifactNodeReconciliationError::Store(error.to_string())
}

fn store_error(error: impl std::fmt::Display) -> ModuleArtifactNodeReconciliationError {
    ModuleArtifactNodeReconciliationError::Store(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use rustok_events::{DomainEvent, EventEnvelope};
    use rustok_outbox::TransactionalEventWriter;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    use super::*;

    #[derive(Clone)]
    struct AllowAuthorizer;

    #[async_trait]
    impl ModuleArtifactNodeReconciliationAuthorizer for AllowAuthorizer {
        async fn authorize_request(
            &self,
            _command: &ModuleArtifactNodeReconciliationRequest,
        ) -> Result<(), ModuleArtifactNodeReconciliationError> {
            Ok(())
        }

        async fn authorize_assignment_claim(
            &self,
            _command: &ModuleArtifactNodeAssignmentClaimCommand,
        ) -> Result<(), ModuleArtifactNodeReconciliationError> {
            Ok(())
        }

        async fn authorize_assignment_heartbeat(
            &self,
            _command: &ModuleArtifactNodeAssignmentHeartbeatCommand,
        ) -> Result<(), ModuleArtifactNodeReconciliationError> {
            Ok(())
        }

        async fn authorize_report(
            &self,
            _command: &ModuleArtifactNodeAssignmentReport,
        ) -> Result<(), ModuleArtifactNodeReconciliationError> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct FixedTopology(ModuleArtifactNodeTopologySnapshot);

    #[async_trait]
    impl ModuleArtifactNodeTopologyResolver for FixedTopology {
        async fn resolve(
            &self,
            _policy_revision: &str,
        ) -> Result<ModuleArtifactNodeTopologySnapshot, String> {
            Ok(self.0.clone())
        }
    }

    #[derive(Clone, Default)]
    struct CapturingEventWriter(Arc<Mutex<Vec<EventEnvelope>>>);

    #[async_trait]
    impl TransactionalEventWriter for CapturingEventWriter {
        async fn write_event(
            &self,
            _transaction: &DatabaseTransaction,
            envelope: EventEnvelope,
        ) -> rustok_core::Result<()> {
            self.0.lock().expect("event writer lock").push(envelope);
            Ok(())
        }
    }

    #[tokio::test]
    async fn owner_fences_topology_digest_converges_exact_identity_and_replays_reports() {
        let database = database().await;
        let installation_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();
        insert_installation(&database, installation_id).await;
        let targets = vec![ModuleArtifactNodeAssignmentTarget {
            node_id,
            installation_id,
        }];
        let topology = ModuleArtifactNodeTopologySnapshot {
            topology_reference: "topology:node-a".to_string(),
            topology_digest: module_artifact_node_topology_digest("topology:node-a", &targets)
                .expect("topology digest"),
            assignments: targets,
        };
        let topology_digest = topology.topology_digest.clone();
        let events = CapturingEventWriter::default();
        let service = SeaOrmModuleArtifactNodeReconciliationService::with_infrastructure(
            database,
            AllowAuthorizer,
            FixedTopology(topology),
            ControlPlaneInfrastructure::default()
                .with_transactional_event_writer(Arc::new(events.clone())),
        );
        let policy_revision = digest('a');
        let request = ModuleArtifactNodeReconciliationRequest {
            expected_reconciliation_state_revision: 0,
            policy_revision: policy_revision.clone(),
            topology_digest,
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: None,
                trace_id: "test:artifact-node-reconciliation".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
        };
        let mut mismatched_topology = request.clone();
        mismatched_topology.topology_digest = digest('f');
        mismatched_topology.context.idempotency_key = Uuid::new_v4();
        assert!(matches!(
            service.request(mismatched_topology).await,
            Err(ModuleArtifactNodeReconciliationError::TopologyDigestMismatch)
        ));
        let mut tenant_scoped = request.clone();
        tenant_scoped.context.tenant_id = Some(Uuid::new_v4());
        tenant_scoped.context.idempotency_key = Uuid::new_v4();
        assert!(matches!(
            service.request(tenant_scoped).await,
            Err(ModuleArtifactNodeReconciliationError::InvalidCommand)
        ));
        let created = service.request(request.clone()).await.expect("request");
        assert!(created.created);
        let operation = service
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT principal_id, trace_id, correlation_id
                 FROM module_artifact_node_reconciliation_operations
                 WHERE idempotency_key = ?1",
                vec![request.context.idempotency_key.to_string().into()],
            ))
            .await
            .expect("request operation query")
            .expect("request operation");
        let stored_actor: String = operation.try_get("", "principal_id").expect("stored actor");
        let stored_trace: Option<String> = operation.try_get("", "trace_id").expect("stored trace");
        let stored_correlation: Option<String> = operation
            .try_get("", "correlation_id")
            .expect("stored correlation");
        assert_eq!(stored_actor, request.context.actor_id.to_string());
        assert_eq!(
            stored_trace.as_deref(),
            Some(request.context.trace_id.as_str())
        );
        let expected_correlation = request.context.correlation_id.to_string();
        assert_eq!(
            stored_correlation.as_deref(),
            Some(expected_correlation.as_str())
        );
        let mut substituted_replay = request.clone();
        substituted_replay.topology_digest = digest('f');
        let mut tracing_replay = request.clone();
        tracing_replay.context.trace_id = "test:changed-trace".to_string();
        assert!(
            !service
                .request(request.clone())
                .await
                .expect("request replay")
                .created
        );
        assert!(matches!(
            service.request(substituted_replay).await,
            Err(ModuleArtifactNodeReconciliationError::IdempotencyConflict)
        ));
        assert!(matches!(
            service.request(tracing_replay).await,
            Err(ModuleArtifactNodeReconciliationError::IdempotencyConflict)
        ));

        let prepared = service
            .claim_assignment(ModuleArtifactNodeAssignmentClaimCommand {
                node_id,
                agent_id: "node-agent-a".to_string(),
            })
            .await
            .expect("prepared claim")
            .expect("prepared work");
        let prepared_receipt = service
            .report(report_from_work(
                &prepared,
                ModuleReconciliationPhase::Prepared,
                None,
                None,
                Uuid::new_v4(),
            ))
            .await
            .expect("prepared report");
        assert_eq!(
            prepared_receipt.reconciliation_status,
            ModuleArtifactNodeReconciliationStatus::Activating
        );

        let healthy = service
            .claim_assignment(ModuleArtifactNodeAssignmentClaimCommand {
                node_id,
                agent_id: "node-agent-a".to_string(),
            })
            .await
            .expect("healthy claim")
            .expect("healthy work");
        let report = report_from_work(
            &healthy,
            ModuleReconciliationPhase::Healthy,
            Some(ModuleReconciliationEvidence {
                reference: "evidence:node-a".to_string(),
                digest: digest('e'),
            }),
            None,
            Uuid::new_v4(),
        );
        let converged = service
            .report(report.clone())
            .await
            .expect("healthy report");
        assert_eq!(
            converged.reconciliation_status,
            ModuleArtifactNodeReconciliationStatus::Converged
        );
        assert!(!service.report(report).await.expect("report replay").created);
        assert_eq!(
            service.state().await.expect("state").converged_id(),
            Some(created.reconciliation_id)
        );
        let reconciliation = service
            .get(created.reconciliation_id)
            .await
            .expect("reconciliation");
        assert_eq!(
            reconciliation.assignments[0].phase,
            ModuleReconciliationPhase::Active
        );
        let readiness =
            SeaOrmArtifactNodeReadiness::new(service.db.clone(), node_id).expect("node readiness");
        let identity = InstallationIdentity {
            installation_scope: ModuleArtifactNodeInstallationScope::Platform,
            release_digest: digest('1'),
            payload_digest: digest('2'),
            payload_kind: ArtifactPayloadKind::Rhai,
            payload_media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
            admission_revision: 1,
            dependency_graph_revision: 1,
            dependency_graph_digest: digest('3'),
            capability_grant_revision: 1,
            executor_abi: "rustok:module/runtime@1".to_string(),
        };
        readiness
            .require_active_identity(installation_id, &identity, &policy_revision)
            .await
            .expect("converged identity is executable");
        assert!(matches!(
            readiness
                .require_active_identity(installation_id, &identity, &digest('f'))
                .await,
            Err(ModuleArtifactNodeReconciliationError::AssignmentUnavailable)
        ));
        service
            .db
            .execute_unprepared(&format!(
                "UPDATE module_artifact_installations SET payload_digest = '{}' WHERE installation_id = '{installation_id}'",
                digest('9')
            ))
            .await
            .expect("mutate live identity");
        assert!(matches!(
            readiness
                .require_active_identity(installation_id, &identity, &policy_revision)
                .await,
            Err(ModuleArtifactNodeReconciliationError::AssignmentUnavailable)
        ));
        service
            .db
            .execute_unprepared(&format!(
                "UPDATE module_artifact_installations SET payload_digest = '{}' WHERE installation_id = '{installation_id}'",
                digest('2')
            ))
            .await
            .expect("restore live identity");
        service
            .db
            .execute_unprepared(&format!(
                "UPDATE module_artifact_installations SET payload_kind = 'sidecar' WHERE installation_id = '{installation_id}'"
            ))
            .await
            .expect("mutate live payload kind");
        assert!(matches!(
            readiness
                .require_active_identity(installation_id, &identity, &policy_revision)
                .await,
            Err(ModuleArtifactNodeReconciliationError::AssignmentUnavailable)
        ));
        service
            .db
            .execute_unprepared(&format!(
                "UPDATE module_artifact_installations SET payload_kind = 'rhai' WHERE installation_id = '{installation_id}'"
            ))
            .await
            .expect("restore live payload kind");
        service
            .db
            .execute_unprepared(&format!(
                "UPDATE module_artifact_admissions SET media_type = 'application/wasm' WHERE installation_id = '{installation_id}'"
            ))
            .await
            .expect("mutate live payload media type");
        assert!(matches!(
            readiness
                .require_active_identity(installation_id, &identity, &policy_revision)
                .await,
            Err(ModuleArtifactNodeReconciliationError::AssignmentUnavailable)
        ));
        service
            .db
            .execute_unprepared(&format!(
                "UPDATE module_artifact_admissions SET media_type = 'application/vnd.rustok.rhai.source.v1' WHERE installation_id = '{installation_id}'"
            ))
            .await
            .expect("restore live payload media type");
        let active = service
            .claim_assignment(ModuleArtifactNodeAssignmentClaimCommand {
                node_id,
                agent_id: "node-agent-a".to_string(),
            })
            .await
            .expect("active claim")
            .expect("active work");
        let degraded = service
            .report(report_from_work(
                &active,
                ModuleReconciliationPhase::Failed,
                None,
                Some(ModuleReconciliationFailure {
                    code: "health_check_failed".to_string(),
                    detail: "the owner-issued health check failed".to_string(),
                }),
                Uuid::new_v4(),
            ))
            .await
            .expect("active failure report");
        assert_eq!(
            degraded.reconciliation_status,
            ModuleArtifactNodeReconciliationStatus::Degraded
        );
        assert_eq!(
            service.state().await.expect("degraded state").observed_id,
            None
        );
        assert!(matches!(
            readiness
                .require_active_identity(installation_id, &identity, &policy_revision)
                .await,
            Err(ModuleArtifactNodeReconciliationError::AssignmentUnavailable)
        ));
        service
            .db
            .execute_unprepared(&format!(
                "UPDATE module_artifact_admissions SET revision = 2 WHERE installation_id = '{installation_id}'"
            ))
            .await
            .expect("lifecycle revision update");
        assert!(matches!(
            service
                .claim_assignment(ModuleArtifactNodeAssignmentClaimCommand {
                    node_id,
                    agent_id: "node-agent-a".to_string(),
                })
                .await,
            Err(ModuleArtifactNodeReconciliationError::StaleReconciliation)
        ));
        let events = events.0.lock().expect("events lock");
        let request_event = events
            .iter()
            .find(|envelope| {
                matches!(
                    &envelope.event,
                    DomainEvent::ModuleArtifactNodeReconciliationRequested { .. }
                )
            })
            .expect("request event");
        assert_eq!(request_event.actor_id, Some(request.context.actor_id));
        assert_eq!(request_event.tenant_id, Uuid::nil());
        assert_eq!(
            request_event.trace_id.as_deref(),
            Some(request.context.trace_id.as_str())
        );
        assert_eq!(request_event.correlation_id, request.context.correlation_id);
        assert!(events.iter().any(|envelope| matches!(
            &envelope.event,
            DomainEvent::ModuleArtifactNodeReconciliationRequested { .. }
        )));
        assert!(events.iter().any(|envelope| matches!(
            &envelope.event,
            DomainEvent::ModuleArtifactNodeReconciliationStatusChanged { status, .. }
            if status == "converged"
        )));
        assert!(events.iter().any(|envelope| matches!(
            &envelope.event,
            DomainEvent::ModuleArtifactNodeReconciliationStatusChanged { status, .. }
            if status == "degraded"
        )));
    }

    #[tokio::test]
    async fn narrow_agent_port_claims_without_topology_or_reconciliation_authoring_access() {
        let owner = crate::ModuleControlPlane::new(database().await).artifact_node_agent();

        assert!(
            owner
                .claim_assignment(ModuleArtifactNodeAssignmentClaimCommand {
                    node_id: Uuid::new_v4(),
                    agent_id: "node-agent-a".to_string(),
                })
                .await
                .expect("agent port claim")
                .is_none()
        );
    }

    #[test]
    fn rejects_agent_selected_activation_and_incomplete_identity() {
        let report = ModuleArtifactNodeAssignmentReport {
            claim_id: Uuid::new_v4(),
            reconciliation_id: Uuid::new_v4(),
            node_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            expected_observation_revision: 0,
            phase: ModuleReconciliationPhase::Active,
            installation_scope: ModuleArtifactNodeInstallationScope::Platform,
            release_digest: digest('a'),
            payload_digest: digest('b'),
            payload_kind: ArtifactPayloadKind::Rhai,
            payload_media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
            admission_revision: 1,
            dependency_graph_revision: 1,
            dependency_graph_digest: digest('c'),
            capability_grant_revision: 1,
            executor_abi: "rustok:module/runtime@1".to_string(),
            policy_revision: digest('d'),
            health_evidence: Some(ModuleReconciliationEvidence {
                reference: "evidence:node-a".to_string(),
                digest: digest('e'),
            }),
            failure: None,
            agent_id: "node-agent-a".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        assert!(matches!(
            validate_report(&report),
            Err(ModuleArtifactNodeReconciliationError::InvalidCommand)
        ));
        let mut incomplete_identity = report;
        incomplete_identity.phase = ModuleReconciliationPhase::Prepared;
        incomplete_identity.health_evidence = None;
        incomplete_identity.payload_media_type.clear();
        assert!(matches!(
            validate_report(&incomplete_identity),
            Err(ModuleArtifactNodeReconciliationError::InvalidCommand)
        ));
    }

    fn report_from_work(
        work: &ModuleArtifactNodeAssignmentWorkItem,
        phase: ModuleReconciliationPhase,
        health_evidence: Option<ModuleReconciliationEvidence>,
        failure: Option<ModuleReconciliationFailure>,
        idempotency_key: Uuid,
    ) -> ModuleArtifactNodeAssignmentReport {
        ModuleArtifactNodeAssignmentReport {
            claim_id: work.claim_id,
            reconciliation_id: work.reconciliation.reconciliation_id,
            node_id: work.assignment.node_id,
            installation_id: work.assignment.installation_id,
            expected_observation_revision: work.expected_observation_revision,
            phase,
            installation_scope: work.assignment.installation_scope,
            release_digest: work.assignment.release_digest.clone(),
            payload_digest: work.assignment.payload_digest.clone(),
            payload_kind: work.assignment.payload_kind,
            payload_media_type: work.assignment.payload_media_type.clone(),
            admission_revision: work.assignment.admission_revision,
            dependency_graph_revision: work.assignment.dependency_graph_revision,
            dependency_graph_digest: work.assignment.dependency_graph_digest.clone(),
            capability_grant_revision: work.assignment.capability_grant_revision,
            executor_abi: work.assignment.executor_abi.clone(),
            policy_revision: work.assignment.policy_revision.clone(),
            health_evidence,
            failure,
            agent_id: "node-agent-a".to_string(),
            idempotency_key,
        }
    }

    async fn database() -> DatabaseConnection {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("foreign keys");
        database
            .execute_unprepared(
                "CREATE TABLE module_artifact_installations ( \
                 installation_id TEXT PRIMARY KEY, scope_kind TEXT NOT NULL, manifest_digest TEXT NOT NULL, \
                 payload_digest TEXT NOT NULL, payload_kind TEXT NOT NULL, \
                 dependency_graph_revision INTEGER NOT NULL, \
                 dependency_graph_digest TEXT NOT NULL, capability_grant_revision INTEGER NOT NULL, \
                 runtime_abi TEXT NOT NULL)",
            )
            .await
            .expect("installations table");
        database
            .execute_unprepared(
                "CREATE TABLE module_artifact_admissions ( \
                 installation_id TEXT PRIMARY KEY, status TEXT NOT NULL, revision INTEGER NOT NULL, \
                 media_type TEXT NOT NULL)",
            )
            .await
            .expect("admissions table");
        database
            .execute_unprepared(
                "CREATE TABLE module_artifact_uninstall_operations (installation_id TEXT NOT NULL)",
            )
            .await
            .expect("uninstall operations table");
        crate::migrations::m20260814_000042_artifact_node_reconciliation::Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("artifact node migration");
        database
    }

    async fn insert_installation(database: &DatabaseConnection, installation_id: Uuid) {
        database
            .execute_unprepared(&format!(
                "INSERT INTO module_artifact_installations ( \
                 installation_id, scope_kind, manifest_digest, payload_digest, payload_kind, \
                 dependency_graph_revision, dependency_graph_digest, capability_grant_revision, runtime_abi \
                 ) VALUES ('{installation_id}', 'platform', '{}', '{}', 'rhai', 1, '{}', 1, 'rustok:module/runtime@1')",
                digest('1'),
                digest('2'),
                digest('3'),
            ))
            .await
            .expect("installation");
        database
            .execute_unprepared(&format!(
                "INSERT INTO module_artifact_admissions (installation_id, status, revision, media_type) \
                 VALUES ('{installation_id}', 'active', 1, 'application/vnd.rustok.rhai.source.v1')"
            ))
            .await
            .expect("admission");
    }

    fn digest(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }
}
