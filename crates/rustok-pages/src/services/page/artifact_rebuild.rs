use chrono::Utc;
use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use rustok_page_builder::{
    PageBuilderPublishRuntimeReviewError, PageBuilderReviewedPublishRuntime,
    PageBuilderStaticLandingMaterializationIdentity, compile_materialized_static_landing,
    sanitize_static_landing_project,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QuerySelect, TransactionTrait,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dto::{RebuildPageArtifactInput, RebuildPageArtifactResult};
use crate::entities::{page_artifact_rebuild_operation, page_publish_rebuild_source};
use crate::error::{PagesError, PagesResult};
use crate::services::page_builder_artifact::{CompiledLandingArtifact, PageBuilderArtifactService};

use super::PageService;
use super::publish_manifest::{RebuildSourceProvenance, is_sha256, rebuild_source_provenance_hash};

pub const PAGE_ARTIFACT_REBUILD_OPERATION_FORMAT: &str = "page_artifact_rebuild_operation_v1";
pub const PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT: &str =
    "PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT";
pub const PAGE_ARTIFACT_REBUILD_SOURCE_INVALID: &str = "PAGE_ARTIFACT_REBUILD_SOURCE_INVALID";
pub const PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY: &str =
    "PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY";
const MAX_REBUILD_IDEMPOTENCY_KEY_BYTES: usize = 191;

impl PageService {
    /// Rebuilds one exact retained Page Builder artifact as a new immutable storage instance.
    ///
    /// The command requires tenant-wide `pages:manage`, verifies the retained provenance and an
    /// explicitly reviewed runtime context, recompiles the sanitized historical source and appends
    /// one new artifact plus an idempotent receipt. It never updates the damaged artifact, changes a
    /// published binding, advances the page version, emits lifecycle events or rotates caches.
    pub async fn rebuild_immutable_artifact(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: RebuildPageArtifactInput,
    ) -> PagesResult<RebuildPageArtifactResult> {
        enforce_tenant_wide_manage(&security)?;
        if tenant_id.is_nil() || page_id.is_nil() || input.source_id.is_nil() {
            return Err(PagesError::validation(
                "artifact rebuild tenant, page and source ids must not be nil",
            ));
        }
        if !is_sha256(&input.expected_provenance_hash) {
            return Err(PagesError::validation(
                "artifact rebuild expected_provenance_hash must be SHA-256",
            ));
        }
        let idempotency_key = normalize_idempotency_key(&input.idempotency_key)?;
        let reviewed: PageBuilderReviewedPublishRuntime =
            input
                .runtime
                .try_into()
                .map_err(|error: PageBuilderPublishRuntimeReviewError| {
                    PagesError::publish_runtime_review_invalid(error.to_string())
                })?;
        let request_hash = stable_rebuild_hash(&(
            PAGE_ARTIFACT_REBUILD_OPERATION_FORMAT,
            tenant_id,
            page_id,
            input.source_id,
            input.expected_provenance_hash.as_str(),
            reviewed.review_hash.as_str(),
        ))?;

        let txn = self.db.begin().await?;
        if let Some(operation) =
            find_operation_in_tx(&txn, tenant_id, page_id, idempotency_key.as_str()).await?
        {
            ensure_same_request(
                &operation,
                input.source_id,
                input.expected_provenance_hash.as_str(),
                reviewed.review_hash.as_str(),
                request_hash.as_str(),
            )?;
            let result = result_from_record(operation, true)?;
            txn.commit().await?;
            return Ok(result);
        }

        let source = load_source_in_tx(&txn, tenant_id, page_id, input.source_id).await?;
        verify_source(&source)?;
        if source.provenance_hash != input.expected_provenance_hash {
            return Err(rebuild_source_invalid(
                "expected provenance does not match the selected immutable source",
            ));
        }
        if source.review_hash != reviewed.review_hash {
            return Err(PagesError::publish_runtime_review_invalid(
                "reviewed runtime hash does not match retained publish provenance",
            ));
        }

        let compiled = compile_exact_rebuild(&source, &reviewed)?;
        let operation_id = Uuid::new_v4();
        let (rebuilt_artifact_id, artifact_instance_key) =
            PageBuilderArtifactService::append_rebuilt_in_tx(
                &txn,
                tenant_id,
                page_id,
                &compiled,
                operation_id,
            )
            .await?;
        let now = Utc::now();
        let operation = page_artifact_rebuild_operation::ActiveModel {
            id: Set(operation_id),
            tenant_id: Set(tenant_id),
            page_id: Set(page_id),
            source_id: Set(source.id),
            source_publish_operation_id: Set(source.operation_id),
            locale: Set(source.locale.clone()),
            idempotency_key: Set(idempotency_key),
            request_hash: Set(request_hash),
            expected_provenance_hash: Set(source.provenance_hash.clone()),
            review_hash: Set(reviewed.review_hash),
            artifact_instance_key: Set(artifact_instance_key),
            source_artifact_id: Set(source.artifact_id),
            rebuilt_artifact_id: Set(rebuilt_artifact_id),
            rebuilt_artifact_hash: Set(compiled.artifact.artifact_hash.clone()),
            rebuilt_materialization_hash: Set(compiled.materialization_hash.clone()),
            created_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;
        let result = result_from_record(operation, false)?;
        txn.commit().await?;
        Ok(result)
    }
}

async fn load_source_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    source_id: Uuid,
) -> PagesResult<page_publish_rebuild_source::Model> {
    let query = || {
        page_publish_rebuild_source::Entity::find_by_id(source_id)
            .filter(page_publish_rebuild_source::Column::TenantId.eq(tenant_id))
            .filter(page_publish_rebuild_source::Column::PageId.eq(page_id))
    };
    match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    }
    .ok_or_else(|| rebuild_source_invalid("immutable rebuild source is unavailable"))
}

