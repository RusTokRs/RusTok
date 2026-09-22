use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use rustok_page_builder::sanitize_static_landing_project;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::{
    page_body, page_publish_operation, page_publish_operation_artifact,
    page_publish_rebuild_source, page_published_landing_artifact, page_static_landing_artifact,
};
use crate::error::{PagesError, PagesResult};

pub(super) const PAGE_PUBLISH_REBUILD_SOURCE_FORMAT: &str = "pages_publish_rebuild_source_v1";

struct PreparedPublishArtifactManifest {
    locale: String,
    page_body_id: Uuid,
    source_format: String,
    source_revision: String,
    artifact_id: Uuid,
    artifact_hash: String,
    materialization_hash: String,
    sanitized_project: Value,
    sanitized_hash: String,
    source_hash: String,
    materialization_identity: Value,
    runtime_snapshots: Value,
    provenance_hash: String,
}

pub(super) struct RebuildSourceProvenance<'a> {
    pub(super) operation_id: Uuid,
    pub(super) tenant_id: Uuid,
    pub(super) page_id: Uuid,
    pub(super) page_body_id: Uuid,
    pub(super) locale: &'a str,
    pub(super) source_format: &'a str,
    pub(super) source_revision: &'a str,
    pub(super) artifact_id: Uuid,
    pub(super) sanitized_hash: &'a str,
    pub(super) source_hash: &'a str,
    pub(super) review_hash: &'a str,
    pub(super) artifact_hash: &'a str,
    pub(super) materialization_hash: &'a str,
    pub(super) materialization_identity: &'a Value,
    pub(super) runtime_snapshots: &'a Value,
}

