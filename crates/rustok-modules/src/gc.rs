//! Independent owner-driven retention and garbage collection adapters with tombstone, grace period, and final recheck.
//!
//! Enforces the platform invariant:
//! - Background GC deletes only already-unreferenced physical identities after tombstone, grace period, and final recheck.
//! - Neither finalization nor elapsed time alone authorizes data deletion.
//! - Independent owner-driven adapters control source CAS, OCI artifacts, build attempts, executable CAS,
//!   artifact-data objects, snapshot/restore copies, encrypted settings recovery points and roots,
//!   browser assets, node slots, operations-tool packages/slots, and diagnostics.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{ArtifactObjectState, RetentionHoldLedger, RetentionTarget};

/// Categories of platform assets managed by owner-driven retention and GC adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GcTargetKind {
    SourceCas,
    OciRegistry,
    BuildAttempts,
    PlatformExecutableCas,
    ArtifactDataObjects,
    SnapshotRestoreCopies,
    EncryptedSettingsRecoveryPoints,
    BrowserAssets,
    NodeSlots,
    OperationsTool,
    Diagnostics,
}

/// Lifecycle status of an asset tombstone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GcTombstoneStatus {
    /// In grace period until the specified timestamp. Physical purge prohibited.
    ActiveGrace,
    /// Grace period expired; eligible for final recheck.
    GraceExpired,
    /// Passed final recheck; authorization execution token issued.
    FinalRecheckPassed,
    /// Physically purged and confirmed with collection receipt.
    Collected,
    /// Revoked because a hold was placed or live reference re-appeared.
    Revoked { reason: String },
}

/// Durable tombstone record marking an unreferenced asset for eventual collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcTombstoneRecord {
    pub tombstone_id: Uuid,
    pub target: RetentionTarget,
    pub target_kind: GcTargetKind,
    pub marked_at: DateTime<Utc>,
    pub grace_period_ends_at: DateTime<Utc>,
    pub reason: String,
    pub status: GcTombstoneStatus,
    pub tombstone_digest: String,
}

impl GcTombstoneRecord {
    pub fn compute_digest(
        tombstone_id: Uuid,
        target: &RetentionTarget,
        marked_at: DateTime<Utc>,
        grace_period_ends_at: DateTime<Utc>,
        reason: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(tombstone_id.as_bytes());
        let (kind, id) = target.identity_key();
        hasher.update(kind.as_bytes());
        hasher.update(id.as_bytes());
        hasher.update(marked_at.to_rfc3339().as_bytes());
        hasher.update(grace_period_ends_at.to_rfc3339().as_bytes());
        hasher.update(reason.as_bytes());
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }
}

/// Short-lived authorization token issued only after a successful final recheck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcExecutionToken {
    pub token_id: Uuid,
    pub tombstone_id: Uuid,
    pub target: RetentionTarget,
    pub authorized_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub recheck_digest: String,
}

/// Decision returned by final recheck evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GcFinalRecheckDecision {
    /// Final recheck passed. Purge is authorized with the enclosed token.
    Authorized { token: GcExecutionToken },
    /// Asset is still within its mandatory grace period.
    DeniedInGracePeriod { remaining_seconds: i64 },
    /// Active retention holds exist in the authoritative ledger.
    DeniedActiveHolds {
        active_holds: usize,
        hold_ids: Vec<Uuid>,
    },
    /// Logical reference or owner guard exists in the domain.
    DeniedLiveReference { reason: String },
    /// Tombstone was revoked or already collected.
    DeniedInactiveStatus { status: GcTombstoneStatus },
}

