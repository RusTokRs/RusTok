use chrono::Utc;
use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dto::{RollbackPageInput, RollbackPageResult};
use crate::entities::{page, page_publish_operation, page_rollback_operation};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_owned_scope;

use super::artifact_set::{
    ArtifactSetMember, artifact_set_hash, load_current_published_set_in_tx,
    load_publish_manifest_in_tx, replace_current_published_set_in_tx,
};
use super::helpers::{apply_transition, enforce_expected_version};
use super::{PAGE_KIND, PageService, PageTransition};

const PAGE_ROLLBACK_OPERATION_FORMAT: &str = "page_rollback_operation_v1";
const MAX_ROLLBACK_IDEMPOTENCY_KEY_BYTES: usize = 191;

impl PageService {
    /// Restores the previous distinct immutable publish artifact set in one transaction.
    ///
    /// Rollback never recompiles the current Fly document. It verifies the target publish manifest,
    /// switches every published locale binding, advances the page version, emits transactional
    /// `NodeUpdated`/`NodePublished` events and stores a replayable rollback receipt atomically.
    pub async fn rollback_to_previous(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: RollbackPageInput,
    ) -> PagesResult<RollbackPageResult> {
        let idempotency_key = normalize_rollback_idempotency_key(&input.idempotency_key)?;
        let txn = self.db.begin().await?;
        let existing_page = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            existing_page.author_id,
        )?;

        if let Some(operation) =
            find_rollback_operation_in_tx(&txn, tenant_id, page_id, &idempotency_key).await?
        {
            let request_hash = rollback_request_hash(
                tenant_id,
                page_id,
                input.expected_version,
                operation.target_publish_operation_id,
            )?;
            ensure_same_rollback_request(
                &operation,
                tenant_id,
                page_id,
                &idempotency_key,
                &request_hash,
            )?;
            let result = rollback_result_from_record(operation, true)?;
            txn.commit().await?;
            return Ok(result);
        }

        enforce_expected_version(Some(input.expected_version), existing_page.version)?;
        if existing_page.status != "published" {
            return Err(PagesError::rollback_requires_published());
        }

        let current_members = load_current_published_set_in_tx(&txn, tenant_id, page_id).await?;
        let source_artifact_set_hash = artifact_set_hash(&current_members)?;
        let (target_operation, target_members) = find_previous_publish_target_in_tx(
            &txn,
            tenant_id,
            page_id,
            existing_page.version,
            &source_artifact_set_hash,
        )
        .await?;
        let target_artifact_set_hash = artifact_set_hash(&target_members)?;
        if target_artifact_set_hash == source_artifact_set_hash {
            return Err(PagesError::rollback_target_unavailable(
                "previous publish artifact set is already active",
            ));
        }
        let request_hash = rollback_request_hash(
            tenant_id,
            page_id,
            input.expected_version,
            target_operation.id,
        )?;

        replace_current_published_set_in_tx(&txn, tenant_id, page_id, &target_members).await?;

        let now = Utc::now();
        let mut active: page::ActiveModel = existing_page.into();
        active.updated_at = Set(now.into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        apply_transition(&mut active, Some(PageTransition::Publish), now);
        let rolled_back_page = active.update(&txn).await?;

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

        let operation = insert_rollback_operation_in_tx(
            &txn,
            tenant_id,
            page_id,
            idempotency_key,
            request_hash,
            target_operation.id,
            source_artifact_set_hash,
            target_artifact_set_hash,
            rolled_back_page.version,
            now,
        )
        .await?;
        let result = rollback_result_from_record(operation, false)?;
        txn.commit().await?;
        Ok(result)
    }
}

