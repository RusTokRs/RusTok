//! Transactional additive expansion and resumable idempotent data backfills.
//!
//! Provides durable page-level checkpoints and crash-safe reconciliation ensuring
//! every intermediate checkpoint preserves the single canonical representation
//! without dual read/write divergence.

use std::sync::Arc;
use chrono::{DateTime, Utc};
use hex::ToHex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

/// Status of a transactional additive data backfill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackfillStatus {
    InProgress,
    Converged,
    FailedClosed,
    UncertainOutcomeReconciling,
}

/// Durable checkpoint committed after every processed page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackfillCheckpoint {
    pub backfill_id: Uuid,
    pub operation_id: Uuid,
    pub module_slug: String,
    pub data_owner_id: Uuid,
    pub cursor: Option<String>,
    pub items_processed: u64,
    pub checkpoint_digest: String,
    pub status: BackfillStatus,
    pub updated_at: DateTime<Utc>,
}

impl BackfillCheckpoint {
    pub fn new(
        backfill_id: Uuid,
        operation_id: Uuid,
        module_slug: impl Into<String>,
        data_owner_id: Uuid,
    ) -> Self {
        let slug = module_slug.into();
        let digest = compute_checkpoint_digest(
            backfill_id,
            operation_id,
            &slug,
            data_owner_id,
            &None,
            0,
            BackfillStatus::InProgress,
        );
        Self {
            backfill_id,
            operation_id,
            module_slug: slug,
            data_owner_id,
            cursor: None,
            items_processed: 0,
            checkpoint_digest: digest,
            status: BackfillStatus::InProgress,
            updated_at: Utc::now(),
        }
    }
}

/// Computes SHA-256 digest over canonical checkpoint state.
pub fn compute_checkpoint_digest(
    backfill_id: Uuid,
    operation_id: Uuid,
    module_slug: &str,
    data_owner_id: Uuid,
    cursor: &Option<String>,
    items_processed: u64,
    status: BackfillStatus,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(backfill_id.as_bytes());
    hasher.update(operation_id.as_bytes());
    hasher.update(module_slug.as_bytes());
    hasher.update(data_owner_id.as_bytes());
    if let Some(c) = cursor {
        hasher.update(c.as_bytes());
    } else {
        hasher.update(b"<start>");
    }
    hasher.update(&items_processed.to_be_bytes());
    let status_byte = match status {
        BackfillStatus::InProgress => &[1_u8],
        BackfillStatus::Converged => &[2_u8],
        BackfillStatus::FailedClosed => &[3_u8],
        BackfillStatus::UncertainOutcomeReconciling => &[4_u8],
    };
    hasher.update(status_byte);
    format!("sha256:{}", hasher.finalize().encode_hex::<String>())
}

/// Result returned by each backfill page processor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackfillPageResult {
    pub processed_count: u64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Error)]
pub enum BackfillError {
    #[error("backfill is already converged")]
    AlreadyConverged,
    #[error("backfill failed closed: {0}")]
    FailedClosed(String),
    #[error("checkpoint digest mismatch: expected `{expected}`, found `{actual}`")]
    DigestMismatch { expected: String, actual: String },
    #[error("storage error: {0}")]
    Storage(String),
    #[error("processor error: {0}")]
    Processor(String),
}

/// In-memory or database port for durable backfill checkpoints.
#[async_trait::async_trait]
pub trait BackfillCheckpointStore: Send + Sync {
    async fn load_checkpoint(&self, backfill_id: Uuid) -> Result<Option<BackfillCheckpoint>, BackfillError>;
    async fn save_checkpoint(&self, checkpoint: &BackfillCheckpoint) -> Result<(), BackfillError>;
}

/// Durable memory store for backfill checkpoints.
#[derive(Clone, Default)]
pub struct InMemoryBackfillCheckpointStore {
    checkpoints: Arc<std::sync::Mutex<std::collections::HashMap<Uuid, BackfillCheckpoint>>>,
}

#[async_trait::async_trait]
impl BackfillCheckpointStore for InMemoryBackfillCheckpointStore {
    async fn load_checkpoint(&self, backfill_id: Uuid) -> Result<Option<BackfillCheckpoint>, BackfillError> {
        let guard = self.checkpoints.lock().map_err(|e| BackfillError::Storage(e.to_string()))?;
        Ok(guard.get(&backfill_id).cloned())
    }

    async fn save_checkpoint(&self, checkpoint: &BackfillCheckpoint) -> Result<(), BackfillError> {
        let mut guard = self.checkpoints.lock().map_err(|e| BackfillError::Storage(e.to_string()))?;
        guard.insert(checkpoint.backfill_id, checkpoint.clone());
        Ok(())
    }
}

