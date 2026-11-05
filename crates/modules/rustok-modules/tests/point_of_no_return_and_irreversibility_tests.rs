//! Integration tests for dynamic data-upgrade phase derivation, irreversibility,
//! point-of-no-return commitment, and traffic/job/write conflict fences.

use std::time::Duration;

use rustok_modules::{
    ConflictKeyKind, DataUpgradeEvidence, DataUpgradePhase, MigrationPreflightReceipt,
    ModuleTransitionCoordinator, ModuleTransitionState, SecurityEpochRegistry,
    StartTransitionInput, TransitionCoordinatorError, UpdateMode, evaluate_data_upgrade_decision,
};
use uuid::Uuid;

fn sample_preflight(operation_id: Uuid, is_additive_safe: bool) -> MigrationPreflightReceipt {
    MigrationPreflightReceipt {
        operation_id,
        module_slug: "orders".to_string(),
        mode: if is_additive_safe {
            UpdateMode::Automatic
        } else {
            UpdateMode::Maintenance
        },
        source_schema_digest:
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        target_schema_digest:
            "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        migration_plan_digest:
            "sha256:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        migration_safety: if is_additive_safe {
            rustok_core::MigrationSafetyAssessment::AdditiveSafe
        } else {
            rustok_core::MigrationSafetyAssessment::MaintenanceRequired {
                reasons: vec![],
                locks_table: true,
                is_destructive: true,
            }
        },
        settings_guard_installed: true,
        denial_reasons: vec![],
        evaluated_at: chrono::Utc::now(),
    }
}

#[test]
fn data_upgrade_cannot_bypass_an_inconsistent_or_denied_preflight() {
    let safe = sample_preflight(Uuid::new_v4(), true);
    let mut unsafe_migration = sample_preflight(safe.operation_id, false);
    unsafe_migration.mode = UpdateMode::Automatic;
    let mut unknown = safe.clone();
    unknown.migration_safety = rustok_core::MigrationSafetyAssessment::Unknown { reasons: vec![] };
    let mut missing_guard = safe.clone();
    missing_guard.settings_guard_installed = false;
    let mut denied = safe.clone();
    denied
        .denial_reasons
        .push("Irreversible external effect".to_owned());
    let mut maintenance = safe;
    maintenance.mode = UpdateMode::Maintenance;
    for receipt in [
        unsafe_migration,
        unknown,
        missing_guard,
        denied,
        maintenance,
    ] {
        let mut evidence = DataUpgradeEvidence {
            preflight: &receipt,
            settings_intersection_valid: true,
            requires_cross_revision_data_copy: false,
            unmigrated_live_objects_count: 0,
            point_of_no_return_committed: false,
        };
        let decision = evaluate_data_upgrade_decision(&evidence);
        assert_eq!(decision.phase, DataUpgradePhase::MaintenancePreCutover);
        assert!(!decision.can_auto_converge);
        evidence.point_of_no_return_committed = true;
        let decision = evaluate_data_upgrade_decision(&evidence);
        assert_eq!(decision.phase, DataUpgradePhase::PointOfNoReturn);
        assert!(!decision.rollback_allowed);
    }
}

