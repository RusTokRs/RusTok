//! Integration tests for multi-node canary and wave rollouts with dual pre-staging barrier,
//! sequential wave mutation, predecessor capacity retention, and bounded wave rollback.

use rustok_modules::{
    WaveAssignmentPhase, WaveCohort, WaveNodeAssignment, WaveRolloutCoordinator,
    WaveRolloutError, WaveRolloutState,
};
use uuid::Uuid;

fn sample_assignment(node_id: &str, role: &str) -> WaveNodeAssignment {
    WaveNodeAssignment {
        node_id: node_id.to_string(),
        role: role.to_string(),
        candidate_digest: "sha256:candidate_bundle_digest_11111111111111111111111111111111".to_string(),
        predecessor_digest: Some("sha256:predecessor_bundle_digest_00000000000000000000000000000000".to_string()),
        pre_staged_candidate: false,
        pre_staged_predecessor: false,
        phase: WaveAssignmentPhase::PreStaging,
    }
}

fn sample_three_wave_plan() -> (Uuid, WaveRolloutCoordinator) {
    let rollout_id = Uuid::new_v4();
    let cohorts = vec![
        WaveCohort {
            wave_index: 0,
            name: "canary".to_string(),
            assignments: vec![sample_assignment("node-canary-1", "monolith")],
        },
        WaveCohort {
            wave_index: 1,
            name: "wave-1".to_string(),
            assignments: vec![
                sample_assignment("node-prod-1", "monolith"),
                sample_assignment("node-prod-2", "monolith"),
            ],
        },
        WaveCohort {
            wave_index: 2,
            name: "wave-2".to_string(),
            assignments: vec![
                sample_assignment("node-prod-3", "monolith"),
                sample_assignment("node-prod-4", "monolith"),
            ],
        },
    ];

    let coordinator = WaveRolloutCoordinator::new(rollout_id, cohorts);
    (rollout_id, coordinator)
}

#[test]
fn test_dual_pre_staging_barrier_enforcement() {
    let (_id, mut coordinator) = sample_three_wave_plan();

    // 1. Initial State: cannot start mutation because no nodes have pre-staged
    let err = coordinator.start_wave_mutation(0).unwrap_err();
    assert_eq!(
        err,
        WaveRolloutError::PreStagingBarrierNotMet("node-canary-1".to_string())
    );

    // 2. Canary node pre-stages candidate ONLY (missing predecessor)
    coordinator.report_node_pre_staged("node-canary-1", true, false);
    let err = coordinator.start_wave_mutation(0).unwrap_err();
    assert_eq!(
        err,
        WaveRolloutError::PreStagingBarrierNotMet("node-canary-1".to_string())
    );

    // 3. Canary pre-stages both, but Wave 1 & 2 are still missing
    coordinator.report_node_pre_staged("node-canary-1", true, true);
    let err = coordinator.start_wave_mutation(0).unwrap_err();
    assert_eq!(
        err,
        WaveRolloutError::PreStagingBarrierNotMet("node-prod-1".to_string())
    );

    // 4. Pre-stage remaining nodes
    coordinator.report_node_pre_staged("node-prod-1", true, true);
    coordinator.report_node_pre_staged("node-prod-2", true, true);
    coordinator.report_node_pre_staged("node-prod-3", true, true);
    coordinator.report_node_pre_staged("node-prod-4", true, true);

    // Barrier passed! State moves to PreStaged
    assert_eq!(coordinator.state, WaveRolloutState::PreStaged);
    assert!(coordinator.verify_dual_pre_staging_barrier().is_ok());
}

