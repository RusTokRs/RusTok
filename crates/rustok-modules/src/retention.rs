//! Retention and CAS hold ledger for rollback safety and garbage collection fencing.
//!
//! Enforces the platform invariant: no source archive, admitted payload CAS blob,
//! node-local slot, recovery point, or incident diagnostic log may be collected or pruned
//! while an active rollback window, predecessor standby hold, or investigation hold exists.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

/// Classification of durable reasons why an asset must be retained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "details")]
pub enum RetentionHoldKind {
    /// Active rollout observation or rollback compatibility window.
    ActiveRolloutWindow {
        operation_id: Uuid,
        expires_at: DateTime<Utc>,
    },
    /// Direct predecessor retained on node standby for instant crash recovery.
    DirectPredecessorStandby { release_digest: String },
    /// Incident investigation preserving diagnostic evidence and memory dumps.
    IncidentInvestigation { incident_id: Uuid, reason: String },
    /// Audit trail or compliance preservation requirement.
    AuditTrail { compliance_id: String },
    /// Explicit legal hold prohibiting any mutation or deletion.
    LegalHold { reference: String },
}

/// State classification of an artifact-data object in tenant storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactObjectState {
    Live,
    Staging,
    LogicallyDeleted,
}

/// Strongly-typed asset targets protected by the retention ledger.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "target_type", content = "identity")]
pub enum RetentionTarget {
    /// Source code archive in platform CAS.
    SourceCasBlob { digest: String },
    /// Admitted executable WASM/Rhai payload in object storage.
    AdmittedPayloadCas { digest: String },
    /// OCI manifest in registry CAS.
    OciManifest { digest: String },
    /// OCI layer blob in registry CAS.
    OciLayer { digest: String },
    /// OCI referrer artifact binding.
    OciReferrer {
        digest: String,
        subject_digest: String,
    },
    /// Build attempt working workspace or intermediate output.
    BuildAttempt { attempt_id: Uuid },
    /// Platform native/compiled executable in platform CAS.
    PlatformExecutableCas { digest: String },
    /// Live, staging, or logically-deleted artifact-data object.
    ArtifactDataObject {
        object_id: Uuid,
        namespace_instance_id: Uuid,
        state: ArtifactObjectState,
    },
    /// Snapshot or backup restore copy.
    SnapshotRestoreCopy { copy_id: Uuid },
    /// Encrypted settings recovery point with KMS key and schema descriptor root.
    EncryptedSettingsRecoveryPoint {
        recovery_point_id: Uuid,
        kms_key_version: String,
        schema_root_digest: String,
    },
    /// Release-qualified browser asset.
    BrowserAsset {
        release_id: String,
        logical_path: String,
        content_digest: String,
    },
    /// Node-local execution slot on a host.
    NodeSlot {
        node_id: String,
        slot_digest: String,
    },
    /// Operations tool package in platform distribution.
    OperationsToolPackage { package_digest: String },
    /// Local operations tool predecessor slot on a host.
    OperationsToolPredecessorSlot {
        host_id: String,
        slot_digest: String,
    },
    /// Database backup or point-in-time recovery point.
    RecoveryPoint { snapshot_id: Uuid },
    /// Diagnostic trace, telemetry, or failure log.
    DiagnosticLog { operation_id: Uuid },
}

