//! Unified durable transition coordinator and single-attempt recovery orchestrator.
//!
//! Orchestrates the complete module release lifecycle:
//! Preflighting -> Fenced -> PreStaging -> Activating -> Observing -> Converged
//! or RollbackTriggered -> RecoveredToPredecessor.
//!
//! Enforces the platform invariant: exactly one automatic recovery attempt is
//! permitted per transition, preventing infinite flapping or release oscillation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

use sea_orm::ConnectionTrait;

use crate::{
    ConflictFenceSet, GlobalSecurityEpoch, MigrationPreflightReceipt, ModuleCommandContext,
    RetentionHoldLedger, RetentionHoldStore, RetentionTarget, SecurityEpochConflictError,
    SecurityEpochRegistry, TransitionCheckpointStore, UpdateMode,
};

/// Lifecycle states of a durable module transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details")]
pub enum ModuleTransitionState {
    /// Initial phase: validating DDL safety, settings compatibility, and epoch.
    Preflighting,
    /// Conflict fences successfully acquired.
    Fenced,
    /// Candidate payload is pre-staged on the node's non-serving standby slot.
    PreStaging,
    /// Traffic cutover in progress.
    Activating,
    /// Serving live traffic under active health observation.
    Observing { timeout_at: DateTime<Utc> },
    /// Point of no return committed: irreversible migration, compensating, or candidate-only settings active. Rollback window is closed.
    PointOfNoReturn {
        reason: String,
        committed_at: DateTime<Utc>,
    },
    /// An incident was detected and automatic recovery was initiated.
    RollbackTriggered {
        reason: String,
        triggered_at: DateTime<Utc>,
    },
    /// Direct predecessor successfully promoted back to serving; incident contained.
    RecoveredToPredecessor {
        failure_reason: String,
        recovered_at: DateTime<Utc>,
    },
    /// Observation window elapsed and transition converged successfully.
    Converged { finalized_at: DateTime<Utc> },
    /// Terminal failure where automatic action is exhausted or forbidden; operator intervention required.
    FailedClosed { failure_reason: String },
}

impl ModuleTransitionState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::RecoveredToPredecessor { .. }
                | Self::Converged { .. }
                | Self::FailedClosed { .. }
        )
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Preflighting => "preflighting",
            Self::Fenced => "fenced",
            Self::PreStaging => "prestaging",
            Self::Activating => "activating",
            Self::Observing { .. } => "observing",
            Self::PointOfNoReturn { .. } => "point_of_no_return",
            Self::RollbackTriggered { .. } => "rollback_triggered",
            Self::RecoveredToPredecessor { .. } => "recovered_to_predecessor",
            Self::Converged { .. } => "converged",
            Self::FailedClosed { .. } => "failed_closed",
        }
    }
}

/// Durable checkpoint capturing the complete immutable facts of a transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleTransitionCheckpoint {
    pub operation_id: Uuid,
    /// Monotonic owner revision used by every transition command for CAS.
    pub revision: u64,
    pub module_slug: String,
    pub tenant_id: Option<Uuid>,
    pub predecessor_digest: Option<String>,
    pub candidate_digest: String,
    pub state: ModuleTransitionState,
    pub security_epoch: GlobalSecurityEpoch,
    pub fences: ConflictFenceSet,
    pub recovery_attempt_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransitionCoordinatorError {
    #[error("Invalid state transition from `{from}` to `{to}`")]
    InvalidStateTransition { from: String, to: String },
    #[error("Security epoch conflict: {0}")]
    SecurityEpochStale(#[from] SecurityEpochConflictError),
    #[error("Preflight validation failed: {0}")]
    PreflightFailed(String),
    #[error("Single-attempt recovery limit exhausted: {0}")]
    RecoveryLimitExhausted(String),
    #[error("Operation is already in terminal state `{0}`")]
    OperationAlreadyTerminal(String),
    #[error(
        "Predecessor retention hold missing for digest `{0}`: predecessor bytes must be protected from GC during transition"
    )]
    PredecessorRetentionMissing(String),
    #[error("Conflict fence validation failed: {0}")]
    ConflictFenceViolation(String),
    #[error("Past point of no return for operation `{0}`: {1}; rollback is forbidden")]
    PastPointOfNoReturn(Uuid, String),
}

