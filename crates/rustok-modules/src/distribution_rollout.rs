//! Durable desired/observed reconciliation for verified native distributions.
//!
//! The control plane records exact topology and node evidence. Deployment
//! agents perform the actual binary rollout outside this crate.

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
    ControlPlaneInfrastructure, ModuleStaticDistributionExecutorMode,
    ModuleStaticDistributionRelease, ModuleStaticDistributionReleaseStatus,
    ModuleStaticDistributionRole,
    data::{now_expression, placeholder, uuid_from_row, uuid_value},
    distribution_release::{commit_recovery_release, load_release_record, load_release_state},
    promotion::{digest_json, valid_digest, valid_reference},
    reconciliation::{
        ModuleDesiredObservedState, ModuleReconciliationEvidence, ModuleReconciliationFailure,
        ModuleReconciliationPhase,
    },
};

const ROLLOUT_STATE_ID: &str = "current";
const TOPOLOGY_DIGEST_CONTRACT: &str = "rustok.static_distribution.topology";
const MAX_TARGET_ASSIGNMENTS: usize = 1024;
const MAX_NODE_ID_BYTES: usize = 128;
const MAX_REFERENCE_BYTES: usize = 512;
const MAX_POLICY_REVISION_BYTES: usize = 128;
const MAX_FAILURE_CODE_BYTES: usize = 128;
const MAX_FAILURE_DETAIL_BYTES: usize = 2_000;
const MAX_AGENT_ID_BYTES: usize = 128;
const ASSIGNMENT_LEASE_SECONDS: i64 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStaticDistributionRolloutStatus {
    Preparing,
    Activating,
    Converged,
    Failed,
    Degraded,
    Superseded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStaticDistributionTransitionKind {
    Update,
    Recovery,
}

impl ModuleStaticDistributionTransitionKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Update => "update",
            Self::Recovery => "recovery",
        }
    }

    fn parse(value: &str) -> Result<Self, ModuleStaticDistributionRolloutError> {
        match value {
            "update" => Ok(Self::Update),
            "recovery" => Ok(Self::Recovery),
            _ => Err(ModuleStaticDistributionRolloutError::Store(
                "static distribution transition kind is invalid".to_string(),
            )),
        }
    }
}

impl ModuleStaticDistributionRolloutStatus {
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

