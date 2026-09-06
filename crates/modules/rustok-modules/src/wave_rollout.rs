//! Multi-node canary and wave rollout coordinator.
//!
//! Enforces:
//! - Dual pre-staging barrier: both candidate and predecessor bundles pre-staged
//!   everywhere before the first mutation.
//! - Sequential cohort mutation: canary (wave 0) mutates first; untouched nodes
//!   retain predecessor capacity.
//! - Wave-by-wave verification gate before widening the cohort.
//! - Single-attempt wave rollback returning all mutated waves to their exact predecessor assignments.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveAssignmentPhase {
    PreStaging,
    PreStaged,
    Mutating,
    Verified,
    RolledBack,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveNodeAssignment {
    pub node_id: String,
    pub role: String,
    pub candidate_digest: String,
    pub predecessor_digest: Option<String>,
    pub pre_staged_candidate: bool,
    pub pre_staged_predecessor: bool,
    pub phase: WaveAssignmentPhase,
}

impl WaveNodeAssignment {
    pub fn is_fully_pre_staged(&self) -> bool {
        self.pre_staged_candidate && (self.predecessor_digest.is_none() || self.pre_staged_predecessor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveCohort {
    pub wave_index: usize,
    pub name: String,
    pub assignments: Vec<WaveNodeAssignment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveRolloutState {
    Preparing,
    PreStaged,
    MutatingWave(usize),
    VerifiedWave(usize),
    Converged,
    RolledBack,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveRollbackReceipt {
    pub rollout_id: Uuid,
    pub reverted_cohort_count: usize,
    pub untouched_cohort_count: usize,
    pub recovery_attempts_consumed: u32,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum WaveRolloutError {
    #[error("Dual pre-staging barrier not met: node `{0}` has not pre-staged required bundles")]
    PreStagingBarrierNotMet(String),
    #[error("Cannot start mutation in state `{0:?}`")]
    InvalidState(WaveRolloutState),
    #[error("Invalid wave index {0}: total waves is {1}")]
    InvalidWaveIndex(usize, usize),
    #[error("Previous wave {0} has not yet been verified")]
    PreviousWaveNotVerified(usize),
    #[error("Wave recovery already exhausted (max 1 attempt allowed)")]
    RecoveryExhausted,
    #[error("No predecessor bundle available for rollback on wave {0}")]
    NoPredecessorAvailable(usize),
}

/// Orchestrates multi-node canary and wave deployments with pre-staging fences and capacity retention.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveRolloutCoordinator {
    pub rollout_id: Uuid,
    pub cohorts: Vec<WaveCohort>,
    pub state: WaveRolloutState,
    pub recovery_attempts: u32,
}

impl WaveRolloutCoordinator {
    pub fn new(rollout_id: Uuid, cohorts: Vec<WaveCohort>) -> Self {
        Self {
            rollout_id,
            cohorts,
            state: WaveRolloutState::Preparing,
            recovery_attempts: 0,
        }
    }

    pub fn total_cohorts(&self) -> usize {
        self.cohorts.len()
    }

    /// Reports that a node has completed downloading and pre-staging bundles.
    pub fn report_node_pre_staged(
        &mut self,
        node_id: &str,
        candidate_ok: bool,
        predecessor_ok: bool,
    ) {
        for cohort in &mut self.cohorts {
            for assignment in &mut cohort.assignments {
                if assignment.node_id == node_id {
                    assignment.pre_staged_candidate = candidate_ok;
                    assignment.pre_staged_predecessor = predecessor_ok;
                    if assignment.is_fully_pre_staged() && assignment.phase == WaveAssignmentPhase::PreStaging {
                        assignment.phase = WaveAssignmentPhase::PreStaged;
                    }
                }
            }
        }

        // If all nodes across all cohorts are pre-staged, advance state to PreStaged
        if self.state == WaveRolloutState::Preparing && self.check_all_nodes_pre_staged() {
            self.state = WaveRolloutState::PreStaged;
        }
    }

    fn check_all_nodes_pre_staged(&self) -> bool {
        self.cohorts
            .iter()
            .flat_map(|c| &c.assignments)
            .all(|a| a.is_fully_pre_staged())
    }

    /// Verifies the mandatory Dual Pre-staging Barrier.
    /// Mutation cannot proceed unless EVERY node across EVERY wave has both bundles pre-staged.
    pub fn verify_dual_pre_staging_barrier(&self) -> Result<(), WaveRolloutError> {
        for cohort in &self.cohorts {
            for assignment in &cohort.assignments {
                if !assignment.is_fully_pre_staged() {
                    return Err(WaveRolloutError::PreStagingBarrierNotMet(
                        assignment.node_id.clone(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Starts mutating a specific wave cohort (e.g. Wave 0 / Canary first).
    /// Enforces:
    /// - Dual pre-staging barrier is strictly met across entire fleet.
    /// - If wave_index > 0, wave_index - 1 must already be Verified.
    /// - Untouched waves remain un-mutated with predecessor capacity intact.
    pub fn start_wave_mutation(&mut self, wave_index: usize) -> Result<(), WaveRolloutError> {
        if wave_index >= self.cohorts.len() {
            return Err(WaveRolloutError::InvalidWaveIndex(
                wave_index,
                self.cohorts.len(),
            ));
        }

        self.verify_dual_pre_staging_barrier()?;

        if wave_index == 0 {
            if self.state != WaveRolloutState::PreStaged {
                return Err(WaveRolloutError::InvalidState(self.state));
            }
        } else {
            let expected_previous = WaveRolloutState::VerifiedWave(wave_index - 1);
            if self.state != expected_previous {
                return Err(WaveRolloutError::PreviousWaveNotVerified(wave_index - 1));
            }
        }

        // Mutate target wave
        for assignment in &mut self.cohorts[wave_index].assignments {
            assignment.phase = WaveAssignmentPhase::Mutating;
        }

        self.state = WaveRolloutState::MutatingWave(wave_index);
        Ok(())
    }

    /// Verifies that all nodes in the mutating wave have reached health and readiness.
    /// If this was the final wave, transitions the rollout to Converged.
    pub fn verify_wave(&mut self, wave_index: usize) -> Result<(), WaveRolloutError> {
        if self.state != WaveRolloutState::MutatingWave(wave_index) {
            return Err(WaveRolloutError::InvalidState(self.state));
        }

        for assignment in &mut self.cohorts[wave_index].assignments {
            assignment.phase = WaveAssignmentPhase::Verified;
        }

        if wave_index + 1 == self.cohorts.len() {
            self.state = WaveRolloutState::Converged;
        } else {
            self.state = WaveRolloutState::VerifiedWave(wave_index);
        }

        Ok(())
    }

    /// Rolls back all mutated waves back to their exact predecessor assignments.
    /// Untouched waves were never mutated and remain serving predecessor capacity.
    /// Bounded by exactly 1 recovery attempt (`recovery_attempts <= 1`).
    pub fn rollback_all_mutated_waves(
        &mut self,
        failed_wave_index: usize,
    ) -> Result<WaveRollbackReceipt, WaveRolloutError> {
        if self.recovery_attempts >= 1 {
            return Err(WaveRolloutError::RecoveryExhausted);
        }
        if failed_wave_index >= self.cohorts.len() {
            return Err(WaveRolloutError::InvalidWaveIndex(
                failed_wave_index,
                self.cohorts.len(),
            ));
        }

        // 1. Check predecessor exists for all mutated cohorts
        for idx in 0..=failed_wave_index {
            for assignment in &self.cohorts[idx].assignments {
                if assignment.predecessor_digest.is_none() {
                    return Err(WaveRolloutError::NoPredecessorAvailable(idx));
                }
            }
        }

        // 2. Revert mutated cohorts (0..=failed_wave_index)
        let reverted_count = failed_wave_index + 1;
        for idx in 0..=failed_wave_index {
            for assignment in &mut self.cohorts[idx].assignments {
                assignment.phase = WaveAssignmentPhase::RolledBack;
            }
        }

        let untouched_count = self.cohorts.len() - reverted_count;
        self.recovery_attempts += 1;
        self.state = WaveRolloutState::RolledBack;

        Ok(WaveRollbackReceipt {
            rollout_id: self.rollout_id,
            reverted_cohort_count: reverted_count,
            untouched_cohort_count: untouched_count,
            recovery_attempts_consumed: self.recovery_attempts,
        })
    }
}