/// Input for starting a governed module transition.
#[derive(Debug, Clone)]
pub struct StartTransitionInput {
    pub operation_id: Uuid,
    pub module_slug: String,
    pub tenant_id: Option<Uuid>,
    pub predecessor_digest: Option<String>,
    pub candidate_digest: String,
    pub affected_nodes: Vec<String>,
    pub preflight_receipt: MigrationPreflightReceipt,
    pub requested_mode: UpdateMode,
}

/// Authenticated recovery command for one exact durable transition revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleTransitionRecoveryCommand {
    pub operation_id: Uuid,
    pub expected_revision: u64,
    pub reason: String,
    pub context: ModuleCommandContext,
    /// Authenticated host fact. The owner rejects a command that did not pass
    /// the platform `modules:manage` authorization boundary.
    pub actor_can_manage_modules: bool,
}

/// Authenticated command that closes the observation window of one exact
/// durable transition revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleTransitionFinalizeCommand {
    pub operation_id: Uuid,
    pub expected_revision: u64,
    pub context: ModuleCommandContext,
    /// Authenticated host fact. The owner rejects a command that did not pass
    /// the platform `modules:manage` authorization boundary.
    pub actor_can_manage_modules: bool,
}

/// Durable transition coordinator managing lifecycle state, fences, and recovery limits.
#[derive(Debug, Clone)]
pub struct ModuleTransitionCoordinator {
    checkpoint: ModuleTransitionCheckpoint,
}

impl ModuleTransitionCoordinator {
    pub fn new(checkpoint: ModuleTransitionCheckpoint) -> Self {
        Self { checkpoint }
    }

    pub fn checkpoint(&self) -> &ModuleTransitionCheckpoint {
        &self.checkpoint
    }

    pub fn state(&self) -> &ModuleTransitionState {
        &self.checkpoint.state
    }

    /// Evaluates preflight, validates security epoch, acquires conflict fences, and starts the transition.
    pub fn start_transition(
        input: StartTransitionInput,
        security_registry: &SecurityEpochRegistry,
    ) -> Result<Self, TransitionCoordinatorError> {
        // 1. Verify that automatic mode is authorized if requested
        if input.requested_mode == UpdateMode::Automatic
            && input.preflight_receipt.mode != UpdateMode::Automatic
        {
            let reasons = input.preflight_receipt.denial_reasons.join("; ");
            return Err(TransitionCoordinatorError::PreflightFailed(format!(
                "Automatic update mode denied by preflight: {}",
                reasons
            )));
        }

        // 2. Validate current security epoch
        let current_epoch = security_registry.current_epoch();
        security_registry.validate_epoch(current_epoch)?;

        // 3. Derive deterministic conflict fence set
        let fences = ConflictFenceSet::derive_module_update_fences(
            &input.module_slug,
            input.tenant_id,
            &input.affected_nodes,
        );

        let now = Utc::now();
        let checkpoint = ModuleTransitionCheckpoint {
            operation_id: input.operation_id,
            revision: 1,
            module_slug: input.module_slug,
            tenant_id: input.tenant_id,
            predecessor_digest: input.predecessor_digest,
            candidate_digest: input.candidate_digest,
            state: ModuleTransitionState::Fenced,
            security_epoch: current_epoch,
            fences,
            recovery_attempt_count: 0,
            created_at: now,
            updated_at: now,
        };

        Ok(Self { checkpoint })
    }

    /// Revalidates that the transition is active, the security epoch has not drifted,
    /// and that any existing predecessor digest is actively protected by a retention hold.
    pub fn revalidate_mutation_preconditions(
        &self,
        security_registry: &SecurityEpochRegistry,
        retention_ledger: Option<&RetentionHoldLedger>,
    ) -> Result<(), TransitionCoordinatorError> {
        self.ensure_active()?;
        security_registry.validate_epoch(self.checkpoint.security_epoch)?;

        if let (Some(predecessor_digest), Some(ledger)) =
            (&self.checkpoint.predecessor_digest, retention_ledger)
        {
            let target_source = RetentionTarget::SourceCasBlob {
                digest: predecessor_digest.clone(),
            };
            let target_payload = RetentionTarget::AdmittedPayloadCas {
                digest: predecessor_digest.clone(),
            };
            if ledger.active_holds_count(&target_source) == 0
                && ledger.active_holds_count(&target_payload) == 0
            {
                return Err(TransitionCoordinatorError::PredecessorRetentionMissing(
                    predecessor_digest.clone(),
                ));
            }
        }

        Ok(())
    }

