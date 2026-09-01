//! Outside-candidate node watchdog and automated recovery supervisor.
//!
//! Operates out-of-process to monitor candidate health throughout the active
//! observation window. Automatically reverts local traffic to the hot-standby
//! predecessor slot if the candidate crashes, panics, or fails consecutive health checks.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use crate::slot_supervisor::{DeploymentSlot, SlotState, SlotSupervisor, SlotSupervisorError};

/// Configuration for node-level candidate health observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchdogConfig {
    /// Total duration of the observation window before candidate is promoted healthy.
    pub observation_window: Duration,
    /// Interval between periodic health probes.
    pub probe_interval: Duration,
    /// Timeout for each individual probe.
    pub probe_timeout: Duration,
    /// Maximum allowable consecutive probe failures before triggering auto-recovery.
    pub max_consecutive_failures: u32,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            observation_window: Duration::from_secs(60),
            probe_interval: Duration::from_secs(5),
            probe_timeout: Duration::from_secs(2),
            max_consecutive_failures: 3,
        }
    }
}

/// Status of the active watchdog observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "details")]
pub enum WatchdogStatus {
    /// Active observation: candidate is serving and being observed.
    Observing { consecutive_failures: u32 },
    /// Observation window elapsed successfully without incident.
    PromotedHealthy { observation_elapsed: Duration },
    /// Incident occurred: watchdog reverted traffic to the predecessor slot.
    RecoveredToPredecessor {
        failure_reason: String,
        recovered_at: DateTime<Utc>,
        predecessor_slot: DeploymentSlot,
    },
}

/// Signed immutable receipt of an automatic node recovery event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchdogRecoveryReceipt {
    pub operation_id: Uuid,
    pub node_id: String,
    pub failed_artifact_digest: String,
    pub predecessor_artifact_digest: String,
    pub recovered_slot: DeploymentSlot,
    pub failure_reason: String,
    pub recovered_at: DateTime<Utc>,
}

/// Outside-candidate watchdog supervisor.
#[derive(Debug, Clone)]
pub struct NodeWatchdog {
    pub config: WatchdogConfig,
    pub operation_id: Uuid,
    pub node_id: String,
    status: WatchdogStatus,
    consecutive_failures: u32,
}

impl NodeWatchdog {
    pub fn new(config: WatchdogConfig, operation_id: Uuid, node_id: String) -> Self {
        Self {
            config,
            operation_id,
            node_id,
            status: WatchdogStatus::Observing {
                consecutive_failures: 0,
            },
            consecutive_failures: 0,
        }
    }

    pub fn status(&self) -> &WatchdogStatus {
        &self.status
    }

    /// Records a successful health probe.
    pub fn record_probe_success(&mut self, elapsed_observation: Duration) -> WatchdogStatus {
        self.consecutive_failures = 0;
        if elapsed_observation >= self.config.observation_window {
            self.status = WatchdogStatus::PromotedHealthy {
                observation_elapsed: elapsed_observation,
            };
        } else {
            self.status = WatchdogStatus::Observing {
                consecutive_failures: 0,
            };
        }
        self.status.clone()
    }

    /// Records a probe failure and automatically triggers fallback if threshold is reached.
    pub fn record_probe_failure(
        &mut self,
        supervisor: &mut SlotSupervisor,
        reason: String,
    ) -> Result<WatchdogStatus, SlotSupervisorError> {
        self.consecutive_failures += 1;

        if self.consecutive_failures >= self.config.max_consecutive_failures {
            let recovered_slot = supervisor.revert_to_predecessor()?;
            let recovered_at = Utc::now();
            self.status = WatchdogStatus::RecoveredToPredecessor {
                failure_reason: reason,
                recovered_at,
                predecessor_slot: recovered_slot,
            };
        } else {
            self.status = WatchdogStatus::Observing {
                consecutive_failures: self.consecutive_failures,
            };
        }

        Ok(self.status.clone())
    }

