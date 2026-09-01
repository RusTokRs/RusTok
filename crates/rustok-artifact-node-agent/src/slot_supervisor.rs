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
