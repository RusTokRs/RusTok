use chrono::Utc;
use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_events::DomainEvent;
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dto::{ReplacePageArtifactBindingInput, ReplacePageArtifactBindingResult};
use crate::entities::{
    page, page_artifact_binding_replacement_operation, page_artifact_rebuild_operation, page_body,
    page_publish_operation, page_publish_rebuild_source, page_published_landing_artifact,
    page_rollback_operation, page_static_landing_artifact,
};
use crate::error::{PagesError, PagesResult};
use crate::services::page_builder_artifact::PageBuilderArtifactService;

use super::helpers::{apply_transition, enforce_expected_version};
use super::publish_manifest::{RebuildSourceProvenance, is_sha256, rebuild_source_provenance_hash};
use super::{PAGE_KIND, PageService, PageTransition};

pub const PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT: &str =
    "page_artifact_binding_replacement_operation_v1";
pub const PAGE_ARTIFACT_BINDING_REPLACEMENT_IDEMPOTENCY_CONFLICT: &str =
    "PAGE_ARTIFACT_BINDING_REPLACEMENT_IDEMPOTENCY_CONFLICT";
pub const PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT: &str =
    "PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT";
pub const PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID: &str =
    "PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID";
pub const PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY: &str =
    "PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY";
const PAGE_ROLLBACK_ACTIVATION_ANCHOR_FORMAT: &str = "page_rollback_operation_v1";
const MAX_REPLACEMENT_IDEMPOTENCY_KEY_BYTES: usize = 191;
const MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS: usize = 256;

struct ArtifactBindingReplacementRequest<'a> {
    tenant_id: Uuid,
    page_id: Uuid,
    rebuild_operation_id: Uuid,
    expected_version: i32,
    expected_current_artifact_id: Uuid,
    idempotency_key: &'a str,
    request_hash: &'a str,
}

