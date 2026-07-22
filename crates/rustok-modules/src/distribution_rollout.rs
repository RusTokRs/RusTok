//! Durable desired/observed reconciliation for verified native distributions.
//!
//! The control plane records exact topology and node evidence. Deployment
//! agents perform the actual binary rollout outside this crate.

use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::DomainEvent;

use crate::{
    data::{now_expression, placeholder, uuid_from_row, uuid_value},
    distribution_release::{load_release_record, load_release_state},
    promotion::{digest_json, valid_digest, valid_reference},
    ControlPlaneInfrastructure, ModuleStaticDistributionExecutorMode,
    ModuleStaticDistributionRelease, ModuleStaticDistributionReleaseStatus,
};

const ROLLOUT_STATE_ID: &str = "current";
const TOPOLOGY_DIGEST_CONTRACT: &str = "rustok.static_distribution.topology";
const MAX_TARGET_NODES: usize = 1024;
const MAX_NODE_ID_BYTES: usize = 128;
const MAX_REFERENCE_BYTES: usize = 512;
const MAX_POLICY_REVISION_BYTES: usize = 128;
const MAX_FAILURE_CODE_BYTES: usize = 128;
const MAX_FAILURE_DETAIL_BYTES: usize = 2_000;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStaticDistributionNodePhase {
    Pending,
    Prepared,
    Healthy,
    Active,
    Failed,
}

