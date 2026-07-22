use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    QueryOrder,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::{
    page_publish_operation, page_publish_operation_artifact, page_published_landing_artifact,
    page_static_landing_artifact,
};
use crate::error::{PagesError, PagesResult};

pub(crate) async fn persist_publish_manifest_after_save<C>(
    db: &C,
    operation: &page_publish_operation::Model,
) -> PagesResult<()>
where
    C: ConnectionTrait,
{
    let existing = page_publish_operation_artifact::Entity::find()
        .filter(page_publish_operation_artifact::Column::OperationId.eq(operation.id))
        .count(db)
        .await?;
    if existing != 0 {
        return Err(PagesError::publish_operation_integrity(format!(
            "publish operation `{}` already has an artifact manifest",
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
        if !is_sha256(&artifact.artifact_hash)
            || artifact
                .materialization_hash
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
        {
            return Err(PagesError::publish_operation_integrity(format!(
                "immutable artifact `{}` has invalid publish identity evidence",
                artifact.id
            )));
        }
        rows.push((
            binding.locale,
            artifact.id,
            artifact.artifact_hash,
            artifact.materialization_hash,
        ));
    }

    let manifest_hash = stable_hash(
        &rows
            .iter()
            .map(|(locale, _, artifact_hash, materialization_hash)| {
                (
                    locale.as_str(),
                    artifact_hash.as_str(),
                    materialization_hash.as_deref(),
                )
            })
            .collect::<Vec<_>>(),
    )?;
    if manifest_hash != operation.artifact_set_hash {
        return Err(PagesError::publish_operation_integrity(
            "current immutable bindings do not match the publish artifact_set_hash",
        ));
    }

    for (locale, artifact_id, artifact_hash, materialization_hash) in rows {
        page_publish_operation_artifact::ActiveModel {
            id: Set(Uuid::new_v4()),
            operation_id: Set(operation.id),
            tenant_id: Set(operation.tenant_id),
            page_id: Set(operation.page_id),
            locale: Set(locale),
            artifact_id: Set(artifact_id),
            artifact_hash: Set(artifact_hash),
            materialization_hash: Set(materialization_hash),
            created_at: Set(operation.created_at),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

fn stable_hash(value: &impl Serialize) -> PagesResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        PagesError::publish_operation_integrity(format!(
            "unable to encode publish artifact manifest: {error}"
        ))
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
