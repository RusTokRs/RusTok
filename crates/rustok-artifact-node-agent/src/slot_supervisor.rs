//! Side-by-side execution slot supervisor for zero-downtime deployment and instant rollback.
//!
//! Manages isolated execution slots (Slot A and Slot B), ensuring that candidate
//! releases are pre-staged, booted, and health-verified on a non-serving slot
//! before traffic is switched, while retaining the predecessor in hot-standby.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Enumeration of isolated deployment slots on a single node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentSlot {
    SlotA,
    SlotB,
}

impl DeploymentSlot {
    /// Returns the alternate slot.
    pub const fn other(self) -> Self {
        match self {
            Self::SlotA => Self::SlotB,
            Self::SlotB => Self::SlotA,
        }
    }
}

/// State of an individual deployment slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details")]
pub enum SlotState {
    /// Slot is unallocated and has no prepared payload.
    Empty,
    /// Artifact bytes are downloaded, verified, and pre-staged.
    PreStaged { artifact_digest: String },
    /// Process is running on the assigned port and actively serving live traffic.
    Serving { artifact_digest: String, port: u16 },
    /// Process is running on standby port, ready for instant traffic promotion or rollback.
    Standby { artifact_digest: String, port: u16 },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SlotSupervisorError {
    #[error("Candidate slot is not in pre-staged or standby state: {0:?}")]
    CandidateNotReady(SlotState),
    #[error("Predecessor slot is not in standby state for rollback: {0:?}")]
    PredecessorNotAvailable(SlotState),
    #[error("Invalid slot transition: {0}")]
    InvalidTransition(String),
}

/// Out-of-process supervisor managing Slot A and Slot B on a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotSupervisor {
    active_slot: DeploymentSlot,
    slot_a: SlotState,
    slot_b: SlotState,
    base_port_a: u16,
    base_port_b: u16,
}

impl SlotSupervisor {
    /// Initializes a slot supervisor with an initial serving release on Slot A.
    pub fn new(initial_digest: String, base_port_a: u16, base_port_b: u16) -> Self {
        Self {
            active_slot: DeploymentSlot::SlotA,
            slot_a: SlotState::Serving {
                artifact_digest: initial_digest,
                port: base_port_a,
            },
            slot_b: SlotState::Empty,
            base_port_a,
            base_port_b,
        }
    }

    /// Initializes a fresh supervisor with empty slots (for cold node bootstrap).
    pub fn empty(base_port_a: u16, base_port_b: u16) -> Self {
        Self {
            active_slot: DeploymentSlot::SlotA,
            slot_a: SlotState::Empty,
            slot_b: SlotState::Empty,
            base_port_a,
            base_port_b,
        }
    }

    pub fn active_slot(&self) -> DeploymentSlot {
        self.active_slot
    }

    pub fn standby_slot(&self) -> DeploymentSlot {
        self.active_slot.other()
    }

    pub fn get_slot_state(&self, slot: DeploymentSlot) -> &SlotState {
        match slot {
            DeploymentSlot::SlotA => &self.slot_a,
            DeploymentSlot::SlotB => &self.slot_b,
        }
    }

    pub fn active_state(&self) -> &SlotState {
        self.get_slot_state(self.active_slot)
    }

    pub fn standby_state(&self) -> &SlotState {
        self.get_slot_state(self.standby_slot())
    }

    pub fn get_slot_port(&self, slot: DeploymentSlot) -> u16 {
        match slot {
            DeploymentSlot::SlotA => self.base_port_a,
            DeploymentSlot::SlotB => self.base_port_b,
        }
    }

    /// Pre-stages candidate payload bytes on the standby slot.
    pub fn pre_stage_candidate(
        &mut self,
        candidate_digest: String,
    ) -> Result<DeploymentSlot, SlotSupervisorError> {
        let target_slot = self.standby_slot();
        match target_slot {
            DeploymentSlot::SlotA => {
                self.slot_a = SlotState::PreStaged {
                    artifact_digest: candidate_digest,
                };
            }
            DeploymentSlot::SlotB => {
                self.slot_b = SlotState::PreStaged {
                    artifact_digest: candidate_digest,
                };
            }
        }
        Ok(target_slot)
    }