impl RetentionTarget {
    /// Returns canonical `(target_type, target_identity)` for store persistence and GraphQL projection.
    pub fn identity_key(&self) -> (&'static str, String) {
        match self {
            RetentionTarget::SourceCasBlob { digest } => ("source_cas", digest.clone()),
            RetentionTarget::AdmittedPayloadCas { digest } => ("payload_cas", digest.clone()),
            RetentionTarget::OciManifest { digest } => ("oci_manifest", digest.clone()),
            RetentionTarget::OciLayer { digest } => ("oci_layer", digest.clone()),
            RetentionTarget::OciReferrer {
                digest,
                subject_digest,
            } => ("oci_referrer", format!("{subject_digest}:{digest}")),
            RetentionTarget::BuildAttempt { attempt_id } => {
                ("build_attempt", attempt_id.to_string())
            }
            RetentionTarget::PlatformExecutableCas { digest } => {
                ("executable_cas", digest.clone())
            }
            RetentionTarget::ArtifactDataObject {
                object_id,
                namespace_instance_id,
                state,
            } => (
                "artifact_data_object",
                format!("{namespace_instance_id}:{object_id}:{state:?}"),
            ),
            RetentionTarget::SnapshotRestoreCopy { copy_id } => {
                ("snapshot_copy", copy_id.to_string())
            }
            RetentionTarget::EncryptedSettingsRecoveryPoint {
                recovery_point_id,
                kms_key_version,
                schema_root_digest,
            } => (
                "settings_recovery_point",
                format!("{recovery_point_id}:{kms_key_version}:{schema_root_digest}"),
            ),
            RetentionTarget::BrowserAsset {
                release_id,
                logical_path,
                content_digest,
            } => (
                "browser_asset",
                format!("{release_id}:{logical_path}:{content_digest}"),
            ),
            RetentionTarget::NodeSlot {
                node_id,
                slot_digest,
            } => ("node_slot", format!("{node_id}:{slot_digest}")),
            RetentionTarget::OperationsToolPackage { package_digest } => {
                ("operations_tool_package", package_digest.clone())
            }
            RetentionTarget::OperationsToolPredecessorSlot {
                host_id,
                slot_digest,
            } => ("operations_tool_slot", format!("{host_id}:{slot_digest}")),
            RetentionTarget::RecoveryPoint { snapshot_id } => {
                ("recovery_point", snapshot_id.to_string())
            }
            RetentionTarget::DiagnosticLog { operation_id } => {
                ("diagnostic_log", operation_id.to_string())
            }
        }
    }
}

/// Durable record of an individual active retention hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionHoldRecord {
    pub hold_id: Uuid,
    pub target: RetentionTarget,
    pub kind: RetentionHoldKind,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetentionError {
    #[error("Retention hold `{0}` not found")]
    HoldNotFound(Uuid),
    #[error(
        "Target `{target:?}` is currently protected by {active_holds} active retention hold(s)"
    )]
    TargetProtectedByActiveHolds {
        target: RetentionTarget,
        active_holds: usize,
    },
}

/// Durable ledger managing active retention holds and garbage collection eligibility.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetentionHoldLedger {
    holds: HashMap<Uuid, RetentionHoldRecord>,
    target_holds: HashMap<RetentionTarget, HashSet<Uuid>>,
}