impl ModuleStaticDistributionNodePhase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Prepared => "prepared",
            Self::Healthy => "healthy",
            Self::Active => "active",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, ModuleStaticDistributionRolloutError> {
        match value {
            "pending" => Ok(Self::Pending),
            "prepared" => Ok(Self::Prepared),
            "healthy" => Ok(Self::Healthy),
            "active" => Ok(Self::Active),
            "failed" => Ok(Self::Failed),
            _ => Err(ModuleStaticDistributionRolloutError::Store(
                "static distribution node phase is invalid".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionTopologySnapshot {
    pub topology_reference: String,
    pub topology_digest: String,
    pub node_ids: Vec<String>,
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
    pub expected_release_revision: u64,
    pub expected_rollout_state_revision: u64,
    pub policy_revision: String,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionHealthEvidence {
    pub reference: String,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionNodeFailure {
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleStaticDistributionNodeReport {
    pub rollout_id: Uuid,
    pub node_id: String,
    pub expected_observation_revision: u64,
    pub phase: ModuleStaticDistributionNodePhase,
    pub distribution_release_id: Uuid,
    pub distribution_release_revision: u64,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub artifact_digest: String,
    pub policy_revision: String,
    pub executor_mode: ModuleStaticDistributionExecutorMode,
    pub health_evidence: Option<ModuleStaticDistributionHealthEvidence>,
    pub failure: Option<ModuleStaticDistributionNodeFailure>,
    pub reporter_id: String,
    pub idempotency_key: Uuid,
}

#[async_trait]
pub trait ModuleStaticDistributionRolloutAuthorizer: Send + Sync {
    async fn authorize_request(
        &self,
        command: &ModuleStaticDistributionRolloutRequest,
    ) -> Result<(), ModuleStaticDistributionRolloutError>;

    async fn authorize_report(
        &self,
        command: &ModuleStaticDistributionNodeReport,
    ) -> Result<(), ModuleStaticDistributionRolloutError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRolloutState {
    pub revision: u64,
    pub desired_rollout_id: Option<Uuid>,
    pub observed_rollout_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRolloutNode {
    pub node_id: String,
    pub ordinal: u16,
    pub observation_revision: u64,
    pub phase: ModuleStaticDistributionNodePhase,
    pub health_evidence: Option<ModuleStaticDistributionHealthEvidence>,
    pub failure: Option<ModuleStaticDistributionNodeFailure>,
    pub reported_by: Option<String>,
    pub last_report_digest: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStaticDistributionRollout {
    pub rollout_id: Uuid,
    pub predecessor_rollout_id: Option<Uuid>,
    pub distribution_release_id: Uuid,
    pub rollout_revision: u64,
    pub distribution_release_revision: u64,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub artifact_reference: String,
    pub artifact_digest: String,
    pub executor_mode: ModuleStaticDistributionExecutorMode,
    pub topology_reference: String,
    pub topology_digest: String,
    pub policy_revision: String,
    pub target_node_count: u16,
    pub status: ModuleStaticDistributionRolloutStatus,
    pub requested_by: Uuid,
    pub failure: Option<ModuleStaticDistributionNodeFailure>,
    pub nodes: Vec<ModuleStaticDistributionRolloutNode>,
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
pub struct ModuleStaticDistributionNodeReportReceipt {
    pub rollout_id: Uuid,
    pub rollout_revision: u64,
    pub rollout_state_revision: u64,
    pub rollout_status: ModuleStaticDistributionRolloutStatus,
    pub node_id: String,
    pub observation_revision: u64,
    pub phase: ModuleStaticDistributionNodePhase,
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
            return replay_request(&operation);
        }

        let release_state = load_release_state(&self.db, false)
            .await
            .map_err(release_error)?;
        if release_state.revision != command.expected_release_revision
            || release_state.active_release_id != Some(command.distribution_release_id)
        {
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
        validate_topology(&topology)?;

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
            return replay_request(&operation);
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
        let predecessor = match state.desired_rollout_id {
            Some(rollout_id) => Some(load_rollout(&transaction, rollout_id, true).await?),
            None => None,
        };
        if predecessor.as_ref().is_some_and(|rollout| {
            matches!(
                rollout.status,
                ModuleStaticDistributionRolloutStatus::Preparing
                    | ModuleStaticDistributionRolloutStatus::Activating
            )
        }) {
            return Err(ModuleStaticDistributionRolloutError::RolloutInProgress);
        }
        if predecessor.as_ref().is_some_and(|rollout| {
            rollout.status == ModuleStaticDistributionRolloutStatus::Converged
                && rollout.distribution_release_id == release.distribution_release_id
                && rollout.topology_digest == topology.topology_digest
                && rollout.policy_revision == command.policy_revision
        }) {
            return Err(ModuleStaticDistributionRolloutError::NoRolloutChange);
        }
        let rollout_revision = predecessor.as_ref().map_or(Ok(1_u64), |rollout| {
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
            rollout_id,
            predecessor.as_ref().map(|rollout| rollout.rollout_id),
            rollout_revision,
            &release,
            &topology,
            &command,
        )
        .await?;
        insert_rollout_nodes(&transaction, rollout_id, &topology.node_ids).await?;
        advance_rollout_state(
            &transaction,
            state.revision,
            rollout_state_revision,
            Some(rollout_id),
            state.observed_rollout_id,
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
                        artifact_digest: release.evidence.artifact_digest,
                        topology_digest: topology.topology_digest,
                        policy_revision: command.policy_revision,
                        target_nodes: u32::try_from(topology.node_ids.len())
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

    pub async fn report(
        &self,
        command: ModuleStaticDistributionNodeReport,
    ) -> Result<ModuleStaticDistributionNodeReportReceipt, ModuleStaticDistributionRolloutError>
    {
        validate_report(&command)?;
        self.authorizer.authorize_report(&command).await?;
        let request_digest = digest_json(&command).map_err(promotion_error)?;
        if let Some(operation) = load_operation(
            &self.db,
            command.idempotency_key,
            "report",
            &request_digest,
            &command.reporter_id,
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
            &command.reporter_id,
        )
        .await?
        {
            transaction.commit().await.map_err(store_error)?;
            return replay_report(&operation);
        }
        let state = load_rollout_state(&transaction, true).await?;
        if state.desired_rollout_id != Some(command.rollout_id) {
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
        let current_release =
            load_release_record(&transaction, rollout.distribution_release_id, true)
                .await
                .map_err(release_error)?;
        if current_release.status != ModuleStaticDistributionReleaseStatus::Active
            || current_release.release_revision != rollout.distribution_release_revision
        {
            return Err(ModuleStaticDistributionRolloutError::StaleRollout);
        }
        validate_report_identity(&command, &rollout)?;
        let node =
            load_rollout_node(&transaction, command.rollout_id, &command.node_id, true).await?;
        if node.observation_revision != command.expected_observation_revision {
            return Err(
                ModuleStaticDistributionRolloutError::ObservationRevisionConflict {
                    expected: command.expected_observation_revision,
                    current: node.observation_revision,
                },
            );
        }
        validate_transition(node.phase, command.phase, rollout.status)?;
        let observation_revision = node
            .observation_revision
            .checked_add(1)
            .ok_or(ModuleStaticDistributionRolloutError::RevisionOverflow)?;
        update_rollout_node(
            &transaction,
            &command,
            observation_revision,
            &request_digest,
        )
        .await?;

        let phase_counts = load_phase_counts(&transaction, command.rollout_id).await?;
        let target_nodes = usize::from(rollout.target_node_count);
        let mut next_status = rollout.status;
        let mut state_revision = state.revision;
        let mut observed_rollout_id = state.observed_rollout_id;
        let mut status_failure = None;
        if command.phase == ModuleStaticDistributionNodePhase::Failed {
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
        ) && phase_counts.ready_for_activation() == target_nodes
        {
            next_status = ModuleStaticDistributionRolloutStatus::Activating;
        } else if rollout.status == ModuleStaticDistributionRolloutStatus::Activating
            && phase_counts.active == target_nodes
        {
            next_status = ModuleStaticDistributionRolloutStatus::Converged;
            observed_rollout_id = Some(rollout.rollout_id);
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
            if next_status == ModuleStaticDistributionRolloutStatus::Converged {
                if let Some(previous_observed) = state
                    .observed_rollout_id
                    .filter(|rollout_id| *rollout_id != rollout.rollout_id)
                {
                    supersede_rollout(&transaction, previous_observed).await?;
                }
            }
            advance_rollout_state(
                &transaction,
                state.revision,
                state_revision,
                state.desired_rollout_id,
                observed_rollout_id,
            )
            .await?;
        }

        let receipt = ModuleStaticDistributionNodeReportReceipt {
            rollout_id: rollout.rollout_id,
            rollout_revision: rollout.rollout_revision,
            rollout_state_revision: state_revision,
            rollout_status: next_status,
            node_id: command.node_id.clone(),
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
                    DomainEvent::ModuleStaticDistributionNodeObserved {
                        rollout_id: rollout.rollout_id,
                        node_id: command.node_id,
                        reporter_id: command.reporter_id,
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
    ) -> Result<ModuleStaticDistributionRolloutState, ModuleStaticDistributionRolloutError> {
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
    for rollout_id in [state.desired_rollout_id, state.observed_rollout_id]
        .into_iter()
        .flatten()
    {
        if !rollout_ids.contains(&rollout_id) {
            rollout_ids.push(rollout_id);
        }
    }
    let mut next_state_revision = state.revision;
    let mut desired_rollout_id = state.desired_rollout_id;
    let mut observed_rollout_id = state.observed_rollout_id;
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
        let failure = ModuleStaticDistributionNodeFailure {
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
    observation_revision: Option<u64>,
    node_phase: Option<ModuleStaticDistributionNodePhase>,
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
        "static distribution node observation revision conflict: expected {expected}, current {current}"
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
    #[error("static distribution rollout node was not found")]
    NodeNotFound,
    #[error("static distribution node report identity does not match the desired rollout")]
    ObservationIdentityMismatch,
    #[error("static distribution node phase transition is invalid")]
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
        || command.expected_release_revision == 0
        || !valid_text(&command.policy_revision, MAX_POLICY_REVISION_BYTES)
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
        || release.release_revision != command.expected_release_revision
        || release.status != ModuleStaticDistributionReleaseStatus::Active
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
    node_ids: &'a [String],
}

pub fn module_static_distribution_topology_digest(
    topology_reference: &str,
    node_ids: &[String],
) -> Result<String, ModuleStaticDistributionRolloutError> {
    digest_json(&TopologyDigestInput {
        contract: TOPOLOGY_DIGEST_CONTRACT,
        topology_reference,
        node_ids,
    })
    .map_err(promotion_error)
}

fn validate_topology(
    topology: &ModuleStaticDistributionTopologySnapshot,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    if !valid_reference(&topology.topology_reference)
        || topology.topology_reference.len() > MAX_REFERENCE_BYTES
        || !valid_digest(&topology.topology_digest)
        || topology.node_ids.is_empty()
        || topology.node_ids.len() > MAX_TARGET_NODES
        || topology
            .node_ids
            .iter()
            .any(|node_id| !valid_text(node_id, MAX_NODE_ID_BYTES))
        || topology
            .node_ids
            .windows(2)
            .any(|nodes| nodes[0] >= nodes[1])
    {
        return Err(ModuleStaticDistributionRolloutError::InvalidTopology);
    }
    let expected_digest = module_static_distribution_topology_digest(
        &topology.topology_reference,
        &topology.node_ids,
    )?;
    if topology.topology_digest != expected_digest {
        return Err(ModuleStaticDistributionRolloutError::InvalidTopology);
    }
    Ok(())
}

fn validate_report(
    command: &ModuleStaticDistributionNodeReport,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let health_valid = command.health_evidence.as_ref().is_some_and(|evidence| {
        valid_reference(&evidence.reference)
            && evidence.reference.len() <= MAX_REFERENCE_BYTES
            && valid_digest(&evidence.digest)
    });
    let failure_valid = command.failure.as_ref().is_some_and(|failure| {
        valid_text(&failure.code, MAX_FAILURE_CODE_BYTES)
            && valid_text(&failure.detail, MAX_FAILURE_DETAIL_BYTES)
    });
    let phase_payload_valid = match command.phase {
        ModuleStaticDistributionNodePhase::Prepared => {
            command.health_evidence.is_none() && command.failure.is_none()
        }
        ModuleStaticDistributionNodePhase::Healthy | ModuleStaticDistributionNodePhase::Active => {
            health_valid && command.failure.is_none()
        }
        ModuleStaticDistributionNodePhase::Failed => {
            command.health_evidence.is_none() && failure_valid
        }
        ModuleStaticDistributionNodePhase::Pending => false,
    };
    if command.rollout_id.is_nil()
        || !valid_text(&command.node_id, MAX_NODE_ID_BYTES)
        || command.distribution_release_id.is_nil()
        || command.distribution_release_revision == 0
        || command.composition_revision == 0
        || !valid_digest(&command.composition_digest)
        || !valid_digest(&command.artifact_digest)
        || !valid_text(&command.policy_revision, MAX_POLICY_REVISION_BYTES)
        || command.executor_mode != ModuleStaticDistributionExecutorMode::StaticNative
        || !phase_payload_valid
        || !valid_text(&command.reporter_id, MAX_NODE_ID_BYTES)
        || command.idempotency_key.is_nil()
    {
        return Err(ModuleStaticDistributionRolloutError::InvalidCommand);
    }
    Ok(())
}

fn validate_report_identity(
    command: &ModuleStaticDistributionNodeReport,
    rollout: &ModuleStaticDistributionRollout,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    if command.distribution_release_id != rollout.distribution_release_id
        || command.distribution_release_revision != rollout.distribution_release_revision
        || command.composition_revision != rollout.composition_revision
        || command.composition_digest != rollout.composition_digest
        || command.artifact_digest != rollout.artifact_digest
        || command.policy_revision != rollout.policy_revision
        || command.executor_mode != rollout.executor_mode
    {
        return Err(ModuleStaticDistributionRolloutError::ObservationIdentityMismatch);
    }
    Ok(())
}

fn validate_transition(
    current: ModuleStaticDistributionNodePhase,
    next: ModuleStaticDistributionNodePhase,
    rollout_status: ModuleStaticDistributionRolloutStatus,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let valid = matches!(
        (current, next),
        (
            ModuleStaticDistributionNodePhase::Pending,
            ModuleStaticDistributionNodePhase::Prepared | ModuleStaticDistributionNodePhase::Failed
        ) | (
            ModuleStaticDistributionNodePhase::Prepared,
            ModuleStaticDistributionNodePhase::Healthy | ModuleStaticDistributionNodePhase::Failed
        ) | (
            ModuleStaticDistributionNodePhase::Healthy,
            ModuleStaticDistributionNodePhase::Failed
        ) | (
            ModuleStaticDistributionNodePhase::Active,
            ModuleStaticDistributionNodePhase::Failed
        )
    ) || (current == ModuleStaticDistributionNodePhase::Healthy
        && next == ModuleStaticDistributionNodePhase::Active
        && rollout_status == ModuleStaticDistributionRolloutStatus::Activating)
        || (current == ModuleStaticDistributionNodePhase::Failed
            && next == ModuleStaticDistributionNodePhase::Prepared
            && rollout_status == ModuleStaticDistributionRolloutStatus::Degraded);
    if !valid {
        return Err(ModuleStaticDistributionRolloutError::InvalidTransition);
    }
    Ok(())
}

async fn insert_rollout(
    transaction: &DatabaseTransaction,
    rollout_id: Uuid,
    predecessor_rollout_id: Option<Uuid>,
    rollout_revision: u64,
    release: &ModuleStaticDistributionRelease,
    topology: &ModuleStaticDistributionTopologySnapshot,
    command: &ModuleStaticDistributionRolloutRequest,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_static_distribution_rollouts
                 (rollout_id, predecessor_rollout_id, distribution_release_id,
                  rollout_revision, distribution_release_revision, composition_revision,
                  composition_digest, artifact_reference, artifact_digest, executor_mode,
                  topology_reference, topology_digest, policy_revision, target_node_count,
                  status, requested_by, requested_at, status_changed_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, 'static_native', {}, {}, {}, {},
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
                now_expression(backend),
                now_expression(backend),
            ),
            vec![
                uuid_value(rollout_id, backend),
                optional_uuid_value(predecessor_rollout_id, backend),
                uuid_value(release.distribution_release_id, backend),
                revision_value(rollout_revision)?,
                revision_value(release.release_revision)?,
                revision_value(release.composition_revision)?,
                release.composition_digest.clone().into(),
                release.evidence.artifact_reference.clone().into(),
                release.evidence.artifact_digest.clone().into(),
                topology.topology_reference.clone().into(),
                topology.topology_digest.clone().into(),
                command.policy_revision.clone().into(),
                i64::try_from(topology.node_ids.len())
                    .map_err(|_| ModuleStaticDistributionRolloutError::InvalidTopology)?
                    .into(),
                uuid_value(command.actor_id, backend),
            ],
        ))
        .await
        .map_err(store_error)?;
    Ok(())
}

async fn insert_rollout_nodes(
    transaction: &DatabaseTransaction,
    rollout_id: Uuid,
    node_ids: &[String],
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    for (ordinal, node_id) in node_ids.iter().enumerate() {
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_static_distribution_rollout_nodes
                     (rollout_id, node_id, ordinal, observation_revision, phase)
                     VALUES ({}, {}, {}, 0, 'pending')",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(rollout_id, backend),
                    node_id.clone().into(),
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

async fn update_rollout_node(
    transaction: &DatabaseTransaction,
    command: &ModuleStaticDistributionNodeReport,
    observation_revision: u64,
    report_digest: &str,
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
                "UPDATE module_static_distribution_rollout_nodes
                 SET observation_revision = {}, phase = {}, observed_release_id = {},
                     observed_release_revision = {}, observed_composition_revision = {},
                     observed_composition_digest = {}, observed_artifact_digest = {},
                     observed_policy_revision = {}, observed_executor_mode = 'static_native',
                     health_evidence_reference = {}, health_evidence_digest = {},
                     failure_code = {}, failure_detail = {}, reported_by = {},
                     last_report_digest = {}, first_reported_at = COALESCE(first_reported_at, {}),
                     last_reported_at = {}
                 WHERE rollout_id = {} AND node_id = {} AND observation_revision = {}",
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
                now_expression(backend),
                now_expression(backend),
                placeholder(backend, 15),
                placeholder(backend, 16),
                placeholder(backend, 17),
            ),
            vec![
                revision_value(observation_revision)?,
                command.phase.as_str().into(),
                uuid_value(command.distribution_release_id, backend),
                revision_value(command.distribution_release_revision)?,
                revision_value(command.composition_revision)?,
                command.composition_digest.clone().into(),
                command.artifact_digest.clone().into(),
                command.policy_revision.clone().into(),
                health_reference.into(),
                health_digest.into(),
                failure_code.into(),
                failure_detail.into(),
                command.reporter_id.clone().into(),
                report_digest.to_owned().into(),
                uuid_value(command.rollout_id, backend),
                command.node_id.clone().into(),
                revision_value_allow_zero(command.expected_observation_revision)?,
            ],
        ))
        .await
        .map_err(store_error)?;
    if updated.rows_affected() != 1 {
        return Err(
            ModuleStaticDistributionRolloutError::ObservationRevisionConflict {
                expected: command.expected_observation_revision,
                current: observation_revision.saturating_sub(1),
            },
        );
    }
    Ok(())
}

async fn update_rollout_status(
    transaction: &DatabaseTransaction,
    rollout_id: Uuid,
    expected_status: ModuleStaticDistributionRolloutStatus,
    next_status: ModuleStaticDistributionRolloutStatus,
    failure: Option<&ModuleStaticDistributionNodeFailure>,
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
                 FROM module_static_distribution_rollout_nodes
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
                "static distribution node count is invalid".to_string(),
            )
        })?;
        match ModuleStaticDistributionNodePhase::parse(&phase)? {
            ModuleStaticDistributionNodePhase::Pending => counts.pending = count,
            ModuleStaticDistributionNodePhase::Prepared => counts.prepared = count,
            ModuleStaticDistributionNodePhase::Healthy => counts.healthy = count,
            ModuleStaticDistributionNodePhase::Active => counts.active = count,
            ModuleStaticDistributionNodePhase::Failed => counts.failed = count,
        }
    }
    Ok(counts)
}

async fn load_rollout_state<C: ConnectionTrait>(
    connection: &C,
    lock_row: bool,
) -> Result<ModuleStaticDistributionRolloutState, ModuleStaticDistributionRolloutError> {
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
    Ok(ModuleStaticDistributionRolloutState {
        revision: revision_from_row(&row, "revision", true)?,
        desired_rollout_id: optional_uuid_from_row(&row, "desired_rollout_id", backend)?,
        observed_rollout_id: optional_uuid_from_row(&row, "observed_rollout_id", backend)?,
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
                "SELECT rollout_id, predecessor_rollout_id, distribution_release_id,
                        rollout_revision, distribution_release_revision, composition_revision,
                        composition_digest, artifact_reference, artifact_digest, executor_mode,
                        topology_reference, topology_digest, policy_revision, target_node_count,
                        status, requested_by, failure_code, failure_detail
                 FROM module_static_distribution_rollouts WHERE rollout_id = {}{lock}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(rollout_id, backend)],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionRolloutError::RolloutNotFound)?;
    let node_rows = connection
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT node_id, ordinal, observation_revision, phase,
                        health_evidence_reference, health_evidence_digest,
                        failure_code, failure_detail, reported_by, last_report_digest
                 FROM module_static_distribution_rollout_nodes
                 WHERE rollout_id = {} ORDER BY ordinal",
                placeholder(backend, 1),
            ),
            vec![uuid_value(rollout_id, backend)],
        ))
        .await
        .map_err(store_error)?;
    let nodes = node_rows
        .iter()
        .map(node_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let target_node_count: i64 = row.try_get("", "target_node_count").map_err(store_error)?;
    let target_node_count = u16::try_from(target_node_count).map_err(|_| {
        ModuleStaticDistributionRolloutError::Store(
            "static distribution target-node count is invalid".to_string(),
        )
    })?;
    if usize::from(target_node_count) != nodes.len() {
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
        (Some(code), Some(detail)) => Some(ModuleStaticDistributionNodeFailure { code, detail }),
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
        rollout_revision: revision_from_row(&row, "rollout_revision", false)?,
        distribution_release_revision: revision_from_row(
            &row,
            "distribution_release_revision",
            false,
        )?,
        composition_revision: revision_from_row(&row, "composition_revision", false)?,
        composition_digest: row.try_get("", "composition_digest").map_err(store_error)?,
        artifact_reference: row.try_get("", "artifact_reference").map_err(store_error)?,
        artifact_digest: row.try_get("", "artifact_digest").map_err(store_error)?,
        executor_mode: ModuleStaticDistributionExecutorMode::StaticNative,
        topology_reference: row.try_get("", "topology_reference").map_err(store_error)?,
        topology_digest: row.try_get("", "topology_digest").map_err(store_error)?,
        policy_revision: row.try_get("", "policy_revision").map_err(store_error)?,
        target_node_count,
        status: ModuleStaticDistributionRolloutStatus::parse(
            &row.try_get::<String>("", "status").map_err(store_error)?,
        )?,
        requested_by: uuid_from_row(&row, "requested_by", backend).map_err(store_error)?,
        failure,
        nodes,
    })
}

async fn load_rollout_node<C: ConnectionTrait>(
    connection: &C,
    rollout_id: Uuid,
    node_id: &str,
    lock_row: bool,
) -> Result<ModuleStaticDistributionRolloutNode, ModuleStaticDistributionRolloutError> {
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
                "SELECT node_id, ordinal, observation_revision, phase,
                        health_evidence_reference, health_evidence_digest,
                        failure_code, failure_detail, reported_by, last_report_digest
                 FROM module_static_distribution_rollout_nodes
                 WHERE rollout_id = {} AND node_id = {}{lock}",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![uuid_value(rollout_id, backend), node_id.to_owned().into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleStaticDistributionRolloutError::NodeNotFound)?;
    node_from_row(&row)
}

fn node_from_row(
    row: &QueryResult,
) -> Result<ModuleStaticDistributionRolloutNode, ModuleStaticDistributionRolloutError> {
    let ordinal: i64 = row.try_get("", "ordinal").map_err(store_error)?;
    let health_reference: Option<String> = row
        .try_get("", "health_evidence_reference")
        .map_err(store_error)?;
    let health_digest: Option<String> = row
        .try_get("", "health_evidence_digest")
        .map_err(store_error)?;
    let health_evidence = match (health_reference, health_digest) {
        (Some(reference), Some(digest)) => {
            Some(ModuleStaticDistributionHealthEvidence { reference, digest })
        }
        (None, None) => None,
        _ => {
            return Err(ModuleStaticDistributionRolloutError::Store(
                "static distribution node health evidence is incomplete".to_string(),
            ));
        }
    };
    let failure_code: Option<String> = row.try_get("", "failure_code").map_err(store_error)?;
    let failure_detail: Option<String> = row.try_get("", "failure_detail").map_err(store_error)?;
    let failure = match (failure_code, failure_detail) {
        (Some(code), Some(detail)) => Some(ModuleStaticDistributionNodeFailure { code, detail }),
        (None, None) => None,
        _ => {
            return Err(ModuleStaticDistributionRolloutError::Store(
                "static distribution node failure is incomplete".to_string(),
            ));
        }
    };
    Ok(ModuleStaticDistributionRolloutNode {
        node_id: row.try_get("", "node_id").map_err(store_error)?,
        ordinal: u16::try_from(ordinal).map_err(|_| {
            ModuleStaticDistributionRolloutError::Store(
                "static distribution node ordinal is invalid".to_string(),
            )
        })?,
        observation_revision: revision_from_row(row, "observation_revision", true)?,
        phase: ModuleStaticDistributionNodePhase::parse(
            &row.try_get::<String>("", "phase").map_err(store_error)?,
        )?,
        health_evidence,
        failure,
        reported_by: row.try_get("", "reported_by").map_err(store_error)?,
        last_report_digest: row.try_get("", "last_report_digest").map_err(store_error)?,
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
                        node_id, observation_revision, node_phase,
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
        observation_revision: optional_revision_from_row(&row, "observation_revision")?,
        node_phase: row
            .try_get::<Option<String>>("", "node_phase")
            .map_err(store_error)?
            .as_deref()
            .map(ModuleStaticDistributionNodePhase::parse)
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
    )
    .await
}

async fn complete_report_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    receipt: &ModuleStaticDistributionNodeReportReceipt,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    complete_operation(
        transaction,
        idempotency_key,
        receipt.rollout_id,
        receipt.rollout_revision,
        receipt.rollout_state_revision,
        receipt.rollout_status,
        Some(&receipt.node_id),
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
    observation_revision: Option<u64>,
    node_phase: Option<ModuleStaticDistributionNodePhase>,
) -> Result<(), ModuleStaticDistributionRolloutError> {
    let backend = transaction.get_database_backend();
    let updated = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_static_distribution_rollout_operations
                 SET rollout_id = {}, rollout_revision = {}, rollout_state_revision = {},
                     rollout_status = {}, node_id = {}, observation_revision = {},
                     node_phase = {}, completed_at = {}
                 WHERE idempotency_key = {} AND completed_at IS NULL",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                now_expression(backend),
                placeholder(backend, 8),
            ),
            vec![
                uuid_value(rollout_id, backend),
                revision_value(rollout_revision)?,
                revision_value(rollout_state_revision)?,
                rollout_status.as_str().into(),
                node_id.map(str::to_owned).into(),
                optional_revision_value(observation_revision)?,
                node_phase.map(|phase| phase.as_str().to_string()).into(),
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
) -> Result<ModuleStaticDistributionRolloutReceipt, ModuleStaticDistributionRolloutError> {
    if !operation.completed
        || operation.operation_kind != "request"
        || operation.node_id.is_some()
        || operation.observation_revision.is_some()
        || operation.node_phase.is_some()
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
) -> Result<ModuleStaticDistributionNodeReportReceipt, ModuleStaticDistributionRolloutError> {
    if !operation.completed || operation.operation_kind != "report" {
        return Err(ModuleStaticDistributionRolloutError::IdempotencyConflict);
    }
    Ok(ModuleStaticDistributionNodeReportReceipt {
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
        observation_revision: operation
            .observation_revision
            .ok_or(ModuleStaticDistributionRolloutError::IdempotencyConflict)?,
        phase: operation
            .node_phase
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
