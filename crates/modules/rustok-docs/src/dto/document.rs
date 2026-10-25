use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{DocsError, DocsResult};

/// Representation of an append-only update item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocUpdateItemDto {
    pub seq_id: i64,
    pub data: Vec<u8>,
}

/// Synchronization payload for clients connecting to or polling a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSyncDto {
    pub page_id: Uuid,
    pub latest_seq_id: i64,
    pub snapshot: Option<Vec<u8>>,
    pub updates: Vec<DocUpdateItemDto>,
}

/// Request payload to apply a batch of binary CRDT deltas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyDeltaDto {
    pub deltas: Vec<Vec<u8>>,
}

impl ApplyDeltaDto {
    pub fn validate(&self) -> DocsResult<()> {
        if self.deltas.len() > 500 {
            return Err(DocsError::Validation(
                "Delta batch size cannot exceed 500 items".to_string(),
            ));
        }
        for (idx, delta) in self.deltas.iter().enumerate() {
            if delta.is_empty() {
                return Err(DocsError::Validation(format!(
                    "Delta item at index {idx} cannot be empty"
                )));
            }
            if delta.len() > 10 * 1024 * 1024 {
                return Err(DocsError::Validation(format!(
                    "Delta item at index {idx} exceeds maximum size of 10MB"
                )));
            }
        }
        Ok(())
    }
}