    fn parse(value: &str) -> Result<Self, ModuleStaticDistributionRolloutError> {
        match value {
            "preparing" => Ok(Self::Preparing),
            "activating" => Ok(Self::Activating),
            "converged" => Ok(Self::Converged),
            "failed" => Ok(Self::Failed),
            "degraded" => Ok(Self::Degraded),
            "superseded" => Ok(Self::Superseded),
            _ => Err(ModuleStaticDistributionRolloutError::Store(
                "static distribution rollout status is invalid".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionTopologySnapshot {
    pub topology_reference: String,
    pub topology_digest: String,
    pub assignments: Vec<ModuleStaticDistributionAssignment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionAssignment {
    pub node_id: String,
    pub role: ModuleStaticDistributionRole,
    pub candidate_artifact_digest: String,
}

#[async_trait]
pub trait ModuleStaticDistributionTopologyResolver: Send + Sync {
    async fn resolve(
        &self,
        release: &ModuleStaticDistributionRelease,
        policy_revision: &str,
    ) -> Result<ModuleStaticDistributionTopologySnapshot, String>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionRolloutRequest {
    pub distribution_release_id: Uuid,
    pub expected_release_state_revision: u64,
    pub expected_distribution_release_revision: u64,
    pub expected_rollout_state_revision: u64,
    pub policy_revision: String,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionRecoveryRequest {
    pub current_rollout_id: Uuid,
    pub expected_release_state_revision: u64,
    pub expected_rollout_state_revision: u64,
    pub policy_revision: String,
    pub reason: String,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionAssignmentReport {
    pub claim_id: Uuid,
    pub rollout_id: Uuid,
    pub node_id: String,
    pub role: ModuleStaticDistributionRole,
    pub candidate_artifact_digest: String,
    pub expected_observation_revision: u64,
    pub phase: ModuleReconciliationPhase,
    pub distribution_release_id: Uuid,
    pub distribution_release_revision: u64,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub bundle_root_digest: String,
    pub role_set_digest: String,
    pub policy_revision: String,
    pub executor_mode: ModuleStaticDistributionExecutorMode,
    pub health_evidence: Option<ModuleReconciliationEvidence>,
    pub failure: Option<ModuleReconciliationFailure>,
    pub agent_id: String,
    pub idempotency_key: Uuid,
}

/// Authenticated pull request from a node agent. The agent can claim only an
/// owner-selected assignment for its own node; it never receives a generic
/// command or chooses a release, role, or artifact identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionAssignmentClaimCommand {
    pub node_id: String,
    pub agent_id: String,
}

/// Extends one exact assignment lease while bounded materialization or a
/// process/health transition is still in progress.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionAssignmentHeartbeatCommand {
    pub claim_id: Uuid,
    pub agent_id: String,
}

/// Immutable work handed to an authenticated outside-candidate node agent.
/// It is valid only while `lease_expires_at` remains current in the owner
/// ledger. The agent reports through the same claim and cannot mutate any
/// other assignment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionAssignmentWorkItem {
    pub claim_id: Uuid,
    pub lease_expires_at: DateTime<Utc>,
    pub expected_observation_revision: u64,
    pub rollout: ModuleStaticDistributionRolloutWorkIdentity,
    pub assignment: ModuleStaticDistributionRolloutAssignment,
}

/// The minimum rollout identity needed by one node agent. It deliberately
/// excludes assignments and observations for other nodes and roles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionRolloutWorkIdentity {
    pub rollout_id: Uuid,
    pub rollout_revision: u64,
    pub distribution_release_id: Uuid,
    pub distribution_release_revision: u64,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub bundle_reference: String,
    pub bundle_root_digest: String,
    pub role_set_digest: String,
    pub policy_revision: String,
    pub executor_mode: ModuleStaticDistributionExecutorMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionAssignmentHeartbeatReceipt {
    pub claim_id: Uuid,
    pub lease_expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait ModuleStaticDistributionRolloutAuthorizer: Send + Sync {
    async fn authorize_request(
        &self,
        command: &ModuleStaticDistributionRolloutRequest,
    ) -> Result<(), ModuleStaticDistributionRolloutError>;

    async fn authorize_report(
        &self,
        command: &ModuleStaticDistributionAssignmentReport,
    ) -> Result<(), ModuleStaticDistributionRolloutError>;

    async fn authorize_assignment_claim(
        &self,
        command: &ModuleStaticDistributionAssignmentClaimCommand,
    ) -> Result<(), ModuleStaticDistributionRolloutError>;

    async fn authorize_assignment_heartbeat(
        &self,
        command: &ModuleStaticDistributionAssignmentHeartbeatCommand,
    ) -> Result<(), ModuleStaticDistributionRolloutError>;

    async fn authorize_recovery(
        &self,
        command: &ModuleStaticDistributionRecoveryRequest,
    ) -> Result<(), ModuleStaticDistributionRolloutError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRolloutAssignment {
    pub node_id: String,
    pub role: ModuleStaticDistributionRole,
    pub candidate_artifact_digest: String,
    pub predecessor_artifact_digest: Option<String>,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRollout {
    pub rollout_id: Uuid,
    pub predecessor_rollout_id: Option<Uuid>,
    pub distribution_release_id: Uuid,
    pub transition_kind: ModuleStaticDistributionTransitionKind,
    pub rollout_revision: u64,
    pub distribution_release_revision: u64,
    pub release_state_revision_at_request: u64,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub bundle_reference: String,
    pub bundle_root_digest: String,
    pub role_set_digest: String,
    pub executor_mode: ModuleStaticDistributionExecutorMode,
    pub topology_reference: String,
    pub topology_digest: String,
    pub policy_revision: String,
    pub target_assignment_count: u16,
    pub status: ModuleStaticDistributionRolloutStatus,
    pub requested_by: Uuid,
    pub failure: Option<ModuleReconciliationFailure>,
    pub assignments: Vec<ModuleStaticDistributionRolloutAssignment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRolloutReceipt {
    pub rollout_id: Uuid,
    pub rollout_revision: u64,
    pub rollout_state_revision: u64,
    pub status: ModuleStaticDistributionRolloutStatus,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionAssignmentReportReceipt {
    pub rollout_id: Uuid,
    pub rollout_revision: u64,
    pub rollout_state_revision: u64,
    pub rollout_status: ModuleStaticDistributionRolloutStatus,
    pub node_id: String,
    pub role: ModuleStaticDistributionRole,
    pub observation_revision: u64,
    pub phase: ModuleReconciliationPhase,
    pub created: bool,
}

#[derive(Clone)]
pub struct SeaOrmModuleStaticDistributionRolloutService<A, T> {
    db: DatabaseConnection,
    authorizer: A,
    topology: T,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A, T> SeaOrmModuleStaticDistributionRolloutService<A, T>
where
    A: ModuleStaticDistributionRolloutAuthorizer,
    T: ModuleStaticDistributionTopologyResolver,
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

    pub async fn request(
        &self,
        command: ModuleStaticDistributionRolloutRequest,
    ) -> Result<ModuleStaticDistributionRolloutReceipt, ModuleStaticDistributionRolloutError> {
        validate_request(&command)?;
        self.authorizer.authorize_request(&command).await?;
        let request_digest = digest_json(&command).map_err(promotion_error)?;
        let principal_id = command.actor_id.to_string();
        if let Some(operation) = load_operation(
            &self.db,
            command.idempotency_key,
            "request",
            &request_digest,
            &principal_id,
        )
        .await?
        {
            return replay_request(&operation, "request");
        }

        let release_state = load_release_state(&self.db, false)
            .await
            .map_err(release_error)?;
        if release_state.revision != command.expected_release_state_revision {
            return Err(ModuleStaticDistributionRolloutError::ReleaseRevisionConflict);
        }
        let release = load_release_record(&self.db, command.distribution_release_id, false)
            .await
            .map_err(release_error)?;
        validate_release(&release, &command)?;
        let topology = self
            .topology
            .resolve(&release, &command.policy_revision)
            .await
            .map_err(ModuleStaticDistributionRolloutError::TopologyResolution)?;
        validate_topology(&topology, &release)?;

        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(operation) = reserve_operation(
            &transaction,
            command.idempotency_key,
            "request",
            &request_digest,
            &principal_id,
        )
        .await?
        {
            transaction.commit().await.map_err(store_error)?;
            return replay_request(&operation, "request");
        }
        let locked_release_state = load_release_state(&transaction, true)
            .await
            .map_err(release_error)?;
        if locked_release_state != release_state {
            return Err(ModuleStaticDistributionRolloutError::ReleaseRevisionConflict);
        }
        let locked_release =
            load_release_record(&transaction, command.distribution_release_id, true)
                .await
                .map_err(release_error)?;
        if locked_release != release {
            return Err(ModuleStaticDistributionRolloutError::ReleaseChanged);
        }
        let state = load_rollout_state(&transaction, true).await?;
        if state.revision != command.expected_rollout_state_revision {
            return Err(ModuleStaticDistributionRolloutError::RevisionConflict {
                expected: command.expected_rollout_state_revision,
                current: state.revision,
            });
        }
        let prior_desired = match state.desired_id {
            Some(rollout_id) => Some(load_rollout(&transaction, rollout_id, true).await?),
            None => None,
        };
        if prior_desired.as_ref().is_some_and(|rollout| {
            matches!(
                rollout.status,
                ModuleStaticDistributionRolloutStatus::Preparing
                    | ModuleStaticDistributionRolloutStatus::Activating
            )
        }) {
            return Err(ModuleStaticDistributionRolloutError::RolloutInProgress);
        }
        let predecessor = match state.observed_id {
            Some(rollout_id) => Some(load_rollout(&transaction, rollout_id, true).await?),
            None => None,
        };
        if predecessor.as_ref().is_some_and(|rollout| {
            rollout.status == ModuleStaticDistributionRolloutStatus::Converged
                && rollout.distribution_release_id == release.distribution_release_id
                && rollout.topology_digest == topology.topology_digest
                && rollout.policy_revision == command.policy_revision
        }) {
            return Err(ModuleStaticDistributionRolloutError::NoRolloutChange);
        }
        let rollout_revision = prior_desired.as_ref().map_or(Ok(1_u64), |rollout| {
            rollout
                .rollout_revision
                .checked_add(1)
                .ok_or(ModuleStaticDistributionRolloutError::RevisionOverflow)
        })?;
        let rollout_state_revision = state
            .revision
            .checked_add(1)
            .ok_or(ModuleStaticDistributionRolloutError::RevisionOverflow)?;
        let rollout_id = self.infrastructure.new_id();
        insert_rollout(
            &transaction,
            RolloutInsert {
                rollout_id,
                predecessor_rollout_id: predecessor.as_ref().map(|rollout| rollout.rollout_id),
                rollout_revision,
                release: &release,
                topology: &topology,
                command: &command,
                transition_kind: ModuleStaticDistributionTransitionKind::Update,
            },
        )
        .await?;
        insert_rollout_assignments(
            &transaction,
            rollout_id,
            &topology.assignments,
            predecessor.as_ref(),
        )
        .await?;
        advance_rollout_state(
            &transaction,
            state.revision,
            rollout_state_revision,
            Some(rollout_id),
            state.observed_id,
        )
        .await?;
        let receipt = ModuleStaticDistributionRolloutReceipt {
            rollout_id,
            rollout_revision,
            rollout_state_revision,
            status: ModuleStaticDistributionRolloutStatus::Preparing,
            created: true,
        };
        complete_request_operation(&transaction, command.idempotency_key, &receipt).await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    Some(command.actor_id),
                    DomainEvent::ModuleStaticDistributionRolloutRequested {
                        rollout_id,
                        predecessor_rollout_id: predecessor.map(|rollout| rollout.rollout_id),
                        distribution_release_id: release.distribution_release_id,
                        rollout_revision,
                        rollout_state_revision,
                        composition_revision: release.composition_revision,
                        composition_digest: release.composition_digest,
                        bundle_root_digest: release.evidence.bundle_root_digest,
                        role_set_digest: release.evidence.role_set_digest,
                        topology_digest: topology.topology_digest,
                        policy_revision: command.policy_revision,
                        target_assignments: u32::try_from(topology.assignments.len())
                            .map_err(|_| ModuleStaticDistributionRolloutError::InvalidTopology)?,
                        executor_mode: "static_native".to_string(),
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(receipt)
    }

    pub async fn recover(
        &self,
        command: ModuleStaticDistributionRecoveryRequest,
    ) -> Result<ModuleStaticDistributionRolloutReceipt, ModuleStaticDistributionRolloutError> {
        validate_recovery_request(&command)?;
        self.authorizer.authorize_recovery(&command).await?;
        let request_digest = digest_json(&command).map_err(promotion_error)?;
        let principal_id = command.actor_id.to_string();
        if let Some(operation) = load_operation(
            &self.db,
            command.idempotency_key,
            "recovery",
            &request_digest,
            &principal_id,
        )
        .await?
        {
            return replay_request(&operation, "recovery");
        }

        let transaction = self.db.begin().await.map_err(store_error)?;
        if let Some(operation) = reserve_operation(
            &transaction,
            command.idempotency_key,
            "recovery",
            &request_digest,
            &principal_id,
        )
        .await?
        {
            transaction.commit().await.map_err(store_error)?;
            return replay_request(&operation, "recovery");
        }
        let release_state = load_release_state(&transaction, true)
            .await
            .map_err(release_error)?;
        if release_state.revision != command.expected_release_state_revision {
            return Err(ModuleStaticDistributionRolloutError::ReleaseRevisionConflict);
        }
        let state = load_rollout_state(&transaction, true).await?;
        if state.revision != command.expected_rollout_state_revision
            || state.desired_id != Some(command.current_rollout_id)
        {
            return Err(ModuleStaticDistributionRolloutError::StaleRollout);
        }
        let current = load_rollout(&transaction, command.current_rollout_id, true).await?;
        if current.transition_kind == ModuleStaticDistributionTransitionKind::Recovery {
            return Err(ModuleStaticDistributionRolloutError::RecoveryUnavailable);
        }
        let predecessor_rollout_id = current
            .predecessor_rollout_id
            .ok_or(ModuleStaticDistributionRolloutError::RecoveryUnavailable)?;
        let predecessor = load_rollout(&transaction, predecessor_rollout_id, true).await?;
        let target_release =
            load_release_record(&transaction, predecessor.distribution_release_id, true)
                .await
                .map_err(release_error)?;
        let source_release =
            load_release_record(&transaction, current.distribution_release_id, true)
                .await
                .map_err(release_error)?;
        let accepted_regression = current.status
            == ModuleStaticDistributionRolloutStatus::Converged
            && state.converged_id() == Some(current.rollout_id)
            && source_release.status == ModuleStaticDistributionReleaseStatus::Active
            && target_release.status == ModuleStaticDistributionReleaseStatus::Superseded
            && release_state.active_release_id == Some(source_release.distribution_release_id);
        let partial_rollout_failure = current.status
            == ModuleStaticDistributionRolloutStatus::Failed
            && state.observed_id == Some(predecessor.rollout_id)
            && source_release.status == ModuleStaticDistributionReleaseStatus::Admitted
            && target_release.status == ModuleStaticDistributionReleaseStatus::Active
            && release_state.active_release_id == Some(target_release.distribution_release_id);
        if (!accepted_regression && !partial_rollout_failure)
            || target_release.predecessor_release_id == Some(current.distribution_release_id)
            || predecessor.assignments.len() != current.assignments.len()
            || current.assignments.iter().any(|assignment| {
                assignment
                    .predecessor_artifact_digest
                    .as_ref()
                    .is_none_or(|digest| {
                        !predecessor.assignments.iter().any(|candidate| {
                            candidate.node_id == assignment.node_id
                                && candidate.role == assignment.role
                                && candidate.candidate_artifact_digest == *digest
                        })
                    })
            })
        {
            return Err(ModuleStaticDistributionRolloutError::RecoveryUnavailable);
        }
        let assignments = predecessor
            .assignments
            .iter()
            .map(|assignment| ModuleStaticDistributionAssignment {
                node_id: assignment.node_id.clone(),
                role: assignment.role,
                candidate_artifact_digest: assignment.candidate_artifact_digest.clone(),
            })
            .collect::<Vec<_>>();
        let topology_digest =
            module_static_distribution_topology_digest(&current.topology_reference, &assignments)?;
        let topology = ModuleStaticDistributionTopologySnapshot {
            topology_reference: current.topology_reference.clone(),
            topology_digest,
            assignments,
        };
        validate_topology(&topology, &target_release)?;

        let rollout_revision = current
            .rollout_revision
            .checked_add(1)
            .ok_or(ModuleStaticDistributionRolloutError::RevisionOverflow)?;
        let rollout_state_revision = state
            .revision
            .checked_add(1)
            .ok_or(ModuleStaticDistributionRolloutError::RevisionOverflow)?;
        let rollout_id = self.infrastructure.new_id();
        let request = ModuleStaticDistributionRolloutRequest {
            distribution_release_id: target_release.distribution_release_id,
            expected_release_state_revision: command.expected_release_state_revision,
            expected_distribution_release_revision: target_release.release_revision,
            expected_rollout_state_revision: command.expected_rollout_state_revision,
            policy_revision: command.policy_revision,
            actor_id: command.actor_id,
            idempotency_key: command.idempotency_key,
        };
        insert_rollout(
            &transaction,
            RolloutInsert {
                rollout_id,
                predecessor_rollout_id: Some(current.rollout_id),
                rollout_revision,
                release: &target_release,
                topology: &topology,
                command: &request,
                transition_kind: ModuleStaticDistributionTransitionKind::Recovery,
            },
        )
        .await?;
        insert_rollout_assignments(&transaction, rollout_id, &topology.assignments, None).await?;
        advance_rollout_state(
            &transaction,
            state.revision,
            rollout_state_revision,
            Some(rollout_id),
            state.observed_id,
        )
        .await?;
        let receipt = ModuleStaticDistributionRolloutReceipt {
            rollout_id,
            rollout_revision,
            rollout_state_revision,
            status: ModuleStaticDistributionRolloutStatus::Preparing,
            created: true,
        };
        complete_request_operation(&transaction, command.idempotency_key, &receipt).await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    None,
                    Some(command.actor_id),
                    DomainEvent::ModuleStaticDistributionRecoveryRequested {
                        rollout_id,
                        predecessor_rollout_id: current.rollout_id,
                        from_release_id: current.distribution_release_id,
                        target_release_id: target_release.distribution_release_id,
                        rollout_revision,
                        rollout_state_revision,
                        topology_digest: topology.topology_digest,
                        policy_revision: request.policy_revision,
                        reason: command.reason,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(receipt)
    }

    /// Returns one exact owner-issued role assignment for the authenticated
    /// node agent. A stale lease may be reclaimed after the agent process
    /// dies; an active lease is never handed to a second agent.
    pub async fn claim_assignment(
        &self,
        command: ModuleStaticDistributionAssignmentClaimCommand,
    ) -> Result<
        Option<ModuleStaticDistributionAssignmentWorkItem>,
        ModuleStaticDistributionRolloutError,
    > {
        validate_assignment_claim(&command)?;
        self.authorizer.authorize_assignment_claim(&command).await?;
        let transaction = self.db.begin().await.map_err(store_error)?;
        let state = load_rollout_state(&transaction, true).await?;
        let Some(rollout_id) = state.desired_id else {
            transaction.commit().await.map_err(store_error)?;
            return Ok(None);
        };
        let rollout = load_rollout(&transaction, rollout_id, true).await?;
        if !matches!(
            rollout.status,
            ModuleStaticDistributionRolloutStatus::Preparing
                | ModuleStaticDistributionRolloutStatus::Activating
        ) {
            transaction.commit().await.map_err(store_error)?;
            return Ok(None);
        }
        validate_rollout_release_identity(&transaction, &rollout).await?;
        let now = self.infrastructure.now();
        let Some(assignment) = load_next_assignment_for_node(
            &transaction,
            rollout_id,
            &command.node_id,
            &command.agent_id,
            &now,
        )
        .await?
        else {
            transaction.commit().await.map_err(store_error)?;
            return Ok(None);
        };
        // Claim replay is deliberately exact. An agent can lose the response
        // after the owner commits a lease; returning the same lease lets its
        // local journal resume work without a second rollout assignment.
        if let (Some(claim_id), Some(claimed_by_agent), Some(lease_expires_at)) = (
            assignment.active_claim_id,
            assignment.claimed_by_agent.as_deref(),
            assignment.claim_expires_at,
        ) && claimed_by_agent == command.agent_id.as_str()
        {
            transaction.commit().await.map_err(store_error)?;
            return Ok(Some(ModuleStaticDistributionAssignmentWorkItem {
                claim_id,
                lease_expires_at,
                expected_observation_revision: assignment.observation_revision,
                rollout: rollout_work_identity(&rollout),
                assignment,
            }));
        }
        let lease_expires_at = now
            .checked_add_signed(Duration::seconds(ASSIGNMENT_LEASE_SECONDS))
            .ok_or(ModuleStaticDistributionRolloutError::LeaseOverflow)?;
        let claim_id = self.infrastructure.new_id();
        claim_rollout_assignment(
            &transaction,
            rollout_id,
            &assignment,
            claim_id,
            &command.agent_id,
            &now,
            &lease_expires_at,
        )
        .await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(Some(ModuleStaticDistributionAssignmentWorkItem {
            claim_id,
            lease_expires_at,
            expected_observation_revision: assignment.observation_revision,
            rollout: rollout_work_identity(&rollout),
            assignment,
        }))
    }

    /// Renews an exact owner-issued assignment lease. This does not change
    /// desired identity or serving state.
    pub async fn heartbeat_assignment(
        &self,
        command: ModuleStaticDistributionAssignmentHeartbeatCommand,
    ) -> Result<
        ModuleStaticDistributionAssignmentHeartbeatReceipt,
        ModuleStaticDistributionRolloutError,
    > {
        validate_assignment_heartbeat(&command)?;
        self.authorizer
            .authorize_assignment_heartbeat(&command)
            .await?;
        let now = self.infrastructure.now();
        let lease_expires_at = now
            .checked_add_signed(Duration::seconds(ASSIGNMENT_LEASE_SECONDS))
            .ok_or(ModuleStaticDistributionRolloutError::LeaseOverflow)?;
        let transaction = self.db.begin().await.map_err(store_error)?;
        heartbeat_rollout_assignment(&transaction, &command, &now, &lease_expires_at).await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ModuleStaticDistributionAssignmentHeartbeatReceipt {
            claim_id: command.claim_id,
            lease_expires_at,
        })
    }

    pub async fn report(
        &self,
        command: ModuleStaticDistributionAssignmentReport,
    ) -> Result<ModuleStaticDistributionAssignmentReportReceipt, ModuleStaticDistributionRolloutError>
    {
        validate_report(&command)?;
        self.authorizer.authorize_report(&command).await?;
        let request_digest = digest_json(&command).map_err(promotion_error)?;
        if let Some(operation) = load_operation(
            &self.db,
            command.idempotency_key,
            "report",
            &request_digest,
            &command.agent_id,
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
        )
        .await?
        {
            transaction.commit().await.map_err(store_error)?;
            return replay_report(&operation);
        }
        let state = load_rollout_state(&transaction, true).await?;
        if state.desired_id != Some(command.rollout_id) {
            return Err(ModuleStaticDistributionRolloutError::StaleRollout);
        }
        let rollout = load_rollout(&transaction, command.rollout_id, true).await?;
        if matches!(
            rollout.status,
            ModuleStaticDistributionRolloutStatus::Failed
                | ModuleStaticDistributionRolloutStatus::Superseded
        ) {
            return Err(ModuleStaticDistributionRolloutError::TerminalRollout);
        }
        let release_state = load_release_state(&transaction, true)
            .await
            .map_err(release_error)?;
        let current_release =
            load_release_record(&transaction, rollout.distribution_release_id, true)
                .await
                .map_err(release_error)?;
        let recovery_source_release =
            if rollout.transition_kind == ModuleStaticDistributionTransitionKind::Recovery {
                let predecessor_rollout_id = rollout
                    .predecessor_rollout_id
                    .ok_or(ModuleStaticDistributionRolloutError::RecoveryUnavailable)?;
                Some(
                    load_rollout(&transaction, predecessor_rollout_id, false)
                        .await?
                        .distribution_release_id,
                )
            } else {
                None
            };
        let release_identity_valid = match rollout.transition_kind {
            ModuleStaticDistributionTransitionKind::Update => {
                current_release.status == ModuleStaticDistributionReleaseStatus::Admitted
                    && release_state.revision == rollout.release_state_revision_at_request
            }
            ModuleStaticDistributionTransitionKind::Recovery => {
                let source_release_id = recovery_source_release
                    .ok_or(ModuleStaticDistributionRolloutError::RecoveryUnavailable)?;
                let source_release = load_release_record(&transaction, source_release_id, true)
                    .await
                    .map_err(release_error)?;
                release_state.revision == rollout.release_state_revision_at_request
                    && ((current_release.status
                        == ModuleStaticDistributionReleaseStatus::Superseded
                        && source_release.status == ModuleStaticDistributionReleaseStatus::Active
                        && release_state.active_release_id == Some(source_release_id))
                        || (current_release.status
                            == ModuleStaticDistributionReleaseStatus::Active
                            && source_release.status
                                == ModuleStaticDistributionReleaseStatus::Admitted
                            && release_state.active_release_id
                                == Some(rollout.distribution_release_id)))
            }
        };
        if !release_identity_valid
            || current_release.release_revision != rollout.distribution_release_revision
        {
            return Err(ModuleStaticDistributionRolloutError::StaleRollout);
        }
        validate_report_identity(&command, &rollout)?;
        let assignment = load_rollout_assignment(
            &transaction,
            command.rollout_id,
            &command.node_id,
            command.role,
            true,
        )
        .await?;
        let now = self.infrastructure.now();
        if assignment.active_claim_id != Some(command.claim_id)
            || assignment.claimed_by_agent.as_deref() != Some(command.agent_id.as_str())
            || assignment
                .claim_expires_at
                .is_none_or(|expires_at| expires_at < now)
        {
            return Err(ModuleStaticDistributionRolloutError::ClaimConflict);
        }
        if assignment.observation_revision != command.expected_observation_revision {
            return Err(
                ModuleStaticDistributionRolloutError::ObservationRevisionConflict {
                    expected: command.expected_observation_revision,
                    current: assignment.observation_revision,
                },
            );
        }
        if assignment.candidate_artifact_digest != command.candidate_artifact_digest {
            return Err(ModuleStaticDistributionRolloutError::ObservationIdentityMismatch);
        }
        validate_transition(assignment.phase, command.phase, rollout.status)?;
        let observation_revision = assignment
            .observation_revision
            .checked_add(1)
            .ok_or(ModuleStaticDistributionRolloutError::RevisionOverflow)?;
        update_rollout_assignment(
            &transaction,
            &command,
            observation_revision,
            &request_digest,
            &now,
        )
        .await?;

        let phase_counts = load_phase_counts(&transaction, command.rollout_id).await?;
        let target_assignments = usize::from(rollout.target_assignment_count);
        let mut next_status = rollout.status;
        let mut state_revision = state.revision;
        let mut observed_rollout_id = state.observed_id;
        let mut status_failure = None;
        let mut release_convergence = None;
        if command.phase == ModuleReconciliationPhase::Failed {
            status_failure = command.failure.clone();
            next_status = if rollout.status == ModuleStaticDistributionRolloutStatus::Converged {
                observed_rollout_id = None;
                ModuleStaticDistributionRolloutStatus::Degraded
            } else {
                ModuleStaticDistributionRolloutStatus::Failed
            };
        } else if matches!(
            rollout.status,
            ModuleStaticDistributionRolloutStatus::Preparing
                | ModuleStaticDistributionRolloutStatus::Degraded
        ) && phase_counts.ready_for_activation() == target_assignments
        {
            next_status = ModuleStaticDistributionRolloutStatus::Activating;
        } else if rollout.status == ModuleStaticDistributionRolloutStatus::Activating
            && phase_counts.active == target_assignments
        {
            next_status = ModuleStaticDistributionRolloutStatus::Converged;
            observed_rollout_id = Some(rollout.rollout_id);
            let from_release_id = release_state.active_release_id;
            let release_state_revision = match rollout.transition_kind {
                ModuleStaticDistributionTransitionKind::Update => {
                    crate::distribution_release::commit_admitted_release(
                        &transaction,
                        rollout.release_state_revision_at_request,
                        from_release_id,
                        rollout.distribution_release_id,
                    )
                    .await
                    .map_err(release_error)?
                }
                ModuleStaticDistributionTransitionKind::Recovery => {
                    if from_release_id == Some(rollout.distribution_release_id) {
                        rollout.release_state_revision_at_request
                    } else {
                        let from_release_id = from_release_id
                            .ok_or(ModuleStaticDistributionRolloutError::RecoveryUnavailable)?;
                        commit_recovery_release(
                            &transaction,
                            rollout.release_state_revision_at_request,
                            from_release_id,
                            rollout.distribution_release_id,
                        )
                        .await
                        .map_err(release_error)?
                    }
                }
            };
            release_convergence = Some((from_release_id, release_state_revision));
        }

        if next_status != rollout.status {
            state_revision = state
                .revision
                .checked_add(1)
                .ok_or(ModuleStaticDistributionRolloutError::RevisionOverflow)?;
            update_rollout_status(
                &transaction,
                rollout.rollout_id,
                rollout.status,
                next_status,
                status_failure.as_ref(),
            )
            .await?;
            if next_status == ModuleStaticDistributionRolloutStatus::Converged
                && let Some(previous_observed) = state
                    .observed_id
                    .filter(|rollout_id| *rollout_id != rollout.rollout_id)
            {
                supersede_rollout(&transaction, previous_observed).await?;
            }
            advance_rollout_state(
                &transaction,
                state.revision,
                state_revision,
                state.desired_id,
                observed_rollout_id,
            )
            .await?;
        }
        if let Some((from_release_id, release_state_revision)) = release_convergence {
            let event = match rollout.transition_kind {
                ModuleStaticDistributionTransitionKind::Update => {
                    DomainEvent::ModuleStaticDistributionReleaseActivated {
                        distribution_release_id: rollout.distribution_release_id,
                        predecessor_release_id: from_release_id,
                        rollout_id: rollout.rollout_id,
                        release_state_revision,
                    }
                }
                ModuleStaticDistributionTransitionKind::Recovery => {
                    DomainEvent::ModuleStaticDistributionRecoveryConverged {
                        rollout_id: rollout.rollout_id,
                        from_release_id: recovery_source_release
                            .ok_or(ModuleStaticDistributionRolloutError::RecoveryUnavailable)?,
                        target_release_id: rollout.distribution_release_id,
                        release_state_revision,
                        rollout_state_revision: state_revision,
                    }
                }
            };
            self.infrastructure
                .write_event(
                    &transaction,
                    self.infrastructure.event_envelope(None, None, event),
                )
                .await
                .map_err(store_error)?;
        }

        let receipt = ModuleStaticDistributionAssignmentReportReceipt {
            rollout_id: rollout.rollout_id,
            rollout_revision: rollout.rollout_revision,
            rollout_state_revision: state_revision,
            rollout_status: next_status,
            node_id: command.node_id.clone(),
            role: command.role,
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
                    DomainEvent::ModuleStaticDistributionAssignmentObserved {
                        rollout_id: rollout.rollout_id,
                        node_id: command.node_id,
                        role: role_name(command.role).to_string(),
                        candidate_artifact_digest: command.candidate_artifact_digest,
                        reporter_id: command.agent_id,
                        observation_revision,
                        phase: command.phase.as_str().to_string(),
                        report_digest: request_digest,
                    },
                ),
            )
            .await
            .map_err(store_error)?;
        if next_status != rollout.status {
            self.infrastructure
                .write_event(
                    &transaction,
                    self.infrastructure.event_envelope(
                        None,
                        None,
                        DomainEvent::ModuleStaticDistributionRolloutStatusChanged {
                            rollout_id: rollout.rollout_id,
                            distribution_release_id: rollout.distribution_release_id,
                            rollout_revision: rollout.rollout_revision,
                            rollout_state_revision: state_revision,
                            status: next_status.as_str().to_string(),
                            observed_rollout_id,
                            failure_code: if matches!(
                                next_status,
                                ModuleStaticDistributionRolloutStatus::Failed
                                    | ModuleStaticDistributionRolloutStatus::Degraded
                            ) {
                                status_failure.map(|failure| failure.code)
                            } else {
                                None
                            },
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
    ) -> Result<ModuleDesiredObservedState, ModuleStaticDistributionRolloutError> {
        load_rollout_state(&self.db, false).await
    }

    pub async fn get(
        &self,
        rollout_id: Uuid,
    ) -> Result<ModuleStaticDistributionRollout, ModuleStaticDistributionRolloutError> {
        if rollout_id.is_nil() {
            return Err(ModuleStaticDistributionRolloutError::InvalidCommand);
        }
        load_rollout(&self.db, rollout_id, false).await
    }
}

/// Invalidates desired and observed rollouts when their release is revoked.
/// The caller must invoke this in the same transaction as release-head CAS.
pub(crate) async fn revoke_rollouts_for_release(
    transaction: &DatabaseTransaction,
    infrastructure: &ControlPlaneInfrastructure,
    release_id: Uuid,
    actor_id: Uuid,
    policy_revision: &str,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let state = load_rollout_state(transaction, true).await?;
    let mut rollout_ids = Vec::new();
    for rollout_id in [state.desired_id, state.observed_id].into_iter().flatten() {
        if !rollout_ids.contains(&rollout_id) {
            rollout_ids.push(rollout_id);
        }
    }
    let mut next_state_revision = state.revision;
    let mut desired_rollout_id = state.desired_id;
    let mut observed_rollout_id = state.observed_id;
    let mut status_events = Vec::new();
    for rollout_id in rollout_ids {
        let rollout = load_rollout(transaction, rollout_id, true).await?;
        if rollout.distribution_release_id != release_id {
            continue;
        }
        let next_status = match rollout.status {
            ModuleStaticDistributionRolloutStatus::Preparing
            | ModuleStaticDistributionRolloutStatus::Activating => {
                ModuleStaticDistributionRolloutStatus::Failed
            }
            ModuleStaticDistributionRolloutStatus::Converged => {
                ModuleStaticDistributionRolloutStatus::Degraded
            }
            _ => continue,
        };
        let failure = ModuleReconciliationFailure {
            code: "release_revoked".to_string(),
            detail: format!("distribution release was revoked under policy `{policy_revision}`"),
        };
        update_rollout_status(
            transaction,
            rollout.rollout_id,
            rollout.status,
            next_status,
            Some(&failure),
        )
        .await?;
        next_state_revision = next_state_revision
            .checked_add(1)
            .ok_or(ModuleStaticDistributionRolloutError::RevisionOverflow)?;
        if desired_rollout_id == Some(rollout.rollout_id) {
            desired_rollout_id = None;
        }
        if observed_rollout_id == Some(rollout.rollout_id) {
            observed_rollout_id = None;
        }
        status_events.push((rollout, next_status, failure));
    }
    if next_state_revision != state.revision {
        advance_rollout_state(
            transaction,
            state.revision,
            next_state_revision,
            desired_rollout_id,
            observed_rollout_id,
        )
        .await?;
        for (rollout, status, failure) in status_events {
            infrastructure
                .write_event(
                    transaction,
                    infrastructure.event_envelope(
                        None,
                        Some(actor_id),
                        DomainEvent::ModuleStaticDistributionRolloutStatusChanged {
                            rollout_id: rollout.rollout_id,
                            distribution_release_id: rollout.distribution_release_id,
                            rollout_revision: rollout.rollout_revision,
                            rollout_state_revision: next_state_revision,
                            status: status.as_str().to_string(),
                            observed_rollout_id,
                            failure_code: Some(failure.code),
                        },
                    ),
                )
                .await
                .map_err(store_error)?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct OperationRecord {
    operation_kind: String,
    request_digest: String,
    principal_id: String,
    rollout_id: Option<Uuid>,
    rollout_revision: Option<u64>,
    rollout_state_revision: Option<u64>,
    rollout_status: Option<ModuleStaticDistributionRolloutStatus>,
    node_id: Option<String>,
    role: Option<ModuleStaticDistributionRole>,
    observation_revision: Option<u64>,
    assignment_phase: Option<ModuleReconciliationPhase>,
    completed: bool,
}

#[derive(Default)]
struct PhaseCounts {
    pending: usize,
    prepared: usize,
    healthy: usize,
    active: usize,
    failed: usize,
}

impl PhaseCounts {
    fn ready_for_activation(&self) -> usize {
        self.healthy + self.active
    }
}

#[derive(Debug, Error)]
pub enum ModuleStaticDistributionRolloutError {
    #[error("static distribution rollout command is invalid")]
    InvalidCommand,
    #[error("static distribution topology is invalid")]
    InvalidTopology,
    #[error("static distribution topology resolution failed: {0}")]
    TopologyResolution(String),
    #[error("static distribution release is not the exact active release revision")]
    ReleaseRevisionConflict,
    #[error("static distribution release changed during rollout request")]
    ReleaseChanged,
    #[error(
        "static distribution rollout state revision conflict: expected {expected}, current {current}"
    )]
    RevisionConflict { expected: u64, current: u64 },
    #[error(
        "static distribution assignment observation revision conflict: expected {expected}, current {current}"
    )]
    ObservationRevisionConflict { expected: u64, current: u64 },
    #[error("a static distribution rollout is already preparing or activating")]
    RolloutInProgress,
    #[error("static distribution rollout does not change release, topology, or policy")]
    NoRolloutChange,
    #[error("static distribution rollout is stale")]
    StaleRollout,
    #[error("static distribution rollout is terminal")]
    TerminalRollout,
    #[error("static distribution rollout was not found")]
    RolloutNotFound,
    #[error("static distribution direct-predecessor recovery is unavailable")]
    RecoveryUnavailable,
    #[error("static distribution rollout assignment was not found")]
    AssignmentNotFound,
    #[error("static distribution rollout assignment claim conflicts or has expired")]
    ClaimConflict,
    #[error("static distribution rollout assignment lease overflow")]
    LeaseOverflow,
    #[error("static distribution assignment report identity does not match the desired rollout")]
    ObservationIdentityMismatch,
    #[error("static distribution assignment phase transition is invalid")]
    InvalidTransition,
    #[error("static distribution rollout idempotency key conflicts with another command")]
    IdempotencyConflict,
    #[error("static distribution rollout revision overflow")]
    RevisionOverflow,
    #[error("static distribution rollout authorization denied: {0}")]
    AuthorizationDenied(String),
    #[error("static distribution rollout store failed: {0}")]
    Store(String),
}

fn validate_request(
    command: &ModuleStaticDistributionRolloutRequest,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    if command.distribution_release_id.is_nil()
        || command.expected_release_state_revision == 0
        || command.expected_distribution_release_revision == 0
        || !valid_text(&command.policy_revision, MAX_POLICY_REVISION_BYTES)
        || command.actor_id.is_nil()
        || command.idempotency_key.is_nil()
    {
        return Err(ModuleStaticDistributionRolloutError::InvalidCommand);
    }
    Ok(())
}

fn validate_recovery_request(
    command: &ModuleStaticDistributionRecoveryRequest,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    if command.current_rollout_id.is_nil()
        || command.expected_release_state_revision == 0
        || !valid_text(&command.policy_revision, MAX_POLICY_REVISION_BYTES)
        || !valid_text(&command.reason, MAX_FAILURE_DETAIL_BYTES)
        || command.actor_id.is_nil()
        || command.idempotency_key.is_nil()
    {
        return Err(ModuleStaticDistributionRolloutError::InvalidCommand);
    }
    Ok(())
}

fn validate_release(
    release: &ModuleStaticDistributionRelease,
    command: &ModuleStaticDistributionRolloutRequest,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    if release.distribution_release_id != command.distribution_release_id
        || release.release_revision != command.expected_distribution_release_revision
        || release.status != ModuleStaticDistributionReleaseStatus::Admitted
        || release
            .items
            .iter()
            .any(|item| item.executor_mode != ModuleStaticDistributionExecutorMode::StaticNative)
    {
        return Err(ModuleStaticDistributionRolloutError::ReleaseRevisionConflict);
    }
    Ok(())
}

#[derive(Serialize)]
struct TopologyDigestInput<'a> {
    contract: &'static str,
    topology_reference: &'a str,
    assignments: &'a [ModuleStaticDistributionAssignment],
}

pub fn module_static_distribution_topology_digest(
    topology_reference: &str,
    assignments: &[ModuleStaticDistributionAssignment],
) -> Result<String, ModuleStaticDistributionRolloutError> {
    digest_json(&TopologyDigestInput {
        contract: TOPOLOGY_DIGEST_CONTRACT,
        topology_reference,
        assignments,
    })
    .map_err(promotion_error)
}

fn validate_topology(
    topology: &ModuleStaticDistributionTopologySnapshot,
    release: &ModuleStaticDistributionRelease,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    if !valid_reference(&topology.topology_reference)
        || topology.topology_reference.len() > MAX_REFERENCE_BYTES
        || !valid_digest(&topology.topology_digest)
        || topology.assignments.is_empty()
        || topology.assignments.len() > MAX_TARGET_ASSIGNMENTS
        || topology.assignments.iter().any(|assignment| {
            !valid_text(&assignment.node_id, MAX_NODE_ID_BYTES)
                || !valid_digest(&assignment.candidate_artifact_digest)
                || !release.evidence.roles.iter().any(|role| {
                    role.role == assignment.role
                        && role.artifact_digest == assignment.candidate_artifact_digest
                })
        })
        || topology.assignments.windows(2).any(|assignments| {
            assignment_sort_key(&assignments[0]) >= assignment_sort_key(&assignments[1])
        })
    {
        return Err(ModuleStaticDistributionRolloutError::InvalidTopology);
    }
    let expected_digest = module_static_distribution_topology_digest(
        &topology.topology_reference,
        &topology.assignments,
    )?;
    if topology.topology_digest != expected_digest {
        return Err(ModuleStaticDistributionRolloutError::InvalidTopology);
    }
    Ok(())
}

fn assignment_sort_key(assignment: &ModuleStaticDistributionAssignment) -> (&str, u8, &str) {
    (
        assignment.node_id.as_str(),
        role_ordinal(assignment.role),
        assignment.candidate_artifact_digest.as_str(),
    )
}

const fn role_ordinal(role: ModuleStaticDistributionRole) -> u8 {
    match role {
        ModuleStaticDistributionRole::Monolith => 0,
        ModuleStaticDistributionRole::Api => 1,
        ModuleStaticDistributionRole::AdminSsr => 2,
        ModuleStaticDistributionRole::StorefrontSsr => 3,
        ModuleStaticDistributionRole::Worker => 4,
        ModuleStaticDistributionRole::Registry => 5,
    }
}

const fn role_name(role: ModuleStaticDistributionRole) -> &'static str {
    match role {
        ModuleStaticDistributionRole::Monolith => "monolith",
        ModuleStaticDistributionRole::Api => "api",
        ModuleStaticDistributionRole::AdminSsr => "admin_ssr",
        ModuleStaticDistributionRole::StorefrontSsr => "storefront_ssr",
        ModuleStaticDistributionRole::Worker => "worker",
        ModuleStaticDistributionRole::Registry => "registry",
    }
}

fn parse_role(
    value: &str,
) -> Result<ModuleStaticDistributionRole, ModuleStaticDistributionRolloutError> {
    match value {
        "monolith" => Ok(ModuleStaticDistributionRole::Monolith),
        "api" => Ok(ModuleStaticDistributionRole::Api),
        "admin_ssr" => Ok(ModuleStaticDistributionRole::AdminSsr),
        "storefront_ssr" => Ok(ModuleStaticDistributionRole::StorefrontSsr),
        "worker" => Ok(ModuleStaticDistributionRole::Worker),
        "registry" => Ok(ModuleStaticDistributionRole::Registry),
        _ => Err(ModuleStaticDistributionRolloutError::Store(
            "static distribution assignment role is invalid".to_string(),
        )),
    }
}

fn parse_reconciliation_phase(
    value: &str,
) -> Result<ModuleReconciliationPhase, ModuleStaticDistributionRolloutError> {
    ModuleReconciliationPhase::parse(value).ok_or_else(|| {
        ModuleStaticDistributionRolloutError::Store(
            "module reconciliation assignment phase is invalid".to_string(),
        )
    })
}

fn validate_report(
    command: &ModuleStaticDistributionAssignmentReport,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let health_evidence_valid = command.health_evidence.as_ref().is_none_or(|evidence| {
        valid_reference(&evidence.reference)
            && evidence.reference.len() <= MAX_REFERENCE_BYTES
            && valid_digest(&evidence.digest)
    });
    let failure_valid = command.failure.as_ref().is_none_or(|failure| {
        valid_text(&failure.code, MAX_FAILURE_CODE_BYTES)
            && valid_text(&failure.detail, MAX_FAILURE_DETAIL_BYTES)
    });
    let phase_payload_valid = command
        .phase
        .permits_report_payload(command.health_evidence.is_some(), command.failure.is_some())
        && health_evidence_valid
        && failure_valid;
    if command.claim_id.is_nil()
        || command.rollout_id.is_nil()
        || !valid_text(&command.node_id, MAX_NODE_ID_BYTES)
        || !valid_digest(&command.candidate_artifact_digest)
        || command.distribution_release_id.is_nil()
        || command.distribution_release_revision == 0
        || command.composition_revision == 0
        || !valid_digest(&command.composition_digest)
        || !valid_digest(&command.bundle_root_digest)
        || !valid_digest(&command.role_set_digest)
        || !valid_text(&command.policy_revision, MAX_POLICY_REVISION_BYTES)
        || command.executor_mode != ModuleStaticDistributionExecutorMode::StaticNative
        || !phase_payload_valid
        || !valid_text(&command.agent_id, MAX_AGENT_ID_BYTES)
        || command.idempotency_key.is_nil()
    {
        return Err(ModuleStaticDistributionRolloutError::InvalidCommand);
    }
    Ok(())
}

fn validate_assignment_claim(
    command: &ModuleStaticDistributionAssignmentClaimCommand,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    if !valid_text(&command.node_id, MAX_NODE_ID_BYTES)
        || !valid_text(&command.agent_id, MAX_AGENT_ID_BYTES)
    {
        return Err(ModuleStaticDistributionRolloutError::InvalidCommand);
    }
    Ok(())
}

fn validate_assignment_heartbeat(
    command: &ModuleStaticDistributionAssignmentHeartbeatCommand,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    if command.claim_id.is_nil() || !valid_text(&command.agent_id, MAX_AGENT_ID_BYTES) {
        return Err(ModuleStaticDistributionRolloutError::InvalidCommand);
    }
    Ok(())
}

fn validate_report_identity(
    command: &ModuleStaticDistributionAssignmentReport,
    rollout: &ModuleStaticDistributionRollout,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    if command.distribution_release_id != rollout.distribution_release_id
        || command.distribution_release_revision != rollout.distribution_release_revision
        || command.composition_revision != rollout.composition_revision
        || command.composition_digest != rollout.composition_digest
        || command.bundle_root_digest != rollout.bundle_root_digest
        || command.role_set_digest != rollout.role_set_digest
        || command.policy_revision != rollout.policy_revision
        || command.executor_mode != rollout.executor_mode
    {
        return Err(ModuleStaticDistributionRolloutError::ObservationIdentityMismatch);
    }
    Ok(())
}

fn validate_transition(
    current: ModuleReconciliationPhase,
    next: ModuleReconciliationPhase,
    rollout_status: ModuleStaticDistributionRolloutStatus,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let valid = current.allows_standard_transition_to(next)
        || (current == ModuleReconciliationPhase::Healthy
            && next == ModuleReconciliationPhase::Active
            && rollout_status == ModuleStaticDistributionRolloutStatus::Activating)
        || (current == ModuleReconciliationPhase::Failed
            && next == ModuleReconciliationPhase::Prepared
            && rollout_status == ModuleStaticDistributionRolloutStatus::Degraded);
    if !valid {
        return Err(ModuleStaticDistributionRolloutError::InvalidTransition);
    }
    Ok(())
}

struct RolloutInsert<'a> {
    rollout_id: Uuid,
    predecessor_rollout_id: Option<Uuid>,
    rollout_revision: u64,
    release: &'a ModuleStaticDistributionRelease,
    topology: &'a ModuleStaticDistributionTopologySnapshot,
    command: &'a ModuleStaticDistributionRolloutRequest,
    transition_kind: ModuleStaticDistributionTransitionKind,
}

async fn insert_rollout(
    transaction: &DatabaseTransaction,
    insert: RolloutInsert<'_>,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_rollouts
                 (rollout_id, predecessor_rollout_id, distribution_release_id, transition_kind,
                  rollout_revision, distribution_release_revision, release_state_revision_at_request,
                  composition_revision,
                  composition_digest, bundle_reference, bundle_root_digest, role_set_digest, executor_mode,
                  topology_reference, topology_digest, policy_revision, target_assignment_count,
                  status, requested_by, requested_at, status_changed_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'static_native', {}, {}, {}, {},
                         'preparing', {}, {}, {})",
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
                now_expression(backend),
                now_expression(backend),
            ),
            vec![
                uuid_value(insert.rollout_id, backend),
                optional_uuid_value(insert.predecessor_rollout_id, backend),
                uuid_value(insert.release.distribution_release_id, backend),
                insert.transition_kind.as_str().into(),
                revision_value(insert.rollout_revision)?,
                revision_value(insert.release.release_revision)?,
                revision_value(insert.command.expected_release_state_revision)?,
                revision_value(insert.release.composition_revision)?,
                insert.release.composition_digest.clone().into(),
                insert.release.evidence.bundle_reference.clone().into(),
                insert.release.evidence.bundle_root_digest.clone().into(),
                insert.release.evidence.role_set_digest.clone().into(),
                insert.topology.topology_reference.clone().into(),
                insert.topology.topology_digest.clone().into(),
                insert.command.policy_revision.clone().into(),
                i64::try_from(insert.topology.assignments.len())
                    .map_err(|_| ModuleStaticDistributionRolloutError::InvalidTopology)?
                    .into(),
                uuid_value(insert.command.actor_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    Ok(())
}

async fn insert_rollout_assignments(
    transaction: &DatabaseTransaction,
    rollout_id: Uuid,
    assignments: &[ModuleStaticDistributionAssignment],
    predecessor: Option<&ModuleStaticDistributionRollout>,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    for (ordinal, assignment) in assignments.iter().enumerate() {
        let predecessor_artifact_digest = predecessor
            .and_then(|rollout| {
                rollout.assignments.iter().find(|candidate| {
                    candidate.node_id == assignment.node_id && candidate.role == assignment.role
                })
            })
            .map(|candidate| candidate.candidate_artifact_digest.clone());
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_static_distribution_rollout_assignments
                     (rollout_id, node_id, role, candidate_artifact_digest,
                      predecessor_artifact_digest, ordinal, observation_revision, phase)
                     VALUES ({}, {}, {}, {}, {}, {}, 0, 'pending')",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                ),
                vec![
                    uuid_value(rollout_id, backend),
                    assignment.node_id.clone().into(),
                    role_name(assignment.role).to_string().into(),
                    assignment.candidate_artifact_digest.clone().into(),
                    predecessor_artifact_digest.into(),
                    i64::try_from(ordinal)
                        .map_err(|_| ModuleStaticDistributionRolloutError::InvalidTopology)?
                        .into(),
                ],
            ))
            .await
            .map_err(store_error)?;
    }
    Ok(())
}

async fn update_rollout_assignment(
    transaction: &DatabaseTransaction,
    command: &ModuleStaticDistributionAssignmentReport,
    observation_revision: u64,
    report_digest: &str,
    now: &DateTime<Utc>,
) -> Result<(), ModuleStaticDistributionRolloutError> {
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
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollout_assignments
                 SET observation_revision = {}, phase = {}, observed_release_id = {},
                     observed_release_revision = {}, observed_composition_revision = {},
                     observed_composition_digest = {}, observed_bundle_root_digest = {},
                     observed_role_set_digest = {},
                     observed_policy_revision = {}, observed_executor_mode = 'static_native',
                     health_evidence_reference = {}, health_evidence_digest = {},
                     failure_code = {}, failure_detail = {}, reported_by = {},
                     last_report_digest = {}, first_reported_at = COALESCE(first_reported_at, {}),
                     last_reported_at = {}, active_claim_id = NULL, claimed_by_agent = NULL,
                     claim_expires_at = NULL
                 WHERE rollout_id = {} AND node_id = {} AND role = {} AND observation_revision = {}
                   AND active_claim_id = {} AND claimed_by_agent = {}
                   AND claim_expires_at >= {}",
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
            ),
            vec![
                revision_value(observation_revision)?,
                command.phase.as_str().into(),
                uuid_value(command.distribution_release_id, backend),
                revision_value(command.distribution_release_revision)?,
                revision_value(command.composition_revision)?,
                command.composition_digest.clone().into(),
                command.bundle_root_digest.clone().into(),
                command.role_set_digest.clone().into(),
                command.policy_revision.clone().into(),
                health_reference.into(),
                health_digest.into(),
                failure_code.into(),
                failure_detail.into(),
                command.agent_id.clone().into(),
                report_digest.to_owned().into(),
                now.to_owned().into(),
                now.to_owned().into(),
                uuid_value(command.rollout_id, backend),
                command.node_id.clone().into(),
                role_name(command.role).to_string().into(),
                revision_value_allow_zero(command.expected_observation_revision)?,
                uuid_value(command.claim_id, backend),
                command.agent_id.clone().into(),
                now.to_owned().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionRolloutError::ClaimConflict);
    }
    Ok(())
}

async fn validate_rollout_release_identity(
    transaction: &DatabaseTransaction,
    rollout: &ModuleStaticDistributionRollout,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let release_state = load_release_state(transaction, true)
        .await
        .map_err(release_error)?;
    let current_release = load_release_record(transaction, rollout.distribution_release_id, true)
        .await
        .map_err(release_error)?;
    let recovery_source_release =
        if rollout.transition_kind == ModuleStaticDistributionTransitionKind::Recovery {
            let predecessor_rollout_id = rollout
                .predecessor_rollout_id
                .ok_or(ModuleStaticDistributionRolloutError::RecoveryUnavailable)?;
            Some(
                load_rollout(transaction, predecessor_rollout_id, false)
                    .await?
                    .distribution_release_id,
            )
        } else {
            None
        };
    let valid = match rollout.transition_kind {
        ModuleStaticDistributionTransitionKind::Update => {
            current_release.status == ModuleStaticDistributionReleaseStatus::Admitted
                && release_state.revision == rollout.release_state_revision_at_request
        }
        ModuleStaticDistributionTransitionKind::Recovery => {
            let source_release_id = recovery_source_release
                .ok_or(ModuleStaticDistributionRolloutError::RecoveryUnavailable)?;
            let source_release = load_release_record(transaction, source_release_id, true)
                .await
                .map_err(release_error)?;
            release_state.revision == rollout.release_state_revision_at_request
                && ((current_release.status == ModuleStaticDistributionReleaseStatus::Superseded
                    && source_release.status == ModuleStaticDistributionReleaseStatus::Active
                    && release_state.active_release_id == Some(source_release_id))
                    || (current_release.status == ModuleStaticDistributionReleaseStatus::Active
                        && source_release.status
                            == ModuleStaticDistributionReleaseStatus::Admitted
                        && release_state.active_release_id
                            == Some(rollout.distribution_release_id)))
        }
    };
    if !valid || current_release.release_revision != rollout.distribution_release_revision {
        return Err(ModuleStaticDistributionRolloutError::StaleRollout);
    }
    Ok(())
}

fn rollout_work_identity(
    rollout: &ModuleStaticDistributionRollout,
) -> ModuleStaticDistributionRolloutWorkIdentity {
    ModuleStaticDistributionRolloutWorkIdentity {
        rollout_id: rollout.rollout_id,
        rollout_revision: rollout.rollout_revision,
        distribution_release_id: rollout.distribution_release_id,
        distribution_release_revision: rollout.distribution_release_revision,
        composition_revision: rollout.composition_revision,
        composition_digest: rollout.composition_digest.clone(),
        bundle_reference: rollout.bundle_reference.clone(),
        bundle_root_digest: rollout.bundle_root_digest.clone(),
        role_set_digest: rollout.role_set_digest.clone(),
        policy_revision: rollout.policy_revision.clone(),
        executor_mode: rollout.executor_mode,
    }
}

async fn update_rollout_status(
    transaction: &DatabaseTransaction,
    rollout_id: Uuid,
    expected_status: ModuleStaticDistributionRolloutStatus,
    next_status: ModuleStaticDistributionRolloutStatus,
    failure: Option<&ModuleReconciliationFailure>,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    let (converged_at, failed_at) = match next_status {
        ModuleStaticDistributionRolloutStatus::Converged => {
            (now_expression(backend).to_string(), "NULL".to_string())
        }
        ModuleStaticDistributionRolloutStatus::Failed
        | ModuleStaticDistributionRolloutStatus::Degraded => {
            ("NULL".to_string(), now_expression(backend).to_string())
        }
        _ => ("NULL".to_string(), "NULL".to_string()),
    };
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollouts
                 SET status = {}, status_changed_at = {}, converged_at = {converged_at},
                     failed_at = {failed_at}, failure_code = {}, failure_detail = {}
                 WHERE rollout_id = {} AND status = {}",
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
                uuid_value(rollout_id, backend),
                expected_status.as_str().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionRolloutError::StaleRollout);
    }
    Ok(())
}

async fn supersede_rollout(
    transaction: &DatabaseTransaction,
    rollout_id: Uuid,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollouts
                 SET status = 'superseded', status_changed_at = {}
                 WHERE rollout_id = {} AND status = 'converged'",
                now_expression(backend),
                placeholder(backend, 1),
            ),
            vec![uuid_value(rollout_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionRolloutError::StaleRollout);
    }
    Ok(())
}

async fn load_phase_counts(
    connection: &impl ConnectionTrait,
    rollout_id: Uuid,
) -> Result<PhaseCounts, ModuleStaticDistributionRolloutError> {
    let backend = connection.get_database_backend();
    let rows = connection
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT phase, COUNT(*) AS count
                 FROM module_static_distribution_rollout_assignments
                 WHERE rollout_id = {} GROUP BY phase",
                placeholder(backend, 1),
            ),
            vec![uuid_value(rollout_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    let mut counts = PhaseCounts::default();
    for row in rows {
        let phase: String = row.try_get("", "phase").map_err(store_error)?;
        let count: i64 = row.try_get("", "count").map_err(store_error)?;
        let count = usize::try_from(count).map_err(|_| {
            ModuleStaticDistributionRolloutError::Store(
                "static distribution assignment count is invalid".to_string(),
            )
        })?;
        match parse_reconciliation_phase(&phase)? {
            ModuleReconciliationPhase::Pending => counts.pending = count,
            ModuleReconciliationPhase::Prepared => counts.prepared = count,
            ModuleReconciliationPhase::Healthy => counts.healthy = count,
            ModuleReconciliationPhase::Active => counts.active = count,
            ModuleReconciliationPhase::Failed => counts.failed = count,
        }
    }
    Ok(counts)
}

async fn load_rollout_state<C: ConnectionTrait>(
    connection: &C,
    lock_row: bool,
) -> Result<ModuleDesiredObservedState, ModuleStaticDistributionRolloutError> {
    let backend = connection.get_database_backend();
    let lock = if lock_row && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT revision, desired_rollout_id, observed_rollout_id
                 FROM module_static_distribution_rollout_state
                 WHERE state_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![ROLLOUT_STATE_ID.into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or_else(|| {
            ModuleStaticDistributionRolloutError::Store(
                "static distribution rollout state is unavailable".to_string(),
            )
        })?;
    Ok(ModuleDesiredObservedState {
        revision: revision_from_row(&row, "revision", true)?,
        desired_id: optional_uuid_from_row(&row, "desired_rollout_id", backend)?,
        observed_id: optional_uuid_from_row(&row, "observed_rollout_id", backend)?,
    })
}

async fn advance_rollout_state(
    transaction: &DatabaseTransaction,
    expected_revision: u64,
    next_revision: u64,
    desired_rollout_id: Option<Uuid>,
    observed_rollout_id: Option<Uuid>,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollout_state
                 SET revision = {}, desired_rollout_id = {}, observed_rollout_id = {},
                     updated_at = {}
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
                optional_uuid_value(desired_rollout_id, backend),
                optional_uuid_value(observed_rollout_id, backend),
                ROLLOUT_STATE_ID.into(),
                revision_value_allow_zero(expected_revision)?,
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        let current = load_rollout_state(transaction, false).await?;
        return Err(ModuleStaticDistributionRolloutError::RevisionConflict {
            expected: expected_revision,
            current: current.revision,
        });
    }
    Ok(())
}

async fn load_rollout<C: ConnectionTrait>(
    connection: &C,
    rollout_id: Uuid,
    lock_row: bool,
) -> Result<ModuleStaticDistributionRollout, ModuleStaticDistributionRolloutError> {
    let backend = connection.get_database_backend();
    let lock = if lock_row && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT rollout_id, predecessor_rollout_id, distribution_release_id, transition_kind,
                        rollout_revision, distribution_release_revision, release_state_revision_at_request,
                        composition_revision,
                        composition_digest, bundle_reference, bundle_root_digest, role_set_digest,
                        executor_mode,
                        topology_reference, topology_digest, policy_revision, target_assignment_count,
                        status, requested_by, failure_code, failure_detail
                 FROM module_static_distribution_rollouts WHERE rollout_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(rollout_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionRolloutError::RolloutNotFound)?;
    let assignment_rows = connection
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT node_id, role, candidate_artifact_digest, predecessor_artifact_digest,
                        ordinal, observation_revision, phase,
                        health_evidence_reference, health_evidence_digest,
                        failure_code, failure_detail, reported_by, last_report_digest,
                        active_claim_id, claimed_by_agent, claim_expires_at
                 FROM module_static_distribution_rollout_assignments
                 WHERE rollout_id = {} ORDER BY ordinal",
                placeholder(backend, 1),
            ),
            vec![uuid_value(rollout_id, backend)],
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
        ModuleStaticDistributionRolloutError::Store(
            "static distribution target-assignment count is invalid".to_string(),
        )
    })?;
    if usize::from(target_assignment_count) != assignments.len() {
        return Err(ModuleStaticDistributionRolloutError::Store(
            "static distribution rollout topology is incomplete".to_string(),
        ));
    }
    let executor_mode: String = row.try_get("", "executor_mode").map_err(store_error)?;
    if executor_mode != "static_native" {
        return Err(ModuleStaticDistributionRolloutError::Store(
            "static distribution rollout executor mode is invalid".to_string(),
        ));
    }
    let failure_code: Option<String> = row.try_get("", "failure_code").map_err(store_error)?;
    let failure_detail: Option<String> = row.try_get("", "failure_detail").map_err(store_error)?;
    let failure = match (failure_code, failure_detail) {
        (Some(code), Some(detail)) => Some(ModuleReconciliationFailure { code, detail }),
        (None, None) => None,
        _ => {
            return Err(ModuleStaticDistributionRolloutError::Store(
                "static distribution rollout failure is incomplete".to_string(),
            ));
        }
    };
    Ok(ModuleStaticDistributionRollout {
        rollout_id: uuid_from_row(&row, "rollout_id", backend).map_err(store_error)?,
        predecessor_rollout_id: optional_uuid_from_row(&row, "predecessor_rollout_id", backend)?,
        distribution_release_id: uuid_from_row(&row, "distribution_release_id", backend)
            .map_err(store_error)?,
        transition_kind: ModuleStaticDistributionTransitionKind::parse(
            &row.try_get::<String>("", "transition_kind")
                .map_err(store_error)?,
        )?,
        rollout_revision: revision_from_row(&row, "rollout_revision", false)?,
        distribution_release_revision: revision_from_row(
            &row,
            "distribution_release_revision",
            false,
        )?,
        release_state_revision_at_request: revision_from_row(
            &row,
            "release_state_revision_at_request",
            false,
        )?,
        composition_revision: revision_from_row(&row, "composition_revision", false)?,
        composition_digest: row.try_get("", "composition_digest").map_err(store_error)?,
        bundle_reference: row.try_get("", "bundle_reference").map_err(store_error)?,
        bundle_root_digest: row.try_get("", "bundle_root_digest").map_err(store_error)?,
        role_set_digest: row.try_get("", "role_set_digest").map_err(store_error)?,
        executor_mode: ModuleStaticDistributionExecutorMode::StaticNative,
        topology_reference: row.try_get("", "topology_reference").map_err(store_error)?,
        topology_digest: row.try_get("", "topology_digest").map_err(store_error)?,
        policy_revision: row.try_get("", "policy_revision").map_err(store_error)?,
        target_assignment_count,
        status: ModuleStaticDistributionRolloutStatus::parse(
            &row.try_get::<String>("", "status").map_err(store_error)?,
        )?,
        requested_by: uuid_from_row(&row, "requested_by", backend).map_err(store_error)?,
        failure,
        assignments,
    })
}

async fn load_rollout_assignment<C: ConnectionTrait>(
    connection: &C,
    rollout_id: Uuid,
    node_id: &str,
    role: ModuleStaticDistributionRole,
    lock_row: bool,
) -> Result<ModuleStaticDistributionRolloutAssignment, ModuleStaticDistributionRolloutError> {
    let backend = connection.get_database_backend();
    let lock = if lock_row && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT node_id, role, candidate_artifact_digest, predecessor_artifact_digest,
                        ordinal, observation_revision, phase,
                        health_evidence_reference, health_evidence_digest,
                        failure_code, failure_detail, reported_by, last_report_digest,
                        active_claim_id, claimed_by_agent, claim_expires_at
                 FROM module_static_distribution_rollout_assignments
                 WHERE rollout_id = {} AND node_id = {} AND role = {}{lock}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(rollout_id, backend),
                node_id.to_owned().into(),
                role_name(role).to_string().into(),
            ],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionRolloutError::AssignmentNotFound)?;
    assignment_from_row(&row, backend)
}

/// Locks the next unclaimed (or expired) exact role assignment for one node.
/// Assignment order is deterministic so crash recovery does not depend on
/// process-local scheduling.
async fn load_next_assignment_for_node(
    transaction: &DatabaseTransaction,
    rollout_id: Uuid,
    node_id: &str,
    agent_id: &str,
    now: &DateTime<Utc>,
) -> Result<Option<ModuleStaticDistributionRolloutAssignment>, ModuleStaticDistributionRolloutError>
{
    let backend = transaction.get_database_backend();
    let lock = if backend == DbBackend::Postgres {
        " FOR UPDATE SKIP LOCKED"
    } else {
        ""
    };
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT node_id, role, candidate_artifact_digest, predecessor_artifact_digest,
                        ordinal, observation_revision, phase,
                        health_evidence_reference, health_evidence_digest,
                        failure_code, failure_detail, reported_by, last_report_digest,
                        active_claim_id, claimed_by_agent, claim_expires_at
                 FROM module_static_distribution_rollout_assignments
                 WHERE rollout_id = {} AND node_id = {}
                   AND phase IN ('pending', 'prepared', 'healthy')
                   AND (active_claim_id IS NULL OR claim_expires_at < {}
                        OR (claimed_by_agent = {} AND claim_expires_at >= {}))
                 ORDER BY CASE WHEN claimed_by_agent = {} AND claim_expires_at >= {} THEN 0 ELSE 1 END,
                          ordinal
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
                uuid_value(rollout_id, backend),
                node_id.to_owned().into(),
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

async fn claim_rollout_assignment(
    transaction: &DatabaseTransaction,
    rollout_id: Uuid,
    assignment: &ModuleStaticDistributionRolloutAssignment,
    claim_id: Uuid,
    agent_id: &str,
    now: &DateTime<Utc>,
    lease_expires_at: &DateTime<Utc>,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollout_assignments
                 SET active_claim_id = {}, claimed_by_agent = {}, claim_expires_at = {},
                     last_claimed_at = {}
                 WHERE rollout_id = {} AND node_id = {} AND role = {}
                   AND phase IN ('pending', 'prepared', 'healthy')
                   AND observation_revision = {}
                   AND (active_claim_id IS NULL OR claim_expires_at < {})",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                placeholder(backend, 9),
            ),
            vec![
                uuid_value(claim_id, backend),
                agent_id.to_owned().into(),
                lease_expires_at.to_owned().into(),
                now.to_owned().into(),
                uuid_value(rollout_id, backend),
                assignment.node_id.clone().into(),
                role_name(assignment.role).to_string().into(),
                revision_value_allow_zero(assignment.observation_revision)?,
                now.to_owned().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionRolloutError::ClaimConflict);
    }
    Ok(())
}