async fn find_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    idempotency_key: &str,
) -> PagesResult<Option<page_artifact_rebuild_operation::Model>> {
    let query = || {
        page_artifact_rebuild_operation::Entity::find()
            .filter(page_artifact_rebuild_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_rebuild_operation::Column::PageId.eq(page_id))
            .filter(page_artifact_rebuild_operation::Column::IdempotencyKey.eq(idempotency_key))
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    })
}

fn compile_exact_rebuild(
    source: &page_publish_rebuild_source::Model,
    reviewed: &PageBuilderReviewedPublishRuntime,
) -> PagesResult<CompiledLandingArtifact> {
    let sanitized =
        sanitize_static_landing_project(&source.sanitized_project).map_err(|error| {
            rebuild_source_invalid(format!(
                "retained source failed canonical sanitization: {error}",
            ))
        })?;
    sanitized.verify_integrity().map_err(|error| {
        rebuild_source_invalid(format!(
            "retained sanitization envelope failed integrity: {error}",
        ))
    })?;
    if sanitized.sanitized_hash() != source.sanitized_hash {
        return Err(rebuild_source_invalid(
            "retained sanitized source hash drifted",
        ));
    }

    let stored_identity: PageBuilderStaticLandingMaterializationIdentity =
        serde_json::from_value(source.materialization_identity.clone()).map_err(|error| {
            rebuild_source_invalid(format!(
                "retained materialization identity is invalid: {error}",
            ))
        })?;
    let reviewed_context_hash = reviewed
        .runtime_context_hash()
        .map_err(|error| PagesError::publish_runtime_review_invalid(error.to_string()))?;
    if stored_identity.runtime_context_hash != reviewed_context_hash
        || stored_identity.runtime_scenario_id.as_deref() != Some(reviewed.scenario_id.as_str())
    {
        return Err(PagesError::publish_runtime_review_invalid(
            "reviewed runtime context does not match retained materialization identity",
        ));
    }

    let runtime = reviewed
        .preview_runtime()
        .map_err(|error| PagesError::publish_runtime_review_invalid(error.to_string()))?;
    let materialized = compile_materialized_static_landing(sanitized.project_data(), runtime)
        .map_err(|error| {
            PagesError::artifact_integrity(format!(
                "artifact rebuild materialization failed: {error}",
            ))
        })?;
    materialized.verify_integrity().map_err(|error| {
        PagesError::artifact_integrity(format!(
            "rebuilt artifact failed materialization integrity: {error}",
        ))
    })?;
    if materialized.artifact.pages.len() != 1 {
        return Err(PagesError::artifact_integrity(format!(
            "artifact rebuild requires exactly one Fly page; found {}",
            materialized.artifact.pages.len(),
        )));
    }
    if materialized.artifact.identity.source_hash != source.source_hash
        || materialized.artifact.artifact_hash != source.artifact_hash
        || materialized.identity.materialization_hash != source.materialization_hash
    {
        return Err(PagesError::artifact_integrity(
            "rebuilt artifact identities do not match retained publish provenance",
        ));
    }
    let materialization_identity =
        serde_json::to_value(&materialized.identity).map_err(|error| {
            PagesError::artifact_integrity(format!(
                "unable to encode rebuilt materialization identity: {error}",
            ))
        })?;
    let runtime_snapshots =
        serde_json::to_value(&materialized.runtime_snapshots).map_err(|error| {
            PagesError::artifact_integrity(format!(
                "unable to encode rebuilt runtime snapshots: {error}",
            ))
        })?;
    if materialization_identity != source.materialization_identity
        || runtime_snapshots != source.runtime_snapshots
    {
        return Err(PagesError::artifact_integrity(
            "rebuilt runtime evidence does not exactly reproduce retained provenance",
        ));
    }
    let page = materialized.artifact.pages[0].clone();
    Ok(CompiledLandingArtifact {
        locale: source.locale.clone(),
        artifact: materialized.artifact,
        page,
        materialization_hash: source.materialization_hash.clone(),
        materialization_identity,
        runtime_snapshots,
    })
}