impl PageService {
    /// Activates one exact rebuilt immutable artifact for its locale in one owner transaction.
    ///
    /// The command requires tenant-wide `pages:manage`, a page-version fence and the exact current
    /// source artifact id. It verifies the rebuild receipt, retained provenance and replacement
    /// artifact. The ordinary path requires the current locale binding to still point at the source
    /// artifact. A deliberately narrow recovery path also admits a physically lost source artifact
    /// only when the locale binding is absent, the retained source body is still current for the
    /// locale, and the current page version is anchored either by the retained publish itself or by
    /// an exact later rollback receipt that reactivated that same publish set. Any version gap after
    /// that anchor must be explained completely by a bounded contiguous chain of earlier activations
    /// from the exact same publish. A locale may repeat only when its prior rebuilt instance is also
    /// physically absent; the latest repair for every other locale must remain bound and intact.
    /// Both paths update one localized binding, advance the page version, emit lifecycle events and
    /// store one replayable activation receipt atomically. The command never compiles, sanitizes,
    /// rebuilds or reads the mutable body as repair authority.
    pub async fn replace_rebuilt_artifact_binding(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: ReplacePageArtifactBindingInput,
    ) -> PagesResult<ReplacePageArtifactBindingResult> {
        enforce_tenant_wide_manage(&security)?;
        if tenant_id.is_nil()
            || page_id.is_nil()
            || input.rebuild_operation_id.is_nil()
            || input.expected_current_artifact_id.is_nil()
        {
            return Err(PagesError::validation(
                "artifact binding replacement tenant, page, rebuild and current artifact ids must not be nil",
            ));
        }
        if input.expected_version <= 0 || input.expected_version == i32::MAX {
            return Err(PagesError::validation(
                "artifact binding replacement expected_version must be a positive incrementable version",
            ));
        }
        let idempotency_key = normalize_idempotency_key(&input.idempotency_key)?;
        let request_hash = stable_replacement_hash(&(
            PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT,
            tenant_id,
            page_id,
            input.rebuild_operation_id,
            input.expected_version,
            input.expected_current_artifact_id,
        ))?;

        let txn = self.db.begin().await?;
        let existing_page = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        if let Some(operation) =
            find_operation_in_tx(&txn, tenant_id, page_id, idempotency_key.as_str()).await?
        {
            ensure_same_request(
                &operation,
                ArtifactBindingReplacementRequest {
                    tenant_id,
                    page_id,
                    rebuild_operation_id: input.rebuild_operation_id,
                    expected_version: input.expected_version,
                    expected_current_artifact_id: input.expected_current_artifact_id,
                    idempotency_key: idempotency_key.as_str(),
                    request_hash: request_hash.as_str(),
                },
            )?;
            let result = result_from_record(operation, true)?;
            txn.commit().await?;
            return Ok(result);
        }

        enforce_expected_version(Some(input.expected_version), existing_page.version)?;
        if existing_page.status != "published" {
            return Err(replacement_current_conflict(
                "artifact binding replacement requires a currently published page",
            ));
        }

        let rebuild =
            load_rebuild_operation_in_tx(&txn, tenant_id, page_id, input.rebuild_operation_id)
                .await?;
        verify_rebuild_receipt(&rebuild)?;
        let source = load_rebuild_source_in_tx(&txn, tenant_id, page_id, rebuild.source_id).await?;
        verify_rebuild_source(&source)?;
        ensure_rebuild_matches_source(&rebuild, &source)?;
        if rebuild.source_artifact_id != input.expected_current_artifact_id {
            return Err(replacement_current_conflict(
                "expected current artifact is not the source artifact of the selected rebuild receipt",
            ));
        }
        if let Some(existing) =
            find_operation_for_rebuild_in_tx(&txn, tenant_id, page_id, rebuild.id).await?
        {
            verify_replacement_operation(&existing)?;
            return Err(replacement_current_conflict(
                "selected rebuild receipt already has an activation receipt",
            ));
        }

        let binding =
            load_binding_for_update_in_tx(&txn, tenant_id, page_id, rebuild.locale.as_str())
                .await?;
        let page_body_id = match binding {
            Some(binding) => {
                if binding.page_body_id != source.page_body_id {
                    return Err(replacement_current_conflict(
                        "current locale binding body does not match retained rebuild provenance",
                    ));
                }
                if binding.artifact_id != input.expected_current_artifact_id {
                    return Err(replacement_current_conflict(format!(
                        "current locale binding changed: expected artifact `{}`, found `{}`",
                        input.expected_current_artifact_id, binding.artifact_id,
                    )));
                }
                if rebuild.rebuilt_artifact_id == binding.artifact_id {
                    return Err(replacement_current_conflict(
                        "rebuilt artifact is already the current locale binding",
                    ));
                }
                binding.page_body_id
            }
            None => {
                ensure_missing_binding_recovery_in_tx(
                    &txn,
                    tenant_id,
                    page_id,
                    input.expected_version,
                    &rebuild,
                    &source,
                )
                .await?;
                source.page_body_id
            }
        };

        let replacement = load_replacement_artifact_in_tx(
            &txn,
            tenant_id,
            page_id,
            rebuild.locale.as_str(),
            rebuild.rebuilt_artifact_id,
        )
        .await?;
        if replacement.instance_key != rebuild.artifact_instance_key
            || replacement.artifact_hash != rebuild.rebuilt_artifact_hash
            || replacement.materialization_hash.as_deref()
                != Some(rebuild.rebuilt_materialization_hash.as_str())
        {
            return Err(replacement_target_invalid(
                "rebuilt artifact no longer matches its immutable rebuild receipt",
            ));
        }

        PageBuilderArtifactService::bind_existing_body_in_tx(
            &txn,
            tenant_id,
            page_id,
            rebuild.locale.as_str(),
            replacement.id,
        )
        .await?;

        let now = Utc::now();
        let mut active: page::ActiveModel = existing_page.into();
        active.updated_at = Set(now.into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        apply_transition(&mut active, Some(PageTransition::Publish), now);
        let updated_page = active.update(&txn).await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodeUpdated {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                },
            )
            .await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodePublished {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                },
            )
            .await?;

        let timestamp: sea_orm::prelude::DateTimeWithTimeZone = now.into();
        let operation = page_artifact_binding_replacement_operation::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            page_id: Set(page_id),
            rebuild_operation_id: Set(rebuild.id),
            page_body_id: Set(page_body_id),
            locale: Set(rebuild.locale),
            idempotency_key: Set(idempotency_key),
            request_hash: Set(request_hash),
            expected_version: Set(input.expected_version),
            expected_current_artifact_id: Set(input.expected_current_artifact_id),
            replacement_artifact_id: Set(replacement.id),
            replacement_artifact_hash: Set(replacement.artifact_hash),
            replacement_materialization_hash: Set(rebuild.rebuilt_materialization_hash),
            result_version: Set(updated_page.version),
            replaced_at: Set(timestamp),
            created_at: Set(timestamp),
        }
        .insert(&txn)
        .await?;
        let result = result_from_record(operation, false)?;
        txn.commit().await?;
        Ok(result)
    }
}