    /// Advances the transition to pre-staging candidate payload on standby node slots.
    pub fn advance_to_prestaging(
        &mut self,
        security_registry: &SecurityEpochRegistry,
    ) -> Result<(), TransitionCoordinatorError> {
        self.revalidate_mutation_preconditions(security_registry, None)?;

        if self.checkpoint.state != ModuleTransitionState::Fenced {
            return Err(TransitionCoordinatorError::InvalidStateTransition {
                from: self.checkpoint.state.name().to_string(),
                to: "prestaging".to_string(),
            });
        }

        self.checkpoint.revision += 1;
        self.checkpoint.state = ModuleTransitionState::PreStaging;
        self.checkpoint.updated_at = Utc::now();
        Ok(())
    }

    /// Advances the transition to activating candidate and entering observation window.
    pub fn advance_to_activating(
        &mut self,
        security_registry: &SecurityEpochRegistry,
        observation_duration: Duration,
    ) -> Result<(), TransitionCoordinatorError> {
        self.advance_to_activating_with_ledger(security_registry, None, observation_duration)
    }

    /// Advances the transition to activating candidate and entering observation window,
    /// strictly verifying predecessor retention hold when a retention ledger is provided.
    pub fn advance_to_activating_with_ledger(
        &mut self,
        security_registry: &SecurityEpochRegistry,
        retention_ledger: Option<&RetentionHoldLedger>,
        observation_duration: Duration,
    ) -> Result<(), TransitionCoordinatorError> {
        self.revalidate_mutation_preconditions(security_registry, retention_ledger)?;

        if self.checkpoint.state != ModuleTransitionState::PreStaging {
            return Err(TransitionCoordinatorError::InvalidStateTransition {
                from: self.checkpoint.state.name().to_string(),
                to: "activating".to_string(),
            });
        }

        let timeout_at = Utc::now() + observation_duration;
        self.checkpoint.revision += 1;
        self.checkpoint.state = ModuleTransitionState::Observing { timeout_at };
        self.checkpoint.updated_at = Utc::now();
        Ok(())
    }

    /// Advances to activating by asynchronously querying the database for active retention holds.
    pub async fn advance_to_activating_with_db<C: ConnectionTrait>(
        &mut self,
        db: &C,
        security_registry: &SecurityEpochRegistry,
        observation_duration: Duration,
    ) -> Result<(), TransitionCoordinatorError> {
        let ledger = RetentionHoldStore::load_active_ledger(db)
            .await
            .map_err(|e| TransitionCoordinatorError::PreflightFailed(e.to_string()))?;
        self.advance_to_activating_with_ledger(
            security_registry,
            Some(&ledger),
            observation_duration,
        )
    }

    /// Records an incident / failure signal and executes single-attempt recovery.
    ///
    /// If the recovery attempt limit (1) is already exhausted, the transition fails closed.
    pub fn record_recovery_trigger(
        &mut self,
        reason: String,
    ) -> Result<(), TransitionCoordinatorError> {
        if self.checkpoint.state.is_terminal() {
            return Err(TransitionCoordinatorError::OperationAlreadyTerminal(
                self.checkpoint.state.name().to_string(),
            ));
        }

        if let ModuleTransitionState::PointOfNoReturn { ref reason, .. } = self.checkpoint.state {
            return Err(TransitionCoordinatorError::PastPointOfNoReturn(
                self.checkpoint.operation_id,
                format!("Cannot trigger recovery past point of no return: {}", reason),
            ));
        }

        // Single-attempt recovery invariant enforcement
        if self.checkpoint.recovery_attempt_count >= 1 {
            let msg = format!(
                "Automatic recovery already attempted ({}); failing closed to prevent oscillation: {}",
                self.checkpoint.recovery_attempt_count, reason
            );
            self.checkpoint.revision += 1;
            self.checkpoint.state = ModuleTransitionState::FailedClosed {
                failure_reason: msg.clone(),
            };
            self.checkpoint.updated_at = Utc::now();
            return Err(TransitionCoordinatorError::RecoveryLimitExhausted(msg));
        }

        self.checkpoint.revision += 1;
        self.checkpoint.recovery_attempt_count += 1;
        self.checkpoint.state = ModuleTransitionState::RecoveredToPredecessor {
            failure_reason: reason,
            recovered_at: Utc::now(),
        };
        self.checkpoint.updated_at = Utc::now();
        Ok(())
    }