async fn heartbeat_rollout_assignment(
    transaction: &DatabaseTransaction,
    command: &ModuleStaticDistributionAssignmentHeartbeatCommand,
    now: &DateTime<Utc>,
    lease_expires_at: &DateTime<Utc>,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollout_assignments
                 SET claim_expires_at = {}, last_claimed_at = {}
                 WHERE active_claim_id = {} AND claimed_by_agent = {}
                   AND claim_expires_at >= {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
            ),
            vec![
                lease_expires_at.to_owned().into(),
                now.to_owned().into(),
                uuid_value(command.claim_id, backend),
                command.agent_id.clone().into(),
                now.to_owned().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionRolloutError::ClaimConflict);
    }
    Ok(())
}

fn assignment_from_row(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ModuleStaticDistributionRolloutAssignment, ModuleStaticDistributionRolloutError> {
    let ordinal: i64 = row.try_get("", "ordinal").map_err(store_error)?;
    let health_reference: Option<String> = row
        .try_get("", "health_evidence_reference")
        .map_err(store_error)?;
    let health_digest: Option<String> = row
        .try_get("", "health_evidence_digest")
        .map_err(store_error)?;
    let health_evidence = match (health_reference, health_digest) {
        (Some(reference), Some(digest)) => Some(ModuleReconciliationEvidence { reference, digest }),
        (None, None) => None,
        _ => {
            return Err(ModuleStaticDistributionRolloutError::Store(
                "static distribution assignment health evidence is incomplete".to_string(),
            ));
        }
    };
    let failure_code: Option<String> = row.try_get("", "failure_code").map_err(store_error)?;
    let failure_detail: Option<String> = row.try_get("", "failure_detail").map_err(store_error)?;
    let failure = match (failure_code, failure_detail) {
        (Some(code), Some(detail)) => Some(ModuleReconciliationFailure { code, detail }),
        (None, None) => None,
        _ => {
            return Err(ModuleStaticDistributionRolloutError::Store(
                "static distribution assignment failure is incomplete".to_string(),
            ));
        }
    };
    Ok(ModuleStaticDistributionRolloutAssignment {
        node_id: row.try_get("", "node_id").map_err(store_error)?,
        role: parse_role(&row.try_get::<String>("", "role").map_err(store_error)?)?,
        candidate_artifact_digest: row
            .try_get("", "candidate_artifact_digest")
            .map_err(store_error)?,
        predecessor_artifact_digest: row
            .try_get("", "predecessor_artifact_digest")
            .map_err(store_error)?,
        ordinal: u16::try_from(ordinal).map_err(|_| {
            ModuleStaticDistributionRolloutError::Store(
                "static distribution assignment ordinal is invalid".to_string(),
            )
        })?,
        observation_revision: revision_from_row(row, "observation_revision", true)?,
        phase: parse_reconciliation_phase(
            &row.try_get::<String>("", "phase").map_err(store_error)?,
        )?,
        health_evidence,
        failure,
        reported_by: row.try_get("", "reported_by").map_err(store_error)?,
        last_report_digest: row.try_get("", "last_report_digest").map_err(store_error)?,
        active_claim_id: optional_uuid_from_row(row, "active_claim_id", backend)?,
        claimed_by_agent: row.try_get("", "claimed_by_agent").map_err(store_error)?,
        claim_expires_at: row
            .try_get::<Option<DateTime<Utc>>>("", "claim_expires_at")
            .map_err(store_error)?,
    })
}

async fn reserve_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    operation_kind: &str,
    request_digest: &str,
    principal_id: &str,
) -> Result<Option<OperationRecord>, ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    let inserted = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_rollout_operations
                 (idempotency_key, operation_kind, request_digest, principal_id, created_at)
                 VALUES ({}, {}, {}, {}, {}) ON CONFLICT (idempotency_key) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                now_expression(backend),
            ),
            vec![
                uuid_value(idempotency_key, backend),
                operation_kind.to_owned().into(),
                request_digest.to_owned().into(),
                principal_id.to_owned().into(),
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
    )
    .await
}