/// Immutable proof of physical collection following successful tombstone, grace period, and final recheck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcCollectionReceipt {
    pub collection_id: Uuid,
    pub tombstone_id: Uuid,
    pub target: RetentionTarget,
    pub target_kind: GcTargetKind,
    pub collected_at: DateTime<Utc>,
    pub recheck_digest: String,
    pub receipt_digest: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GcError {
    #[error("Tombstone `{0}` not found")]
    TombstoneNotFound(Uuid),
    #[error("Target `{0:?}` is already tombstoned with id `{1}`")]
    AlreadyTombstoned(RetentionTarget, Uuid),
    #[error("Invalid target kind `{actual:?}` for adapter `{expected:?}`")]
    TargetKindMismatch {
        expected: GcTargetKind,
        actual: GcTargetKind,
    },
    #[error("Final recheck denied: {0}")]
    FinalRecheckDenied(String),
    #[error("Execution token expired at `{0}`")]
    TokenExpired(DateTime<Utc>),
    #[error("Invalid execution token: {0}")]
    InvalidToken(String),
    #[error("Owner physical purge failed: {0}")]
    PhysicalPurgeFailed(String),
    #[error("Live artifact data object cannot be tombstoned or purged")]
    LiveArtifactDataProhibited,
}

/// Common trait implemented by independent owner-driven retention and GC adapters.
pub trait GcAdapter: Send + Sync {
    /// The owner category handled by this adapter.
    fn target_kind(&self) -> GcTargetKind;

    /// Verifies whether the target currently has active references, live bindings, or dependency locks.
    /// Returns `Ok(Some(reason))` if references exist, or `Ok(None)` if proven unreferenced.
    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError>;

    /// Executes the physical deletion of the asset from storage/node/registry.
    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError>;
}

// ---------------------------------------------------------------------------
// 1. Source CAS Adapter
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct SourceCasGcAdapter {
    pub stored_digests: HashSet<String>,
    pub referenced_digests: HashMap<String, String>, // digest -> release_id / build_id
}