async fn load_rebuild_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    rebuild_operation_id: Uuid,
) -> PagesResult<page_artifact_rebuild_operation::Model> {
    let query = || {
        page_artifact_rebuild_operation::Entity::find_by_id(rebuild_operation_id)
            .filter(page_artifact_rebuild_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_rebuild_operation::Column::PageId.eq(page_id))
    };
    match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    }
    .ok_or_else(|| replacement_target_invalid("selected rebuild receipt is unavailable"))
}

async fn load_rebuild_source_in_tx(
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
    .ok_or_else(|| replacement_target_invalid("selected rebuild provenance is unavailable"))
}

async fn load_binding_for_update_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
) -> PagesResult<Option<page_published_landing_artifact::Model>> {
    let query = || {
        page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.eq(page_id))
            .filter(page_published_landing_artifact::Column::Locale.eq(locale))
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
    })
}

async fn ensure_missing_binding_recovery_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    expected_version: i32,
    rebuild: &page_artifact_rebuild_operation::Model,
    source: &page_publish_rebuild_source::Model,
) -> PagesResult<()> {
    let source_artifact_query = || {
        page_static_landing_artifact::Entity::find_by_id(rebuild.source_artifact_id)
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
            .filter(page_static_landing_artifact::Column::Locale.eq(rebuild.locale.as_str()))
    };
    let source_artifact = match txn.get_database_backend() {
        DbBackend::Sqlite => source_artifact_query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => {
            source_artifact_query().lock_shared().one(txn).await?
        }
    };
    if source_artifact.is_some() {
        return Err(replacement_current_conflict(
            "current locale binding is unavailable while the retained source artifact still exists",
        ));
    }

    let body_query = || {
        page_body::Entity::find_by_id(source.page_body_id)
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::PageId.eq(page_id))
            .filter(page_body::Column::Locale.eq(rebuild.locale.as_str()))
    };
    let body = match txn.get_database_backend() {
        DbBackend::Sqlite => body_query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => body_query().lock_shared().one(txn).await?,
    }
    .ok_or_else(|| {
        replacement_current_conflict(
            "retained source page body is unavailable for missing-binding recovery",
        )
    })?;
    if body.id != source.page_body_id {
        return Err(replacement_current_conflict(
            "retained source page body identity changed before missing-binding recovery",
        ));
    }

    let publish_query = || {
        page_publish_operation::Entity::find_by_id(source.operation_id)
            .filter(page_publish_operation::Column::TenantId.eq(tenant_id))
            .filter(page_publish_operation::Column::PageId.eq(page_id))
    };
    let publish = match txn.get_database_backend() {
        DbBackend::Sqlite => publish_query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => publish_query().lock_shared().one(txn).await?,
    }
    .ok_or_else(|| {
        replacement_current_conflict(
            "retained source publish operation is unavailable for missing-binding recovery",
        )
    })?;
    if publish.id != rebuild.source_publish_operation_id || publish.id != source.operation_id {
        return Err(replacement_current_conflict(
            "retained source publish operation does not match rebuild provenance",
        ));
    }
    if publish.result_version > expected_version {
        return Err(replacement_current_conflict(format!(
            "retained source publish version is stale: expected current version `{expected_version}`, found future source version `{}`",
            publish.result_version,
        )));
    }

    let anchor_version = if publish.result_version == expected_version {
        publish.result_version
    } else {
        resolve_missing_binding_recovery_anchor_in_tx(
            txn,
            tenant_id,
            page_id,
            expected_version,
            &publish,
        )
        .await?
    };
    if anchor_version > expected_version {
        return Err(replacement_current_conflict(
            "missing-binding recovery activation anchor is newer than the current page version",
        ));
    }
    if anchor_version < expected_version {
        ensure_sequential_missing_binding_recovery_version_chain_in_tx(
            txn,
            tenant_id,
            page_id,
            anchor_version,
            expected_version,
            &publish,
            rebuild.locale.as_str(),
        )
        .await?;
    }
    Ok(())
}