    /// Marks the candidate as booted and verified healthy on its isolated port.
    pub fn mark_candidate_ready(
        &mut self,
        candidate_digest: String,
    ) -> Result<DeploymentSlot, SlotSupervisorError> {
        let target_slot = self.standby_slot();
        let port = self.get_slot_port(target_slot);
        match target_slot {
            DeploymentSlot::SlotA => {
                self.slot_a = SlotState::Standby {
                    artifact_digest: candidate_digest,
                    port,
                };
            }
            DeploymentSlot::SlotB => {
                self.slot_b = SlotState::Standby {
                    artifact_digest: candidate_digest,
                    port,
                };
            }
        }
        Ok(target_slot)
    }

    /// Performs an atomic traffic cutover: candidate slot becomes Serving,
    /// predecessor slot becomes Standby.
    pub fn commit_traffic_switch(&mut self) -> Result<DeploymentSlot, SlotSupervisorError> {
        let candidate_slot = self.standby_slot();
        let predecessor_slot = self.active_slot;

        let candidate_state = self.get_slot_state(candidate_slot).clone();
        let predecessor_state = self.get_slot_state(predecessor_slot).clone();

        let (candidate_digest, candidate_port) = match candidate_state {
            SlotState::Standby {
                artifact_digest,
                port,
            } => (artifact_digest, port),
            other => return Err(SlotSupervisorError::CandidateNotReady(other)),
        };

        // Transition candidate to Serving
        match candidate_slot {
            DeploymentSlot::SlotA => {
                self.slot_a = SlotState::Serving {
                    artifact_digest: candidate_digest,
                    port: candidate_port,
                };
            }
            DeploymentSlot::SlotB => {
                self.slot_b = SlotState::Serving {
                    artifact_digest: candidate_digest,
                    port: candidate_port,
                };
            }
        }

        // Transition predecessor to Standby (hot-standby for instant rollback)
        if let SlotState::Serving {
            artifact_digest,
            port,
        } = predecessor_state
        {
            match predecessor_slot {
                DeploymentSlot::SlotA => {
                    self.slot_a = SlotState::Standby {
                        artifact_digest,
                        port,
                    };
                }
                DeploymentSlot::SlotB => {
                    self.slot_b = SlotState::Standby {
                        artifact_digest,
                        port,
                    };
                }
            }
        }

        self.active_slot = candidate_slot;
        Ok(self.active_slot)
    }

    /// Instantly reverts traffic back to the standby predecessor slot.
    pub fn revert_to_predecessor(&mut self) -> Result<DeploymentSlot, SlotSupervisorError> {
        let predecessor_slot = self.standby_slot();
        let failed_candidate_slot = self.active_slot;

        let predecessor_state = self.get_slot_state(predecessor_slot).clone();
        let failed_candidate_state = self.get_slot_state(failed_candidate_slot).clone();

        let (predecessor_digest, predecessor_port) = match predecessor_state {
            SlotState::Standby {
                artifact_digest,
                port,
            } => (artifact_digest, port),
            other => return Err(SlotSupervisorError::PredecessorNotAvailable(other)),
        };

        // Promote predecessor back to Serving
        match predecessor_slot {
            DeploymentSlot::SlotA => {
                self.slot_a = SlotState::Serving {
                    artifact_digest: predecessor_digest,
                    port: predecessor_port,
                };
            }
            DeploymentSlot::SlotB => {
                self.slot_b = SlotState::Serving {
                    artifact_digest: predecessor_digest,
                    port: predecessor_port,
                };
            }
        }

        // Demote failed candidate to Empty or Standby
        if let SlotState::Serving {
            artifact_digest,
            port,
        } = failed_candidate_state
        {
            match failed_candidate_slot {
                DeploymentSlot::SlotA => {
                    self.slot_a = SlotState::Standby {
                        artifact_digest,
                        port,
                    };
                }
                DeploymentSlot::SlotB => {
                    self.slot_b = SlotState::Standby {
                        artifact_digest,
                        port,
                    };
                }
            }
        }

        self.active_slot = predecessor_slot;
        Ok(self.active_slot)
    }
}

/// Evidence receipt when candidate fails before traffic switch.
/// Consumes exactly 0 recovery attempts, and predecessor capacity is 100% retained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreSwitchFailureReceipt {
    pub candidate_digest: String,
    pub candidate_slot: DeploymentSlot,
    pub recovery_attempts_consumed: u32,
    pub predecessor_capacity_retained: bool,
    pub active_serving_port: u16,
}