async fn load_operation<C: ConnectionTrait>(
    connection: &C,
    idempotency_key: Uuid,
    operation_kind: &str,
    request_digest: &str,
    principal_id: &str,
) -> Result<Option<OperationRecord>, ModuleStaticDistributionRolloutError> {
    let backend = connection.get_database_backend();
    let Some(row) = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_kind, request_digest, principal_id, rollout_id,
                        rollout_revision, rollout_state_revision, rollout_status,
                        node_id, role, observation_revision, assignment_phase,
                        CASE WHEN completed_at IS NULL THEN 0 ELSE 1 END AS completed
                 FROM module_static_distribution_rollout_operations
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
        rollout_id: optional_uuid_from_row(&row, "rollout_id", backend)?,
        rollout_revision: optional_revision_from_row(&row, "rollout_revision")?,
        rollout_state_revision: optional_revision_from_row(&row, "rollout_state_revision")?,
        rollout_status: row
            .try_get::<Option<String>>("", "rollout_status")
            .map_err(store_error)?
            .as_deref()
            .map(ModuleStaticDistributionRolloutStatus::parse)
            .transpose()?,
        node_id: row.try_get("", "node_id").map_err(store_error)?,
        role: row
            .try_get::<Option<String>>("", "role")
            .map_err(store_error)?
            .as_deref()
            .map(parse_role)
            .transpose()?,
        observation_revision: optional_revision_from_row(&row, "observation_revision")?,
        assignment_phase: row
            .try_get::<Option<String>>("", "assignment_phase")
            .map_err(store_error)?
            .as_deref()
            .map(parse_reconciliation_phase)
            .transpose()?,
        completed: row.try_get::<i64>("", "completed").map_err(store_error)? == 1,
    };
    if record.operation_kind != operation_kind
        || record.request_digest != request_digest
        || record.principal_id != principal_id
    {
        return Err(ModuleStaticDistributionRolloutError::IdempotencyConflict);
    }
    Ok(Some(record))
}