fn verify_source(source: &page_publish_rebuild_source::Model) -> PagesResult<()> {
    if source.id.is_nil()
        || source.operation_id.is_nil()
        || source.tenant_id.is_nil()
        || source.page_id.is_nil()
        || source.page_body_id.is_nil()
        || source.artifact_id.is_nil()
        || source.locale.trim().is_empty()
        || source.locale.trim() != source.locale
        || source.source_format != PAGE_BUILDER_DOCUMENT_FORMAT
        || source.source_revision.trim().is_empty()
    {
        return Err(rebuild_source_invalid(
            "retained source identity is invalid",
        ));
    }
    for value in [
        source.sanitized_hash.as_str(),
        source.source_hash.as_str(),
        source.review_hash.as_str(),
        source.artifact_hash.as_str(),
        source.materialization_hash.as_str(),
        source.provenance_hash.as_str(),
    ] {
        if !is_sha256(value) {
            return Err(rebuild_source_invalid(
                "retained source contains an invalid SHA-256 identity",
            ));
        }
    }
    let expected = rebuild_source_provenance_hash(RebuildSourceProvenance {
        operation_id: source.operation_id,
        tenant_id: source.tenant_id,
        page_id: source.page_id,
        page_body_id: source.page_body_id,
        locale: source.locale.as_str(),
        source_format: source.source_format.as_str(),
        source_revision: source.source_revision.as_str(),
        artifact_id: source.artifact_id,
        sanitized_hash: source.sanitized_hash.as_str(),
        source_hash: source.source_hash.as_str(),
        review_hash: source.review_hash.as_str(),
        artifact_hash: source.artifact_hash.as_str(),
        materialization_hash: source.materialization_hash.as_str(),
        materialization_identity: &source.materialization_identity,
        runtime_snapshots: &source.runtime_snapshots,
    })
    .map_err(|error| rebuild_source_invalid(error.to_string()))?;
    if expected != source.provenance_hash {
        return Err(rebuild_source_invalid("retained provenance hash mismatch"));
    }
    Ok(())
}