/// Evidence receipt upon atomic router/proxy traffic cutover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficSwitchReceipt {
    pub active_slot: DeploymentSlot,
    pub active_serving_port: u16,
    pub predecessor_slot: DeploymentSlot,
    pub predecessor_standby_port: u16,
}

/// Evidence receipt upon post-switch candidate failure and rollback to predecessor.
/// Consumes exactly 1 recovery attempt, returning traffic to the pre-staged predecessor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostSwitchRecoveryReceipt {
    pub active_slot: DeploymentSlot,
    pub active_serving_port: u16,
    pub recovery_attempts_consumed: u32,
    pub demoted_candidate_slot: DeploymentSlot,
}

/// Single-node side-by-side HTTP/SSR switching coordinator that controls local proxy/router routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpSsrSwitchingCoordinator {
    supervisor: SlotSupervisor,
    proxy_target_port: u16,
    recovery_attempts_consumed: u32,
}

impl HttpSsrSwitchingCoordinator {
    pub fn new(initial_digest: String, base_port_a: u16, base_port_b: u16) -> Self {
        let supervisor = SlotSupervisor::new(initial_digest, base_port_a, base_port_b);
        Self {
            proxy_target_port: base_port_a,
            supervisor,
            recovery_attempts_consumed: 0,
        }
    }

    pub fn supervisor(&self) -> &SlotSupervisor {
        &self.supervisor
    }

    pub fn proxy_target_port(&self) -> u16 {
        self.proxy_target_port
    }

    pub fn recovery_attempts_consumed(&self) -> u32 {
        self.recovery_attempts_consumed
    }

    /// Pre-stages candidate on standby slot.
    pub fn pre_stage_candidate(
        &mut self,
        candidate_digest: String,
    ) -> Result<DeploymentSlot, SlotSupervisorError> {
        self.supervisor.pre_stage_candidate(candidate_digest)
    }

    /// Boots candidate on isolated standby port and verifies its health without serving live traffic.
    pub fn mark_candidate_ready(
        &mut self,
        candidate_digest: String,
    ) -> Result<DeploymentSlot, SlotSupervisorError> {
        self.supervisor.mark_candidate_ready(candidate_digest)
    }

    /// Records candidate startup or health check failure on standby slot BEFORE traffic switch.
    /// Crucial contract: consumes NO recovery attempt (`recovery_attempts_consumed == 0`),
    /// proxy target port is untouched, and predecessor capacity continues serving uninterrupted.
    pub fn record_pre_switch_failure(
        &mut self,
        candidate_digest: &str,
    ) -> Result<PreSwitchFailureReceipt, SlotSupervisorError> {
        let standby_slot = self.supervisor.standby_slot();
        match self.supervisor.get_slot_state(standby_slot) {
            SlotState::PreStaged { artifact_digest } | SlotState::Standby { artifact_digest, .. } => {
                if artifact_digest != candidate_digest {
                    return Err(SlotSupervisorError::InvalidTransition(format!(
                        "Digest mismatch on standby slot: expected {candidate_digest}, got {artifact_digest}"
                    )));
                }
            }
            SlotState::Empty => {
                return Err(SlotSupervisorError::InvalidTransition(
                    "No candidate in standby slot to fail".to_string(),
                ));
            }
            SlotState::Serving { .. } => {
                return Err(SlotSupervisorError::InvalidTransition(
                    "Standby slot cannot be in Serving state".to_string(),
                ));
            }
        }

        // Demote standby slot back to Empty
        match standby_slot {
            DeploymentSlot::SlotA => self.supervisor.slot_a = SlotState::Empty,
            DeploymentSlot::SlotB => self.supervisor.slot_b = SlotState::Empty,
        }

        Ok(PreSwitchFailureReceipt {
            candidate_digest: candidate_digest.to_string(),
            candidate_slot: standby_slot,
            recovery_attempts_consumed: 0,
            predecessor_capacity_retained: true,
            active_serving_port: self.proxy_target_port,
        })
    }

