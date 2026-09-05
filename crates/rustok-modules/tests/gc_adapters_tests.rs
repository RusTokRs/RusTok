use chrono::{Duration, Utc};
use rustok_modules::{
    ArtifactDataObjectGcAdapter, ArtifactObjectState, BrowserAssetGcAdapter,
    BuildAttemptGcAdapter, BuildAttemptStatus, DiagnosticLogGcAdapter,
    EncryptedSettingsRecoveryPointGcAdapter, GcCoordinator, GcError,
    GcFinalRecheckDecision, GcTargetKind, GcTombstoneStatus,
    NodeSlotGcAdapter, OciArtifactGcAdapter, OperationsToolGcAdapter,
    PlatformExecutableCasGcAdapter, RetentionHoldKind, RetentionHoldLedger, RetentionTarget,
    SnapshotRestoreCopyGcAdapter, SourceCasGcAdapter,
};
use uuid::Uuid;

#[test]
fn test_tombstone_initiates_grace_period_and_blocks_premature_final_recheck() {
    let mut coordinator = GcCoordinator::new();
    let ledger = RetentionHoldLedger::new();
    let mut adapter = SourceCasGcAdapter::new();

    let digest = "sha256:abc123sourcecasblob0001".to_string();
    adapter.stored_digests.insert(digest.clone());

    let target = RetentionTarget::SourceCasBlob {
        digest: digest.clone(),
    };
    let now = Utc::now();
    let grace_duration = Duration::hours(24);

    let tombstone = coordinator
        .tombstone_candidate(
            target.clone(),
            grace_duration,
            "Replaced by release N+1",
            now,
        )
        .expect("Tombstone registration must succeed");

    assert_eq!(tombstone.status, GcTombstoneStatus::ActiveGrace);
    assert_eq!(tombstone.target_kind, GcTargetKind::SourceCas);
    assert!(tombstone.tombstone_digest.starts_with("sha256:"));

    // Attempting final recheck immediately (within grace period) must be denied
    let decision = coordinator.evaluate_final_recheck(tombstone.tombstone_id, &ledger, &adapter, now);
    match decision {
        GcFinalRecheckDecision::DeniedInGracePeriod { remaining_seconds } => {
            assert!(remaining_seconds > 0);
        }
        other => panic!("Expected DeniedInGracePeriod, got: {other:?}"),
    }
}

#[test]
fn test_authoritative_retention_holds_block_final_recheck_after_grace() {
    let mut coordinator = GcCoordinator::new();
    let mut ledger = RetentionHoldLedger::new();
    let mut adapter = SourceCasGcAdapter::new();

    let digest = "sha256:abc123sourcecasblob0002".to_string();
    adapter.stored_digests.insert(digest.clone());

    let target = RetentionTarget::SourceCasBlob {
        digest: digest.clone(),
    };
    let now = Utc::now();
    let grace_duration = Duration::hours(1);

    let tombstone = coordinator
        .tombstone_candidate(
            target.clone(),
            grace_duration,
            "Orphaned staging object",
            now,
        )
        .expect("Tombstone registration must succeed");

    // Place an active retention hold on this asset in the authoritative ledger
    let hold_id = ledger.place_hold(
        target.clone(),
        RetentionHoldKind::ActiveRolloutWindow {
            operation_id: Uuid::new_v4(),
            expires_at: now + Duration::days(7),
        },
    );

    // Fast forward past grace period
    let after_grace = now + Duration::hours(2);

    let decision = coordinator.evaluate_final_recheck(tombstone.tombstone_id, &ledger, &adapter, after_grace);
    match decision {
        GcFinalRecheckDecision::DeniedActiveHolds {
            active_holds,
            hold_ids,
        } => {
            assert_eq!(active_holds, 1);
            assert_eq!(hold_ids, vec![hold_id]);
        }
        other => panic!("Expected DeniedActiveHolds, got: {other:?}"),
    }

    // Release the hold
    ledger
        .release_hold(hold_id)
        .expect("Release hold must succeed");

    // Recheck again: now passes and issues token!
    let second_decision =
        coordinator.evaluate_final_recheck(tombstone.tombstone_id, &ledger, &adapter, after_grace);
    match second_decision {
        GcFinalRecheckDecision::Authorized { token } => {
            assert_eq!(token.tombstone_id, tombstone.tombstone_id);
            assert_eq!(token.target, target);
            assert!(token.recheck_digest.starts_with("sha256:"));

            // Physical purge with token succeeds
            let receipt = coordinator
                .collect_with_token(token.clone(), &mut adapter, after_grace)
                .expect("Physical collection must succeed");

            assert_eq!(receipt.tombstone_id, tombstone.tombstone_id);
            assert_eq!(receipt.target_kind, GcTargetKind::SourceCas);
            assert!(receipt.receipt_digest.starts_with("sha256:"));
            assert!(!adapter.stored_digests.contains(&digest));

            // Re-using token must fail
            let err = coordinator
                .collect_with_token(token, &mut adapter, after_grace)
                .unwrap_err();
            assert!(matches!(err, GcError::InvalidToken(_)));
        }
        other => panic!("Expected Authorized, got: {other:?}"),
    }
}

