use rustok_artifact_node_agent::{
    DeploymentSlot, FencedWorkerGenerationCoordinator, HttpSsrSwitchingCoordinator, NodeWatchdog,
    SlotState, SlotSupervisor, WatchdogConfig, WatchdogStatus,
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

#[test]
fn test_side_by_side_pre_switch_failure_consumes_zero_attempts() {
    let mut coordinator = HttpSsrSwitchingCoordinator::new(
        "sha256:predecessor_stable".to_string(),
        8081,
        8082,
    );

    assert_eq!(coordinator.proxy_target_port(), 8081);
    assert_eq!(coordinator.recovery_attempts_consumed(), 0);

    // 1. Pre-stage candidate on standby port (8082)
    let candidate_digest = "sha256:candidate_broken_build".to_string();
    let slot = coordinator
        .pre_stage_candidate(candidate_digest.clone())
        .unwrap();
    assert_eq!(slot, DeploymentSlot::SlotB);

    // 2. Candidate fails before traffic switch (e.g. panic on startup or health probe failure)
    let failure_receipt = coordinator
        .record_pre_switch_failure(&candidate_digest)
        .expect("recording pre-switch failure should succeed");

    // CRITICAL: Consumes 0 recovery attempts, predecessor capacity is 100% retained!
    assert_eq!(failure_receipt.recovery_attempts_consumed, 0);
    assert!(failure_receipt.predecessor_capacity_retained);
    assert_eq!(failure_receipt.active_serving_port, 8081);
    assert_eq!(coordinator.proxy_target_port(), 8081);
    assert_eq!(coordinator.recovery_attempts_consumed(), 0);

    // Standby slot is demoted to Empty
    assert_eq!(coordinator.supervisor().standby_state(), &SlotState::Empty);
    // Active slot is still serving predecessor on 8081
    assert!(matches!(
        coordinator.supervisor().active_state(),
        SlotState::Serving { port: 8081, .. }
    ));
}

#[test]
fn test_side_by_side_traffic_switch_and_post_switch_recovery() {
    let mut coordinator = HttpSsrSwitchingCoordinator::new(
        "sha256:predecessor_v1".to_string(),
        8081,
        8082,
    );

    let candidate_digest = "sha256:candidate_v2".to_string();

    // 1. Pre-stage and mark candidate ready on standby slot B
    coordinator.pre_stage_candidate(candidate_digest.clone()).unwrap();
    coordinator.mark_candidate_ready(candidate_digest).unwrap();

    // 2. Commit atomic traffic switch
    let switch_receipt = coordinator
        .commit_traffic_switch()
        .expect("traffic switch to verified candidate should succeed");

    assert_eq!(switch_receipt.active_slot, DeploymentSlot::SlotB);
    assert_eq!(switch_receipt.active_serving_port, 8082);
    assert_eq!(switch_receipt.predecessor_slot, DeploymentSlot::SlotA);
    assert_eq!(switch_receipt.predecessor_standby_port, 8081);

    // Proxy target port points to candidate
    assert_eq!(coordinator.proxy_target_port(), 8082);
    // Zero recovery attempts consumed on happy-path switch
    assert_eq!(coordinator.recovery_attempts_consumed(), 0);

    // 3. Post-switch failure requires rollback: reverts proxy to predecessor on 8081
    let recovery_receipt = coordinator
        .trigger_post_switch_recovery()
        .expect("reverting to hot-standby predecessor should succeed");

    assert_eq!(recovery_receipt.active_slot, DeploymentSlot::SlotA);
    assert_eq!(recovery_receipt.active_serving_port, 8081);
    assert_eq!(recovery_receipt.recovery_attempts_consumed, 1);
    assert_eq!(coordinator.proxy_target_port(), 8081);
    assert_eq!(coordinator.recovery_attempts_consumed(), 1);

    // 4. Second recovery attempt fails closed (max 1 attempt)
    let second_err = coordinator.trigger_post_switch_recovery();
    assert!(second_err.is_err());
}

#[test]
fn test_fenced_worker_generation_handoff_and_rollback() {
    let mut coordinator = FencedWorkerGenerationCoordinator::new("outbox_worker", 1);
    assert_eq!(coordinator.active_generation(), 1);
    assert!(coordinator.claims_permitted());
    assert_eq!(coordinator.recovery_attempts_consumed(), 0);

    // 1. Prepare candidate generation 2 (cannot claim work yet)
    coordinator.prepare_candidate(2).unwrap();

    // 2. Fence active generation: stops new claims before candidate handoff
    let fence_receipt = coordinator
        .fence_active_generation()
        .expect("fencing active generation should succeed");
    assert_eq!(fence_receipt.fenced_generation, 1);
    assert!(!fence_receipt.claims_permitted);
    assert!(!coordinator.claims_permitted());

    // 3. Authorize candidate generation 2: completes fenced handoff
    let handoff_receipt = coordinator
        .authorize_candidate_generation()
        .expect("authorizing candidate generation should succeed");
    assert_eq!(handoff_receipt.previous_generation, 1);
    assert_eq!(handoff_receipt.active_generation, 2);
    assert!(handoff_receipt.claims_permitted);
    assert_eq!(coordinator.active_generation(), 2);
    assert!(coordinator.claims_permitted());

    // 4. Symmetric rollback to predecessor generation 1: restores predecessor without duplicate claims
    let rollback_receipt = coordinator
        .rollback_generation(1)
        .expect("symmetric rollback to predecessor generation should succeed");
    assert_eq!(rollback_receipt.revoked_generation, 2);
    assert_eq!(rollback_receipt.restored_generation, 1);
    assert_eq!(rollback_receipt.recovery_attempts_consumed, 1);
    assert_eq!(coordinator.active_generation(), 1);
    assert_eq!(coordinator.recovery_attempts_consumed(), 1);

    // 5. Subsequent rollback attempt fails closed (max 1 attempt)
    let second_rollback = coordinator.rollback_generation(0);
    assert!(second_rollback.is_err());
}