    /// Atomically switches proxy/router traffic to verified candidate on standby port.
    /// Predecessor slot becomes hot-standby.
    pub fn commit_traffic_switch(&mut self) -> Result<TrafficSwitchReceipt, SlotSupervisorError> {
        let predecessor_slot = self.supervisor.active_slot();
        let predecessor_port = self.supervisor.get_slot_port(predecessor_slot);

        let candidate_slot = self.supervisor.commit_traffic_switch()?;
        let active_serving_port = self.supervisor.get_slot_port(candidate_slot);

        self.proxy_target_port = active_serving_port;

        Ok(TrafficSwitchReceipt {
            active_slot: candidate_slot,
            active_serving_port,
            predecessor_slot,
            predecessor_standby_port: predecessor_port,
        })
    }

    /// Reverts proxy/router traffic back to hot-standby predecessor upon post-switch failure.
    /// Consumes exactly 1 recovery attempt (`recovery_attempts_consumed <= 1`).
    pub fn trigger_post_switch_recovery(&mut self) -> Result<PostSwitchRecoveryReceipt, SlotSupervisorError> {
        if self.recovery_attempts_consumed >= 1 {
            return Err(SlotSupervisorError::InvalidTransition(
                "Predecessor recovery already exhausted (max 1 attempt)".to_string(),
            ));
        }

        let failed_candidate_slot = self.supervisor.active_slot();
        let predecessor_slot = self.supervisor.revert_to_predecessor()?;
        let active_serving_port = self.supervisor.get_slot_port(predecessor_slot);

        self.proxy_target_port = active_serving_port;
        self.recovery_attempts_consumed += 1;

        Ok(PostSwitchRecoveryReceipt {
            active_slot: predecessor_slot,
            active_serving_port,
            recovery_attempts_consumed: self.recovery_attempts_consumed,
            demoted_candidate_slot: failed_candidate_slot,
        })
    }
}