    /// Commits point-of-no-return and expands conflict fences to include full traffic, job, and write fences
    /// before any compensating, non-transactional, destructive, or irreversible effect.
    ///
    /// Once committed, rollback is permanently forbidden and recovery attempts fail closed.
    pub fn commit_point_of_no_return(
        &mut self,
        reason: String,
        security_registry: &SecurityEpochRegistry,
        affected_nodes: &[String],
    ) -> Result<(), TransitionCoordinatorError> {
        self.revalidate_mutation_preconditions(security_registry, None)?;

        if self.checkpoint.state.is_terminal() {
            return Err(TransitionCoordinatorError::OperationAlreadyTerminal(
                self.checkpoint.state.name().to_string(),
            ));
        }

        let fences = ConflictFenceSet::derive_point_of_no_return_fences(
            &self.checkpoint.module_slug,
            self.checkpoint.tenant_id,
            affected_nodes,
        );

        self.checkpoint.revision += 1;
        self.checkpoint.fences = fences;
        self.checkpoint.state = ModuleTransitionState::PointOfNoReturn {
            reason,
            committed_at: Utc::now(),
        };
        self.checkpoint.updated_at = Utc::now();
        Ok(())
    }

    /// Advances from PointOfNoReturn to Converged once the irreversible effect has completed.
    pub fn advance_point_of_no_return_to_converged(
        &mut self,
        security_registry: &SecurityEpochRegistry,
    ) -> Result<(), TransitionCoordinatorError> {
        self.revalidate_mutation_preconditions(security_registry, None)?;

        match self.checkpoint.state {
            ModuleTransitionState::PointOfNoReturn { .. } => {
                self.checkpoint.revision += 1;
                self.checkpoint.state = ModuleTransitionState::Converged {
                    finalized_at: Utc::now(),
                };
                self.checkpoint.updated_at = Utc::now();
                Ok(())
            }
            _ => Err(TransitionCoordinatorError::InvalidStateTransition {
                from: self.checkpoint.state.name().to_string(),
                to: "converged".to_string(),
            }),
        }
    }

    /// Finalizes the transition upon successful observation window or point-of-no-return completion.
    pub fn finalize_convergence(
        &mut self,
        security_registry: &SecurityEpochRegistry,
    ) -> Result<(), TransitionCoordinatorError> {
        self.revalidate_mutation_preconditions(security_registry, None)?;

        match self.checkpoint.state {
            ModuleTransitionState::Observing { .. }
            | ModuleTransitionState::PointOfNoReturn { .. } => {
                self.checkpoint.revision += 1;
                self.checkpoint.state = ModuleTransitionState::Converged {
                    finalized_at: Utc::now(),
                };
                self.checkpoint.updated_at = Utc::now();
                Ok(())
            }
            _ => Err(TransitionCoordinatorError::InvalidStateTransition {
                from: self.checkpoint.state.name().to_string(),
                to: "converged".to_string(),
            }),
        }
    }

