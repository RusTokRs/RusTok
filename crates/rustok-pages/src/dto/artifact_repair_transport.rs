use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{RebuildPageArtifactResult, ReplacePageArtifactBindingResult};

/// Bounded public receipt returned after one explicit immutable artifact rebuild.
///
/// Internal provenance row ids, publish operation ids, storage instance keys, idempotency keys and
/// reviewed runtime payloads are deliberately omitted from the transport result.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RebuildPageArtifactTransportResult {
    pub operation_id: Uuid,
    pub page_id: Uuid,
    pub locale: String,
    pub source_artifact_id: Uuid,
    pub rebuilt_artifact_id: Uuid,
    pub artifact_hash: String,
    pub materialization_hash: String,
    pub replayed: bool,
    pub rebuilt_at: String,
}

impl From<RebuildPageArtifactResult> for RebuildPageArtifactTransportResult {
    fn from(result: RebuildPageArtifactResult) -> Self {
        Self {
            operation_id: result.operation_id,
            page_id: result.page_id,
            locale: result.locale,
            source_artifact_id: result.source_artifact_id,
            rebuilt_artifact_id: result.rebuilt_artifact_id,
            artifact_hash: result.artifact_hash,
            materialization_hash: result.materialization_hash,
            replayed: result.replayed,
            rebuilt_at: result.rebuilt_at,
        }
    }
}

/// Bounded public receipt returned after explicit activation of a rebuilt artifact.
///
/// The transport does not echo the activation idempotency key or any retained provenance/runtime
/// payload. The rebuild receipt id remains visible because it is the explicit activation authority.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ActivateRebuiltPageArtifactTransportResult {
    pub operation_id: Uuid,
    pub page_id: Uuid,
    pub version: i32,
    pub locale: String,
    pub rebuild_operation_id: Uuid,
    pub previous_artifact_id: Uuid,
    pub replacement_artifact_id: Uuid,
    pub replacement_artifact_hash: String,
    pub replacement_materialization_hash: String,
    pub replayed: bool,
    pub replaced_at: String,
}

impl From<ReplacePageArtifactBindingResult> for ActivateRebuiltPageArtifactTransportResult {
    fn from(result: ReplacePageArtifactBindingResult) -> Self {
        Self {
            operation_id: result.operation_id,
            page_id: result.page_id,
            version: result.version,
            locale: result.locale,
            rebuild_operation_id: result.rebuild_operation_id,
            previous_artifact_id: result.previous_artifact_id,
            replacement_artifact_id: result.replacement_artifact_id,
            replacement_artifact_hash: result.replacement_artifact_hash,
            replacement_materialization_hash: result.replacement_materialization_hash,
            replayed: result.replayed,
            replaced_at: result.replaced_at,
        }
    }
}