/// Fenced worker generation handoff coordinator ensuring no concurrent job execution across generations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FencedWorkerGenerationCoordinator {
    role: String,
    active_generation: u64,
    fenced_generation: Option<u64>,
    candidate_generation: Option<u64>,
    claims_permitted: bool,
    recovery_attempts_consumed: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerFenceReceipt {
    pub role: String,
    pub fenced_generation: u64,
    pub claims_permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerHandoffReceipt {
    pub role: String,
    pub previous_generation: u64,
    pub active_generation: u64,
    pub claims_permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRollbackReceipt {
    pub role: String,
    pub revoked_generation: u64,
    pub restored_generation: u64,
    pub claims_permitted: bool,
    pub recovery_attempts_consumed: u32,
}

impl FencedWorkerGenerationCoordinator {
    pub fn new(role: impl Into<String>, initial_generation: u64) -> Self {
        Self {
            role: role.into(),
            active_generation: initial_generation,
            fenced_generation: None,
            candidate_generation: None,
            claims_permitted: true,
            recovery_attempts_consumed: 0,
        }
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn active_generation(&self) -> u64 {
        self.active_generation
    }

    pub fn claims_permitted(&self) -> bool {
        self.claims_permitted
    }

    pub fn recovery_attempts_consumed(&self) -> u32 {
        self.recovery_attempts_consumed
    }

    /// Prepares candidate generation. Candidate cannot claim work yet.
    pub fn prepare_candidate(&mut self, candidate_generation: u64) -> Result<(), SlotSupervisorError> {
        if candidate_generation <= self.active_generation {
            return Err(SlotSupervisorError::InvalidTransition(format!(
                "Candidate generation {candidate_generation} must be greater than active generation {}",
                self.active_generation
            )));
        }
        self.candidate_generation = Some(candidate_generation);
        Ok(())
    }

    /// Checkpoint and fence active generation: stops new claims before candidate handoff.
    pub fn fence_active_generation(&mut self) -> Result<WorkerFenceReceipt, SlotSupervisorError> {
        if self.candidate_generation.is_none() {
            return Err(SlotSupervisorError::InvalidTransition(
                "Cannot fence active generation without a prepared candidate generation".to_string(),
            ));
        }
        self.fenced_generation = Some(self.active_generation);
        self.claims_permitted = false;

        Ok(WorkerFenceReceipt {
            role: self.role.clone(),
            fenced_generation: self.active_generation,
            claims_permitted: false,
        })
    }

    /// Authorizes candidate generation to claim work, completing the fenced handoff.
    pub fn authorize_candidate_generation(&mut self) -> Result<WorkerHandoffReceipt, SlotSupervisorError> {
        let candidate = self.candidate_generation.ok_or_else(|| {
            SlotSupervisorError::InvalidTransition("No candidate generation prepared".to_string())
        })?;

        if self.fenced_generation != Some(self.active_generation) {
            return Err(SlotSupervisorError::InvalidTransition(
                "Active generation must be fenced before candidate authorization".to_string(),
            ));
        }

        let previous = self.active_generation;
        self.active_generation = candidate;
        self.candidate_generation = None;
        self.fenced_generation = None;
        self.claims_permitted = true;

        Ok(WorkerHandoffReceipt {
            role: self.role.clone(),
            previous_generation: previous,
            active_generation: self.active_generation,
            claims_permitted: true,
        })
    }

    /// Symmetric rollback: fences failed generation and restores predecessor generation.
    pub fn rollback_generation(&mut self, predecessor_generation: u64) -> Result<WorkerRollbackReceipt, SlotSupervisorError> {
        if self.recovery_attempts_consumed >= 1 {
            return Err(SlotSupervisorError::InvalidTransition(
                "Worker generation recovery already exhausted (max 1 attempt)".to_string(),
            ));
        }
        if predecessor_generation >= self.active_generation {
            return Err(SlotSupervisorError::InvalidTransition(format!(
                "Predecessor generation {predecessor_generation} must be less than active generation {}",
                self.active_generation
            )));
        }

        let revoked = self.active_generation;
        self.active_generation = predecessor_generation;
        self.candidate_generation = None;
        self.fenced_generation = None;
        self.claims_permitted = true;
        self.recovery_attempts_consumed += 1;

        Ok(WorkerRollbackReceipt {
            role: self.role.clone(),
            revoked_generation: revoked,
            restored_generation: predecessor_generation,
            claims_permitted: true,
            recovery_attempts_consumed: self.recovery_attempts_consumed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_side_by_side_slot_lifecycle() {
        // 1. Initial State: Slot A is Serving release N
        let mut supervisor = SlotSupervisor::new("sha256:release_n".to_string(), 8081, 8082);
        assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotA);
        assert_eq!(supervisor.standby_slot(), DeploymentSlot::SlotB);
        assert!(matches!(
            supervisor.active_state(),
            SlotState::Serving { port: 8081, .. }
        ));
        assert_eq!(supervisor.standby_state(), &SlotState::Empty);

        // 2. Pre-stage candidate N+1 on Slot B
        let slot = supervisor
            .pre_stage_candidate("sha256:release_n_plus_1".to_string())
            .unwrap();
        assert_eq!(slot, DeploymentSlot::SlotB);
        assert!(matches!(
            supervisor.standby_state(),
            SlotState::PreStaged { .. }
        ));

        // 3. Mark candidate ready after passing health checks
        supervisor
            .mark_candidate_ready("sha256:release_n_plus_1".to_string())
            .unwrap();
        assert!(matches!(
            supervisor.standby_state(),
            SlotState::Standby { port: 8082, .. }
        ));

        // 4. Commit traffic cutover
        let active = supervisor.commit_traffic_switch().unwrap();
        assert_eq!(active, DeploymentSlot::SlotB);
        assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotB);
        assert!(matches!(
            supervisor.active_state(),
            SlotState::Serving { port: 8082, .. }
        ));
        // Predecessor is preserved on Slot A in standby!
        assert!(matches!(
            supervisor.standby_state(),
            SlotState::Standby { port: 8081, .. }
        ));

        // 5. Simulate incident and instant rollback to predecessor
        let reverted_slot = supervisor.revert_to_predecessor().unwrap();
        assert_eq!(reverted_slot, DeploymentSlot::SlotA);
        assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotA);
        assert!(matches!(
            supervisor.active_state(),
            SlotState::Serving { port: 8081, .. }
        ));
    }

    #[test]
    fn test_cutover_fails_if_candidate_not_ready() {
        let mut supervisor = SlotSupervisor::new("sha256:release_n".to_string(), 8081, 8082);
        supervisor
            .pre_stage_candidate("sha256:release_n_plus_1".to_string())
            .unwrap();

        // Trying to switch while in PreStaged (not yet verified ready) must fail
        let err = supervisor.commit_traffic_switch();
        assert!(matches!(
            err,
            Err(SlotSupervisorError::CandidateNotReady(
                SlotState::PreStaged { .. }
            ))
        ));
    }
}