    fn ensure_active(&self) -> Result<(), TransitionCoordinatorError> {
        if self.checkpoint.state.is_terminal() {
            Err(TransitionCoordinatorError::OperationAlreadyTerminal(
                self.checkpoint.state.name().to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Evaluates all active transition observation windows against deadlines and security epochs.
///
/// Automatically finalizes convergence when observation deadline expires without issues,
/// releases temporary `ActiveRolloutWindow` retention holds, and triggers single-attempt
/// recovery if a security epoch preemption or invalidation is observed.
pub async fn evaluate_transition_watchdog<C: ConnectionTrait>(
    db: &C,
    security_registry: &SecurityEpochRegistry,
) -> Result<Vec<ModuleTransitionCheckpoint>, TransitionCoordinatorError> {
    let active_checkpoints = TransitionCheckpointStore::list_active_checkpoints(db)
        .await
        .map_err(|e| TransitionCoordinatorError::PreflightFailed(e.to_string()))?;

    let mut updated = Vec::new();
    let now = Utc::now();

    for checkpoint in active_checkpoints {
        let mut coordinator = ModuleTransitionCoordinator::new(checkpoint);
        let mut changed = false;

        // 1. Check for stale security epoch preemption
        if let Err(_epoch_err) =
            security_registry.validate_epoch(coordinator.checkpoint().security_epoch)
        {
            let msg = format!(
                "Security epoch preemption: Epoch {:?} is stale (current: {:?})",
                coordinator.checkpoint().security_epoch,
                security_registry.current_epoch()
            );
            // Try single-attempt recovery or fail-closed
            let _ = coordinator.record_recovery_trigger(msg);
            changed = true;
        } else {
            // 2. Check for observation timeout expiration
            if let ModuleTransitionState::Observing { timeout_at } = coordinator.state() {
                if now >= *timeout_at {
                    if coordinator.finalize_convergence(security_registry).is_ok() {
                        changed = true;
                        // Release GC retention holds associated with this rollout
                        let _ = RetentionHoldStore::release_holds_for_operation(
                            db,
                            coordinator.checkpoint().operation_id,
                        )
                        .await;
                    }
                }
            }
        }

        if changed {
            TransitionCheckpointStore::save_checkpoint(db, coordinator.checkpoint())
                .await
                .map_err(|e| TransitionCoordinatorError::PreflightFailed(e.to_string()))?;
            updated.push(coordinator.checkpoint().clone());
        }
    }

    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_preflight_receipt(is_safe: bool) -> MigrationPreflightReceipt {
        MigrationPreflightReceipt {
            operation_id: Uuid::new_v4(),
            module_slug: "orders".to_string(),
            mode: if is_safe {
                UpdateMode::Automatic
            } else {
                UpdateMode::Maintenance
            },
            source_schema_digest: "sha256:source".to_string(),
            target_schema_digest: "sha256:target".to_string(),
            migration_plan_digest: "sha256:plan".to_string(),
            is_additive_safe: is_safe,
            settings_guard_installed: is_safe,
            evaluated_at: Utc::now(),
            denial_reasons: if is_safe {
                vec![]
            } else {
                vec!["Destructive table drop".to_string()]
            },
        }
    }

    #[test]
    fn test_happy_path_transition_convergence() {
        let registry = SecurityEpochRegistry::new();
        let input = StartTransitionInput {
            operation_id: Uuid::new_v4(),
            module_slug: "orders".to_string(),
            tenant_id: None,
            predecessor_digest: Some("sha256:v1".to_string()),
            candidate_digest: "sha256:v2".to_string(),
            affected_nodes: vec!["node-1".to_string()],
            preflight_receipt: dummy_preflight_receipt(true),
            requested_mode: UpdateMode::Automatic,
        };

        let mut coordinator =
            ModuleTransitionCoordinator::start_transition(input, &registry).unwrap();
        assert_eq!(coordinator.state(), &ModuleTransitionState::Fenced);

        coordinator.advance_to_prestaging(&registry).unwrap();
        assert_eq!(coordinator.state(), &ModuleTransitionState::PreStaging);

        coordinator
            .advance_to_activating(&registry, Duration::from_secs(60))
            .unwrap();
        assert!(matches!(
            coordinator.state(),
            ModuleTransitionState::Observing { .. }
        ));

        coordinator.finalize_convergence(&registry).unwrap();
        assert!(matches!(
            coordinator.state(),
            ModuleTransitionState::Converged { .. }
        ));
        assert!(coordinator.state().is_terminal());
    }

    #[test]
    fn test_single_attempt_recovery_invariant_and_oscillation_protection() {
        let registry = SecurityEpochRegistry::new();
        let input = StartTransitionInput {
            operation_id: Uuid::new_v4(),
            module_slug: "orders".to_string(),
            tenant_id: None,
            predecessor_digest: Some("sha256:v1".to_string()),
            candidate_digest: "sha256:v2_faulty".to_string(),
            affected_nodes: vec!["node-1".to_string()],
            preflight_receipt: dummy_preflight_receipt(true),
            requested_mode: UpdateMode::Automatic,
        };

        let mut coordinator =
            ModuleTransitionCoordinator::start_transition(input, &registry).unwrap();
        coordinator.advance_to_prestaging(&registry).unwrap();
        coordinator
            .advance_to_activating(&registry, Duration::from_secs(60))
            .unwrap();

        // 1. Candidate fails on node -> automatic recovery attempt 1 succeeds!
        coordinator
            .record_recovery_trigger("Node watchdog: Candidate process crashed".to_string())
            .unwrap();
        assert!(matches!(
            coordinator.state(),
            ModuleTransitionState::RecoveredToPredecessor { .. }
        ));
        assert_eq!(coordinator.checkpoint().recovery_attempt_count, 1);

        // 2. Subsequent failure signal triggers -> must be rejected by terminal check
        let second_trigger =
            coordinator.record_recovery_trigger("Second failure signal".to_string());
        assert!(second_trigger.is_err());
    }

    #[test]
    fn test_security_epoch_preemption_blocks_activation() {
        let mut registry = SecurityEpochRegistry::new();
        let input = StartTransitionInput {
            operation_id: Uuid::new_v4(),
            module_slug: "orders".to_string(),
            tenant_id: None,
            predecessor_digest: Some("sha256:v1".to_string()),
            candidate_digest: "sha256:v2".to_string(),
            affected_nodes: vec!["node-1".to_string()],
            preflight_receipt: dummy_preflight_receipt(true),
            requested_mode: UpdateMode::Automatic,
        };

        let mut coordinator =
            ModuleTransitionCoordinator::start_transition(input, &registry).unwrap();
        coordinator.advance_to_prestaging(&registry).unwrap();

        // Global quarantine event advances security epoch!
        registry.advance_epoch("Global security quarantine on supply-chain artifact");

        // Activation attempt must fail closed due to stale epoch!
        let activation_result =
            coordinator.advance_to_activating(&registry, Duration::from_secs(60));
        assert!(activation_result.is_err());
    }

    #[test]
    fn test_predecessor_retention_hold_revalidation() {
        let registry = SecurityEpochRegistry::new();
        let input = StartTransitionInput {
            operation_id: Uuid::new_v4(),
            module_slug: "orders".to_string(),
            tenant_id: None,
            predecessor_digest: Some("sha256:predecessor_v1".to_string()),
            candidate_digest: "sha256:candidate_v2".to_string(),
            affected_nodes: vec!["node-1".to_string()],
            preflight_receipt: dummy_preflight_receipt(true),
            requested_mode: UpdateMode::Automatic,
        };

        let mut coordinator =
            ModuleTransitionCoordinator::start_transition(input, &registry).unwrap();
        coordinator.advance_to_prestaging(&registry).unwrap();

        // 1. Attempt activation with an empty retention ledger -> MUST fail with PredecessorRetentionMissing
        let mut empty_ledger = RetentionHoldLedger::new();
        let err = coordinator
            .advance_to_activating_with_ledger(
                &registry,
                Some(&empty_ledger),
                Duration::from_secs(60),
            )
            .unwrap_err();
        assert!(matches!(
            err,
            TransitionCoordinatorError::PredecessorRetentionMissing(digest) if digest == "sha256:predecessor_v1"
        ));

        // 2. Place an active rollout retention hold protecting predecessor payload
        empty_ledger.place_hold(
            RetentionTarget::AdmittedPayloadCas {
                digest: "sha256:predecessor_v1".to_string(),
            },
            crate::RetentionHoldKind::ActiveRolloutWindow {
                operation_id: coordinator.checkpoint().operation_id,
                expires_at: Utc::now() + chrono::Duration::seconds(300),
            },
        );

        // 3. Activation must now succeed
        coordinator
            .advance_to_activating_with_ledger(
                &registry,
                Some(&empty_ledger),
                Duration::from_secs(60),
            )
            .unwrap();
        assert!(matches!(
            coordinator.state(),
            ModuleTransitionState::Observing { .. }
        ));
    }
}
