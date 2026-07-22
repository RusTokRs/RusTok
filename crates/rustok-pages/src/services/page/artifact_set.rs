use rustok_core::CONTENT_FORMAT_GRAPESJS;
use sea_orm::{
    ColumnTrait, DatabaseTransaction, DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::{
    page_body, page_publish_operation, page_publish_operation_artifact,
    page_published_landing_artifact, page_static_landing_artifact,
};
use crate::error::{PagesError, PagesResult};
use crate::services::PageBuilderArtifactService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ArtifactSetMember {
    pub locale: String,
    pub artifact_id: Uuid,
    pub artifact_hash: String,
    pub materialization_hash: Option<String>,
}

impl ArtifactSetMember {
    pub(super) fn new(
        locale: impl Into<String>,
        artifact_id: Uuid,
        artifact_hash: impl Into<String>,
        materialization_hash: Option<String>,
    ) -> Self {
        Self {
            locale: locale.into(),
            artifact_id,
            artifact_hash: artifact_hash.into(),
            materialization_hash,
        }
    }
}

pub(super) fn artifact_set_hash(members: &[ArtifactSetMember]) -> PagesResult<String> {
    validate_member_identity(members)?;
    let mut identity = members
        .iter()
        .map(|member| {
            (
                member.locale.as_str(),
                member.artifact_hash.as_str(),
                member.materialization_hash.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    identity.sort_by(|left, right| left.0.cmp(right.0));
    stable_hash(&identity)
}

pub(super) async fn load_publish_manifest_in_tx(
    txn: &DatabaseTransaction,
    operation: &page_publish_operation::Model,
) -> PagesResult<Vec<ArtifactSetMember>> {
    let query = || {
        page_publish_operation_artifact::Entity::find()
            .filter(page_publish_operation_artifact::Column::OperationId.eq(operation.id))
            .filter(page_publish_operation_artifact::Column::TenantId.eq(operation.tenant_id))
            .filter(page_publish_operation_artifact::Column::PageId.eq(operation.page_id))
            .order_by_asc(page_publish_operation_artifact::Column::Locale)
    };
    let rows = match txn.get_database_backend() {
        DbBackend::Sqlite => query().all(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().all(txn).await?,
    };
    if rows.is_empty() {
        return Err(PagesError::rollback_target_unavailable(format!(
            "publish operation `{}` has no immutable artifact manifest",
            operation.id
        )));
    }
    let members = rows
        .into_iter()
        .map(|row| {
            ArtifactSetMember::new(
                row.locale,
                row.artifact_id,
                row.artifact_hash,
                row.materialization_hash,
            )
        })
        .collect::<Vec<_>>();
    verify_members_in_tx(txn, operation.tenant_id, operation.page_id, &members).await?;
    let manifest_hash = artifact_set_hash(&members)?;
    if manifest_hash != operation.artifact_set_hash {
        return Err(PagesError::rollback_target_unavailable(format!(
            "publish operation `{}` artifact manifest failed hash validation",
            operation.id
        )));
    }
    Ok(members)
}

pub(super) async fn load_current_published_set_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
) -> PagesResult<Vec<ArtifactSetMember>> {
    let query = || {
        page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.eq(page_id))
            .order_by_asc(page_published_landing_artifact::Column::Locale)
    };
    let bindings = match txn.get_database_backend() {
        DbBackend::Sqlite => query().all(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().all(txn).await?,
    };
    if bindings.is_empty() {
        return Err(PagesError::rollback_target_unavailable(
            "published page has no current immutable artifact bindings",
        ));
    }
    let mut members = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let record = page_static_landing_artifact::Entity::find_by_id(binding.artifact_id)
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
            .filter(page_static_landing_artifact::Column::Locale.eq(&binding.locale))
            .one(txn)
            .await?
            .ok_or_else(|| {
                PagesError::artifact_integrity(format!(
                    "published binding `{}` references a missing immutable artifact",
                    binding.page_body_id
                ))
            })?;
        PageBuilderArtifactService::bind_existing_body_in_tx(
            txn,
            tenant_id,
            page_id,
            &binding.locale,
            binding.artifact_id,
        )
        .await?;
        members.push(ArtifactSetMember::new(
            record.locale,
            record.id,
            record.artifact_hash,
            record.materialization_hash,
        ));
    }
    validate_member_identity(&members)?;
    Ok(members)
}

pub(super) async fn replace_current_published_set_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    members: &[ArtifactSetMember],
) -> PagesResult<()> {
    verify_members_in_tx(txn, tenant_id, page_id, members).await?;
    let body_query = || {
        page_body::Entity::find()
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::PageId.eq(page_id))
            .order_by_asc(page_body::Column::Locale)
    };
    let bodies = match txn.get_database_backend() {
        DbBackend::Sqlite => body_query().all(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => {
            body_query().lock_exclusive().all(txn).await?
        }
    };
    for member in members {
        if !bodies
            .iter()
            .any(|body| body.locale == member.locale && body.format == CONTENT_FORMAT_GRAPESJS)
        {
            return Err(PagesError::rollback_target_unavailable(format!(
                "rollback target locale `{}` has no current Page Builder body",
                member.locale
            )));
        }
    }

    page_published_landing_artifact::Entity::delete_many()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(page_id))
        .exec(txn)
        .await?;

    for member in members {
        PageBuilderArtifactService::bind_existing_body_in_tx(
            txn,
            tenant_id,
            page_id,
            &member.locale,
            member.artifact_id,
        )
        .await?;
    }
    Ok(())
}

async fn verify_members_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    members: &[ArtifactSetMember],
) -> PagesResult<()> {
    validate_member_identity(members)?;
    for member in members {
        let record = page_static_landing_artifact::Entity::find_by_id(member.artifact_id)
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
            .filter(page_static_landing_artifact::Column::Locale.eq(&member.locale))
            .one(txn)
            .await?
            .ok_or_else(|| {
                PagesError::rollback_target_unavailable(format!(
                    "immutable artifact `{}` for locale `{}` is unavailable",
                    member.artifact_id, member.locale
                ))
            })?;
        if record.artifact_hash != member.artifact_hash
            || record.materialization_hash != member.materialization_hash
        {
            return Err(PagesError::rollback_target_unavailable(format!(
                "immutable artifact `{}` no longer matches its publish manifest",
                member.artifact_id
            )));
        }
    }
    Ok(())
}

fn validate_member_identity(members: &[ArtifactSetMember]) -> PagesResult<()> {
    if members.is_empty() {
        return Err(PagesError::rollback_target_unavailable(
            "immutable artifact set must not be empty",
        ));
    }
    let mut locales = std::collections::BTreeSet::new();
    for member in members {
        if member.artifact_id.is_nil()
            || member.locale.trim().is_empty()
            || !is_sha256(&member.artifact_hash)
            || member
                .materialization_hash
                .as_deref()
                .is_some_and(|hash| !is_sha256(hash))
            || !locales.insert(member.locale.clone())
        {
            return Err(PagesError::rollback_target_unavailable(
                "immutable artifact set contains invalid or duplicate identity evidence",
            ));
        }
    }
    Ok(())
}

fn stable_hash(value: &impl Serialize) -> PagesResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        PagesError::rollback_operation_integrity(format!(
            "unable to encode page artifact set identity: {error}"
        ))
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