async fn complete_request_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    receipt: &ModuleStaticDistributionRolloutReceipt,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    complete_operation(
        transaction,
        idempotency_key,
        receipt.rollout_id,
        receipt.rollout_revision,
        receipt.rollout_state_revision,
        receipt.status,
        None,
        None,
        None,
        None,
    )
    .await
}

async fn complete_report_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    receipt: &ModuleStaticDistributionAssignmentReportReceipt,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    complete_operation(
        transaction,
        idempotency_key,
        receipt.rollout_id,
        receipt.rollout_revision,
        receipt.rollout_state_revision,
        receipt.rollout_status,
        Some(&receipt.node_id),
        Some(receipt.role),
        Some(receipt.observation_revision),
        Some(receipt.phase),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn complete_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    rollout_id: Uuid,
    rollout_revision: u64,
    rollout_state_revision: u64,
    rollout_status: ModuleStaticDistributionRolloutStatus,
    node_id: Option<&str>,
    role: Option<ModuleStaticDistributionRole>,
    observation_revision: Option<u64>,
    assignment_phase: Option<ModuleReconciliationPhase>,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollout_operations
                 SET rollout_id = {}, rollout_revision = {}, rollout_state_revision = {},
                     rollout_status = {}, node_id = {}, role = {}, observation_revision = {},
                     assignment_phase = {}, completed_at = {}
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
                uuid_value(rollout_id, backend),
                revision_value(rollout_revision)?,
                revision_value(rollout_state_revision)?,
                rollout_status.as_str().into(),
                node_id.map(str::to_owned).into(),
                role.map(|value| role_name(value).to_string()).into(),
                optional_revision_value(observation_revision)?,
                assignment_phase
                    .map(|phase| phase.as_str().to_string())
                    .into(),
                uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModuleStaticDistributionRolloutError::IdempotencyConflict);
    }
    Ok(())
}