impl RetentionHoldLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Places a durable retention hold on a specific target asset.
    pub fn place_hold(&mut self, target: RetentionTarget, kind: RetentionHoldKind) -> Uuid {
        let hold_id = Uuid::new_v4();
        let record = RetentionHoldRecord {
            hold_id,
            target: target.clone(),
            kind,
            created_at: Utc::now(),
        };

        self.holds.insert(hold_id, record);
        self.target_holds.entry(target).or_default().insert(hold_id);

        hold_id
    }

    /// Releases an individual retention hold by its unique ID.
    pub fn release_hold(&mut self, hold_id: Uuid) -> Result<RetentionHoldRecord, RetentionError> {
        let record = self
            .holds
            .remove(&hold_id)
            .ok_or(RetentionError::HoldNotFound(hold_id))?;

        if let Some(set) = self.target_holds.get_mut(&record.target) {
            set.remove(&hold_id);
            if set.is_empty() {
                self.target_holds.remove(&record.target);
            }
        }

        Ok(record)
    }

    /// Returns the number of active holds currently protecting the given target.
    pub fn active_holds_count(&self, target: &RetentionTarget) -> usize {
        self.target_holds.get(target).map_or(0, |set| set.len())
    }

    /// Determines whether garbage collection or pruning is strictly permitted for this asset.
    ///
    /// Returns `true` if and only if there are **zero** active retention holds on the asset.
    pub fn is_collection_allowed(&self, target: &RetentionTarget) -> bool {
        self.active_holds_count(target) == 0
    }

    /// Retrieves all active hold records currently attached to a target.
    pub fn get_active_holds(&self, target: &RetentionTarget) -> Vec<&RetentionHoldRecord> {
        match self.target_holds.get(target) {
            Some(hold_ids) => hold_ids
                .iter()
                .filter_map(|id| self.holds.get(id))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Evaluates a batch of candidate assets for garbage collection and returns only
    /// the subset of targets that have zero active holds and are safe to delete.
    pub fn garbage_collect_eligible_targets<'a>(
        &self,
        candidates: &'a [RetentionTarget],
    ) -> Vec<&'a RetentionTarget> {
        candidates
            .iter()
            .filter(|target| self.is_collection_allowed(target))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_hold_blocks_and_allows_garbage_collection() {
        let mut ledger = RetentionHoldLedger::new();
        let payload_target = RetentionTarget::AdmittedPayloadCas {
            digest: "sha256:wasm_module_v1".to_string(),
        };

        // 1. Initially, with no holds, collection is allowed
        assert!(ledger.is_collection_allowed(&payload_target));

        // 2. Place active rollout window hold
        let hold_id = ledger.place_hold(
            payload_target.clone(),
            RetentionHoldKind::ActiveRolloutWindow {
                operation_id: Uuid::new_v4(),
                expires_at: Utc::now() + chrono::Duration::hours(24),
            },
        );

        // 3. Now collection must be strictly blocked!
        assert!(!ledger.is_collection_allowed(&payload_target));
        assert_eq!(ledger.active_holds_count(&payload_target), 1);

        // 4. Release hold upon rollback window finalization
        let released = ledger.release_hold(hold_id).unwrap();
        assert_eq!(released.hold_id, hold_id);

        // 5. Now collection is safe and allowed again
        assert!(ledger.is_collection_allowed(&payload_target));
        assert_eq!(ledger.active_holds_count(&payload_target), 0);
    }

    #[test]
    fn test_multi_hold_isolation() {
        let mut ledger = RetentionHoldLedger::new();
        let recovery_point = RetentionTarget::RecoveryPoint {
            snapshot_id: Uuid::new_v4(),
        };

        // Place two separate holds on the same recovery point
        let hold_rollout = ledger.place_hold(
            recovery_point.clone(),
            RetentionHoldKind::DirectPredecessorStandby {
                release_digest: "sha256:v1".to_string(),
            },
        );
        let hold_incident = ledger.place_hold(
            recovery_point.clone(),
            RetentionHoldKind::IncidentInvestigation {
                incident_id: Uuid::new_v4(),
                reason: "Investigating unexpected high memory usage".to_string(),
            },
        );

        assert_eq!(ledger.active_holds_count(&recovery_point), 2);
        assert!(!ledger.is_collection_allowed(&recovery_point));

        // Release the rollout hold (e.g. candidate converged)
        ledger.release_hold(hold_rollout).unwrap();

        // Target MUST STILL be protected because incident hold is active!
        assert_eq!(ledger.active_holds_count(&recovery_point), 1);
        assert!(!ledger.is_collection_allowed(&recovery_point));

        // Release incident hold
        ledger.release_hold(hold_incident).unwrap();

        // Now target is eligible for cleanup
        assert!(ledger.is_collection_allowed(&recovery_point));
    }

    #[test]
    fn test_batch_garbage_collection_filtering() {
        let mut ledger = RetentionHoldLedger::new();

        let target_active = RetentionTarget::AdmittedPayloadCas {
            digest: "sha256:active_candidate".to_string(),
        };
        let target_predecessor = RetentionTarget::AdmittedPayloadCas {
            digest: "sha256:predecessor_standby".to_string(),
        };
        let target_orphaned_old = RetentionTarget::AdmittedPayloadCas {
            digest: "sha256:old_collected_v0".to_string(),
        };

        // Hold the active candidate and predecessor
        ledger.place_hold(
            target_active.clone(),
            RetentionHoldKind::ActiveRolloutWindow {
                operation_id: Uuid::new_v4(),
                expires_at: Utc::now() + chrono::Duration::hours(1),
            },
        );
        ledger.place_hold(
            target_predecessor.clone(),
            RetentionHoldKind::DirectPredecessorStandby {
                release_digest: "sha256:predecessor_standby".to_string(),
            },
        );

        let candidates = vec![
            target_active.clone(),
            target_predecessor.clone(),
            target_orphaned_old.clone(),
        ];

        let eligible = ledger.garbage_collect_eligible_targets(&candidates);

        // Only the unheld old artifact is returned for GC!
        assert_eq!(eligible.len(), 1);
        assert_eq!(eligible[0], &target_orphaned_old);
    }
}