#[test]
fn test_live_references_block_oci_artifact_final_recheck() {
    let mut coordinator = GcCoordinator::new();
    let ledger = RetentionHoldLedger::new();
    let mut adapter = OciArtifactGcAdapter::new();

    let layer_digest = "sha256:layer999".to_string();
    let manifest_digest = "sha256:manifest111".to_string();

    adapter.layers.insert(layer_digest.clone());
    adapter.manifests.insert(manifest_digest.clone());
    adapter
        .manifest_layer_references
        .entry(manifest_digest.clone())
        .or_default()
        .insert(layer_digest.clone());
    adapter.active_manifests.insert(manifest_digest.clone());

    let target = RetentionTarget::OciLayer {
        digest: layer_digest.clone(),
    };
    let now = Utc::now();
    let tombstone = coordinator
        .tombstone_candidate(
            target.clone(),
            Duration::minutes(30),
            "Unused layer candidate",
            now,
        )
        .expect("Tombstone must succeed");

    let after_grace = now + Duration::hours(1);

    // Recheck: layer is still referenced by active manifest -> denied
    let decision = coordinator.evaluate_final_recheck(tombstone.tombstone_id, &ledger, &adapter, after_grace);
    match decision {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("referenced by active manifest"));
        }
        other => panic!("Expected DeniedLiveReference, got: {other:?}"),
    }

    // Deactivate manifest
    adapter.active_manifests.remove(&manifest_digest);

    // Recheck: now passes!
    let second_decision =
        coordinator.evaluate_final_recheck(tombstone.tombstone_id, &ledger, &adapter, after_grace);
    match second_decision {
        GcFinalRecheckDecision::Authorized { token } => {
            let receipt = coordinator
                .collect_with_token(token, &mut adapter, after_grace)
                .expect("Collection must succeed");
            assert_eq!(receipt.target_kind, GcTargetKind::OciRegistry);
            assert!(!adapter.layers.contains(&layer_digest));
        }
        other => panic!("Expected Authorized, got: {other:?}"),
    }
}

#[test]
fn test_live_artifact_data_object_strictly_prohibited_from_tombstone() {
    let mut coordinator = GcCoordinator::new();
    let object_id = Uuid::new_v4();
    let namespace_instance_id = Uuid::new_v4();

    let live_target = RetentionTarget::ArtifactDataObject {
        object_id,
        namespace_instance_id,
        state: ArtifactObjectState::Live,
    };

    let err = coordinator
        .tombstone_candidate(
            live_target,
            Duration::hours(1),
            "Trying to tombstone live object",
            Utc::now(),
        )
        .unwrap_err();

    assert_eq!(err, GcError::LiveArtifactDataProhibited);

    // Staging or LogicallyDeleted is allowed
    let deleted_target = RetentionTarget::ArtifactDataObject {
        object_id,
        namespace_instance_id,
        state: ArtifactObjectState::LogicallyDeleted,
    };

    let tombstone = coordinator
        .tombstone_candidate(
            deleted_target,
            Duration::hours(1),
            "Purged object",
            Utc::now(),
        )
        .expect("Logically deleted object must succeed tombstone");

    assert_eq!(tombstone.target_kind, GcTargetKind::ArtifactDataObjects);
}