fn replay_request(
    operation: &OperationRecord,
    operation_kind: &str,
) -> Result<ModuleStaticDistributionRolloutReceipt, ModuleStaticDistributionRolloutError> {
    if !operation.completed
        || operation.operation_kind != operation_kind
        || operation.node_id.is_some()
        || operation.role.is_some()
        || operation.observation_revision.is_some()
        || operation.assignment_phase.is_some()
    {
        return Err(ModuleStaticDistributionRolloutError::IdempotencyConflict);
    }
    Ok(ModuleStaticDistributionRolloutReceipt {
        rollout_id: operation
            .rollout_id
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        rollout_revision: operation
            .rollout_revision
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        rollout_state_revision: operation
            .rollout_state_revision
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        status: operation
            .rollout_status
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        created: false,
    })
}

fn replay_report(
    operation: &OperationRecord,
) -> Result<ModuleStaticDistributionAssignmentReportReceipt, ModuleStaticDistributionRolloutError> {
    if !operation.completed || operation.operation_kind != "report" {
        return Err(ModuleStaticDistributionRolloutError::IdempotencyConflict);
    }
    Ok(ModuleStaticDistributionAssignmentReportReceipt {
        rollout_id: operation
            .rollout_id
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        rollout_revision: operation
            .rollout_revision
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        rollout_state_revision: operation
            .rollout_state_revision
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        rollout_status: operation
            .rollout_status
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        node_id: operation
            .node_id
            .clone()
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        role: operation
            .role
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        observation_revision: operation
            .observation_revision
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        phase: operation
            .assignment_phase
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
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
        (DbBackend::Postgres, value) => sea_orm::Value::Uuid(value.map(Box::new)),
        (_, Some(value)) => value.to_string().into(),
        (_, None) => sea_orm::Value::String(None),
    }
}

fn optional_uuid_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Option<Uuid>, ModuleStaticDistributionRolloutError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(store_error),
        _ => row
            .try_get::<Option<String>>("", column)
            .map_err(store_error)?
            .map(|value| Uuid::parse_str(&value).map_err(store_error))
            .transpose(),
    }
}