#[test]
fn test_data_upgrade_phase_derivation_from_owner_evidence() {
    // 1. Fully reversible compatible zero-downtime evolution
    let preflight_safe = sample_preflight(Uuid::new_v4(), true);
    let evidence_safe = DataUpgradeEvidence {
        preflight: &preflight_safe,
        settings_intersection_valid: true,
        requires_cross_revision_data_copy: false,
        unmigrated_live_objects_count: 0,
        point_of_no_return_committed: false,
    };
    let decision_safe = evaluate_data_upgrade_decision(&evidence_safe);
    assert_eq!(decision_safe.phase, DataUpgradePhase::Compatible);
    assert!(!decision_safe.is_irreversible);
    assert!(decision_safe.rollback_allowed);
    assert!(decision_safe.can_auto_converge);

    // 2. Settings incompatibility: one-sided settings forces maintenance pre-cutover
    let evidence_bad_settings = DataUpgradeEvidence {
        preflight: &preflight_safe,
        settings_intersection_valid: false,
        requires_cross_revision_data_copy: false,
        unmigrated_live_objects_count: 0,
        point_of_no_return_committed: false,
    };
    let decision_bad_settings = evaluate_data_upgrade_decision(&evidence_bad_settings);
    assert_eq!(
        decision_bad_settings.phase,
        DataUpgradePhase::MaintenancePreCutover
    );
    assert!(!decision_bad_settings.is_irreversible);
    assert!(!decision_bad_settings.can_auto_converge);

    // 3. Destructive migration steps: requires explicit point of no return before execution
    let preflight_destructive = sample_preflight(Uuid::new_v4(), false);
    let evidence_destructive = DataUpgradeEvidence {
        preflight: &preflight_destructive,
        settings_intersection_valid: true,
        requires_cross_revision_data_copy: false,
        unmigrated_live_objects_count: 0,
        point_of_no_return_committed: false,
    };
    let decision_destructive = evaluate_data_upgrade_decision(&evidence_destructive);
    assert_eq!(
        decision_destructive.phase,
        DataUpgradePhase::MaintenancePreCutover
    );
    assert!(!decision_destructive.is_irreversible);
    assert!(!decision_destructive.can_auto_converge);

    // 4. Point of no return committed: irreversibility enforced, rollback forbidden
    let evidence_ponr = DataUpgradeEvidence {
        preflight: &preflight_destructive,
        settings_intersection_valid: true,
        requires_cross_revision_data_copy: false,
        unmigrated_live_objects_count: 0,
        point_of_no_return_committed: true,
    };
    let decision_ponr = evaluate_data_upgrade_decision(&evidence_ponr);
    assert_eq!(decision_ponr.phase, DataUpgradePhase::PointOfNoReturn);
    assert!(decision_ponr.is_irreversible);
    assert!(!decision_ponr.rollback_allowed);
    assert!(!decision_ponr.can_auto_converge);
}

#[test]
fn test_commit_point_of_no_return_enforces_fences_and_strictly_forbids_rollback() {
    let operation_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let module_slug = "orders".to_string();
    let security_registry = SecurityEpochRegistry::new();

    let input = StartTransitionInput {
        operation_id,
        module_slug: module_slug.clone(),
        tenant_id: Some(tenant_id),
        predecessor_digest: Some(
            "sha256:predecessor00000000000000000000000000000000000000000000000000000".to_string(),
        ),
        candidate_digest: "sha256:candidate00000000000000000000000000000000000000000000000000000"
            .to_string(),
        affected_nodes: vec!["node-a".to_string()],
        preflight_receipt: sample_preflight(operation_id, true),
        requested_mode: UpdateMode::Automatic,
    };

    let mut coordinator = ModuleTransitionCoordinator::start_transition(input, &security_registry)
        .expect("start transition");

    // Advance to Observing state
    coordinator
        .advance_to_prestaging(&security_registry)
        .expect("prestaging");
    coordinator
        .advance_to_activating(&security_registry, Duration::from_secs(30))
        .expect("activating");

    assert!(matches!(
        coordinator.state(),
        ModuleTransitionState::Observing { .. }
    ));

    // Commit Point of No Return before destructive effect
    let affected_nodes = vec!["node-a".to_string()];
    coordinator
        .commit_point_of_no_return(
            "Executing destructive partition swap".to_string(),
            &security_registry,
            &affected_nodes,
        )
        .expect("commit point of no return succeeds");

    // 1. Verify state changed to PointOfNoReturn
    assert!(matches!(
        coordinator.state(),
        ModuleTransitionState::PointOfNoReturn { .. }
    ));

    // 2. Verify traffic, job, and write fences are enforced
    let fences = coordinator.checkpoint().fences.keys();
    assert!(
        fences.iter().any(|k| k.kind == ConflictKeyKind::Traffic),
        "Traffic fence must be present"
    );
    assert!(
        fences.iter().any(|k| k.kind == ConflictKeyKind::JobQueue),
        "JobQueue fence must be present"
    );
    assert!(
        fences
            .iter()
            .any(|k| k.kind == ConflictKeyKind::DataMigrationOwner),
        "DataMigrationOwner (write) fence must be present"
    );

    // 3. Verify rollback / recovery is strictly FORBIDDEN past point of no return
    let rollback_err = coordinator
        .record_predecessor_recovery("attempt incident recovery".to_string())
        .expect_err("recovery must be strictly forbidden past point of no return");

    match rollback_err {
        TransitionCoordinatorError::PastPointOfNoReturn(op_id, msg) => {
            assert_eq!(op_id, operation_id);
            assert!(msg.contains("Executing destructive partition swap"));
        }
        other => panic!("expected PastPointOfNoReturn, got {other:?}"),
    }

    // 4. Verify completion to Converged
    coordinator
        .advance_point_of_no_return_to_converged(&security_registry)
        .expect("converge past point of no return");
    assert!(matches!(
        coordinator.state(),
        ModuleTransitionState::Converged { .. }
    ));
}
