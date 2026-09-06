use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Explicit tenant-admin request to activate one rebuilt immutable artifact.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplacePageArtifactBindingInput {
    pub rebuild_operation_id: Uuid,
    pub expected_version: i32,
    pub expected_current_artifact_id: Uuid,
    pub idempotency_key: String,
}

/// Durable receipt for one explicit immutable artifact binding replacement.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplacePageArtifactBindingResult {
    pub operation_id: Uuid,
    pub page_id: Uuid,
    pub version: i32,
    pub locale: String,
    pub idempotency_key: String,
    pub rebuild_operation_id: Uuid,
    pub previous_artifact_id: Uuid,
    pub replacement_artifact_id: Uuid,
    pub replacement_artifact_hash: String,
    pub replacement_materialization_hash: String,
    pub replayed: bool,
    pub replaced_at: String,
}