async fn resolve_missing_binding_recovery_anchor_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    expected_version: i32,
    publish: &page_publish_operation::Model,
) -> PagesResult<i32> {
    let query = || {
        page_rollback_operation::Entity::find()
            .filter(page_rollback_operation::Column::TenantId.eq(tenant_id))
            .filter(page_rollback_operation::Column::PageId.eq(page_id))
            .filter(page_rollback_operation::Column::TargetPublishOperationId.eq(publish.id))
            .filter(
                page_rollback_operation::Column::TargetArtifactSetHash
                    .eq(publish.artifact_set_hash.as_str()),
            )
            .filter(page_rollback_operation::Column::ResultVersion.lte(expected_version))
            .order_by_desc(page_rollback_operation::Column::ResultVersion)
            .order_by_desc(page_rollback_operation::Column::Id)
    };
    let rollback = match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    };
    let Some(rollback) = rollback else {
        return Ok(publish.result_version);
    };

    if rollback.id.is_nil()
        || rollback.tenant_id != tenant_id
        || rollback.page_id != page_id
        || rollback.target_publish_operation_id != publish.id
        || rollback.idempotency_key.trim().is_empty()
        || rollback.result_version <= publish.result_version
        || rollback.result_version > expected_version
        || !is_sha256(&rollback.request_hash)
        || !is_sha256(&rollback.source_artifact_set_hash)
        || !is_sha256(&rollback.target_artifact_set_hash)
        || rollback.source_artifact_set_hash == rollback.target_artifact_set_hash
        || rollback.target_artifact_set_hash != publish.artifact_set_hash
    {
        return Err(replacement_current_conflict(
            "rollback activation anchor failed identity or hash validation",
        ));
    }
    let rollback_expected_version = rollback
        .result_version
        .checked_sub(1)
        .filter(|v| *v > 0)
        .ok_or_else(|| {
            replacement_current_conflict("rollback activation anchor has an invalid result version")
        })?;
    let expected_request_hash = stable_replacement_hash(&(
        PAGE_ROLLBACK_ACTIVATION_ANCHOR_FORMAT,
        tenant_id,
        page_id,
        rollback_expected_version,
        publish.id,
    ))?;
    if rollback.request_hash != expected_request_hash {
        return Err(replacement_current_conflict(
            "rollback activation anchor request hash failed validation",
        ));
    }
    Ok(rollback.result_version)
}

