use chrono::Utc;
use rustok_modules::{
    MigrationPreflightReceipt, ModuleTransitionCoordinator, ModuleTransitionState,
    RetentionHoldKind, RetentionHoldLedger, RetentionTarget, SecurityEpochRegistry,
    StartTransitionInput, TransitionCoordinatorError, UpdateMode,
};
use std::time::Duration;
use uuid::Uuid;

fn make_preflight(is_safe: bool) -> MigrationPreflightReceipt {
    MigrationPreflightReceipt {
        operation_id: Uuid::new_v4(),
        module_slug: "customer".to_string(),
        mode: if is_safe {
            UpdateMode::Automatic
        } else {
            UpdateMode::Maintenance
        },
        source_schema_digest: "sha256:source_hash".to_string(),
        target_schema_digest: "sha256:target_hash".to_string(),
        migration_plan_digest: "sha256:plan_hash".to_string(),
        is_additive_safe: is_safe,
        settings_guard_installed: is_safe,
        evaluated_at: Utc::now(),
        denial_reasons: if is_safe {
            vec![]
        } else {
            vec!["Non-concurrent table lock detected on 'customers'".to_string()]
        },
    }
}

#[test]
fn test_end_to_end_governed_rollout_convergence() {
    let security_registry = SecurityEpochRegistry::new();
    let operation_id = Uuid::new_v4();

    let input = StartTransitionInput {
        operation_id,
        module_slug: "customer".to_string(),
        tenant_id: None,
        predecessor_digest: Some("sha256:customer_v1".to_string()),
        candidate_digest: "sha256:customer_v2".to_string(),
        affected_nodes: vec!["node-1".to_string(), "node-2".to_string()],
        preflight_receipt: make_preflight(true),
        requested_mode: UpdateMode::Automatic,
    };

    let mut coordinator = ModuleTransitionCoordinator::start_transition(input, &security_registry)
        .expect("transition start should succeed with safe preflight");

    assert_eq!(coordinator.state(), &ModuleTransitionState::Fenced);
    assert_eq!(coordinator.checkpoint().fences.len(), 4); // release_unit + data_owner + 2 nodes

    // Pre-stage candidate
    coordinator
        .advance_to_prestaging(&security_registry)
        .expect("pre-staging should succeed");
    assert_eq!(coordinator.state(), &ModuleTransitionState::PreStaging);

    // Switch traffic & start observation
    coordinator
        .advance_to_activating(&security_registry, Duration::from_secs(60))
        .expect("activating should succeed");
    assert!(matches!(
        coordinator.state(),
        ModuleTransitionState::Observing { .. }
    ));

    // Finalize convergence
    coordinator
        .finalize_convergence(&security_registry)
        .expect("finalizing convergence should succeed");
    assert!(matches!(
        coordinator.state(),
        ModuleTransitionState::Converged { .. }
    ));
    assert!(coordinator.state().is_terminal());
}

#[test]
fn test_incident_recovery_and_zero_flapping_invariant() {
    let security_registry = SecurityEpochRegistry::new();
    let operation_id = Uuid::new_v4();

    let input = StartTransitionInput {
        operation_id,
        module_slug: "customer".to_string(),
        tenant_id: None,
        predecessor_digest: Some("sha256:customer_v1".to_string()),
        candidate_digest: "sha256:customer_v2_broken".to_string(),
        affected_nodes: vec!["node-1".to_string()],
        preflight_receipt: make_preflight(true),
        requested_mode: UpdateMode::Automatic,
    };

    let mut coordinator =
        ModuleTransitionCoordinator::start_transition(input, &security_registry).unwrap();
    coordinator
        .advance_to_prestaging(&security_registry)
        .unwrap();
    coordinator
        .advance_to_activating(&security_registry, Duration::from_secs(60))
        .unwrap();

    // 1. Candidate panics during observation window -> trigger automatic recovery
    coordinator
        .record_recovery_trigger("Watchdog: Process exited with status 137".to_string())
        .expect("initial recovery trigger must succeed");

    assert!(matches!(
        coordinator.state(),
        ModuleTransitionState::RecoveredToPredecessor { .. }
    ));
    assert_eq!(coordinator.checkpoint().recovery_attempt_count, 1);
    assert!(coordinator.state().is_terminal());

    // 2. A subsequent recovery attempt must be rejected (single-attempt limit)
    let second_attempt = coordinator.record_recovery_trigger("Subsequent incident".to_string());
    assert!(matches!(
        second_attempt,
        Err(TransitionCoordinatorError::OperationAlreadyTerminal(..))
    ));
}

#[test]
fn test_destructive_preflight_strictly_denies_automatic_start() {
    let security_registry = SecurityEpochRegistry::new();
    let operation_id = Uuid::new_v4();

    let input = StartTransitionInput {
        operation_id,
        module_slug: "customer".to_string(),
        tenant_id: None,
        predecessor_digest: Some("sha256:customer_v1".to_string()),
        candidate_digest: "sha256:customer_v2_destructive".to_string(),
        affected_nodes: vec!["node-1".to_string()],
        preflight_receipt: make_preflight(false), // Destructive DDL
        requested_mode: UpdateMode::Automatic,
    };

    let result = ModuleTransitionCoordinator::start_transition(input, &security_registry);
    assert!(matches!(
        result,
        Err(TransitionCoordinatorError::PreflightFailed(msg)) if msg.contains("Automatic update mode denied")
    ));
}

#[test]
fn test_predecessor_retention_revalidation_enforcement() {
    let security_registry = SecurityEpochRegistry::new();
    let operation_id = Uuid::new_v4();

    let input = StartTransitionInput {
        operation_id,
        module_slug: "customer".to_string(),
        tenant_id: None,
        predecessor_digest: Some("sha256:predecessor_safe".to_string()),
        candidate_digest: "sha256:candidate_v2".to_string(),
        affected_nodes: vec!["node-1".to_string()],
        preflight_receipt: make_preflight(true),
        requested_mode: UpdateMode::Automatic,
    };

    let mut coordinator =
        ModuleTransitionCoordinator::start_transition(input, &security_registry).unwrap();
    coordinator
        .advance_to_prestaging(&security_registry)
        .unwrap();

    // With empty ledger, activation must fail closed because predecessor is unprotected from GC
    let mut ledger = RetentionHoldLedger::new();
    let denied = coordinator.advance_to_activating_with_ledger(
        &security_registry,
        Some(&ledger),
        Duration::from_secs(60),
    );
    assert!(matches!(
        denied,
        Err(TransitionCoordinatorError::PredecessorRetentionMissing(digest)) if digest == "sha256:predecessor_safe"
    ));

    // After placing active rollout retention hold, activation succeeds
    ledger.place_hold(
        RetentionTarget::AdmittedPayloadCas {
            digest: "sha256:predecessor_safe".to_string(),
        },
        RetentionHoldKind::ActiveRolloutWindow {
            operation_id,
            expires_at: Utc::now() + chrono::Duration::seconds(300),
        },
    );

    coordinator
        .advance_to_activating_with_ledger(
            &security_registry,
            Some(&ledger),
            Duration::from_secs(60),
        )
        .expect("activation should succeed once predecessor retention hold is confirmed");
    assert!(matches!(
        coordinator.state(),
        ModuleTransitionState::Observing { .. }
    ));
}

