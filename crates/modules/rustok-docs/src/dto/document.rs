use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
