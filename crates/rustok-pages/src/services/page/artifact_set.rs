use rustok_core::CONTENT_FORMAT_GRAPESJS;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::{
    page, page_artifact_binding_replacement_operation, page_artifact_rebuild_operation, page_body,
    page_publish_operation, page_publish_operation_artifact, page_publish_rebuild_source,
    page_published_landing_artifact, page_rollback_operation, page_static_landing_artifact,
};
use crate::error::{PagesError, PagesResult};
use crate::services::PageBuilderArtifactService;

use super::artifact_binding_replacement::PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT;
use super::publish_manifest::{RebuildSourceProvenance, is_sha256, rebuild_source_provenance_hash};

const PAGE_ROLLBACK_ACTIVATION_ANCHOR_FORMAT: &str = "page_rollback_operation_v1";
const MAX_RECOVERED_ACTIVATION_PREFIX: usize = 256;

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
    match load_strict_publish_manifest_in_tx(txn, operation).await {
        Ok(members) => Ok(members),
        Err(PagesError::RollbackTargetUnavailable(strict_message)) => {
            match load_recovered_current_publish_set_in_tx(txn, operation).await {
                Ok(members) => Ok(members),
                Err(PagesError::Database(error)) => Err(PagesError::Database(error)),
                Err(_) => Err(PagesError::RollbackTargetUnavailable(strict_message)),
            }
        }
        Err(error) => Err(error),
    }
}