#[test]
fn test_encrypted_settings_recovery_point_kms_and_schema_root_protection() {
    let mut coordinator = GcCoordinator::new();
    let ledger = RetentionHoldLedger::new();
    let mut adapter = EncryptedSettingsRecoveryPointGcAdapter::new();

    let recovery_point_id = Uuid::new_v4();
    let kms_key_version = "kms:v2:root".to_string();
    let schema_root_digest = "sha256:schema999".to_string();

    adapter.recovery_points.insert(recovery_point_id);
    adapter
        .active_kms_key_versions
        .insert(kms_key_version.clone());
    adapter
        .active_schema_roots
        .insert(schema_root_digest.clone());

    let target = RetentionTarget::EncryptedSettingsRecoveryPoint {
        recovery_point_id,
        kms_key_version: kms_key_version.clone(),
        schema_root_digest: schema_root_digest.clone(),
    };

    let now = Utc::now();
    let tombstone = coordinator
        .tombstone_candidate(
            target.clone(),
            Duration::hours(1),
            "Expired recovery point",
            now,
        )
        .expect("Tombstone must succeed");

    let after_grace = now + Duration::hours(2);

    // Denied while KMS key version is active
    let dec1 = coordinator.evaluate_final_recheck(tombstone.tombstone_id, &ledger, &adapter, after_grace);
    match dec1 {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("active KMS key root"));
        }
        other => panic!("Expected DeniedLiveReference for KMS key, got: {other:?}"),
    }

    // Retiring KMS key version still keeps schema root active
    adapter.active_kms_key_versions.remove(&kms_key_version);
    let dec2 = coordinator.evaluate_final_recheck(tombstone.tombstone_id, &ledger, &adapter, after_grace);
    match dec2 {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("active schema root"));
        }
        other => panic!("Expected DeniedLiveReference for schema root, got: {other:?}"),
    }

    // Retiring schema root allows collection
    adapter.active_schema_roots.remove(&schema_root_digest);
    let dec3 = coordinator.evaluate_final_recheck(tombstone.tombstone_id, &ledger, &adapter, after_grace);
    match dec3 {
        GcFinalRecheckDecision::Authorized { token } => {
            let receipt = coordinator
                .collect_with_token(token, &mut adapter, after_grace)
                .expect("Collection must succeed");
            assert_eq!(
                receipt.target_kind,
                GcTargetKind::EncryptedSettingsRecoveryPoints
            );
            assert!(!adapter.recovery_points.contains(&recovery_point_id));
        }
        other => panic!("Expected Authorized, got: {other:?}"),
    }
}

#[test]
fn test_operations_tool_and_node_slot_adapters() {
    let mut coordinator = GcCoordinator::new();
    let ledger = RetentionHoldLedger::new();
    let mut ops_adapter = OperationsToolGcAdapter::new();
    let mut node_adapter = NodeSlotGcAdapter::new();

    let now = Utc::now();
    let after_grace = now + Duration::hours(2);

    // 1. Operations Tool Predecessor Slot protection
    let ops_target = RetentionTarget::OperationsToolPredecessorSlot {
        host_id: "host-alpha".to_string(),
        slot_digest: "sha256:predecessor_ops_slot".to_string(),
    };
    ops_adapter
        .predecessor_slots
        .insert("host-alpha:sha256:predecessor_ops_slot".to_string());
    ops_adapter
        .protected_predecessor_slots
        .insert("host-alpha:sha256:predecessor_ops_slot".to_string());

    let tomb_ops = coordinator
        .tombstone_candidate(
            ops_target.clone(),
            Duration::hours(1),
            "Old ops predecessor slot",
            now,
        )
        .expect("Tombstone must succeed");

    let dec_ops =
        coordinator.evaluate_final_recheck(tomb_ops.tombstone_id, &ledger, &ops_adapter, after_grace);
    match dec_ops {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("protected for fast crash recovery"));
        }
        other => panic!("Expected DeniedLiveReference, got: {other:?}"),
    }

    // 2. Node slot serving protection
    let slot_target = RetentionTarget::NodeSlot {
        node_id: "node-1".to_string(),
        slot_digest: "sha256:slot123".to_string(),
    };
    node_adapter
        .slots
        .insert("node-1:sha256:slot123".to_string());
    node_adapter
        .serving_slots
        .insert("node-1:sha256:slot123".to_string());

    let tomb_slot = coordinator
        .tombstone_candidate(
            slot_target.clone(),
            Duration::hours(1),
            "Old serving slot",
            now,
        )
        .expect("Tombstone must succeed");

    let dec_slot =
        coordinator.evaluate_final_recheck(tomb_slot.tombstone_id, &ledger, &node_adapter, after_grace);
    match dec_slot {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("actively serving"));
        }
        other => panic!("Expected DeniedLiveReference, got: {other:?}"),
    }

    // Mark slot not serving
    node_adapter
        .serving_slots
        .remove("node-1:sha256:slot123");
    let dec_slot2 =
        coordinator.evaluate_final_recheck(tomb_slot.tombstone_id, &ledger, &node_adapter, after_grace);
    match dec_slot2 {
        GcFinalRecheckDecision::Authorized { token } => {
            coordinator
                .collect_with_token(token, &mut node_adapter, after_grace)
                .expect("Collection must succeed");
            assert!(!node_adapter.slots.contains("node-1:sha256:slot123"));
        }
        other => panic!("Expected Authorized, got: {other:?}"),
    }
}

