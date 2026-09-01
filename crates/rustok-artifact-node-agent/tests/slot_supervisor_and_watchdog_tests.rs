use rustok_artifact_node_agent::{
    DeploymentSlot, NodeWatchdog, SlotState, SlotSupervisor, WatchdogConfig, WatchdogStatus,
};
use std::time::Duration;
use uuid::Uuid;

#[test]
fn test_end_to_end_side_by_side_rollout_with_health_promotion() {
    let operation_id = Uuid::new_v4();
    let node_id = "node-alpha-1".to_string();

    // 1. Initial State: Node running serving release N on Slot A (port 8081)
    let mut supervisor = SlotSupervisor::new("sha256:release_v1_digest".to_string(), 8081, 8082);
    assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotA);
    assert!(matches!(
        supervisor.active_state(),
        SlotState::Serving { port: 8081, .. }
    ));

    // 2. Pre-stage candidate N+1 on Slot B
    let candidate_digest = "sha256:release_v2_candidate_digest".to_string();
    let candidate_slot = supervisor
        .pre_stage_candidate(candidate_digest.clone())
        .expect("pre-staging candidate on standby slot should succeed");
    assert_eq!(candidate_slot, DeploymentSlot::SlotB);

    // 3. Mark candidate ready after passing isolated port health checks
    supervisor
        .mark_candidate_ready(candidate_digest)
        .expect("marking candidate ready should succeed");

    // 4. Commit traffic cutover
    let active_slot = supervisor
        .commit_traffic_switch()
        .expect("traffic cutover to verified candidate should succeed");
    assert_eq!(active_slot, DeploymentSlot::SlotB);
    assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotB);
    // Crucial safety guarantee: Predecessor remains in standby on Slot A!
    assert!(matches!(
        supervisor.standby_state(),
        SlotState::Standby { port: 8081, .. }
    ));

    // 5. Watchdog monitors observation window
    let mut watchdog = NodeWatchdog::new(
        WatchdogConfig {
            observation_window: Duration::from_secs(30),
            probe_interval: Duration::from_secs(5),
            probe_timeout: Duration::from_secs(1),
            max_consecutive_failures: 2,
        },
        operation_id,
        node_id,
    );

    // Initial probe
    let status_1 = watchdog.record_probe_success(Duration::from_secs(10));
    assert!(matches!(status_1, WatchdogStatus::Observing { .. }));

    // Observation window completes successfully
    let status_final = watchdog.record_probe_success(Duration::from_secs(30));
    assert!(matches!(
        status_final,
        WatchdogStatus::PromotedHealthy { .. }
    ));
    assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotB);
}

#[test]
fn test_end_to_end_watchdog_auto_rollback_on_candidate_crash() {
    let operation_id = Uuid::new_v4();
    let node_id = "node-beta-2".to_string();

    // 1. Initial State: Node serving release N on Slot A
    let mut supervisor = SlotSupervisor::new("sha256:release_n_stable".to_string(), 8081, 8082);

    // 2. Candidate N+1 deployed and traffic switched to Slot B
    supervisor
        .pre_stage_candidate("sha256:release_n_plus_1_faulty".to_string())
        .unwrap();
    supervisor
        .mark_candidate_ready("sha256:release_n_plus_1_faulty".to_string())
        .unwrap();
    supervisor.commit_traffic_switch().unwrap();
    assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotB);

    // 3. Initialize outside-candidate watchdog
    let mut watchdog = NodeWatchdog::new(WatchdogConfig::default(), operation_id, node_id.clone());

    // 4. Candidate panics / crashes (exit code 101 or SIGSEGV)
    let recovery_receipt = watchdog
        .trigger_immediate_crash_recovery(
            &mut supervisor,
            Some(101),
            "panic at src/server.rs:42: unhandled null reference".to_string(),
        )
        .expect("immediate crash recovery should execute without failure");

    // 5. Verify instant fallback to Slot A!
    assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotA);
    assert_eq!(recovery_receipt.recovered_slot, DeploymentSlot::SlotA);
    assert_eq!(
        recovery_receipt.failed_artifact_digest,
        "sha256:release_n_plus_1_faulty"
    );
    assert_eq!(
        recovery_receipt.predecessor_artifact_digest,
        "sha256:release_n_stable"
    );
    assert_eq!(recovery_receipt.node_id, node_id);
    assert!(recovery_receipt.failure_reason.contains("panic"));
}