async fn load_strict_publish_manifest_in_tx(
    txn: &DatabaseTransaction,
    operation: &page_publish_operation::Model,
) -> PagesResult<Vec<ArtifactSetMember>> {
    let rows = load_publish_manifest_rows_in_tx(txn, operation).await?;
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

async fn load_publish_manifest_rows_in_tx(
    txn: &DatabaseTransaction,
    operation: &page_publish_operation::Model,
) -> PagesResult<Vec<page_publish_operation_artifact::Model>> {
    let query = || {
        page_publish_operation_artifact::Entity::find()
            .filter(page_publish_operation_artifact::Column::OperationId.eq(operation.id))
            .filter(page_publish_operation_artifact::Column::TenantId.eq(operation.tenant_id))
            .filter(page_publish_operation_artifact::Column::PageId.eq(operation.page_id))
            .order_by_asc(page_publish_operation_artifact::Column::Locale)
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().all(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().all(txn).await?,
    })
}

/// Recovers only the identity of the currently active publish cursor after an explicit immutable
/// artifact repair. Historical rollback targets still require their original manifest and live
/// immutable artifacts through `load_strict_publish_manifest_in_tx`.
async fn load_recovered_current_publish_set_in_tx(
    txn: &DatabaseTransaction,
    operation: &page_publish_operation::Model,
) -> PagesResult<Vec<ArtifactSetMember>> {
    let page_query = || {
        page::Entity::find_by_id(operation.page_id)
            .filter(page::Column::TenantId.eq(operation.tenant_id))
    };
    let current_page = match txn.get_database_backend() {
        DbBackend::Sqlite => page_query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => page_query().lock_shared().one(txn).await?,
    }
    .ok_or_else(|| {
        PagesError::rollback_target_unavailable("repaired publish cursor page is unavailable")
    })?;
    if current_page.status != "published" || current_page.version < operation.result_version {
        return Err(PagesError::rollback_target_unavailable(
            "repaired publish cursor is not the current published page state",
        ));
    }

    let current_members =
        load_current_published_set_in_tx(txn, operation.tenant_id, operation.page_id).await?;
    if artifact_set_hash(&current_members)? != operation.artifact_set_hash {
        return Err(PagesError::rollback_target_unavailable(
            "current repaired artifact set does not match the selected publish receipt",
        ));
    }

    let source_query = || {
        page_publish_rebuild_source::Entity::find()
            .filter(page_publish_rebuild_source::Column::OperationId.eq(operation.id))
            .filter(page_publish_rebuild_source::Column::TenantId.eq(operation.tenant_id))
            .filter(page_publish_rebuild_source::Column::PageId.eq(operation.page_id))
            .order_by_asc(page_publish_rebuild_source::Column::Locale)
    };
    let sources = match txn.get_database_backend() {
        DbBackend::Sqlite => source_query().all(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => source_query().lock_shared().all(txn).await?,
    };
    if sources.is_empty() || sources.len() != current_members.len() {
        return Err(PagesError::rollback_target_unavailable(
            "repaired publish cursor lacks complete retained rebuild provenance",
        ));
    }

    let mut source_members = Vec::with_capacity(sources.len());
    for source in &sources {
        verify_rebuild_source_for_rollback(operation, source)?;
        source_members.push(ArtifactSetMember::new(
            source.locale.clone(),
            source.artifact_id,
            source.artifact_hash.clone(),
            Some(source.materialization_hash.clone()),
        ));
    }
    if artifact_set_hash(&source_members)? != operation.artifact_set_hash {
        return Err(PagesError::rollback_target_unavailable(
            "retained rebuild provenance does not reproduce the publish artifact set",
        ));
    }

    let surviving_manifest = load_publish_manifest_rows_in_tx(txn, operation).await?;
    let mut surviving_locales = std::collections::BTreeSet::new();
    for row in &surviving_manifest {
        if !surviving_locales.insert(row.locale.clone()) {
            return Err(PagesError::rollback_target_unavailable(
                "surviving publish manifest contains duplicate locale identity",
            ));
        }
        let source = sources
            .iter()
            .find(|source| source.locale == row.locale)
            .ok_or_else(|| {
                PagesError::rollback_target_unavailable(format!(
                    "surviving publish manifest locale `{}` has no retained provenance",
                    row.locale
                ))
            })?;
        if row.artifact_id != source.artifact_id
            || row.artifact_hash != source.artifact_hash
            || row.materialization_hash.as_deref() != Some(source.materialization_hash.as_str())
        {
            return Err(PagesError::rollback_target_unavailable(format!(
                "surviving publish manifest locale `{}` does not match retained provenance",
                row.locale
            )));
        }
    }

    let mut repaired_locales = 0usize;
    let mut physically_lost_manifest_locales = std::collections::BTreeSet::new();
    for source in &sources {
        let current = current_members
            .iter()
            .find(|member| member.locale == source.locale)
            .ok_or_else(|| {
                PagesError::rollback_target_unavailable(format!(
                    "current repaired artifact set is missing locale `{}`",
                    source.locale
                ))
            })?;
        if current.artifact_hash != source.artifact_hash
            || current.materialization_hash.as_deref()
                != Some(source.materialization_hash.as_str())
        {
            return Err(PagesError::rollback_target_unavailable(format!(
                "current locale `{}` does not match retained publish provenance",
                source.locale
            )));
        }
        let manifest_row_survives = surviving_locales.contains(&source.locale);
        if current.artifact_id == source.artifact_id {
            if !manifest_row_survives {
                return Err(PagesError::rollback_target_unavailable(format!(
                    "unchanged locale `{}` is missing its original publish manifest row",
                    source.locale
                )));
            }
            continue;
        }
        repaired_locales += 1;
        if !manifest_row_survives
            && source_artifact_exists_in_tx(txn, operation, source).await?
        {
            return Err(PagesError::rollback_target_unavailable(format!(
                "repaired locale `{}` is missing its manifest while the source artifact still exists",
                source.locale
            )));
        }

        let rebuild = load_rebuild_for_current_artifact_in_tx(
            txn,
            operation,
            source,
            current.artifact_id,
        )
        .await?;
        verify_rebuild_receipt_for_rollback(&rebuild)?;
        ensure_rebuild_matches_source_for_rollback(&rebuild, source)?;

        let activation = load_activation_for_current_artifact_in_tx(
            txn,
            operation,
            rebuild.id,
            current.artifact_id,
        )
        .await?;
        verify_activation_receipt_for_rollback(&activation)?;
        if activation.page_body_id != source.page_body_id
            || activation.locale != source.locale
            || activation.expected_current_artifact_id != source.artifact_id
            || activation.replacement_artifact_id != current.artifact_id
            || activation.replacement_artifact_hash != source.artifact_hash
            || activation.replacement_materialization_hash != source.materialization_hash
            || activation.result_version <= operation.result_version
            || activation.result_version > current_page.version
        {
            return Err(PagesError::rollback_target_unavailable(
                "current repaired artifact does not match its exact activation receipt",
            ));
        }
        if !manifest_row_survives {
            physically_lost_manifest_locales.insert(source.locale.clone());
        }
    }
    if repaired_locales == 0 {
        return Err(PagesError::rollback_target_unavailable(
            "publish manifest recovery requires at least one explicitly rebuilt and activated locale",
        ));
    }
    if physically_lost_manifest_locales.is_empty() {
        return Err(PagesError::rollback_target_unavailable(
            "publish manifest recovery requires at least one repaired locale whose source artifact and manifest were physically lost",
        ));
    }
    verify_physical_loss_activation_prefix_in_tx(
        txn,
        operation,
        &sources,
        &current_members,
        &physically_lost_manifest_locales,
        current_page.version,
    )
    .await?;

    Ok(current_members)
}

async fn verify_physical_loss_activation_prefix_in_tx(
    txn: &DatabaseTransaction,
    operation: &page_publish_operation::Model,
    sources: &[page_publish_rebuild_source::Model],
    current_members: &[ArtifactSetMember],
    required_locales: &std::collections::BTreeSet<String>,
    current_page_version: i32,
) -> PagesResult<()> {
    if required_locales.is_empty() || required_locales.len() > MAX_RECOVERED_ACTIVATION_PREFIX {
        return Err(PagesError::rollback_target_unavailable(
            "physical-loss activation prefix has an invalid required locale count",
        ));
    }

    let anchor_version = resolve_repair_activation_anchor_in_tx(
        txn,
        operation,
        current_page_version,
    )
    .await?;
    if anchor_version > current_page_version {
        return Err(PagesError::rollback_target_unavailable(
            "repair activation anchor is newer than the current page version",
        ));
    }

    let query = || {
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId.eq(operation.tenant_id),
            )
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(operation.page_id))
            .filter(
                page_artifact_binding_replacement_operation::Column::ResultVersion
                    .gt(anchor_version),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::ResultVersion
                    .lte(current_page_version),
            )
            .order_by_asc(page_artifact_binding_replacement_operation::Column::ResultVersion)
            .order_by_asc(page_artifact_binding_replacement_operation::Column::Id)
            .limit((MAX_RECOVERED_ACTIVATION_PREFIX + 1) as u64)
    };
    let activations = match txn.get_database_backend() {
        DbBackend::Sqlite => query().all(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().all(txn).await?,
    };

    let required_current_artifacts = required_locales
        .iter()
        .map(|locale| {
            let member = current_members
                .iter()
                .find(|member| member.locale == *locale)
                .ok_or_else(|| {
                    PagesError::rollback_target_unavailable(format!(
                        "physical-loss activation prefix required locale `{locale}` is not current",
                    ))
                })?;
            Ok((locale.clone(), member.artifact_id))
        })
        .collect::<PagesResult<std::collections::BTreeMap<_, _>>>()?;

    let mut cursor = anchor_version;
    let mut latest_by_locale = std::collections::BTreeMap::<
        String,
        (Uuid, page_artifact_rebuild_operation::Model),
    >::new();
    let mut proven_required_locales = std::collections::BTreeSet::new();
    for (index, activation) in activations.into_iter().enumerate() {
        if index >= MAX_RECOVERED_ACTIVATION_PREFIX {
            return Err(PagesError::rollback_target_unavailable(
                "physical-loss activation prefix exceeds the bounded activation limit",
            ));
        }
        if activation.expected_version != cursor || activation.result_version != cursor + 1 {
            return Err(PagesError::rollback_target_unavailable(
                "physical-loss activation prefix is not a contiguous page-version chain",
            ));
        }
        verify_activation_receipt_for_rollback(&activation)?;
        let source = sources
            .iter()
            .find(|source| source.locale == activation.locale)
            .ok_or_else(|| {
                PagesError::rollback_target_unavailable(format!(
                    "physical-loss activation prefix locale `{}` has no retained source for the selected publish",
                    activation.locale
                ))
            })?;
        let rebuild = load_rebuild_for_current_artifact_in_tx(
            txn,
            operation,
            source,
            activation.replacement_artifact_id,
        )
        .await?;
        verify_rebuild_receipt_for_rollback(&rebuild)?;
        ensure_rebuild_matches_source_for_rollback(&rebuild, source)?;
        if activation.rebuild_operation_id != rebuild.id
            || activation.page_body_id != source.page_body_id
            || activation.locale != source.locale
            || activation.expected_current_artifact_id != source.artifact_id
            || activation.replacement_artifact_id != rebuild.rebuilt_artifact_id
            || activation.replacement_artifact_hash != rebuild.rebuilt_artifact_hash
            || activation.replacement_materialization_hash != rebuild.rebuilt_materialization_hash
        {
            return Err(PagesError::rollback_target_unavailable(
                "physical-loss activation prefix receipt does not match its exact publish repair source",
            ));
        }

        if let Some((_, previous_rebuild)) = latest_by_locale.get(&activation.locale)
            && recovery_artifact_if_present_for_rollback_in_tx(
                txn,
                operation,
                activation.locale.as_str(),
                previous_rebuild.rebuilt_artifact_id,
            )
            .await?
            .is_some()
        {
            return Err(PagesError::rollback_target_unavailable(
                "physical-loss activation prefix repeated a locale while its prior rebuilt artifact still exists",
            ));
        }

        latest_by_locale.insert(
            activation.locale.clone(),
            (source.page_body_id, rebuild),
        );
        if required_current_artifacts
            .get(&activation.locale)
            .is_some_and(|artifact_id| *artifact_id == activation.replacement_artifact_id)
        {
            proven_required_locales.insert(activation.locale.clone());
        }

        cursor = activation.result_version;
        if required_locales.is_subset(&proven_required_locales) {
            for (locale, (page_body_id, latest_rebuild)) in &latest_by_locale {
                let current = current_members
                    .iter()
                    .find(|member| member.locale == *locale)
                    .ok_or_else(|| {
                        PagesError::rollback_target_unavailable(format!(
                            "physical-loss activation prefix locale `{locale}` is no longer current",
                        ))
                    })?;
                if current.artifact_id != latest_rebuild.rebuilt_artifact_id
                    || current.artifact_hash != latest_rebuild.rebuilt_artifact_hash
                    || current.materialization_hash.as_deref()
                        != Some(latest_rebuild.rebuilt_materialization_hash.as_str())
                {
                    return Err(PagesError::rollback_target_unavailable(format!(
                        "physical-loss activation prefix latest locale `{locale}` does not match the current repaired artifact",
                    )));
                }
                let artifact = recovery_artifact_if_present_for_rollback_in_tx(
                    txn,
                    operation,
                    locale.as_str(),
                    latest_rebuild.rebuilt_artifact_id,
                )
                .await?
                .ok_or_else(|| {
                    PagesError::rollback_target_unavailable(
                        "physical-loss activation prefix latest rebuilt artifact is unavailable",
                    )
                })?;
                if artifact.instance_key != latest_rebuild.artifact_instance_key
                    || artifact.artifact_hash != latest_rebuild.rebuilt_artifact_hash
                    || artifact.materialization_hash.as_deref()
                        != Some(latest_rebuild.rebuilt_materialization_hash.as_str())
                {
                    return Err(PagesError::rollback_target_unavailable(
                        "physical-loss activation prefix latest rebuilt artifact drifted from its receipt",
                    ));
                }
                if *page_body_id == Uuid::nil() {
                    return Err(PagesError::rollback_target_unavailable(
                        "physical-loss activation prefix latest repaired body identity is invalid",
                    ));
                }
            }
            return Ok(());
        }
    }

    Err(PagesError::rollback_target_unavailable(
        "physical-loss activation prefix does not reach every current repaired locale whose manifest was lost",
    ))
}

async fn recovery_artifact_if_present_for_rollback_in_tx(
    txn: &DatabaseTransaction,
    operation: &page_publish_operation::Model,
    locale: &str,
    artifact_id: Uuid,
) -> PagesResult<Option<page_static_landing_artifact::Model>> {
    let query = || {
        page_static_landing_artifact::Entity::find_by_id(artifact_id)
            .filter(page_static_landing_artifact::Column::TenantId.eq(operation.tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(operation.page_id))
            .filter(page_static_landing_artifact::Column::Locale.eq(locale))
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    })
}

async fn resolve_repair_activation_anchor_in_tx(
    txn: &DatabaseTransaction,
    operation: &page_publish_operation::Model,
    current_page_version: i32,
) -> PagesResult<i32> {
    let query = || {
        page_rollback_operation::Entity::find()
            .filter(page_rollback_operation::Column::TenantId.eq(operation.tenant_id))
            .filter(page_rollback_operation::Column::PageId.eq(operation.page_id))
            .filter(page_rollback_operation::Column::TargetPublishOperationId.eq(operation.id))
            .filter(
                page_rollback_operation::Column::TargetArtifactSetHash
                    .eq(operation.artifact_set_hash.as_str()),
            )
            .filter(page_rollback_operation::Column::ResultVersion.lte(current_page_version))
            .order_by_desc(page_rollback_operation::Column::ResultVersion)
            .order_by_desc(page_rollback_operation::Column::Id)
    };
    let rollback = match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    };
    let Some(rollback) = rollback else {
        return Ok(operation.result_version);
    };

    if rollback.id.is_nil()
        || rollback.tenant_id != operation.tenant_id
        || rollback.page_id != operation.page_id
        || rollback.target_publish_operation_id != operation.id
        || rollback.idempotency_key.trim().is_empty()
        || rollback.result_version <= operation.result_version
        || rollback.result_version > current_page_version
        || !is_sha256(&rollback.request_hash)
        || !is_sha256(&rollback.source_artifact_set_hash)
        || !is_sha256(&rollback.target_artifact_set_hash)
        || rollback.source_artifact_set_hash == rollback.target_artifact_set_hash
        || rollback.target_artifact_set_hash != operation.artifact_set_hash
    {
        return Err(PagesError::rollback_target_unavailable(
            "repair rollback activation anchor failed identity or hash validation",
        ));
    }
    let rollback_expected_version = rollback
        .result_version
        .checked_sub(1)
        .filter(|version| *version > 0)
        .ok_or_else(|| {
            PagesError::rollback_target_unavailable(
                "repair rollback activation anchor has an invalid result version",
            )
        })?;
    let expected_request_hash = stable_hash(&(
        PAGE_ROLLBACK_ACTIVATION_ANCHOR_FORMAT,
        operation.tenant_id,
        operation.page_id,
        rollback_expected_version,
        operation.id,
    ))?;
    if rollback.request_hash != expected_request_hash {
        return Err(PagesError::rollback_target_unavailable(
            "repair rollback activation anchor request hash failed validation",
        ));
    }
    Ok(rollback.result_version)
}

