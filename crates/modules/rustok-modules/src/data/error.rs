//! Artifact data errors.

use thiserror::Error;

use super::*;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactDataError {
    #[error("artifact data scope is invalid")]
    InvalidScope,
    #[error("artifact data key is invalid")]
    InvalidKey,
    #[error("artifact data object metadata is invalid")]
    InvalidObject,
    #[error("artifact data object failed its stored integrity check")]
    ObjectIntegrity,
    #[error("artifact data page is invalid")]
    InvalidPage,
    #[error("artifact data index query is invalid")]
    InvalidIndexQuery,
    #[error("artifact data index query is unavailable")]
    IndexQueryUnavailable,
    #[error("artifact data batch is invalid")]
    InvalidBatch,
    #[error("artifact data upgrade request is invalid")]
    InvalidUpgrade,
    #[error("artifact data upgrade hook failed: {0}")]
    UpgradeHook(String),
    #[error("artifact data upgrade plan is stale")]
    StaleUpgradePlan,
    #[error("artifact data migration checkpoint failed: {0}")]
    MigrationCheckpoint(String),
    #[error("artifact data contract is unavailable for the injected installation scope")]
    DataContractUnavailable,
    #[error("artifact data contract schema is invalid")]
    DataContractSchemaInvalid,
    #[error("artifact data value does not satisfy the admitted data contract schema")]
    DataContractSchemaViolation,
    #[error("artifact data revision conflict")]
    RevisionConflict,
    #[error("artifact data namespace was purged")]
    NamespacePurged,
    #[error("artifact data namespace is held by a maintenance operation")]
    NamespaceHeld,
    #[error("artifact data purge precondition failed")]
    PurgePrecondition,
    #[error("artifact data export precondition failed")]
    ExportPrecondition,
    #[error("artifact data snapshot precondition failed")]
    SnapshotPrecondition,
    #[error("artifact data snapshot exceeds the bounded owner limits")]
    SnapshotLimitExceeded,
    #[error("artifact data snapshot integrity check failed")]
    SnapshotIntegrity,
    #[error("artifact data restore precondition failed")]
    RestorePrecondition,
    #[error("artifact data snapshot retention precondition failed")]
    SnapshotRetentionPrecondition,
    #[error("artifact data snapshot collection precondition failed")]
    SnapshotCollectionPrecondition,
    #[error("artifact data idempotency key is invalid")]
    InvalidIdempotencyKey,
    #[error("artifact data idempotency key was reused for a different key")]
    IdempotencyConflict,
    #[error("artifact data value exceeds {limit} bytes (received {actual})")]
    ValueTooLarge { limit: usize, actual: usize },
    #[error("artifact data quota policy is invalid")]
    InvalidQuota,
    #[error("artifact data {resource} quota exceeded: limit {limit}, attempted {attempted}")]
    QuotaExceeded {
        resource: &'static str,
        limit: u64,
        attempted: u64,
    },
    #[error("artifact data policy denied the operation")]
    PolicyDenied,
    #[error("artifact data storage failed: {0}")]
    Storage(String),
}