async fn find_previous_publish_target_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    current_page_version: i32,
    current_artifact_set_hash: &str,
) -> PagesResult<(page_publish_operation::Model, Vec<ArtifactSetMember>)> {
    let cursor = find_current_publish_cursor_in_tx(
        txn,
        tenant_id,
        page_id,
        current_page_version,
        current_artifact_set_hash,
    )
    .await?;

    let query = || {
        page_publish_operation::Entity::find()
            .filter(page_publish_operation::Column::TenantId.eq(tenant_id))
            .filter(page_publish_operation::Column::PageId.eq(page_id))
            .filter(page_publish_operation::Column::ResultVersion.lt(cursor.result_version))
            .order_by_desc(page_publish_operation::Column::ResultVersion)
            .order_by_desc(page_publish_operation::Column::PublishedAt)
    };
    let operations = match txn.get_database_backend() {
        DbBackend::Sqlite => query().all(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().all(txn).await?,
    };
    for operation in operations {
        verify_publish_operation_for_rollback(&operation)?;
        if operation.artifact_set_hash == current_artifact_set_hash {
            continue;
        }
        let manifest = load_publish_manifest_in_tx(txn, &operation).await?;
        return Ok((operation, manifest));
    }
    Err(PagesError::rollback_target_unavailable(
        "no older distinct immutable publish artifact set is available",
    ))
}

async fn find_current_publish_cursor_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    current_page_version: i32,
    current_artifact_set_hash: &str,
) -> PagesResult<page_publish_operation::Model> {
    let publish_query = || {
        page_publish_operation::Entity::find()
            .filter(page_publish_operation::Column::TenantId.eq(tenant_id))
            .filter(page_publish_operation::Column::PageId.eq(page_id))
            .filter(page_publish_operation::Column::ArtifactSetHash.eq(current_artifact_set_hash))
            .filter(page_publish_operation::Column::ResultVersion.lte(current_page_version))
            .order_by_desc(page_publish_operation::Column::ResultVersion)
    };
    let rollback_query = || {
        page_rollback_operation::Entity::find()
            .filter(page_rollback_operation::Column::TenantId.eq(tenant_id))
            .filter(page_rollback_operation::Column::PageId.eq(page_id))
            .filter(
                page_rollback_operation::Column::TargetArtifactSetHash
                    .eq(current_artifact_set_hash),
            )
            .filter(page_rollback_operation::Column::ResultVersion.lte(current_page_version))
            .order_by_desc(page_rollback_operation::Column::ResultVersion)
    };
    let latest_publish = match txn.get_database_backend() {
        DbBackend::Sqlite => publish_query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => publish_query().lock_shared().one(txn).await?,
    };
    let latest_rollback = match txn.get_database_backend() {
        DbBackend::Sqlite => rollback_query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => rollback_query().lock_shared().one(txn).await?,
    };

    let cursor = match (latest_publish, latest_rollback) {
        (Some(publish), Some(rollback)) if rollback.result_version > publish.result_version => {
            verify_rollback_operation(&rollback)?;
            find_publish_operation_by_id_in_tx(
                txn,
                tenant_id,
                page_id,
                rollback.target_publish_operation_id,
            )
            .await?
        }
        (Some(publish), _) => publish,
        (None, Some(rollback)) => {
            verify_rollback_operation(&rollback)?;
            find_publish_operation_by_id_in_tx(
                txn,
                tenant_id,
                page_id,
                rollback.target_publish_operation_id,
            )
            .await?
        }
        (None, None) => {
            return Err(PagesError::rollback_target_unavailable(
                "the active immutable artifact set is not traceable to a publish or rollback receipt",
            ));
        }
    };
    verify_publish_operation_for_rollback(&cursor)?;
    if cursor.artifact_set_hash != current_artifact_set_hash {
        return Err(PagesError::rollback_target_unavailable(
            "the active immutable artifact set does not match its activation receipt",
        ));
    }
    load_publish_manifest_in_tx(txn, &cursor).await?;
    Ok(cursor)
}

async fn find_publish_operation_by_id_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    operation_id: Uuid,
) -> PagesResult<page_publish_operation::Model> {
    let query = || {
        page_publish_operation::Entity::find_by_id(operation_id)
            .filter(page_publish_operation::Column::TenantId.eq(tenant_id))
            .filter(page_publish_operation::Column::PageId.eq(page_id))
    };
    match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
    }
    .ok_or_else(|| {
        PagesError::rollback_target_unavailable(format!(
            "rollback activation references unavailable publish operation `{operation_id}`"
        ))
    })
}

async fn find_rollback_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    idempotency_key: &str,
) -> PagesResult<Option<page_rollback_operation::Model>> {
    let query = || {
        page_rollback_operation::Entity::find()
            .filter(page_rollback_operation::Column::TenantId.eq(tenant_id))
            .filter(page_rollback_operation::Column::PageId.eq(page_id))
            .filter(page_rollback_operation::Column::IdempotencyKey.eq(idempotency_key))
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
    })
}