async fn ensure_sequential_missing_binding_recovery_version_chain_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    anchor_version: i32,
    expected_version: i32,
    publish: &page_publish_operation::Model,
    target_locale: &str,
) -> PagesResult<()> {
    let version_gap = expected_version
        .checked_sub(anchor_version)
        .filter(|gap| *gap > 0)
        .ok_or_else(|| {
            replacement_current_conflict(format!(
                "recovery activation anchor version is stale: expected current version `{expected_version}`, found `{anchor_version}`",
            ))
        })?;
    let version_gap = usize::try_from(version_gap).map_err(|_| {
        replacement_current_conflict(
            "recovery activation anchor version is stale: sequential recovery version gap is invalid",
        )
    })?;
    if version_gap > MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS {
        return Err(replacement_current_conflict(format!(
            "recovery activation anchor version is stale: sequential recovery gap `{version_gap}` exceeds the bounded activation limit `{MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS}`",
        )));
    }

    let query = || {
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(page_id))
            .filter(
                page_artifact_binding_replacement_operation::Column::ResultVersion
                    .gt(anchor_version),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::ResultVersion
                    .lte(expected_version),
            )
            .order_by_asc(page_artifact_binding_replacement_operation::Column::ResultVersion)
            .order_by_asc(page_artifact_binding_replacement_operation::Column::Id)
            .limit((MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS + 1) as u64)
    };
    let operations = match txn.get_database_backend() {
        DbBackend::Sqlite => query().all(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().all(txn).await?,
    };
    if operations.len() != version_gap {
        return Err(replacement_current_conflict(
            "recovery activation anchor version is stale: current version gap is not fully explained by prior artifact activations from the selected publish",
        ));
    }

    let mut cursor = anchor_version;
    let mut latest_by_locale =
        std::collections::BTreeMap::<String, (Uuid, page_artifact_rebuild_operation::Model)>::new();
    for operation in operations {
        verify_replacement_operation(&operation)?;
        if operation.expected_version != cursor || operation.result_version != cursor + 1 {
            return Err(replacement_current_conflict(
                "recovery activation anchor version is stale: prior artifact activations are not a contiguous version chain",
            ));
        }

        let expected_request_hash = stable_replacement_hash(&(
            PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT,
            tenant_id,
            page_id,
            operation.rebuild_operation_id,
            operation.expected_version,
            operation.expected_current_artifact_id,
        ))?;
        if operation.request_hash != expected_request_hash {
            return Err(replacement_operation_integrity(
                "prior sequential recovery activation receipt request hash failed validation",
            ));
        }

        let prior_rebuild =
            load_rebuild_operation_in_tx(txn, tenant_id, page_id, operation.rebuild_operation_id)
                .await?;
        verify_rebuild_receipt(&prior_rebuild)?;
        let prior_source =
            load_rebuild_source_in_tx(txn, tenant_id, page_id, prior_rebuild.source_id).await?;
        verify_rebuild_source(&prior_source)?;
        ensure_rebuild_matches_source(&prior_rebuild, &prior_source)?;
        if prior_rebuild.source_publish_operation_id != publish.id
            || prior_source.operation_id != publish.id
            || operation.page_body_id != prior_source.page_body_id
            || operation.locale != prior_source.locale
            || operation.expected_current_artifact_id != prior_rebuild.source_artifact_id
            || operation.replacement_artifact_id != prior_rebuild.rebuilt_artifact_id
            || operation.replacement_artifact_hash != prior_rebuild.rebuilt_artifact_hash
            || operation.replacement_materialization_hash
                != prior_rebuild.rebuilt_materialization_hash
        {
            return Err(replacement_current_conflict(
                "recovery activation anchor version is stale: a prior activation does not belong to the exact selected publish repair chain",
            ));
        }

        if let Some((_, previous_rebuild)) = latest_by_locale.get(&operation.locale)
            && recovery_artifact_if_present_in_tx(
                txn,
                tenant_id,
                page_id,
                operation.locale.as_str(),
                previous_rebuild.rebuilt_artifact_id,
            )
            .await?
            .is_some()
        {
            return Err(replacement_current_conflict(
                "recovery activation anchor version is stale: a repeated locale still has its prior rebuilt immutable artifact",
            ));
        }

        latest_by_locale.insert(
            operation.locale.clone(),
            (prior_source.page_body_id, prior_rebuild),
        );
        cursor = operation.result_version;
    }
    if cursor != expected_version {
        return Err(replacement_current_conflict(format!(
            "recovery activation chain ends at version `{cursor}` instead of current version `{expected_version}`",
        )));
    }

    for (locale, (page_body_id, latest_rebuild)) in latest_by_locale {
        let binding =
            load_binding_for_update_in_tx(txn, tenant_id, page_id, locale.as_str()).await?;
        if locale == target_locale {
            if binding.is_some() {
                return Err(replacement_current_conflict(
                    "recovery activation anchor version is stale: target locale binding unexpectedly became active before repeated recovery",
                ));
            }
            if recovery_artifact_if_present_in_tx(
                txn,
                tenant_id,
                page_id,
                locale.as_str(),
                latest_rebuild.rebuilt_artifact_id,
            )
            .await?
            .is_some()
            {
                return Err(replacement_current_conflict(
                    "recovery activation anchor version is stale: target locale prior rebuilt immutable artifact still exists",
                ));
            }
            continue;
        }

        let binding = binding.ok_or_else(|| {
            replacement_current_conflict(
                "recovery activation anchor version is stale: latest repaired locale binding is no longer active",
            )
        })?;
        if binding.page_body_id != page_body_id
            || binding.artifact_id != latest_rebuild.rebuilt_artifact_id
        {
            return Err(replacement_current_conflict(
                "recovery activation anchor version is stale: latest repaired locale binding changed after activation",
            ));
        }

        let artifact = recovery_artifact_if_present_in_tx(
            txn,
            tenant_id,
            page_id,
            locale.as_str(),
            latest_rebuild.rebuilt_artifact_id,
        )
        .await?
        .ok_or_else(|| {
            replacement_current_conflict(
                "recovery activation anchor version is stale: latest repaired immutable artifact is unavailable",
            )
        })?;
        if artifact.instance_key != latest_rebuild.artifact_instance_key
            || artifact.artifact_hash != latest_rebuild.rebuilt_artifact_hash
            || artifact.materialization_hash.as_deref()
                != Some(latest_rebuild.rebuilt_materialization_hash.as_str())
        {
            return Err(replacement_current_conflict(
                "recovery activation anchor version is stale: latest repaired immutable artifact drifted from its rebuild receipt",
            ));
        }
    }
    Ok(())
}