    /// Triggers immediate auto-recovery upon process crash, panic, or unhandled exit.
    pub fn trigger_immediate_crash_recovery(
        &mut self,
        supervisor: &mut SlotSupervisor,
        exit_code: Option<i32>,
        diagnostic_message: String,
    ) -> Result<WatchdogRecoveryReceipt, SlotSupervisorError> {
        let (failed_digest, predecessor_digest) = {
            let active_state = supervisor.active_state();
            let standby_state = supervisor.standby_state();

            let active_d = match active_state {
                SlotState::Serving {
                    artifact_digest, ..
                } => artifact_digest.clone(),
                _ => "unknown_candidate".to_string(),
            };
            let standby_d = match standby_state {
                SlotState::Standby {
                    artifact_digest, ..
                } => artifact_digest.clone(),
                _ => "unknown_predecessor".to_string(),
            };
            (active_d, standby_d)
        };

        let recovered_slot = supervisor.revert_to_predecessor()?;
        let recovered_at = Utc::now();

        let reason = format!(
            "Process crash (exit code: {:?}): {}",
            exit_code, diagnostic_message
        );

        self.status = WatchdogStatus::RecoveredToPredecessor {
            failure_reason: reason.clone(),
            recovered_at,
            predecessor_slot: recovered_slot,
        };

        Ok(WatchdogRecoveryReceipt {
            operation_id: self.operation_id,
            node_id: self.node_id.clone(),
            failed_artifact_digest: failed_digest,
            predecessor_artifact_digest: predecessor_digest,
            recovered_slot,
            failure_reason: reason,
            recovered_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watchdog_promotes_healthy_after_observation_window() {
        let mut supervisor = SlotSupervisor::new("sha256:predecessor".to_string(), 8081, 8082);
        supervisor
            .pre_stage_candidate("sha256:candidate".to_string())
            .unwrap();
        supervisor
            .mark_candidate_ready("sha256:candidate".to_string())
            .unwrap();
        supervisor.commit_traffic_switch().unwrap();

        let mut watchdog = NodeWatchdog::new(
            WatchdogConfig {
                observation_window: Duration::from_secs(30),
                probe_interval: Duration::from_secs(5),
                probe_timeout: Duration::from_secs(1),
                max_consecutive_failures: 2,
            },
            Uuid::new_v4(),
            "node-1".to_string(),
        );

        // Probing during window
        let status = watchdog.record_probe_success(Duration::from_secs(10));
        assert!(matches!(status, WatchdogStatus::Observing { .. }));

        // Window finished
        let final_status = watchdog.record_probe_success(Duration::from_secs(30));
        assert!(matches!(
            final_status,
            WatchdogStatus::PromotedHealthy { .. }
        ));
        assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotB);
    }

    #[test]
    fn test_watchdog_auto_reverts_on_consecutive_probe_failures() {
        let mut supervisor = SlotSupervisor::new("sha256:predecessor".to_string(), 8081, 8082);
        supervisor
            .pre_stage_candidate("sha256:candidate".to_string())
            .unwrap();
        supervisor
            .mark_candidate_ready("sha256:candidate".to_string())
            .unwrap();
        supervisor.commit_traffic_switch().unwrap();

        let mut watchdog = NodeWatchdog::new(
            WatchdogConfig {
                observation_window: Duration::from_secs(60),
                max_consecutive_failures: 2,
                ..Default::default()
            },
            Uuid::new_v4(),
            "node-1".to_string(),
        );

        // Failure 1 (still observing)
        let s1 = watchdog
            .record_probe_failure(&mut supervisor, "HTTP 500 error".to_string())
            .unwrap();
        assert!(matches!(
            s1,
            WatchdogStatus::Observing {
                consecutive_failures: 1
            }
        ));
        assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotB);

        // Failure 2 (reaches threshold -> instant revert to Slot A!)
        let s2 = watchdog
            .record_probe_failure(&mut supervisor, "HTTP 500 error".to_string())
            .unwrap();
        assert!(matches!(
            s2,
            WatchdogStatus::RecoveredToPredecessor {
                predecessor_slot: DeploymentSlot::SlotA,
                ..
            }
        ));
        // Active slot is reverted back to Slot A!
        assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotA);
    }

    #[test]
    fn test_watchdog_immediate_crash_recovery() {
        let mut supervisor = SlotSupervisor::new("sha256:predecessor".to_string(), 8081, 8082);
        supervisor
            .pre_stage_candidate("sha256:candidate".to_string())
            .unwrap();
        supervisor
            .mark_candidate_ready("sha256:candidate".to_string())
            .unwrap();
        supervisor.commit_traffic_switch().unwrap();

        let mut watchdog = NodeWatchdog::new(
            WatchdogConfig::default(),
            Uuid::new_v4(),
            "node-1".to_string(),
        );

        let receipt = watchdog
            .trigger_immediate_crash_recovery(
                &mut supervisor,
                Some(137), // OOM Kill / SIGKILL
                "Process killed by OOM killer".to_string(),
            )
            .unwrap();

        assert_eq!(receipt.recovered_slot, DeploymentSlot::SlotA);
        assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotA);
        assert!(receipt.failure_reason.contains("OOM killer"));
    }
}