/// Coordinator for transactional additive data expansions with uncertain-outcome crash recovery.
pub struct DataBackfillCoordinator<S: BackfillCheckpointStore> {
    store: S,
}

impl<S: BackfillCheckpointStore> DataBackfillCoordinator<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Initializes a new backfill or resumes an interrupted one from its last verified checkpoint.
    pub async fn start_or_resume(
        &self,
        backfill_id: Uuid,
        operation_id: Uuid,
        module_slug: &str,
        data_owner_id: Uuid,
    ) -> Result<BackfillCheckpoint, BackfillError> {
        if let Some(existing) = self.store.load_checkpoint(backfill_id).await? {
            // Verify checkpoint integrity
            let expected_digest = compute_checkpoint_digest(
                existing.backfill_id,
                existing.operation_id,
                &existing.module_slug,
                existing.data_owner_id,
                &existing.cursor,
                existing.items_processed,
                existing.status,
            );
            if existing.checkpoint_digest != expected_digest {
                return Err(BackfillError::DigestMismatch {
                    expected: expected_digest,
                    actual: existing.checkpoint_digest,
                });
            }
            if existing.status == BackfillStatus::UncertainOutcomeReconciling {
                // Reconcile uncertain outcome back to in-progress for safe resumption
                let mut reconciled = existing;
                reconciled.status = BackfillStatus::InProgress;
                reconciled.updated_at = Utc::now();
                reconciled.checkpoint_digest = compute_checkpoint_digest(
                    reconciled.backfill_id,
                    reconciled.operation_id,
                    &reconciled.module_slug,
                    reconciled.data_owner_id,
                    &reconciled.cursor,
                    reconciled.items_processed,
                    reconciled.status,
                );
                self.store.save_checkpoint(&reconciled).await?;
                return Ok(reconciled);
            }
            return Ok(existing);
        }

        let new_checkpoint = BackfillCheckpoint::new(backfill_id, operation_id, module_slug, data_owner_id);
        self.store.save_checkpoint(&new_checkpoint).await?;
        Ok(new_checkpoint)
    }

    /// Executes a single backfill page and commits the updated durable checkpoint atomically.
    pub async fn process_page<F, Fut>(
        &self,
        backfill_id: Uuid,
        page_processor: F,
    ) -> Result<BackfillCheckpoint, BackfillError>
    where
        F: FnOnce(Option<String>) -> Fut,
        Fut: std::future::Future<Output = Result<BackfillPageResult, String>>,
    {
        let current = self
            .store
            .load_checkpoint(backfill_id)
            .await?
            .ok_or_else(|| BackfillError::Storage("checkpoint not found".to_string()))?;

        if current.status == BackfillStatus::Converged {
            return Ok(current);
        }
        if current.status == BackfillStatus::FailedClosed {
            return Err(BackfillError::FailedClosed("backfill marked failed closed".to_string()));
        }

        let page_result = page_processor(current.cursor.clone())
            .await
            .map_err(BackfillError::Processor)?;

        let new_items = current.items_processed + page_result.processed_count;
        let new_status = if page_result.has_more {
            BackfillStatus::InProgress
        } else {
            BackfillStatus::Converged
        };

        let new_digest = compute_checkpoint_digest(
            current.backfill_id,
            current.operation_id,
            &current.module_slug,
            current.data_owner_id,
            &page_result.next_cursor,
            new_items,
            new_status,
        );

        let updated_checkpoint = BackfillCheckpoint {
            backfill_id: current.backfill_id,
            operation_id: current.operation_id,
            module_slug: current.module_slug,
            data_owner_id: current.data_owner_id,
            cursor: page_result.next_cursor,
            items_processed: new_items,
            checkpoint_digest: new_digest,
            status: new_status,
            updated_at: Utc::now(),
        };

        self.store.save_checkpoint(&updated_checkpoint).await?;
        Ok(updated_checkpoint)
    }

    /// Marks the backfill as entering uncertain-outcome reconciliation following a timeout or ambiguous crash.
    pub async fn record_uncertain_outcome(&self, backfill_id: Uuid) -> Result<BackfillCheckpoint, BackfillError> {
        let mut current = self
            .store
            .load_checkpoint(backfill_id)
            .await?
            .ok_or_else(|| BackfillError::Storage("checkpoint not found".to_string()))?;

        current.status = BackfillStatus::UncertainOutcomeReconciling;
        current.updated_at = Utc::now();
        current.checkpoint_digest = compute_checkpoint_digest(
            current.backfill_id,
            current.operation_id,
            &current.module_slug,
            current.data_owner_id,
            &current.cursor,
            current.items_processed,
            current.status,
        );

        self.store.save_checkpoint(&current).await?;
        Ok(current)
    }
}