fn revision_value(value: u64) -> Result<sea_orm::Value, ModuleStaticDistributionRolloutError> {
    i64::try_from(value)
        .map(Into::into)
        .map_err(|_| ModuleStaticDistributionRolloutError::RevisionOverflow)
}

fn revision_value_allow_zero(
    value: u64,
) -> Result<sea_orm::Value, ModuleStaticDistributionRolloutError> {
    revision_value(value)
}

fn optional_revision_value(
    value: Option<u64>,
) -> Result<sea_orm::Value, ModuleStaticDistributionRolloutError> {
    match value {
        Some(value) => revision_value(value),
        None => Ok(sea_orm::Value::BigInt(None)),
    }
}

fn revision_from_row(
    row: &QueryResult,
    column: &str,
    allow_zero: bool,
) -> Result<u64, ModuleStaticDistributionRolloutError> {
    let value: i64 = row.try_get("", column).map_err(store_error)?;
    if value < 0 || (!allow_zero && value == 0) {
        return Err(ModuleStaticDistributionRolloutError::Store(format!(
            "static distribution rollout revision `{column}` is invalid"
        )));
    }
    u64::try_from(value).map_err(|_| ModuleStaticDistributionRolloutError::RevisionOverflow)
}

fn optional_revision_from_row(
    row: &QueryResult,
    column: &str,
) -> Result<Option<u64>, ModuleStaticDistributionRolloutError> {
    row.try_get::<Option<i64>>("", column)
        .map_err(store_error)?
        .map(|value| {
            if value <= 0 {
                Err(ModuleStaticDistributionRolloutError::Store(format!(
                    "static distribution rollout revision `{column}` is invalid"
                )))
            } else {
                u64::try_from(value)
                    .map_err(|_| ModuleStaticDistributionRolloutError::RevisionOverflow)
            }
        })
        .transpose()
}