#[test]
fn test_tombstone_revocation() {
    let mut coordinator = GcCoordinator::new();
    let ledger = RetentionHoldLedger::new();
    let adapter = SourceCasGcAdapter::new();

    let target = RetentionTarget::SourceCasBlob {
        digest: "sha256:revoketest".to_string(),
    };
    let now = Utc::now();
    let tombstone = coordinator
        .tombstone_candidate(target, Duration::hours(1), "Testing revocation", now)
        .expect("Tombstone must succeed");

    coordinator
        .revoke_tombstone(
            tombstone.tombstone_id,
            "Re-referenced in active release rollout",
        )
        .expect("Revoke must succeed");

    let decision =
        coordinator.evaluate_final_recheck(tombstone.tombstone_id, &ledger, &adapter, now + Duration::hours(2));
    match decision {
        GcFinalRecheckDecision::DeniedInactiveStatus {
            status: GcTombstoneStatus::Revoked { reason },
        } => {
            assert!(reason.contains("Re-referenced"));
        }
        other => panic!("Expected DeniedInactiveStatus with Revoked, got: {other:?}"),
    }
}

#[test]
fn test_build_attempt_and_diagnostic_log_adapters() {
    let mut coordinator = GcCoordinator::new();
    let ledger = RetentionHoldLedger::new();
    let mut build_adapter = BuildAttemptGcAdapter::new();
    let mut diag_adapter = DiagnosticLogGcAdapter::new();

    let now = Utc::now();
    let after_grace = now + Duration::hours(2);

    // 1. Build Attempt
    let attempt_id = Uuid::new_v4();
    build_adapter
        .attempts
        .insert(attempt_id, BuildAttemptStatus::Running);

    let build_target = RetentionTarget::BuildAttempt { attempt_id };
    let tomb_build = coordinator
        .tombstone_candidate(build_target.clone(), Duration::hours(1), "Old build", now)
        .expect("Tombstone must succeed");

    let dec_build = coordinator.evaluate_final_recheck(
        tomb_build.tombstone_id,
        &ledger,
        &build_adapter,
        after_grace,
    );
    match dec_build {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("currently running"));
        }
        other => panic!("Expected DeniedLiveReference for running build, got: {other:?}"),
    }

    build_adapter
        .attempts
        .insert(attempt_id, BuildAttemptStatus::Finished);
    let dec_build2 = coordinator.evaluate_final_recheck(
        tomb_build.tombstone_id,
        &ledger,
        &build_adapter,
        after_grace,
    );
    match dec_build2 {
        GcFinalRecheckDecision::Authorized { token } => {
            coordinator
                .collect_with_token(token, &mut build_adapter, after_grace)
                .expect("Collection must succeed");
            assert!(!build_adapter.attempts.contains_key(&attempt_id));
        }
        other => panic!("Expected Authorized for finished build, got: {other:?}"),
    }

    // 2. Diagnostic Log
    let operation_id = Uuid::new_v4();
    diag_adapter.logs.insert(operation_id);
    diag_adapter
        .active_incident_investigations
        .insert(operation_id);

    let diag_target = RetentionTarget::DiagnosticLog { operation_id };
    let tomb_diag = coordinator
        .tombstone_candidate(diag_target, Duration::hours(1), "Trace log", now)
        .expect("Tombstone must succeed");

    let dec_diag = coordinator.evaluate_final_recheck(
        tomb_diag.tombstone_id,
        &ledger,
        &diag_adapter,
        after_grace,
    );
    match dec_diag {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("active incident investigation"));
        }
        other => panic!("Expected DeniedLiveReference for incident log, got: {other:?}"),
    }

    diag_adapter
        .active_incident_investigations
        .remove(&operation_id);
    let dec_diag2 = coordinator.evaluate_final_recheck(
        tomb_diag.tombstone_id,
        &ledger,
        &diag_adapter,
        after_grace,
    );
    match dec_diag2 {
        GcFinalRecheckDecision::Authorized { token } => {
            coordinator
                .collect_with_token(token, &mut diag_adapter, after_grace)
                .expect("Collection must succeed");
            assert!(!diag_adapter.logs.contains(&operation_id));
        }
        other => panic!("Expected Authorized for cleared incident log, got: {other:?}"),
    }
}