async fn recovery_artifact_if_present_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    artifact_id: Uuid,
) -> PagesResult<Option<page_static_landing_artifact::Model>> {
    let query = || {
        page_static_landing_artifact::Entity::find_by_id(artifact_id)
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
            .filter(page_static_landing_artifact::Column::Locale.eq(locale))
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    })
}

async fn load_replacement_artifact_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    artifact_id: Uuid,
) -> PagesResult<page_static_landing_artifact::Model> {
    let query = || {
        page_static_landing_artifact::Entity::find_by_id(artifact_id)
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
            .filter(page_static_landing_artifact::Column::Locale.eq(locale))
    };
    match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    }
    .ok_or_else(|| replacement_target_invalid("rebuilt replacement artifact is unavailable"))
}

async fn find_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    idempotency_key: &str,
) -> PagesResult<Option<page_artifact_binding_replacement_operation::Model>> {
    let query = || {
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(page_id))
            .filter(
                page_artifact_binding_replacement_operation::Column::IdempotencyKey
                    .eq(idempotency_key),
            )
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
    })
}

async fn find_operation_for_rebuild_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    rebuild_operation_id: Uuid,
) -> PagesResult<Option<page_artifact_binding_replacement_operation::Model>> {
    let query = || {
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(page_id))
            .filter(
                page_artifact_binding_replacement_operation::Column::RebuildOperationId
                    .eq(rebuild_operation_id),
            )
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
    })
}

fn ensure_same_request(
    operation: &page_artifact_binding_replacement_operation::Model,
    request: ArtifactBindingReplacementRequest<'_>,
) -> PagesResult<()> {
    verify_replacement_operation(operation)?;
    if operation.tenant_id != request.tenant_id
        || operation.page_id != request.page_id
        || operation.rebuild_operation_id != request.rebuild_operation_id
        || operation.expected_version != request.expected_version
        || operation.expected_current_artifact_id != request.expected_current_artifact_id
        || operation.idempotency_key != request.idempotency_key
        || operation.request_hash != request.request_hash
    {
        return Err(replacement_idempotency_conflict(
            "idempotency key is bound to another artifact binding replacement request",
        ));
    }
    Ok(())
}

fn verify_rebuild_receipt(operation: &page_artifact_rebuild_operation::Model) -> PagesResult<()> {
    if operation.id.is_nil()
        || operation.tenant_id.is_nil()
        || operation.page_id.is_nil()
        || operation.source_id.is_nil()
        || operation.source_publish_operation_id.is_nil()
        || operation.source_artifact_id.is_nil()
        || operation.rebuilt_artifact_id.is_nil()
        || operation.source_artifact_id == operation.rebuilt_artifact_id
        || operation.locale.trim().is_empty()
        || operation.locale.trim() != operation.locale
        || operation.idempotency_key.trim().is_empty()
        || !is_sha256(&operation.request_hash)
        || !is_sha256(&operation.expected_provenance_hash)
        || !is_sha256(&operation.review_hash)
        || !is_sha256(&operation.rebuilt_artifact_hash)
        || !is_sha256(&operation.rebuilt_materialization_hash)
        || operation.artifact_instance_key != format!("rebuild:{}", operation.id)
    {
        return Err(replacement_target_invalid(
            "selected artifact rebuild receipt failed integrity validation",
        ));
    }
    Ok(())
}