pub(crate) async fn persist_publish_manifest_after_save<C>(
    db: &C,
    operation: &page_publish_operation::Model,
) -> PagesResult<()>
where
    C: ConnectionTrait,
{
    let existing_manifest = page_publish_operation_artifact::Entity::find()
        .filter(page_publish_operation_artifact::Column::OperationId.eq(operation.id))
        .count(db)
        .await?;
    let existing_sources = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::OperationId.eq(operation.id))
        .count(db)
        .await?;
    if existing_manifest != 0 || existing_sources != 0 {
        return Err(PagesError::publish_operation_integrity(format!(
            "publish operation `{}` already has an artifact manifest or rebuild provenance",
            operation.id
        )));
    }

    let bindings = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(operation.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(operation.page_id))
        .order_by_asc(page_published_landing_artifact::Column::Locale)
        .all(db)
        .await?;
    if bindings.is_empty() {
        return Err(PagesError::publish_operation_integrity(
            "publish receipt cannot be stored without immutable artifact bindings",
        ));
    }

    let mut rows = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let artifact = page_static_landing_artifact::Entity::find_by_id(binding.artifact_id)
            .filter(page_static_landing_artifact::Column::TenantId.eq(operation.tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(operation.page_id))
            .filter(page_static_landing_artifact::Column::Locale.eq(&binding.locale))
            .one(db)
            .await?
            .ok_or_else(|| {
                PagesError::publish_operation_integrity(format!(
                    "published binding `{}` references a missing immutable artifact",
                    binding.page_body_id
                ))
            })?;
        let body = page_body::Entity::find_by_id(binding.page_body_id)
            .filter(page_body::Column::TenantId.eq(operation.tenant_id))
            .filter(page_body::Column::PageId.eq(operation.page_id))
            .filter(page_body::Column::Locale.eq(&binding.locale))
            .one(db)
            .await?
            .ok_or_else(|| {
                PagesError::publish_operation_integrity(format!(
                    "published binding `{}` references a missing source body",
                    binding.page_body_id
                ))
            })?;
        if body.format != PAGE_BUILDER_DOCUMENT_FORMAT {
            return Err(PagesError::publish_operation_integrity(format!(
                "published source body `{}` is not a Page Builder document",
                body.id
            )));
        }

        let project_data: Value = serde_json::from_str(&body.content).map_err(|error| {
            PagesError::publish_operation_integrity(format!(
                "published source body `{}` is not valid Page Builder JSON: {error}",
                body.id
            ))
        })?;
        let sanitized = sanitize_static_landing_project(&project_data).map_err(|error| {
            PagesError::publish_operation_integrity(format!(
                "published source body `{}` failed rebuild provenance sanitization: {error}",
                body.id
            ))
        })?;
        sanitized.verify_integrity().map_err(|error| {
            PagesError::publish_operation_integrity(format!(
                "published source body `{}` produced invalid rebuild provenance: {error}",
                body.id
            ))
        })?;

        let materialization_hash = artifact.materialization_hash.clone().ok_or_else(|| {
            PagesError::publish_operation_integrity(format!(
                "reviewed artifact `{}` is missing materialization hash provenance",
                artifact.id
            ))
        })?;
        let materialization_identity =
            artifact.materialization_identity.clone().ok_or_else(|| {
                PagesError::publish_operation_integrity(format!(
                    "reviewed artifact `{}` is missing materialization identity provenance",
                    artifact.id
                ))
            })?;
        let runtime_snapshots = artifact.runtime_snapshots.clone().ok_or_else(|| {
            PagesError::publish_operation_integrity(format!(
                "reviewed artifact `{}` is missing runtime snapshot provenance",
                artifact.id
            ))
        })?;
        for (label, value) in [
            ("operation review", operation.review_hash.as_str()),
            ("artifact source", artifact.source_hash.as_str()),
            ("artifact", artifact.artifact_hash.as_str()),
            ("materialization", materialization_hash.as_str()),
            ("sanitized project", sanitized.sanitized_hash()),
        ] {
            if !is_sha256(value) {
                return Err(PagesError::publish_operation_integrity(format!(
                    "{label} hash is invalid for immutable artifact `{}`",
                    artifact.id
                )));
            }
        }

        let source_revision = body.updated_at.to_string();
        let sanitized_hash = sanitized.sanitized_hash().to_string();
        let provenance_hash = rebuild_source_provenance_hash(RebuildSourceProvenance {
            operation_id: operation.id,
            tenant_id: operation.tenant_id,
            page_id: operation.page_id,
            page_body_id: body.id,
            locale: body.locale.as_str(),
            source_format: body.format.as_str(),
            source_revision: source_revision.as_str(),
            artifact_id: artifact.id,
            sanitized_hash: sanitized_hash.as_str(),
            source_hash: artifact.source_hash.as_str(),
            review_hash: operation.review_hash.as_str(),
            artifact_hash: artifact.artifact_hash.as_str(),
            materialization_hash: materialization_hash.as_str(),
            materialization_identity: &materialization_identity,
            runtime_snapshots: &runtime_snapshots,
        })?;

        rows.push(PreparedPublishArtifactManifest {
            locale: binding.locale,
            page_body_id: body.id,
            source_format: body.format,
            source_revision,
            artifact_id: artifact.id,
            artifact_hash: artifact.artifact_hash,
            materialization_hash,
            sanitized_project: sanitized.project_data().clone(),
            sanitized_hash,
            source_hash: artifact.source_hash,
            materialization_identity,
            runtime_snapshots,
            provenance_hash,
        });
    }

    let artifact_manifest_hash = stable_hash(
        &rows
            .iter()
            .map(|row| {
                (
                    row.locale.as_str(),
                    row.artifact_hash.as_str(),
                    Some(row.materialization_hash.as_str()),
                )
            })
            .collect::<Vec<_>>(),
    )?;
    if artifact_manifest_hash != operation.artifact_set_hash {
        return Err(PagesError::publish_operation_integrity(
            "current immutable bindings do not match the publish artifact_set_hash",
        ));
    }

    let sanitized_manifest_hash = stable_hash(
        &rows
            .iter()
            .map(|row| (row.locale.as_str(), row.sanitized_hash.as_str()))
            .collect::<Vec<_>>(),
    )?;
    if sanitized_manifest_hash != operation.sanitized_set_hash {
        return Err(PagesError::publish_operation_integrity(
            "current immutable source snapshots do not match the publish sanitized_set_hash",
        ));
    }

    for row in rows {
        page_publish_operation_artifact::ActiveModel {
            id: Set(Uuid::new_v4()),
            operation_id: Set(operation.id),
            tenant_id: Set(operation.tenant_id),
            page_id: Set(operation.page_id),
            locale: Set(row.locale.clone()),
            artifact_id: Set(row.artifact_id),
            artifact_hash: Set(row.artifact_hash.clone()),
            materialization_hash: Set(Some(row.materialization_hash.clone())),
            created_at: Set(operation.created_at),
        }
        .insert(db)
        .await?;

        page_publish_rebuild_source::ActiveModel {
            id: Set(Uuid::new_v4()),
            operation_id: Set(operation.id),
            tenant_id: Set(operation.tenant_id),
            page_id: Set(operation.page_id),
            page_body_id: Set(row.page_body_id),
            locale: Set(row.locale),
            artifact_id: Set(row.artifact_id),
            source_format: Set(row.source_format),
            source_revision: Set(row.source_revision),
            sanitized_project: Set(row.sanitized_project),
            sanitized_hash: Set(row.sanitized_hash),
            source_hash: Set(row.source_hash),
            review_hash: Set(operation.review_hash.clone()),
            artifact_hash: Set(row.artifact_hash),
            materialization_hash: Set(row.materialization_hash),
            materialization_identity: Set(row.materialization_identity),
            runtime_snapshots: Set(row.runtime_snapshots),
            provenance_hash: Set(row.provenance_hash),
            created_at: Set(operation.created_at),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

pub(super) fn rebuild_source_provenance_hash(
    source: RebuildSourceProvenance<'_>,
) -> PagesResult<String> {
    let materialization_identity_hash = stable_hash(source.materialization_identity)?;
    let runtime_snapshots_hash = stable_hash(source.runtime_snapshots)?;
    stable_hash(&(
        PAGE_PUBLISH_REBUILD_SOURCE_FORMAT,
        source.operation_id,
        source.tenant_id,
        source.page_id,
        source.page_body_id,
        source.locale,
        source.source_format,
        source.source_revision,
        source.artifact_id,
        source.sanitized_hash,
        source.source_hash,
        source.review_hash,
        source.artifact_hash,
        source.materialization_hash,
        materialization_identity_hash.as_str(),
        runtime_snapshots_hash.as_str(),
    ))
}

fn stable_hash(value: &impl Serialize) -> PagesResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        PagesError::publish_operation_integrity(format!(
            "unable to encode publish artifact manifest: {error}"
        ))
    })?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