#[test]
fn test_browser_asset_and_platform_executable_cas_adapters() {
    let mut coordinator = GcCoordinator::new();
    let ledger = RetentionHoldLedger::new();
    let mut browser_adapter = BrowserAssetGcAdapter::new();
    let mut exec_adapter = PlatformExecutableCasGcAdapter::new();

    let now = Utc::now();
    let after_grace = now + Duration::hours(2);

    // 1. Browser Asset
    let release_id = "release-100".to_string();
    let logical_path = "/assets/app.js".to_string();
    let content_digest = "sha256:appjs123".to_string();
    let asset_key = format!("{release_id}:{logical_path}");

    browser_adapter.assets.insert(asset_key);
    browser_adapter
        .active_or_retained_releases
        .insert(release_id.clone());

    let browser_target = RetentionTarget::BrowserAsset {
        release_id: release_id.clone(),
        logical_path,
        content_digest,
    };
    let tomb_browser = coordinator
        .tombstone_candidate(browser_target, Duration::hours(1), "Old asset", now)
        .expect("Tombstone must succeed");

    let dec_browser = coordinator.evaluate_final_recheck(
        tomb_browser.tombstone_id,
        &ledger,
        &browser_adapter,
        after_grace,
    );
    match dec_browser {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("active or retained release"));
        }
        other => panic!("Expected DeniedLiveReference for browser asset, got: {other:?}"),
    }

    browser_adapter
        .active_or_retained_releases
        .remove(&release_id);
    let dec_browser2 = coordinator.evaluate_final_recheck(
        tomb_browser.tombstone_id,
        &ledger,
        &browser_adapter,
        after_grace,
    );
    match dec_browser2 {
        GcFinalRecheckDecision::Authorized { token } => {
            coordinator
                .collect_with_token(token, &mut browser_adapter, after_grace)
                .expect("Collection must succeed");
        }
        other => panic!("Expected Authorized for expired browser asset, got: {other:?}"),
    }

    // 2. Platform Executable CAS
    let exec_digest = "sha256:server_bin_xyz".to_string();
    exec_adapter.executables.insert(exec_digest.clone());
    exec_adapter.deployed_digests.insert(exec_digest.clone());

    let exec_target = RetentionTarget::PlatformExecutableCas {
        digest: exec_digest.clone(),
    };
    let tomb_exec = coordinator
        .tombstone_candidate(exec_target, Duration::hours(1), "Old binary", now)
        .expect("Tombstone must succeed");

    let dec_exec = coordinator.evaluate_final_recheck(
        tomb_exec.tombstone_id,
        &ledger,
        &exec_adapter,
        after_grace,
    );
    match dec_exec {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("actively deployed"));
        }
        other => panic!("Expected DeniedLiveReference for deployed binary, got: {other:?}"),
    }

    exec_adapter.deployed_digests.remove(&exec_digest);
    let dec_exec2 = coordinator.evaluate_final_recheck(
        tomb_exec.tombstone_id,
        &ledger,
        &exec_adapter,
        after_grace,
    );
    match dec_exec2 {
        GcFinalRecheckDecision::Authorized { token } => {
            coordinator
                .collect_with_token(token, &mut exec_adapter, after_grace)
                .expect("Collection must succeed");
            assert!(!exec_adapter.executables.contains(&exec_digest));
        }
        other => panic!("Expected Authorized for undeployed binary, got: {other:?}"),
    }
}