#[test]
fn test_sequential_wave_mutation_and_capacity_retention() {
    let (_id, mut coordinator) = sample_three_wave_plan();

    // Pre-stage all nodes across all waves
    for node in ["node-canary-1", "node-prod-1", "node-prod-2", "node-prod-3", "node-prod-4"] {
        coordinator.report_node_pre_staged(node, true, true);
    }
    assert_eq!(coordinator.state, WaveRolloutState::PreStaged);

    // 1. Cannot jump ahead to Wave 1 without Canary
    let skip_err = coordinator.start_wave_mutation(1).unwrap_err();
    assert_eq!(skip_err, WaveRolloutError::PreviousWaveNotVerified(0));

    // 2. Start Canary (Wave 0)
    coordinator.start_wave_mutation(0).expect("starting wave 0 succeeds");
    assert_eq!(coordinator.state, WaveRolloutState::MutatingWave(0));

    // Crucial check: Wave 1 & Wave 2 remain PreStaged (untouched, running predecessor capacity)
    assert_eq!(coordinator.cohorts[0].assignments[0].phase, WaveAssignmentPhase::Mutating);
    assert_eq!(coordinator.cohorts[1].assignments[0].phase, WaveAssignmentPhase::PreStaged);
    assert_eq!(coordinator.cohorts[2].assignments[0].phase, WaveAssignmentPhase::PreStaged);

    // 3. Verify Canary
    coordinator.verify_wave(0).expect("verifying wave 0 succeeds");
    assert_eq!(coordinator.state, WaveRolloutState::VerifiedWave(0));
    assert_eq!(coordinator.cohorts[0].assignments[0].phase, WaveAssignmentPhase::Verified);

    // 4. Start Wave 1
    coordinator.start_wave_mutation(1).expect("starting wave 1 succeeds");
    assert_eq!(coordinator.state, WaveRolloutState::MutatingWave(1));
    assert_eq!(coordinator.cohorts[1].assignments[0].phase, WaveAssignmentPhase::Mutating);
    assert_eq!(coordinator.cohorts[2].assignments[0].phase, WaveAssignmentPhase::PreStaged); // Wave 2 still untouched!

    // 5. Verify Wave 1
    coordinator.verify_wave(1).expect("verifying wave 1 succeeds");
    assert_eq!(coordinator.state, WaveRolloutState::VerifiedWave(1));

    // 6. Start Wave 2 (Final Wave)
    coordinator.start_wave_mutation(2).expect("starting wave 2 succeeds");
    assert_eq!(coordinator.state, WaveRolloutState::MutatingWave(2));

    // 7. Verify Wave 2 -> Converged!
    coordinator.verify_wave(2).expect("verifying final wave succeeds");
    assert_eq!(coordinator.state, WaveRolloutState::Converged);
    assert_eq!(coordinator.cohorts[2].assignments[0].phase, WaveAssignmentPhase::Verified);
}

#[test]
fn test_wave_failure_and_rollback_with_predecessor_retention() {
    let (rollout_id, mut coordinator) = sample_three_wave_plan();

    // Pre-stage all nodes
    for node in ["node-canary-1", "node-prod-1", "node-prod-2", "node-prod-3", "node-prod-4"] {
        coordinator.report_node_pre_staged(node, true, true);
    }

    // 1. Canary passes
    coordinator.start_wave_mutation(0).unwrap();
    coordinator.verify_wave(0).unwrap();

    // 2. Wave 1 starts mutation and encounters failure
    coordinator.start_wave_mutation(1).unwrap();

    // 3. Trigger wave rollback on failed wave index 1
    let receipt = coordinator
        .rollback_all_mutated_waves(1)
        .expect("wave rollback succeeds");

    assert_eq!(receipt.rollout_id, rollout_id);
    assert_eq!(receipt.reverted_cohort_count, 2); // Canary + Wave 1 reverted
    assert_eq!(receipt.untouched_cohort_count, 1); // Wave 2 untouched!
    assert_eq!(receipt.recovery_attempts_consumed, 1);

    assert_eq!(coordinator.state, WaveRolloutState::RolledBack);

    // Mutated waves are RolledBack
    assert_eq!(coordinator.cohorts[0].assignments[0].phase, WaveAssignmentPhase::RolledBack);
    assert_eq!(coordinator.cohorts[1].assignments[0].phase, WaveAssignmentPhase::RolledBack);
    // Untouched wave remains PreStaged
    assert_eq!(coordinator.cohorts[2].assignments[0].phase, WaveAssignmentPhase::PreStaged);

    // 4. Second rollback fails (recovery already exhausted, max 1 attempt)
    let second_err = coordinator.rollback_all_mutated_waves(1).unwrap_err();
    assert_eq!(second_err, WaveRolloutError::RecoveryExhausted);
}
