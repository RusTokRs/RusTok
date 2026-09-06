use chrono::{Duration, Utc};
use rustok_modules::{RetentionHoldKind, RetentionHoldLedger, RetentionTarget};
use uuid::Uuid;

#[test]
fn test_end_to_end_rollout_retention_and_finalization() {
    let mut ledger = RetentionHoldLedger::new();
    let operation_id = Uuid::new_v4();

    let candidate_target = RetentionTarget::AdmittedPayloadCas {
        digest: "sha256:orders_candidate_v2".to_string(),
    };
    let predecessor_target = RetentionTarget::AdmittedPayloadCas {
        digest: "sha256:orders_predecessor_v1".to_string(),
    };
    let node_slot_target = RetentionTarget::NodeSlot {
        node_id: "node-worker-1".to_string(),
        slot_digest: "sha256:orders_predecessor_v1".to_string(),
    };

    // 1. Rollout starts: hold candidate, predecessor in CAS, and predecessor standby slot on node
    let hold_candidate = ledger.place_hold(
        candidate_target.clone(),
        RetentionHoldKind::ActiveRolloutWindow {
            operation_id,
            expires_at: Utc::now() + Duration::hours(2),
        },
    );
    let hold_predecessor = ledger.place_hold(
        predecessor_target.clone(),
        RetentionHoldKind::DirectPredecessorStandby {
            release_digest: "sha256:orders_predecessor_v1".to_string(),
        },
    );
    let hold_slot = ledger.place_hold(
        node_slot_target.clone(),
        RetentionHoldKind::DirectPredecessorStandby {
            release_digest: "sha256:orders_predecessor_v1".to_string(),
        },
    );

    // Verify all 3 targets are strictly retained
    assert!(!ledger.is_collection_allowed(&candidate_target));
    assert!(!ledger.is_collection_allowed(&predecessor_target));
    assert!(!ledger.is_collection_allowed(&node_slot_target));

    // 2. Rollout successfully converges -> finalization closes the rollback window
    ledger.release_hold(hold_predecessor).unwrap();
    ledger.release_hold(hold_slot).unwrap();

    // Predecessor is now safe for collection, while candidate is still protected under active lease/window
    assert!(ledger.is_collection_allowed(&predecessor_target));
    assert!(ledger.is_collection_allowed(&node_slot_target));
    assert!(!ledger.is_collection_allowed(&candidate_target));

    // Release candidate rollout window hold
    ledger.release_hold(hold_candidate).unwrap();
    assert!(ledger.is_collection_allowed(&candidate_target));
}

#[test]
fn test_incident_investigation_and_diagnostic_retention() {
    let mut ledger = RetentionHoldLedger::new();
    let operation_id = Uuid::new_v4();
    let incident_id = Uuid::new_v4();

    let diag_log = RetentionTarget::DiagnosticLog { operation_id };
    let recovery_point = RetentionTarget::RecoveryPoint {
        snapshot_id: Uuid::new_v4(),
    };

    // Place incident investigation holds
    let hold_diag = ledger.place_hold(
        diag_log.clone(),
        RetentionHoldKind::IncidentInvestigation {
            incident_id,
            reason: "Core dump and memory allocation trace analysis".to_string(),
        },
    );
    let hold_snapshot = ledger.place_hold(
        recovery_point.clone(),
        RetentionHoldKind::IncidentInvestigation {
            incident_id,
            reason: "Point-in-time snapshot before rollback".to_string(),
        },
    );

    // GC must not touch diagnostics or recovery point
    assert!(!ledger.is_collection_allowed(&diag_log));
    assert!(!ledger.is_collection_allowed(&recovery_point));

    // Conclude incident investigation
    ledger.release_hold(hold_diag).unwrap();
    ledger.release_hold(hold_snapshot).unwrap();

    assert!(ledger.is_collection_allowed(&diag_log));
    assert!(ledger.is_collection_allowed(&recovery_point));
}

#[test]
fn test_batch_gc_filter_with_diverse_targets() {
    let mut ledger = RetentionHoldLedger::new();

    let held_source = RetentionTarget::SourceCasBlob {
        digest: "sha256:source_held".to_string(),
    };
    let unheld_source = RetentionTarget::SourceCasBlob {
        digest: "sha256:source_stale".to_string(),
    };
    let held_slot = RetentionTarget::NodeSlot {
        node_id: "node-1".to_string(),
        slot_digest: "sha256:slot_standby".to_string(),
    };
    let unheld_slot = RetentionTarget::NodeSlot {
        node_id: "node-2".to_string(),
        slot_digest: "sha256:slot_obsolete".to_string(),
    };

    ledger.place_hold(
        held_source.clone(),
        RetentionHoldKind::LegalHold {
            reference: "AUDIT-2026-Q3".to_string(),
        },
    );
    ledger.place_hold(
        held_slot.clone(),
        RetentionHoldKind::DirectPredecessorStandby {
            release_digest: "sha256:slot_standby".to_string(),
        },
    );

    let batch = vec![
        held_source.clone(),
        unheld_source.clone(),
        held_slot.clone(),
        unheld_slot.clone(),
    ];

    let eligible = ledger.garbage_collect_eligible_targets(&batch);

    assert_eq!(eligible.len(), 2);
    assert!(eligible.contains(&&unheld_source));
    assert!(eligible.contains(&&unheld_slot));
    assert!(!eligible.contains(&&held_source));
    assert!(!eligible.contains(&&held_slot));
}