#[test]
fn test_snapshot_restore_copy_and_artifact_data_staging_adapters() {
    let mut coordinator = GcCoordinator::new();
    let ledger = RetentionHoldLedger::new();
    let mut snap_adapter = SnapshotRestoreCopyGcAdapter::new();
    let mut data_adapter = ArtifactDataObjectGcAdapter::new();

    let now = Utc::now();
    let after_grace = now + Duration::hours(2);

    // 1. Snapshot restore copy
    let copy_id = Uuid::new_v4();
    snap_adapter.copies.insert(copy_id);
    snap_adapter.active_restore_operations.insert(copy_id);

    let snap_target = RetentionTarget::SnapshotRestoreCopy { copy_id };
    let tomb_snap = coordinator
        .tombstone_candidate(snap_target, Duration::hours(1), "Old snapshot copy", now)
        .expect("Tombstone must succeed");

    let dec_snap = coordinator.evaluate_final_recheck(
        tomb_snap.tombstone_id,
        &ledger,
        &snap_adapter,
        after_grace,
    );
    match dec_snap {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("active restore operation"));
        }
        other => panic!("Expected DeniedLiveReference for active restore snapshot, got: {other:?}"),
    }

    snap_adapter.active_restore_operations.remove(&copy_id);
    let dec_snap2 = coordinator.evaluate_final_recheck(
        tomb_snap.tombstone_id,
        &ledger,
        &snap_adapter,
        after_grace,
    );
    match dec_snap2 {
        GcFinalRecheckDecision::Authorized { token } => {
            coordinator
                .collect_with_token(token, &mut snap_adapter, after_grace)
                .expect("Collection must succeed");
            assert!(!snap_adapter.copies.contains(&copy_id));
        }
        other => panic!("Expected Authorized for unreferenced snapshot, got: {other:?}"),
    }

    // 2. Artifact Data Object in Staging with active intent
    let object_id = Uuid::new_v4();
    let namespace_instance_id = Uuid::new_v4();
    data_adapter
        .objects
        .insert(object_id, ArtifactObjectState::Staging);
    data_adapter.active_staging_intents.insert(object_id);

    let data_target = RetentionTarget::ArtifactDataObject {
        object_id,
        namespace_instance_id,
        state: ArtifactObjectState::Staging,
    };
    let tomb_data = coordinator
        .tombstone_candidate(data_target, Duration::hours(1), "Staging object", now)
        .expect("Tombstone must succeed");

    let dec_data = coordinator.evaluate_final_recheck(
        tomb_data.tombstone_id,
        &ledger,
        &data_adapter,
        after_grace,
    );
    match dec_data {
        GcFinalRecheckDecision::DeniedLiveReference { reason } => {
            assert!(reason.contains("active staging intent"));
        }
        other => panic!("Expected DeniedLiveReference for staging intent, got: {other:?}"),
    }

    data_adapter.active_staging_intents.remove(&object_id);
    let dec_data2 = coordinator.evaluate_final_recheck(
        tomb_data.tombstone_id,
        &ledger,
        &data_adapter,
        after_grace,
    );
    match dec_data2 {
        GcFinalRecheckDecision::Authorized { token } => {
            coordinator
                .collect_with_token(token, &mut data_adapter, after_grace)
                .expect("Collection must succeed");
            assert!(!data_adapter.objects.contains_key(&object_id));
        }
        other => panic!("Expected Authorized for cleared staging object, got: {other:?}"),
    }
}