fn promotion_error(error: impl std::fmt::Display) -> ModuleStaticDistributionRolloutError {
    ModuleStaticDistributionRolloutError::Store(error.to_string())
}

fn release_error(error: impl std::fmt::Display) -> ModuleStaticDistributionRolloutError {
    ModuleStaticDistributionRolloutError::Store(error.to_string())
}

fn store_error(error: impl std::fmt::Display) -> ModuleStaticDistributionRolloutError {
    ModuleStaticDistributionRolloutError::Store(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ModuleReconciliationPhase, ModuleStaticDistributionAssignment,
        ModuleStaticDistributionAssignmentClaimCommand,
        ModuleStaticDistributionAssignmentHeartbeatCommand,
        ModuleStaticDistributionAssignmentReport, ModuleStaticDistributionRolloutError,
        module_static_distribution_topology_digest, validate_assignment_claim,
        validate_assignment_heartbeat, validate_report,
    };
    use crate::ModuleStaticDistributionRole;
    use uuid::Uuid;

    #[test]
    fn topology_digest_binds_each_role_digest_on_the_same_node() {
        let node_id = "portable-instance-a".to_string();
        let first = vec![
            ModuleStaticDistributionAssignment {
                node_id: node_id.clone(),
                role: ModuleStaticDistributionRole::Api,
                candidate_artifact_digest: format!("sha256:{}", "a".repeat(64)),
            },
            ModuleStaticDistributionAssignment {
                node_id,
                role: ModuleStaticDistributionRole::Worker,
                candidate_artifact_digest: format!("sha256:{}", "b".repeat(64)),
            },
        ];
        let mut changed = first.clone();
        changed[1].candidate_artifact_digest = format!("sha256:{}", "c".repeat(64));

        let first_digest =
            module_static_distribution_topology_digest("topology:portable-a", &first).unwrap();
        let changed_digest =
            module_static_distribution_topology_digest("topology:portable-a", &changed).unwrap();

        assert_ne!(first_digest, changed_digest);
    }

    #[test]
    fn node_agent_contract_rejects_unidentified_claims_and_reports() {
        assert!(matches!(
            validate_assignment_claim(&ModuleStaticDistributionAssignmentClaimCommand {
                node_id: "node-a".to_string(),
                agent_id: " ".to_string(),
            }),
            Err(ModuleStaticDistributionRolloutError::InvalidCommand)
        ));
        assert!(matches!(
            validate_assignment_heartbeat(&ModuleStaticDistributionAssignmentHeartbeatCommand {
                claim_id: Uuid::nil(),
                agent_id: "agent-a".to_string(),
            }),
            Err(ModuleStaticDistributionRolloutError::InvalidCommand)
        ));

        let report = ModuleStaticDistributionAssignmentReport {
            claim_id: Uuid::nil(),
            rollout_id: Uuid::new_v4(),
            node_id: "node-a".to_string(),
            role: ModuleStaticDistributionRole::Api,
            candidate_artifact_digest: digest('a'),
            expected_observation_revision: 0,
            phase: ModuleReconciliationPhase::Prepared,
            distribution_release_id: Uuid::new_v4(),
            distribution_release_revision: 1,
            composition_revision: 1,
            composition_digest: digest('b'),
            bundle_root_digest: digest('c'),
            role_set_digest: digest('d'),
            policy_revision: "policy-a".to_string(),
            executor_mode: crate::ModuleStaticDistributionExecutorMode::StaticNative,
            health_evidence: None,
            failure: None,
            agent_id: "agent-a".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        assert!(matches!(
            validate_report(&report),
            Err(ModuleStaticDistributionRolloutError::InvalidCommand)
        ));
    }
    fn digest(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }
}