async fn source_artifact_exists_in_tx(
    txn: &DatabaseTransaction,
    operation: &page_publish_operation::Model,
    source: &page_publish_rebuild_source::Model,
) -> PagesResult<bool> {
    let query = || {
        page_static_landing_artifact::Entity::find_by_id(source.artifact_id)
            .filter(page_static_landing_artifact::Column::TenantId.eq(operation.tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(operation.page_id))
            .filter(page_static_landing_artifact::Column::Locale.eq(source.locale.as_str()))
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    }
    .is_some())
}

fn verify_rebuild_source_for_rollback(
    operation: &page_publish_operation::Model,
    source: &page_publish_rebuild_source::Model,
) -> PagesResult<()> {
    if source.id.is_nil()
        || source.operation_id != operation.id
        || source.tenant_id != operation.tenant_id
        || source.page_id != operation.page_id
        || source.page_body_id.is_nil()
        || source.artifact_id.is_nil()
        || source.locale.trim().is_empty()
        || source.locale.trim() != source.locale
        || source.source_format != CONTENT_FORMAT_GRAPESJS
        || source.source_revision.trim().is_empty()
        || source.review_hash != operation.review_hash
    {
        return Err(PagesError::rollback_target_unavailable(
            "retained rebuild provenance has invalid publish identity evidence",
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
            return Err(PagesError::rollback_target_unavailable(
                "retained rebuild provenance contains an invalid SHA-256 identity",
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
    .map_err(|error| PagesError::rollback_target_unavailable(error.to_string()))?;
    if expected != source.provenance_hash {
        return Err(PagesError::rollback_target_unavailable(
            "retained rebuild provenance hash failed validation",
        ));
    }
    Ok(())
}

async fn load_rebuild_for_current_artifact_in_tx(
    txn: &DatabaseTransaction,
    operation: &page_publish_operation::Model,
    source: &page_publish_rebuild_source::Model,
    rebuilt_artifact_id: Uuid,
) -> PagesResult<page_artifact_rebuild_operation::Model> {
    let query = || {
        page_artifact_rebuild_operation::Entity::find()
            .filter(page_artifact_rebuild_operation::Column::TenantId.eq(operation.tenant_id))
            .filter(page_artifact_rebuild_operation::Column::PageId.eq(operation.page_id))
            .filter(page_artifact_rebuild_operation::Column::SourceId.eq(source.id))
            .filter(
                page_artifact_rebuild_operation::Column::RebuiltArtifactId.eq(rebuilt_artifact_id),
            )
    };
    match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    }
    .ok_or_else(|| {
        PagesError::rollback_target_unavailable(
            "current repaired artifact has no exact rebuild receipt",
        )
    })
}

fn verify_rebuild_receipt_for_rollback(
    rebuild: &page_artifact_rebuild_operation::Model,
) -> PagesResult<()> {
    if rebuild.id.is_nil()
        || rebuild.tenant_id.is_nil()
        || rebuild.page_id.is_nil()
        || rebuild.source_id.is_nil()
        || rebuild.source_publish_operation_id.is_nil()
        || rebuild.source_artifact_id.is_nil()
        || rebuild.rebuilt_artifact_id.is_nil()
        || rebuild.source_artifact_id == rebuild.rebuilt_artifact_id
        || rebuild.locale.trim().is_empty()
        || rebuild.locale.trim() != rebuild.locale
        || rebuild.idempotency_key.trim().is_empty()
        || !is_sha256(&rebuild.request_hash)
        || !is_sha256(&rebuild.expected_provenance_hash)
        || !is_sha256(&rebuild.review_hash)
        || !is_sha256(&rebuild.rebuilt_artifact_hash)
        || !is_sha256(&rebuild.rebuilt_materialization_hash)
        || rebuild.artifact_instance_key != format!("rebuild:{}", rebuild.id)
    {
        return Err(PagesError::rollback_target_unavailable(
            "current repaired artifact rebuild receipt failed integrity validation",
        ));
    }
    Ok(())
}

fn ensure_rebuild_matches_source_for_rollback(
    rebuild: &page_artifact_rebuild_operation::Model,
    source: &page_publish_rebuild_source::Model,
) -> PagesResult<()> {
    if rebuild.source_id != source.id
        || rebuild.source_publish_operation_id != source.operation_id
        || rebuild.tenant_id != source.tenant_id
        || rebuild.page_id != source.page_id
        || rebuild.locale != source.locale
        || rebuild.source_artifact_id != source.artifact_id
        || rebuild.expected_provenance_hash != source.provenance_hash
        || rebuild.review_hash != source.review_hash
        || rebuild.rebuilt_artifact_hash != source.artifact_hash
        || rebuild.rebuilt_materialization_hash != source.materialization_hash
    {
        return Err(PagesError::rollback_target_unavailable(
            "current repaired artifact rebuild receipt does not match retained provenance",
        ));
    }
    Ok(())
}

async fn load_activation_for_current_artifact_in_tx(
    txn: &DatabaseTransaction,
    operation: &page_publish_operation::Model,
    rebuild_operation_id: Uuid,
    replacement_artifact_id: Uuid,
) -> PagesResult<page_artifact_binding_replacement_operation::Model> {
    let query = || {
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId.eq(operation.tenant_id),
            )
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(operation.page_id))
            .filter(
                page_artifact_binding_replacement_operation::Column::RebuildOperationId
                    .eq(rebuild_operation_id),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::ReplacementArtifactId
                    .eq(replacement_artifact_id),
            )
    };
    match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    }
    .ok_or_else(|| {
        PagesError::rollback_target_unavailable(
            "current repaired artifact has no exact activation receipt",
        )
    })
}

fn verify_activation_receipt_for_rollback(
    activation: &page_artifact_binding_replacement_operation::Model,
) -> PagesResult<()> {
    if activation.id.is_nil()
        || activation.tenant_id.is_nil()
        || activation.page_id.is_nil()
        || activation.rebuild_operation_id.is_nil()
        || activation.page_body_id.is_nil()
        || activation.expected_current_artifact_id.is_nil()
        || activation.replacement_artifact_id.is_nil()
        || activation.expected_current_artifact_id == activation.replacement_artifact_id
        || activation.locale.trim().is_empty()
        || activation.locale.trim() != activation.locale
        || activation.idempotency_key.trim().is_empty()
        || activation.expected_version <= 0
        || activation.expected_version == i32::MAX
        || activation.result_version != activation.expected_version + 1
        || !is_sha256(&activation.request_hash)
        || !is_sha256(&activation.replacement_artifact_hash)
        || !is_sha256(&activation.replacement_materialization_hash)
    {
        return Err(PagesError::rollback_target_unavailable(
            "current repaired artifact activation receipt failed integrity validation",
        ));
    }
    let expected_request_hash = stable_hash(&(
        PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT,
        activation.tenant_id,
        activation.page_id,
        activation.rebuild_operation_id,
        activation.expected_version,
        activation.expected_current_artifact_id,
    ))?;
    if activation.request_hash != expected_request_hash {
        return Err(PagesError::rollback_target_unavailable(
            "current repaired artifact activation receipt request hash failed validation",
        ));
    }
    Ok(())
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
        DbBackend::Postgres | DbBackend::MySql => body_query().lock_exclusive().all(txn).await?,
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
            "unable to encode page artifact identity: {error}"
        ))
    })?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