#[allow(clippy::too_many_arguments)]
async fn insert_rollback_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    idempotency_key: String,
    request_hash: String,
    target_publish_operation_id: Uuid,
    source_artifact_set_hash: String,
    target_artifact_set_hash: String,
    result_version: i32,
    rolled_back_at: chrono::DateTime<Utc>,
) -> PagesResult<page_rollback_operation::Model> {
    let timestamp: sea_orm::prelude::DateTimeWithTimeZone = rolled_back_at.into();
    page_rollback_operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        idempotency_key: Set(idempotency_key),
        request_hash: Set(request_hash),
        target_publish_operation_id: Set(target_publish_operation_id),
        source_artifact_set_hash: Set(source_artifact_set_hash),
        target_artifact_set_hash: Set(target_artifact_set_hash),
        result_version: Set(result_version),
        rolled_back_at: Set(timestamp.clone()),
        created_at: Set(timestamp),
    }
    .insert(txn)
    .await
    .map_err(Into::into)
}

fn rollback_request_hash(
    tenant_id: Uuid,
    page_id: Uuid,
    expected_version: i32,
    target_publish_operation_id: Uuid,
) -> PagesResult<String> {
    stable_hash(&(
        PAGE_ROLLBACK_OPERATION_FORMAT,
        tenant_id,
        page_id,
        expected_version,
        target_publish_operation_id,
    ))
}

fn ensure_same_rollback_request(
    operation: &page_rollback_operation::Model,
    tenant_id: Uuid,
    page_id: Uuid,
    idempotency_key: &str,
    request_hash: &str,
) -> PagesResult<()> {
    verify_rollback_operation(operation)?;
    if operation.tenant_id != tenant_id
        || operation.page_id != page_id
        || operation.idempotency_key != idempotency_key
        || operation.request_hash != request_hash
    {
        return Err(PagesError::rollback_idempotency_conflict(format!(
            "idempotency key `{idempotency_key}` is already bound to a different page rollback request"
        )));
    }
    Ok(())
}

fn verify_publish_operation_for_rollback(
    operation: &page_publish_operation::Model,
) -> PagesResult<()> {
    if operation.id.is_nil()
        || operation.tenant_id.is_nil()
        || operation.page_id.is_nil()
        || operation.result_version <= 0
        || !is_sha256(&operation.request_hash)
        || !is_sha256(&operation.review_hash)
        || !is_sha256(&operation.sanitized_set_hash)
        || !is_sha256(&operation.artifact_set_hash)
    {
        return Err(PagesError::rollback_target_unavailable(
            "stored page publish operation contains invalid rollback target evidence",
        ));
    }
    Ok(())
}

fn verify_rollback_operation(operation: &page_rollback_operation::Model) -> PagesResult<()> {
    if operation.id.is_nil()
        || operation.tenant_id.is_nil()
        || operation.page_id.is_nil()
        || operation.target_publish_operation_id.is_nil()
        || operation.idempotency_key.trim().is_empty()
        || operation.result_version <= 0
        || !is_sha256(&operation.request_hash)
        || !is_sha256(&operation.source_artifact_set_hash)
        || !is_sha256(&operation.target_artifact_set_hash)
        || operation.source_artifact_set_hash == operation.target_artifact_set_hash
    {
        return Err(PagesError::rollback_operation_integrity(
            "stored page rollback operation contains invalid identity or hash evidence",
        ));
    }
    Ok(())
}

fn rollback_result_from_record(
    operation: page_rollback_operation::Model,
    replayed: bool,
) -> PagesResult<RollbackPageResult> {
    verify_rollback_operation(&operation)?;
    Ok(RollbackPageResult {
        operation_id: operation.id,
        page_id: operation.page_id,
        version: operation.result_version,
        idempotency_key: operation.idempotency_key,
        target_publish_operation_id: operation.target_publish_operation_id,
        source_artifact_set_hash: operation.source_artifact_set_hash,
        target_artifact_set_hash: operation.target_artifact_set_hash,
        replayed,
        rolled_back_at: operation.rolled_back_at.to_string(),
    })
}

fn normalize_rollback_idempotency_key(value: &str) -> PagesResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.len() > MAX_ROLLBACK_IDEMPOTENCY_KEY_BYTES {
        return Err(PagesError::validation(format!(
            "rollback idempotency_key must contain 1 to {MAX_ROLLBACK_IDEMPOTENCY_KEY_BYTES} bytes"
        )));
    }
    Ok(normalized.to_string())
}

fn stable_hash(value: &impl Serialize) -> PagesResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        PagesError::rollback_operation_integrity(format!(
            "unable to encode page rollback identity: {error}"
        ))
    })?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