impl SourceCasGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for SourceCasGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::SourceCas
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::SourceCasBlob { digest } => {
                if let Some(ref_id) = self.referenced_digests.get(digest) {
                    Ok(Some(format!("Referenced by active build/release: {ref_id}")))
                } else {
                    Ok(None)
                }
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::SourceCas,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::SourceCasBlob { digest } => {
                self.stored_digests.remove(digest);
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::SourceCas,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// 2. OCI Registry Adapter (Manifests, Layers, Referrers)
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct OciArtifactGcAdapter {
    pub manifests: HashSet<String>,
    pub layers: HashSet<String>,
    pub referrers: HashSet<String>,
    pub manifest_layer_references: HashMap<String, HashSet<String>>, // manifest -> layers
    pub active_manifests: HashSet<String>,
}

impl OciArtifactGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for OciArtifactGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::OciRegistry
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::OciManifest { digest } => {
                if self.active_manifests.contains(digest) {
                    Ok(Some(format!("OCI manifest {digest} is currently active")))
                } else {
                    Ok(None)
                }
            }
            RetentionTarget::OciLayer { digest } => {
                // Check if any active manifest references this layer
                for (manifest, layers) in &self.manifest_layer_references {
                    if self.active_manifests.contains(manifest) && layers.contains(digest) {
                        return Ok(Some(format!(
                            "OCI layer {digest} is referenced by active manifest {manifest}"
                        )));
                    }
                }
                Ok(None)
            }
            RetentionTarget::OciReferrer {
                digest,
                subject_digest,
            } => {
                if self.active_manifests.contains(subject_digest) {
                    Ok(Some(format!(
                        "OCI referrer {digest} attached to active subject manifest {subject_digest}"
                    )))
                } else {
                    Ok(None)
                }
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::OciRegistry,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::OciManifest { digest } => {
                self.manifests.remove(digest);
                self.manifest_layer_references.remove(digest);
                Ok(())
            }
            RetentionTarget::OciLayer { digest } => {
                self.layers.remove(digest);
                Ok(())
            }
            RetentionTarget::OciReferrer { digest, .. } => {
                self.referrers.remove(digest);
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::OciRegistry,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Build Attempt Adapter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildAttemptStatus {
    Running,
    Finished,
}

#[derive(Debug, Default)]
pub struct BuildAttemptGcAdapter {
    pub attempts: HashMap<Uuid, BuildAttemptStatus>,
}

impl BuildAttemptGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for BuildAttemptGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::BuildAttempts
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::BuildAttempt { attempt_id } => {
                match self.attempts.get(attempt_id) {
                    Some(BuildAttemptStatus::Running) => Ok(Some(format!(
                        "Build attempt {attempt_id} is currently running"
                    ))),
                    _ => Ok(None),
                }
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::BuildAttempts,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::BuildAttempt { attempt_id } => {
                self.attempts.remove(attempt_id);
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::BuildAttempts,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Platform Executable CAS Adapter
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct PlatformExecutableCasGcAdapter {
    pub executables: HashSet<String>,
    pub deployed_digests: HashSet<String>,
}

impl PlatformExecutableCasGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for PlatformExecutableCasGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::PlatformExecutableCas
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::PlatformExecutableCas { digest } => {
                if self.deployed_digests.contains(digest) {
                    Ok(Some(format!(
                        "Executable CAS {digest} is actively deployed on platform nodes"
                    )))
                } else {
                    Ok(None)
                }
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::PlatformExecutableCas,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::PlatformExecutableCas { digest } => {
                self.executables.remove(digest);
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::PlatformExecutableCas,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// 5. Artifact Data Objects Adapter (Live, Staging, Logically Deleted)
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct ArtifactDataObjectGcAdapter {
    pub objects: HashMap<Uuid, ArtifactObjectState>,
    pub active_staging_intents: HashSet<Uuid>,
}

impl ArtifactDataObjectGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for ArtifactDataObjectGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::ArtifactDataObjects
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::ArtifactDataObject {
                object_id, state, ..
            } => {
                if *state == ArtifactObjectState::Live {
                    return Err(GcError::LiveArtifactDataProhibited);
                }
                if *state == ArtifactObjectState::Staging && self.active_staging_intents.contains(object_id) {
                    return Ok(Some(format!(
                        "Artifact data object {object_id} has active staging intent"
                    )));
                }
                Ok(None)
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::ArtifactDataObjects,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::ArtifactDataObject { object_id, .. } => {
                self.objects.remove(object_id);
                self.active_staging_intents.remove(object_id);
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::ArtifactDataObjects,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// 6. Snapshot / Restore Copies Adapter
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct SnapshotRestoreCopyGcAdapter {
    pub copies: HashSet<Uuid>,
    pub active_restore_operations: HashSet<Uuid>,
}

impl SnapshotRestoreCopyGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for SnapshotRestoreCopyGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::SnapshotRestoreCopies
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::SnapshotRestoreCopy { copy_id } => {
                if self.active_restore_operations.contains(copy_id) {
                    Ok(Some(format!(
                        "Snapshot copy {copy_id} is bound to an active restore operation"
                    )))
                } else {
                    Ok(None)
                }
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::SnapshotRestoreCopies,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::SnapshotRestoreCopy { copy_id } => {
                self.copies.remove(copy_id);
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::SnapshotRestoreCopies,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// 7. Encrypted Settings Recovery Point & Roots Adapter
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct EncryptedSettingsRecoveryPointGcAdapter {
    pub recovery_points: HashSet<Uuid>,
    pub active_kms_key_versions: HashSet<String>,
    pub active_schema_roots: HashSet<String>,
}

impl EncryptedSettingsRecoveryPointGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for EncryptedSettingsRecoveryPointGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::EncryptedSettingsRecoveryPoints
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::EncryptedSettingsRecoveryPoint {
                recovery_point_id,
                kms_key_version,
                schema_root_digest,
            } => {
                if self.active_kms_key_versions.contains(kms_key_version) {
                    return Ok(Some(format!(
                        "Recovery point {recovery_point_id} is protected by active KMS key root {kms_key_version}"
                    )));
                }
                if self.active_schema_roots.contains(schema_root_digest) {
                    return Ok(Some(format!(
                        "Recovery point {recovery_point_id} is protected by active schema root {schema_root_digest}"
                    )));
                }
                Ok(None)
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::EncryptedSettingsRecoveryPoints,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::EncryptedSettingsRecoveryPoint { recovery_point_id, .. } => {
                self.recovery_points.remove(recovery_point_id);
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::EncryptedSettingsRecoveryPoints,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// 8. Browser Assets Adapter
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct BrowserAssetGcAdapter {
    pub assets: HashSet<String>, // format: "{release_id}:{logical_path}"
    pub active_or_retained_releases: HashSet<String>,
}

impl BrowserAssetGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for BrowserAssetGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::BrowserAssets
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::BrowserAsset {
                release_id,
                logical_path,
                ..
            } => {
                if self.active_or_retained_releases.contains(release_id) {
                    Ok(Some(format!(
                        "Browser asset {logical_path} belongs to active or retained release {release_id}"
                    )))
                } else {
                    Ok(None)
                }
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::BrowserAssets,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::BrowserAsset {
                release_id,
                logical_path,
                ..
            } => {
                self.assets.remove(&format!("{release_id}:{logical_path}"));
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::BrowserAssets,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// 9. Node Slot Adapter
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct NodeSlotGcAdapter {
    pub slots: HashSet<String>, // "{node_id}:{slot_digest}"
    pub serving_slots: HashSet<String>,
}

impl NodeSlotGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for NodeSlotGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::NodeSlots
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::NodeSlot {
                node_id,
                slot_digest,
            } => {
                let key = format!("{node_id}:{slot_digest}");
                if self.serving_slots.contains(&key) {
                    Ok(Some(format!("Node slot {key} is actively serving")))
                } else {
                    Ok(None)
                }
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::NodeSlots,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::NodeSlot {
                node_id,
                slot_digest,
            } => {
                self.slots.remove(&format!("{node_id}:{slot_digest}"));
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::NodeSlots,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// 10. Operations-Tool Adapter (Packages & Local Predecessor Slots)
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct OperationsToolGcAdapter {
    pub packages: HashSet<String>,
    pub predecessor_slots: HashSet<String>, // "{host_id}:{slot_digest}"
    pub active_assignments: HashSet<String>,
    pub protected_predecessor_slots: HashSet<String>,
}

impl OperationsToolGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for OperationsToolGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::OperationsTool
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::OperationsToolPackage { package_digest } => {
                if self.active_assignments.contains(package_digest) {
                    Ok(Some(format!(
                        "Operations tool package {package_digest} is actively assigned to host"
                    )))
                } else {
                    Ok(None)
                }
            }
            RetentionTarget::OperationsToolPredecessorSlot {
                host_id,
                slot_digest,
            } => {
                let key = format!("{host_id}:{slot_digest}");
                if self.protected_predecessor_slots.contains(&key) {
                    Ok(Some(format!(
                        "Operations tool predecessor slot {key} is protected for fast crash recovery"
                    )))
                } else {
                    Ok(None)
                }
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::OperationsTool,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::OperationsToolPackage { package_digest } => {
                self.packages.remove(package_digest);
                Ok(())
            }
            RetentionTarget::OperationsToolPredecessorSlot {
                host_id,
                slot_digest,
            } => {
                self.predecessor_slots.remove(&format!("{host_id}:{slot_digest}"));
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::OperationsTool,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// 11. Diagnostic Logs Adapter
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct DiagnosticLogGcAdapter {
    pub logs: HashSet<Uuid>,
    pub active_incident_investigations: HashSet<Uuid>,
}

impl DiagnosticLogGcAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl GcAdapter for DiagnosticLogGcAdapter {
    fn target_kind(&self) -> GcTargetKind {
        GcTargetKind::Diagnostics
    }

    fn check_live_references(&self, target: &RetentionTarget) -> Result<Option<String>, GcError> {
        match target {
            RetentionTarget::DiagnosticLog { operation_id } => {
                if self.active_incident_investigations.contains(operation_id) {
                    Ok(Some(format!(
                        "Diagnostic log {operation_id} is locked under active incident investigation"
                    )))
                } else {
                    Ok(None)
                }
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::Diagnostics,
                actual: target_to_kind(target),
            }),
        }
    }

    fn execute_physical_purge(&mut self, target: &RetentionTarget) -> Result<(), GcError> {
        match target {
            RetentionTarget::DiagnosticLog { operation_id } => {
                self.logs.remove(operation_id);
                Ok(())
            }
            _ => Err(GcError::TargetKindMismatch {
                expected: GcTargetKind::Diagnostics,
                actual: target_to_kind(target),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// GcCoordinator (Tombstone -> Grace -> Final Recheck -> Execution Token)
// ---------------------------------------------------------------------------

/// Durable coordinator managing asset tombstones, grace period tracking, final recheck, and execution tokens.
#[derive(Debug, Default)]
pub struct GcCoordinator {
    tombstones: HashMap<Uuid, GcTombstoneRecord>,
    target_to_tombstone: HashMap<RetentionTarget, Uuid>,
    issued_tokens: HashMap<Uuid, GcExecutionToken>,
    collection_receipts: HashMap<Uuid, GcCollectionReceipt>,
}

impl GcCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Phase 1: Tombstone candidate asset.
    ///
    /// Live artifact data objects are rejected immediately.
    /// Initiates mandatory grace period during which physical purge is strictly forbidden.
    pub fn tombstone_candidate(
        &mut self,
        target: RetentionTarget,
        grace_duration: Duration,
        reason: impl Into<String>,
        now: DateTime<Utc>,
    ) -> Result<GcTombstoneRecord, GcError> {
        if let RetentionTarget::ArtifactDataObject { state, .. } = &target {
            if *state == ArtifactObjectState::Live {
                return Err(GcError::LiveArtifactDataProhibited);
            }
        }

        if let Some(existing_id) = self.target_to_tombstone.get(&target) {
            if let Some(tombstone) = self.tombstones.get(existing_id) {
                if matches!(
                    tombstone.status,
                    GcTombstoneStatus::ActiveGrace | GcTombstoneStatus::GraceExpired
                ) {
                    return Err(GcError::AlreadyTombstoned(target, *existing_id));
                }
            }
        }

        let tombstone_id = Uuid::new_v4();
        let target_kind = target_to_kind(&target);
        let grace_period_ends_at = now + grace_duration;
        let reason_str = reason.into();

        let tombstone_digest = GcTombstoneRecord::compute_digest(
            tombstone_id,
            &target,
            now,
            grace_period_ends_at,
            &reason_str,
        );

        let record = GcTombstoneRecord {
            tombstone_id,
            target: target.clone(),
            target_kind,
            marked_at: now,
            grace_period_ends_at,
            reason: reason_str,
            status: GcTombstoneStatus::ActiveGrace,
            tombstone_digest,
        };

        self.tombstones.insert(tombstone_id, record.clone());
        self.target_to_tombstone.insert(target, tombstone_id);

        Ok(record)
    }

    /// Revokes an active tombstone if a live reference, standby requirement, or hold is placed.
    pub fn revoke_tombstone(
        &mut self,
        tombstone_id: Uuid,
        reason: impl Into<String>,
    ) -> Result<(), GcError> {
        let tombstone = self
            .tombstones
            .get_mut(&tombstone_id)
            .ok_or(GcError::TombstoneNotFound(tombstone_id))?;

        tombstone.status = GcTombstoneStatus::Revoked {
            reason: reason.into(),
        };
        Ok(())
    }

    /// Phase 2 & 3: Evaluate grace period expiration and execute atomic Final Recheck.
    ///
    /// Rechecks:
    /// 1. Status is active (not revoked or already collected).
    /// 2. Grace period has expired (`now >= grace_period_ends_at`).
    /// 3. Authoritative `RetentionHoldLedger` has exactly 0 active holds.
    /// 4. Owner adapter confirms zero live references or active reservations.
    ///
    /// If all rechecks pass, issues short-lived `GcExecutionToken` (5 minute validity).
    pub fn evaluate_final_recheck(
        &mut self,
        tombstone_id: Uuid,
        ledger: &RetentionHoldLedger,
        adapter: &dyn GcAdapter,
        now: DateTime<Utc>,
    ) -> GcFinalRecheckDecision {
        let tombstone = match self.tombstones.get_mut(&tombstone_id) {
            Some(t) => t,
            None => return GcFinalRecheckDecision::DeniedInactiveStatus {
                status: GcTombstoneStatus::Revoked {
                    reason: "Tombstone not found".to_string(),
                },
            },
        };

        if !matches!(
            tombstone.status,
            GcTombstoneStatus::ActiveGrace | GcTombstoneStatus::GraceExpired
        ) {
            return GcFinalRecheckDecision::DeniedInactiveStatus {
                status: tombstone.status.clone(),
            };
        }

        // 1. Grace period check
        if now < tombstone.grace_period_ends_at {
            let remaining = (tombstone.grace_period_ends_at - now).num_seconds();
            return GcFinalRecheckDecision::DeniedInGracePeriod {
                remaining_seconds: remaining.max(1),
            };
        }

        tombstone.status = GcTombstoneStatus::GraceExpired;

        // 2. Authoritative retention holds check
        let active_holds = ledger.active_holds_count(&tombstone.target);
        if active_holds > 0 {
            let hold_ids = ledger
                .get_active_holds(&tombstone.target)
                .iter()
                .map(|h| h.hold_id)
                .collect();
            return GcFinalRecheckDecision::DeniedActiveHolds {
                active_holds,
                hold_ids,
            };
        }

        // 3. Adapter live reference check
        match adapter.check_live_references(&tombstone.target) {
            Ok(Some(reason)) => return GcFinalRecheckDecision::DeniedLiveReference { reason },
            Err(err) => {
                return GcFinalRecheckDecision::DeniedLiveReference {
                    reason: err.to_string(),
                };
            }
            Ok(None) => {}
        }

        // Passed all fences! Issue GcExecutionToken
        tombstone.status = GcTombstoneStatus::FinalRecheckPassed;

        let token_id = Uuid::new_v4();
        let expires_at = now + Duration::minutes(5);

        let mut hasher = Sha256::new();
        hasher.update(token_id.as_bytes());
        hasher.update(tombstone_id.as_bytes());
        hasher.update(tombstone.tombstone_digest.as_bytes());
        hasher.update(now.to_rfc3339().as_bytes());
        hasher.update(expires_at.to_rfc3339().as_bytes());
        let recheck_digest = format!("sha256:{}", hex::encode(hasher.finalize()));

        let token = GcExecutionToken {
            token_id,
            tombstone_id,
            target: tombstone.target.clone(),
            authorized_at: now,
            expires_at,
            recheck_digest,
        };

        self.issued_tokens.insert(token_id, token.clone());
        GcFinalRecheckDecision::Authorized { token }
    }

    /// Physical Purge execution with verified authorization token.
    ///
    /// Validates token freshness and digest, executes adapter purge, marks tombstone `Collected`,
    /// and issues immutable `GcCollectionReceipt`.
    pub fn collect_with_token(
        &mut self,
        token: GcExecutionToken,
        adapter: &mut dyn GcAdapter,
        now: DateTime<Utc>,
    ) -> Result<GcCollectionReceipt, GcError> {
        if now > token.expires_at {
            return Err(GcError::TokenExpired(token.expires_at));
        }

        let recorded_token = self
            .issued_tokens
            .remove(&token.token_id)
            .ok_or_else(|| GcError::InvalidToken("Token not found or already consumed".to_string()))?;

        if recorded_token != token {
            return Err(GcError::InvalidToken("Token mismatch".to_string()));
        }

        let tombstone = self
            .tombstones
            .get_mut(&token.tombstone_id)
            .ok_or(GcError::TombstoneNotFound(token.tombstone_id))?;

        if tombstone.status != GcTombstoneStatus::FinalRecheckPassed {
            return Err(GcError::FinalRecheckDenied(format!(
                "Tombstone status is not FinalRecheckPassed: {:?}",
                tombstone.status
            )));
        }

        // Execute physical deletion via adapter
        adapter.execute_physical_purge(&token.target)?;

        // Transition tombstone to Collected
        tombstone.status = GcTombstoneStatus::Collected;

        let collection_id = Uuid::new_v4();
        let mut hasher = Sha256::new();
        hasher.update(collection_id.as_bytes());
        hasher.update(token.tombstone_id.as_bytes());
        hasher.update(token.recheck_digest.as_bytes());
        hasher.update(now.to_rfc3339().as_bytes());
        let receipt_digest = format!("sha256:{}", hex::encode(hasher.finalize()));

        let receipt = GcCollectionReceipt {
            collection_id,
            tombstone_id: token.tombstone_id,
            target: token.target.clone(),
            target_kind: tombstone.target_kind,
            collected_at: now,
            recheck_digest: token.recheck_digest,
            receipt_digest,
        };

        self.collection_receipts.insert(collection_id, receipt.clone());
        Ok(receipt)
    }

    /// Retrieves tombstone by ID.
    pub fn get_tombstone(&self, tombstone_id: Uuid) -> Option<&GcTombstoneRecord> {
        self.tombstones.get(&tombstone_id)
    }

    /// Retrieves collection receipt by ID.
    pub fn get_receipt(&self, collection_id: Uuid) -> Option<&GcCollectionReceipt> {
        self.collection_receipts.get(&collection_id)
    }
}

fn target_to_kind(target: &RetentionTarget) -> GcTargetKind {
    match target {
        RetentionTarget::SourceCasBlob { .. } => GcTargetKind::SourceCas,
        RetentionTarget::AdmittedPayloadCas { .. } => GcTargetKind::PlatformExecutableCas,
        RetentionTarget::OciManifest { .. }
        | RetentionTarget::OciLayer { .. }
        | RetentionTarget::OciReferrer { .. } => GcTargetKind::OciRegistry,
        RetentionTarget::BuildAttempt { .. } => GcTargetKind::BuildAttempts,
        RetentionTarget::PlatformExecutableCas { .. } => GcTargetKind::PlatformExecutableCas,
        RetentionTarget::ArtifactDataObject { .. } => GcTargetKind::ArtifactDataObjects,
        RetentionTarget::SnapshotRestoreCopy { .. } => GcTargetKind::SnapshotRestoreCopies,
        RetentionTarget::EncryptedSettingsRecoveryPoint { .. } => {
            GcTargetKind::EncryptedSettingsRecoveryPoints
        }
        RetentionTarget::BrowserAsset { .. } => GcTargetKind::BrowserAssets,
        RetentionTarget::NodeSlot { .. } => GcTargetKind::NodeSlots,
        RetentionTarget::OperationsToolPackage { .. }
        | RetentionTarget::OperationsToolPredecessorSlot { .. } => GcTargetKind::OperationsTool,
        RetentionTarget::RecoveryPoint { .. } => GcTargetKind::SnapshotRestoreCopies,
        RetentionTarget::DiagnosticLog { .. } => GcTargetKind::Diagnostics,
    }
}