fn verify_rebuild_source(source: &page_publish_rebuild_source::Model) -> PagesResult<()> {
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
        return Err(replacement_target_invalid(
            "selected rebuild provenance has invalid identity evidence",
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
            return Err(replacement_target_invalid(
                "selected rebuild provenance contains an invalid SHA-256 identity",
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
    .map_err(|error| replacement_target_invalid(error.to_string()))?;
    if expected != source.provenance_hash {
        return Err(replacement_target_invalid(
            "selected rebuild provenance hash failed validation",
        ));
    }
    Ok(())
}

fn ensure_rebuild_matches_source(
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
        return Err(replacement_target_invalid(
            "selected rebuild receipt does not match its retained provenance source",
        ));
    }
    Ok(())
}

fn verify_replacement_operation(
    operation: &page_artifact_binding_replacement_operation::Model,
) -> PagesResult<()> {
    if operation.id.is_nil()
        || operation.tenant_id.is_nil()
        || operation.page_id.is_nil()
        || operation.rebuild_operation_id.is_nil()
        || operation.page_body_id.is_nil()
        || operation.expected_current_artifact_id.is_nil()
        || operation.replacement_artifact_id.is_nil()
        || operation.expected_current_artifact_id == operation.replacement_artifact_id
        || operation.locale.trim().is_empty()
        || operation.locale.trim() != operation.locale
        || operation.idempotency_key.trim().is_empty()
        || operation.expected_version <= 0
        || operation.expected_version == i32::MAX
        || operation.result_version != operation.expected_version + 1
        || !is_sha256(&operation.request_hash)
        || !is_sha256(&operation.replacement_artifact_hash)
        || !is_sha256(&operation.replacement_materialization_hash)
    {
        return Err(replacement_operation_integrity(
            "stored artifact binding replacement receipt failed integrity validation",
        ));
    }
    Ok(())
}

fn result_from_record(
    operation: page_artifact_binding_replacement_operation::Model,
    replayed: bool,
) -> PagesResult<ReplacePageArtifactBindingResult> {
    verify_replacement_operation(&operation)?;
    Ok(ReplacePageArtifactBindingResult {
        operation_id: operation.id,
        page_id: operation.page_id,
        version: operation.result_version,
        locale: operation.locale,
        idempotency_key: operation.idempotency_key,
        rebuild_operation_id: operation.rebuild_operation_id,
        previous_artifact_id: operation.expected_current_artifact_id,
        replacement_artifact_id: operation.replacement_artifact_id,
        replacement_artifact_hash: operation.replacement_artifact_hash,
        replacement_materialization_hash: operation.replacement_materialization_hash,
        replayed,
        replaced_at: operation.replaced_at.to_string(),
    })
}

fn stable_replacement_hash(value: &impl Serialize) -> PagesResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        replacement_operation_integrity(format!(
            "unable to encode artifact binding replacement request identity: {error}",
        ))
    })?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn replacement_idempotency_conflict(message: impl Into<String>) -> PagesError {
    PagesError::rollback_idempotency_conflict(format!(
        "{PAGE_ARTIFACT_BINDING_REPLACEMENT_IDEMPOTENCY_CONFLICT}: {}",
        message.into(),
    ))
}

fn replacement_current_conflict(message: impl Into<String>) -> PagesError {
    PagesError::rollback_target_unavailable(format!(
        "{PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT}: {}",
        message.into(),
    ))
}

fn replacement_target_invalid(message: impl Into<String>) -> PagesError {
    PagesError::rollback_target_unavailable(format!(
        "{PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID}: {}",
        message.into(),
    ))
}

fn replacement_operation_integrity(message: impl Into<String>) -> PagesError {
    PagesError::rollback_operation_integrity(format!(
        "{PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY}: {}",
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
            "artifact binding replacement requires tenant-wide pages:manage",
        ))
    }
}

fn normalize_idempotency_key(value: &str) -> PagesResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.len() > MAX_REPLACEMENT_IDEMPOTENCY_KEY_BYTES {
        return Err(PagesError::validation(format!(
            "artifact binding replacement idempotency_key must contain 1 to {MAX_REPLACEMENT_IDEMPOTENCY_KEY_BYTES} bytes",
        )));
    }
    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_idempotency_key_is_bounded() {
        assert!(normalize_idempotency_key("replacement-1").is_ok());
        assert!(normalize_idempotency_key("").is_err());
        assert!(normalize_idempotency_key(&"x".repeat(192)).is_err());
    }
}
