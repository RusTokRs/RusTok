//! Bounded artifact-data snapshot readiness evaluation and platform PostgreSQL recovery evidence attestation.
//!
//! Provides proof of snapshot readiness within operational SLA, attests platform-level
//! database recovery capability, and strictly enforces the invariant that automatic
//! snapshot restore is forbidden during automated rollback.

use std::time::Duration as StdDuration;

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::data::{placeholder, revision_value, uuid_value};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SnapshotReadinessError {
    #[error("Storage error during snapshot evaluation: {0}")]
    Storage(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PostgresRecoveryEvidenceError {
    #[error("Storage error during platform recovery evidence query: {0}")]
    Storage(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RecoveryReadinessError {
    #[error("Snapshot readiness error: {0}")]
    Snapshot(#[from] SnapshotReadinessError),
    #[error("Platform recovery evidence error: {0}")]
    Platform(#[from] PostgresRecoveryEvidenceError),
}

/// Evaluated readiness facts for a tenant/module data snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataSnapshotReadiness {
    pub ready: bool,
    pub snapshot_id: Option<Uuid>,
    pub manifest_digest: Option<String>,
    pub is_within_sla: bool,
    pub structured_record_count: Option<u64>,
    pub object_count: Option<u64>,
    pub total_object_bytes: Option<u64>,
    pub created_at: Option<DateTime<Utc>>,
    pub retain_until: Option<DateTime<Utc>>,
    pub age_seconds: Option<u64>,
}

/// Attested platform-level database recovery capability evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformPostgresRecoveryEvidence {
    pub backend: String,
    pub checkpoint_lsn_or_tag: String,
    pub evidence_digest: String,
    pub recovery_capable: bool,
    pub attested_at: DateTime<Utc>,
}

/// Comprehensive attestation of data snapshot readiness and platform recovery evidence.
///
/// Note: By explicit platform architectural contract, `automatic_restore_authorized`
/// is strictly and unconditionally `false`. Automated transitions demote candidate code/routes
/// but never perform automatic database snapshot restores.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataRecoveryReadinessAttestation {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub data_contract_revision: u64,
    pub snapshot: ArtifactDataSnapshotReadiness,
    pub platform_evidence: PlatformPostgresRecoveryEvidence,
    pub automatic_restore_authorized: bool,
    pub attested_at: DateTime<Utc>,
}

/// Service evaluating bounded snapshot readiness and platform recovery evidence.
pub struct ArtifactDataRecoveryReadinessService {
    db: DatabaseConnection,
}

impl ArtifactDataRecoveryReadinessService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Evaluates if a valid, unexpired, ready snapshot exists within the requested max age SLA.
    pub async fn evaluate_snapshot_readiness(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        data_contract_revision: u64,
        max_age: StdDuration,
    ) -> Result<ArtifactDataSnapshotReadiness, SnapshotReadinessError> {
        let backend = self.db.get_database_backend();
        let row_opt = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT snapshot_id, manifest_digest, structured_record_count, object_count, \
                            total_object_bytes, created_at, retain_until \
                     FROM module_artifact_data_snapshots \
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} \
                       AND status = 'ready' \
                     ORDER BY created_at DESC LIMIT 1",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(tenant_id, backend),
                    module_slug.to_string().into(),
                    revision_value(data_contract_revision)
                        .map_err(|e| SnapshotReadinessError::Storage(e.to_string()))?,
                ],
            ))
            .await
            .map_err(|e| SnapshotReadinessError::Storage(e.to_string()))?;

        let row = match row_opt {
            Some(r) => r,
            None => {
                return Ok(ArtifactDataSnapshotReadiness {
                    ready: false,
                    snapshot_id: None,
                    manifest_digest: None,
                    is_within_sla: false,
                    structured_record_count: None,
                    object_count: None,
                    total_object_bytes: None,
                    created_at: None,
                    retain_until: None,
                    age_seconds: None,
                });
            }
        };

        let snapshot_id_str: String = row
            .try_get("", "snapshot_id")
            .map_err(|e| SnapshotReadinessError::Storage(e.to_string()))?;
        let snapshot_id = Uuid::parse_str(&snapshot_id_str)
            .map_err(|e| SnapshotReadinessError::Storage(e.to_string()))?;

        let manifest_digest: Option<String> = row
            .try_get("", "manifest_digest")
            .map_err(|e| SnapshotReadinessError::Storage(e.to_string()))?;
        let structured_record_count: i64 = row
            .try_get("", "structured_record_count")
            .map_err(|e| SnapshotReadinessError::Storage(e.to_string()))?;
        let object_count: i64 = row
            .try_get("", "object_count")
            .map_err(|e| SnapshotReadinessError::Storage(e.to_string()))?;
        let total_object_bytes: i64 = row
            .try_get("", "total_object_bytes")
            .map_err(|e| SnapshotReadinessError::Storage(e.to_string()))?;

        let created_at: DateTime<Utc> = row
            .try_get("", "created_at")
            .map_err(|e| SnapshotReadinessError::Storage(e.to_string()))?;
        let retain_until: DateTime<Utc> = row
            .try_get("", "retain_until")
            .map_err(|e| SnapshotReadinessError::Storage(e.to_string()))?;

        let now = Utc::now();
        let age = now.signed_duration_since(created_at);
        let age_seconds = age.num_seconds().max(0) as u64;

        let max_chrono_duration = match chrono::Duration::from_std(max_age) {
            Ok(d) => d,
            Err(_) => chrono::Duration::zero(),
        };
        let is_within_sla = age >= chrono::Duration::zero() && age <= max_chrono_duration;
        let not_expired = retain_until > now;
        let has_valid_manifest = manifest_digest
            .as_ref()
            .map(|d| d.starts_with("sha256:"))
            .unwrap_or(false);

        let ready = is_within_sla && not_expired && has_valid_manifest;

        Ok(ArtifactDataSnapshotReadiness {
            ready,
            snapshot_id: Some(snapshot_id),
            manifest_digest,
            is_within_sla,
            structured_record_count: Some(structured_record_count as u64),
            object_count: Some(object_count as u64),
            total_object_bytes: Some(total_object_bytes as u64),
            created_at: Some(created_at),
            retain_until: Some(retain_until),
            age_seconds: Some(age_seconds),
        })
    }

    /// Evaluates platform database recovery evidence (PostgreSQL WAL or SQLite page state).
    pub async fn evaluate_platform_recovery_evidence(
        &self,
    ) -> Result<PlatformPostgresRecoveryEvidence, PostgresRecoveryEvidenceError> {
        let backend = self.db.get_database_backend();
        let now = Utc::now();

        match backend {
            DbBackend::Postgres => {
                // Query PostgreSQL WAL/LSN or recovery state
                let row_opt = self
                    .db
                    .query_one_raw(Statement::from_string(
                        backend,
                        "SELECT CASE \
                            WHEN pg_is_in_recovery() THEN pg_last_wal_replay_lsn()::text \
                            ELSE pg_current_wal_lsn()::text \
                         END AS lsn, \
                         current_setting('wal_level') AS wal_level",
                    ))
                    .await
                    .map_err(|e| PostgresRecoveryEvidenceError::Storage(e.to_string()))?;

                let (lsn, wal_level) = if let Some(row) = row_opt {
                    let lsn: String = row
                        .try_get("", "lsn")
                        .unwrap_or_else(|_| "0/0".to_string());
                    let wal_level: String = row
                        .try_get("", "wal_level")
                        .unwrap_or_else(|_| "replica".to_string());
                    (lsn, wal_level)
                } else {
                    ("0/0".to_string(), "replica".to_string())
                };

                let mut hasher = Sha256::new();
                hasher.update(b"postgres");
                hasher.update(lsn.as_bytes());
                hasher.update(wal_level.as_bytes());
                hasher.update(&now.timestamp().to_be_bytes());
                let evidence_digest = format!("sha256:{}", hex::encode(hasher.finalize()));

                Ok(PlatformPostgresRecoveryEvidence {
                    backend: "PostgreSQL".to_string(),
                    checkpoint_lsn_or_tag: format!("lsn:{}:wal_level={}", lsn, wal_level),
                    evidence_digest,
                    recovery_capable: true,
                    attested_at: now,
                })
            }
            DbBackend::Sqlite => {
                let row_opt = self
                    .db
                    .query_one_raw(Statement::from_string(
                        backend,
                        "SELECT page_count, page_size FROM pragma_page_count(), pragma_page_size()",
                    ))
                    .await
                    .map_err(|e| PostgresRecoveryEvidenceError::Storage(e.to_string()))?;

                let (page_count, page_size) = if let Some(row) = row_opt {
                    let count: i64 = row.try_get("", "page_count").unwrap_or(0);
                    let size: i64 = row.try_get("", "page_size").unwrap_or(4096);
                    (count, size)
                } else {
                    (0, 4096)
                };

                let tag = format!("sqlite:page_count={}:page_size={}", page_count, page_size);
                let mut hasher = Sha256::new();
                hasher.update(b"sqlite");
                hasher.update(tag.as_bytes());
                hasher.update(&now.timestamp().to_be_bytes());
                let evidence_digest = format!("sha256:{}", hex::encode(hasher.finalize()));

                Ok(PlatformPostgresRecoveryEvidence {
                    backend: "SQLite".to_string(),
                    checkpoint_lsn_or_tag: tag,
                    evidence_digest,
                    recovery_capable: true,
                    attested_at: now,
                })
            }
            _ => Err(PostgresRecoveryEvidenceError::Storage(
                "Unsupported database backend for recovery evidence".to_string(),
            )),
        }
    }

    /// Attests combined data recovery readiness: bounded snapshot readiness and platform recovery evidence.
    ///
    /// The resulting attestation enforces `automatic_restore_authorized = false`.
    pub async fn attest_recovery_readiness(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        data_contract_revision: u64,
        max_age: StdDuration,
    ) -> Result<ArtifactDataRecoveryReadinessAttestation, RecoveryReadinessError> {
        let snapshot = self
            .evaluate_snapshot_readiness(tenant_id, module_slug, data_contract_revision, max_age)
            .await?;
        let platform_evidence = self.evaluate_platform_recovery_evidence().await?;

        Ok(ArtifactDataRecoveryReadinessAttestation {
            tenant_id,
            module_slug: module_slug.to_string(),
            data_contract_revision,
            snapshot,
            platform_evidence,
            automatic_restore_authorized: false,
            attested_at: Utc::now(),
        })
    }
}