fn ensure_same_request(
    operation: &page_artifact_rebuild_operation::Model,
    source_id: Uuid,
    expected_provenance_hash: &str,
    review_hash: &str,
    request_hash: &str,
) -> PagesResult<()> {
    verify_operation(operation)?;
    if operation.source_id != source_id
        || operation.expected_provenance_hash != expected_provenance_hash
        || operation.review_hash != review_hash
        || operation.request_hash != request_hash
    {
        return Err(rebuild_idempotency_conflict(
            "idempotency key is bound to another rebuild request",
        ));
    }
    Ok(())
}

fn verify_operation(operation: &page_artifact_rebuild_operation::Model) -> PagesResult<()> {
    if operation.id.is_nil()
        || operation.tenant_id.is_nil()
        || operation.page_id.is_nil()
        || operation.source_id.is_nil()
        || operation.source_publish_operation_id.is_nil()
        || operation.source_artifact_id.is_nil()
        || operation.rebuilt_artifact_id.is_nil()
        || operation.locale.trim().is_empty()
        || operation.idempotency_key.trim().is_empty()
        || !is_sha256(&operation.request_hash)
        || !is_sha256(&operation.expected_provenance_hash)
        || !is_sha256(&operation.review_hash)
        || !is_sha256(&operation.rebuilt_artifact_hash)
        || !is_sha256(&operation.rebuilt_materialization_hash)
        || operation.artifact_instance_key != format!("rebuild:{}", operation.id)
    {
        return Err(rebuild_operation_integrity(
            "stored artifact rebuild receipt failed integrity validation",
        ));
    }
    Ok(())
}

fn result_from_record(
    operation: page_artifact_rebuild_operation::Model,
    replayed: bool,
) -> PagesResult<RebuildPageArtifactResult> {
    verify_operation(&operation)?;
    Ok(RebuildPageArtifactResult {
        operation_id: operation.id,
        page_id: operation.page_id,
        source_id: operation.source_id,
        source_publish_operation_id: operation.source_publish_operation_id,
        locale: operation.locale,
        source_artifact_id: operation.source_artifact_id,
        rebuilt_artifact_id: operation.rebuilt_artifact_id,
        artifact_instance_key: operation.artifact_instance_key,
        artifact_hash: operation.rebuilt_artifact_hash,
        materialization_hash: operation.rebuilt_materialization_hash,
        replayed,
        rebuilt_at: operation.created_at.to_string(),
    })
}

fn stable_rebuild_hash(value: &impl Serialize) -> PagesResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        rebuild_operation_integrity(format!(
            "unable to encode artifact rebuild request identity: {error}",
        ))
    })?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn rebuild_idempotency_conflict(message: impl Into<String>) -> PagesError {
    PagesError::publish_idempotency_conflict(format!(
        "{PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT}: {}",
        message.into(),
    ))
}

fn rebuild_source_invalid(message: impl Into<String>) -> PagesError {
    PagesError::publish_operation_integrity(format!(
        "{PAGE_ARTIFACT_REBUILD_SOURCE_INVALID}: {}",
        message.into(),
    ))
}

fn rebuild_operation_integrity(message: impl Into<String>) -> PagesError {
    PagesError::publish_operation_integrity(format!(
        "{PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY}: {}",
        message.into(),
    ))
}

fn enforce_tenant_wide_manage(security: &SecurityContext) -> PagesResult<()> {
    if matches!(
        security.get_scope(Resource::Pages, Action::Manage),
        PermissionScope::All
    ) {
        Ok(())
    } else {
        Err(PagesError::forbidden(
            "artifact rebuild requires tenant-wide pages:manage",
        ))
    }
}

fn normalize_idempotency_key(value: &str) -> PagesResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.len() > MAX_REBUILD_IDEMPOTENCY_KEY_BYTES {
        return Err(PagesError::validation(format!(
            "artifact rebuild idempotency_key must contain 1 to {MAX_REBUILD_IDEMPOTENCY_KEY_BYTES} bytes",
        )));
    }
    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_idempotency_key_is_bounded() {
        assert!(normalize_idempotency_key("rebuild-1").is_ok());
        assert!(normalize_idempotency_key("").is_err());
        assert!(normalize_idempotency_key(&"x".repeat(192)).is_err());
    }
}
