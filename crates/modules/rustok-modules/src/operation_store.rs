use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

use crate::ModuleOperationStatus;
use crate::backend::SupportedBackend;

/// Maximum number of attention-needed operations returned per tenant query.
pub const MAX_FAILED_OPERATIONS_FOR_TENANT: u32 = 200;

/// Maximum number of stale lifecycle operations reconciled by one reaper
/// invocation. The watchdog repeats bounded passes instead of holding one
/// unbounded maintenance transaction.
pub const MAX_STALE_MODULE_OPERATIONS_PER_REAP: usize = 100;

/// Lease granted to a static lifecycle aggregate claim. A claim whose owner
/// crashed (or never released) may be reclaimed by a later command only
/// after this interval; the previous holder is fenced out because every
/// subsequent advance/release matches on its own idempotency key.
pub const STATIC_LIFECYCLE_CLAIM_LEASE_SECONDS: i64 = 300;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleOperationRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub requested_enabled: bool,
    pub previous_effective_enabled: bool,
    pub requested_by: Option<String>,
    pub trace_id: Option<String>,
    pub correlation_id: String,
    pub idempotency_key: Option<Uuid>,
    /// Original failed operation for recovery commands; part of replay identity.
    pub recovery_source_id: Option<Uuid>,
    /// Lifecycle commands bind this precondition into the durable
    /// idempotency identity. New executor and recovery commands always carry
    /// a revision; `None` exists only for legacy journal rows and direct
    /// store callers that predate the unified claim/CAS pipeline.
    pub expected_revision: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleOperationRecord {
    pub id: Uuid,
}

/// Result of recording a lifecycle command with a caller-supplied idempotency
/// key. A replay must not dispatch the lifecycle hook a second time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleOperationRecordOutcome {
    Recorded(ModuleOperationRecord),
    Replayed(ModuleOperationRecord),
}

/// Durable lifecycle journal data exposed without leaking server ORM entities
/// into the module control-plane contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleOperationSnapshot {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub requested_enabled: bool,
    pub previous_effective_enabled: bool,
    pub status: ModuleOperationStatus,
    pub requested_by: Option<String>,
    pub trace_id: Option<String>,
    pub correlation_id: Option<String>,
    pub idempotency_key: Option<Uuid>,
    pub recovery_source_id: Option<Uuid>,
    pub expected_revision: Option<u64>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Outcome of one bounded stale-operation reconciliation pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StaleModuleOperationReapReport {
    /// Candidate rows inspected in this pass.
    pub scanned: usize,
    /// `Validated`/`Running` rows proven abandoned and moved to
    /// `orphaned`.
    pub orphaned: usize,
    /// `CommittedHookPending` rows whose post-commit hook outcome could not
    /// be proven because the owning process disappeared; moved to
    /// `committed_hook_failed` for the existing recovery path.
    pub committed_hook_unknown: usize,
    /// Aggregate claims released because their owning operation reached (or
    /// is being moved to) a terminal status and the claim lease expired.
    pub released_claims: usize,
}

impl StaleModuleOperationReapReport {
    /// Whether this pass changed any operation or aggregate claim. A scan
    /// that only observed live claims is intentionally not a change.
    pub const fn has_changes(self) -> bool {
        self.orphaned > 0 || self.committed_hook_unknown > 0 || self.released_claims > 0
    }
}

#[derive(Debug, Error)]
pub enum ModuleOperationStoreError {
    #[error("module operation store database error: {0}")]
    Database(String),
    #[error("module operation store unique constraint violation: {0}")]
    UniqueViolation(String),
    #[error("module operation `{0}` was not found")]
    OperationNotFound(Uuid),
    #[error(
        "module operation `{operation_id}` cannot transition from `{current}` to `{requested}`"
    )]
    InvalidStatusTransition {
        operation_id: Uuid,
        current: ModuleOperationStatus,
        requested: ModuleOperationStatus,
    },
    #[error("module `{0}` is not enabled for this tenant")]
    ModuleNotEnabled(String),
    #[error("module operation idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("module operation idempotency key is required")]
    MissingIdempotencyKey,
    #[error("SQLite tenant or module operation identity is missing, unsupported, or ambiguous")]
    UnverifiableSqliteIdentity,
}

/// Owner-owned persistence for lifecycle operation journaling.
///
/// The generic connection parameter retains the caller's transaction boundary.
pub struct ModuleOperationJournal;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TenantModuleStateRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TenantModuleStateRecord {
    pub id: Uuid,
    /// Tenant enablement intent recorded after this operation, or `None`
    /// when the tenant has recorded no intent and the module inherits the
    /// platform default.
    pub enabled: Option<bool>,
    /// Tenant enablement intent recorded before this operation, or `None`
    /// when none was recorded (no `tenant_modules` row existed, or the row
    /// carried settings without intent). (Previously the insert path
    /// fabricated `!enabled` here, inventing a predecessor state that never
    /// existed.)
    pub previous_enabled: Option<bool>,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TenantModuleSettingsRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub settings: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TenantModuleSettingsRecord {
    pub id: Uuid,
    pub module_slug: String,
    /// Recorded tenant enablement intent after the save, or `None` when the
    /// row carries settings without intent (platform default applies).
    pub enabled: Option<bool>,
    pub settings: serde_json::Value,
}

/// Durable concurrency snapshot for one static module lifecycle aggregate.
/// The aggregate exists independently from the `tenant_modules` override
/// projection, so an inherited/default state has revision zero until its first
/// lifecycle command claims the aggregate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticTenantLifecycleSnapshot {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub revision: u64,
    pub active_idempotency_key: Option<Uuid>,
}

/// Result of admitting one static lifecycle operation to its aggregate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StaticTenantLifecycleClaim {
    Acquired(StaticTenantLifecycleSnapshot),
    AlreadyHeld(StaticTenantLifecycleSnapshot),
}

/// Database-clock view of the aggregate claim used only by stale-operation
/// reconciliation in this owner module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StaticTenantLifecycleClaimState {
    active_idempotency_key: Option<Uuid>,
    expired: bool,
}

#[derive(Debug, Error)]
pub enum StaticTenantLifecycleStoreError {
    #[error("static module lifecycle store database error: {0}")]
    Database(String),
    #[error(
        "static module lifecycle revision conflict for `{module_slug}`: expected {expected}, current {current}"
    )]
    RevisionConflict {
        module_slug: String,
        expected: u64,
        current: u64,
    },
    #[error("static module lifecycle operation is already active for `{module_slug}`")]
    OperationInProgress { module_slug: String },
    #[error("static module lifecycle revision overflow for `{0}")]
    RevisionOverflow(String),
    #[error("static module lifecycle state is inconsistent for `{0}")]
    InconsistentState(String),
}

/// Owner-owned persistence for a tenant's explicit module state row.
pub struct TenantModuleStateStore;

/// Owner-owned persistence for the static module lifecycle revision aggregate.
pub struct StaticTenantLifecycleStore;

impl ModuleOperationJournal {
    pub async fn find<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<Option<ModuleOperationSnapshot>, ModuleOperationStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let sql = match backend {
            SupportedBackend::Postgres => format!("{} WHERE id = $1", operation_select_sql()),
            SupportedBackend::Sqlite => "WITH selected AS (SELECT * FROM module_operations WHERE id IN (?1, ?2) LIMIT 2), \
                candidate AS (SELECT selected.*, CASE \
                  WHEN typeof(tenant_id) = 'blob' AND length(tenant_id) = 16 THEN \
                    lower(substr(hex(tenant_id), 1, 8) || '-' || substr(hex(tenant_id), 9, 4) || '-' || \
                          substr(hex(tenant_id), 13, 4) || '-' || substr(hex(tenant_id), 17, 4) || '-' || substr(hex(tenant_id), 21, 12)) \
                  WHEN typeof(tenant_id) = 'text' THEN unhex(replace(tenant_id, '-', '')) \
                  ELSE NULL END AS tenant_alternate FROM selected) \
                SELECT candidate.*, candidate.module_slug AS matched_module, \
                (SELECT COUNT(*) FROM tenant_identity_keys WHERE id IN (candidate.tenant_id, candidate.tenant_alternate)) AS parent_count, \
                (candidate.tenant_id = (SELECT id FROM tenant_identity_keys WHERE id IN (candidate.tenant_id, candidate.tenant_alternate) LIMIT 1)) AS matches_parent, \
                (EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = candidate.tenant_id) AND \
                 EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = candidate.tenant_alternate)) AS mixed_identity, \
                ((EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = candidate.tenant_id) AND \
                  NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = candidate.tenant_id)) OR \
                 (EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = candidate.tenant_alternate) AND \
                  NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = candidate.tenant_alternate))) AS orphaned_identity, \
                typeof(candidate.tenant_id) AS tenant_id_storage, typeof(candidate.tenant_alternate) AS alternate_storage, \
                length(candidate.tenant_alternate) AS alternate_length, typeof(candidate.id) AS operation_id_storage, \
                typeof(candidate.idempotency_key) AS idempotency_key_storage FROM candidate".to_string(),
        };
        let mut values = vec![operation_id.into()];
        if matches!(backend, SupportedBackend::Sqlite) {
            values.push(operation_id.to_string().into());
        }
        let mut rows = db
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                sql,
                values,
            ))
            .await
            .map_err(database_error)?;
        if rows.len() > 1 {
            return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
        }
        let Some(row) = rows.pop() else {
            return Ok(None);
        };
        if matches!(backend, SupportedBackend::Sqlite) {
            if !validate_sqlite_journal_identity(&row)? {
                return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
            }
            let tenant_storage: String = row.try_get("", "tenant_id_storage").map_err(database_error)?;
            let alternate_storage: String = row.try_get("", "alternate_storage").map_err(database_error)?;
            let alternate_length: Option<i64> = row.try_get("", "alternate_length").map_err(database_error)?;
            if !matches!((tenant_storage.as_str(), alternate_storage.as_str(), alternate_length),
                ("blob", "text", Some(36)) | ("text", "blob", Some(16))) {
                return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
            }
            let storage: String = row
                .try_get("", "operation_id_storage")
                .map_err(database_error)?;
            if storage != "blob" {
                return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
            }
            let key_storage: String = row
                .try_get("", "idempotency_key_storage")
                .map_err(database_error)?;
            if key_storage != "blob" && key_storage != "null" {
                return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
            }
        }
        Ok(Some(operation_snapshot(backend, row)?))
    }

    /// Lists operations needing operator attention for one tenant: hard
    /// failures plus state-committed operations whose post-hook is still
    /// outstanding. Bounded to the most recent
    /// `MAX_FAILED_OPERATIONS_FOR_TENANT` rows.
    pub async fn failed_for_tenant<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: Option<&str>,
    ) -> Result<Vec<ModuleOperationSnapshot>, ModuleOperationStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let statuses = format!(
            "status IN ('{}', '{}', '{}')",
            ModuleOperationStatus::Failed.as_str(),
            ModuleOperationStatus::CommittedHookFailed.as_str(),
            ModuleOperationStatus::Orphaned.as_str()
        );
        if matches!(backend, SupportedBackend::Sqlite) {
            let sql = format!(
                "WITH identity AS (SELECT COUNT(*) AS parent_count FROM tenant_identity_keys WHERE id IN (?1, ?2)), \
                 matched AS (SELECT * FROM module_operations WHERE tenant_id IN (?1, ?2) AND {statuses} \
                 AND (?3 IS NULL OR module_slug = ?3) ORDER BY created_at DESC, id DESC LIMIT {MAX_FAILED_OPERATIONS_FOR_TENANT}) \
                 SELECT matched.*, matched.module_slug AS matched_module, identity.parent_count, \
                 (matched.tenant_id = (SELECT id FROM tenant_identity_keys WHERE id IN (?1, ?2) LIMIT 1)) AS matches_parent, \
                 (EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?1) AND EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?2)) AS mixed_identity, \
                 ((EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?1) AND NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = ?1)) OR \
                  (EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?2) AND NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = ?2))) AS orphaned_identity, \
                 typeof(matched.id) AS operation_id_storage, typeof(matched.idempotency_key) AS idempotency_key_storage \
                 FROM identity LEFT JOIN matched ON TRUE ORDER BY matched.created_at DESC, matched.id DESC"
            );
            let rows = db
                .query_all_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    sql,
                    vec![
                        tenant_id.into(),
                        tenant_id.to_string().into(),
                        module_slug.map(str::to_owned).into(),
                    ],
                ))
                .await
                .map_err(database_error)?;
            let mut operations = Vec::with_capacity(rows.len());
            for row in rows {
                if !validate_sqlite_journal_identity(&row)? {
                    continue;
                }
                let storage: String = row
                    .try_get("", "operation_id_storage")
                    .map_err(database_error)?;
                if storage != "blob" {
                    return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
                }
                let key_storage: String = row
                    .try_get("", "idempotency_key_storage")
                    .map_err(database_error)?;
                if key_storage != "blob" && key_storage != "null" {
                    return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
                }
                operations.push(operation_snapshot(backend, row)?);
            }
            return Ok(operations);
        }
        let (sql, values) = match module_slug {
            Some(module_slug) => (
                format!(
                    "{} WHERE tenant_id = $1 AND {statuses} AND module_slug = $2 ORDER BY created_at DESC, id DESC LIMIT {}",
                    operation_select_sql(),
                    MAX_FAILED_OPERATIONS_FOR_TENANT
                ),
                vec![tenant_id.into(), module_slug.into()],
            ),
            None => (
                format!(
                    "{} WHERE tenant_id = $1 AND {statuses} ORDER BY created_at DESC, id DESC LIMIT {}",
                    operation_select_sql(),
                    MAX_FAILED_OPERATIONS_FOR_TENANT
                ),
                vec![tenant_id.into()],
            ),
        };
        db.query_all_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            sql,
            values,
        ))
        .await
        .map_err(database_error)?
        .into_iter()
        .map(|row| operation_snapshot(backend, row))
        .collect()
    }

    pub async fn record_idempotent<C: ConnectionTrait>(
        db: &C,
        request: ModuleOperationRequest,
    ) -> Result<ModuleOperationRecordOutcome, ModuleOperationStoreError> {
        let idempotency_key = request
            .idempotency_key
            .ok_or(ModuleOperationStoreError::MissingIdempotencyKey)?;
        if let Some(existing) =
            Self::find_by_idempotency_key(db, request.tenant_id, idempotency_key).await?
        {
            return replay_or_conflict(&request, existing);
        }

        match Self::record(db, request.clone()).await {
            Ok(record) => Ok(ModuleOperationRecordOutcome::Recorded(record)),
            Err(error @ ModuleOperationStoreError::UniqueViolation(_)) => {
                if let Some(existing) =
                    Self::find_by_idempotency_key(db, request.tenant_id, idempotency_key).await?
                {
                    replay_or_conflict(&request, existing)
                } else {
                    Err(error)
                }
            }
            Err(error) => Err(error),
        }
    }

    /// Returns a prior matching command before transient state validation.
    /// This makes a completed or failed command replay stable even if the
    /// tenant state changes after its first execution.
    pub async fn replay_idempotent<C: ConnectionTrait>(
        db: &C,
        request: &ModuleOperationRequest,
    ) -> Result<Option<ModuleOperationRecord>, ModuleOperationStoreError> {
        let idempotency_key = request
            .idempotency_key
            .ok_or(ModuleOperationStoreError::MissingIdempotencyKey)?;
        Self::find_by_idempotency_key(db, request.tenant_id, idempotency_key)
            .await?
            .map(|existing| match replay_or_conflict(request, existing)? {
                ModuleOperationRecordOutcome::Replayed(record) => Ok(record),
                ModuleOperationRecordOutcome::Recorded(_) => {
                    unreachable!("existing record replays")
                }
            })
            .transpose()
    }

    /// Replays a lifecycle command before recomputing transient effective
    /// state. The caller must still use `record_idempotent` to retain the full
    /// previous-state fingerprint during the initial write.
    pub async fn replay_idempotent_command<C: ConnectionTrait>(
        db: &C,
        request: &ModuleOperationRequest,
    ) -> Result<Option<ModuleOperationSnapshot>, ModuleOperationStoreError> {
        let idempotency_key = request
            .idempotency_key
            .ok_or(ModuleOperationStoreError::MissingIdempotencyKey)?;
        let Some(existing) =
            Self::find_by_idempotency_key(db, request.tenant_id, idempotency_key).await?
        else {
            return Ok(None);
        };
        if existing.module_slug != request.module_slug
            || existing.requested_enabled != request.requested_enabled
            || existing.expected_revision != request.expected_revision
            || existing.recovery_source_id != request.recovery_source_id
        {
            return Err(ModuleOperationStoreError::IdempotencyConflict);
        }
        Ok(Some(existing))
    }

    pub async fn record<C: ConnectionTrait>(
        db: &C,
        request: ModuleOperationRequest,
    ) -> Result<ModuleOperationRecord, ModuleOperationStoreError> {
        let id = rustok_core::generate_id();
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let expected_revision = request
            .expected_revision
            .map(i64::try_from)
            .transpose()
            .map_err(|_| {
                ModuleOperationStoreError::Database(
                    "module lifecycle expected revision exceeds storage range".to_string(),
                )
            })?;
        let sql = match backend {
            SupportedBackend::Postgres => render_parameters(
                "INSERT INTO module_operations (id, tenant_id, module_slug, requested_enabled, previous_effective_enabled, status, requested_by, trace_id, correlation_id, idempotency_key, expected_revision, recovery_source_id, error_message) VALUES ({1}, {2}, {3}, {4}, {5}, {6}, {7}, {8}, {9}, {10}, {11}, {12}, NULL)", backend),
            SupportedBackend::Sqlite => "INSERT INTO module_operations (id, tenant_id, module_slug, requested_enabled, previous_effective_enabled, status, requested_by, trace_id, correlation_id, idempotency_key, expected_revision, recovery_source_id, error_message) \
                SELECT ?1, parent.id, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL \
                FROM tenant_identity_keys parent WHERE parent.id IN (?2, ?13) \
                AND (SELECT COUNT(*) FROM tenant_identity_keys WHERE id IN (?2, ?13)) = 1 \
                AND NOT EXISTS (SELECT 1 FROM module_operations o WHERE o.tenant_id IN (?2, ?13) AND o.tenant_id != parent.id)".to_string(),
        };
        let mut values = vec![
            id.into(),
            request.tenant_id.into(),
            request.module_slug.into(),
            request.requested_enabled.into(),
            request.previous_effective_enabled.into(),
            ModuleOperationStatus::Validated.as_str().into(),
            request.requested_by.into(),
            request.trace_id.into(),
            request.correlation_id.into(),
            request.idempotency_key.into(),
            expected_revision.into(),
            request.recovery_source_id.map(|id| id.to_string()).into(),
        ];
        if matches!(backend, SupportedBackend::Sqlite) {
            values.push(request.tenant_id.to_string().into());
        }
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                sql,
                values,
            ))
            .await
            .map_err(|error| {
                if backend.is_unique_violation(&error) {
                    ModuleOperationStoreError::UniqueViolation(error.to_string())
                } else {
                    ModuleOperationStoreError::Database(error.to_string())
                }
            })?;
        if result.rows_affected() != 1 {
            if matches!(backend, SupportedBackend::Sqlite) && result.rows_affected() == 0 {
                return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
            }
            return Err(ModuleOperationStoreError::Database(format!(
                "expected module operation insert to affect exactly one row, affected {}",
                result.rows_affected()
            )));
        }
        Ok(ModuleOperationRecord { id })
    }

    /// Removes a newly journaled operation that never acquired its
    /// aggregate claim. Claim contention and revision conflicts are not
    /// operation failures: retaining the row would poison the caller's
    /// idempotency key with a terminal outcome for work that never ran.
    /// The guarded delete can only remove the still-`Validated` row.
    pub async fn discard_validated<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<bool, ModuleOperationStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                render_parameters(
                    "DELETE FROM module_operations WHERE id = {1} AND status = {2}",
                    backend,
                ),
                vec![
                    operation_id.into(),
                    ModuleOperationStatus::Validated.as_str().into(),
                ],
            ))
            .await
            .map_err(database_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn find_by_idempotency_key<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        idempotency_key: Uuid,
    ) -> Result<Option<ModuleOperationSnapshot>, ModuleOperationStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let (sql, values) = match backend {
            SupportedBackend::Postgres => (
                format!("{} WHERE tenant_id = $1 AND idempotency_key = $2 LIMIT 1", operation_select_sql()),
                vec![tenant_id.into(), idempotency_key.into()],
            ),
            SupportedBackend::Sqlite => (
                "WITH identity AS (SELECT COUNT(*) AS parent_count FROM tenant_identity_keys WHERE id IN (?1, ?3)), \
                 matched AS (SELECT * FROM module_operations WHERE tenant_id IN (?1, ?3) AND idempotency_key IN (?2, ?4)) \
                 SELECT matched.*, matched.module_slug AS matched_module, identity.parent_count, \
                 (matched.tenant_id = (SELECT id FROM tenant_identity_keys WHERE id IN (?1, ?3) LIMIT 1)) AS matches_parent, \
                 (EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?1) AND EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?3)) AS mixed_identity, \
                 ((EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?1) AND NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = ?1)) OR \
                  (EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?3) AND NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = ?3))) AS orphaned_identity, \
                 typeof(matched.id) AS operation_id_storage, typeof(matched.idempotency_key) AS idempotency_key_storage \
                 FROM identity LEFT JOIN matched ON TRUE LIMIT 2".to_string(),
                vec![tenant_id.into(), idempotency_key.into(), tenant_id.to_string().into(), idempotency_key.to_string().into()],
            ),
        };
        let mut rows = db
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                sql,
                values,
            ))
            .await
            .map_err(database_error)?;
        if rows.len() > 1 {
            return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
        }
        let Some(row) = rows.pop() else {
            return Ok(None);
        };
        if matches!(backend, SupportedBackend::Sqlite) {
            if !validate_sqlite_journal_identity(&row)? {
                return Ok(None);
            }
            for column in ["operation_id_storage", "idempotency_key_storage"] {
                let storage: String = row.try_get("", column).map_err(database_error)?;
                if storage != "blob" {
                    return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
                }
            }
        }
        Ok(Some(operation_snapshot(backend, row)?))
    }

    pub async fn mark_running<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<(), ModuleOperationStoreError> {
        Self::mark_status(
            db,
            operation_id,
            ModuleOperationStatus::Running,
            &[ModuleOperationStatus::Validated],
            None,
        )
        .await
    }

    /// Records, inside the durable state-commit transaction, that the state
    /// is in effect but the post-hook has not yet completed. Keeping this
    /// state distinct from terminal `Committed` lets recovery distinguish a
    /// crashed post-commit owner from a fully successful operation.
    pub async fn mark_committed_hook_pending<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<(), ModuleOperationStoreError> {
        Self::mark_status(
            db,
            operation_id,
            ModuleOperationStatus::CommittedHookPending,
            &[ModuleOperationStatus::Running],
            None,
        )
        .await
    }

    pub async fn mark_committed<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<(), ModuleOperationStoreError> {
        Self::mark_status(
            db,
            operation_id,
            ModuleOperationStatus::Committed,
            &[
                ModuleOperationStatus::Validated,
                ModuleOperationStatus::Running,
                // The normal executor reaches terminal success only after
                // its post-hook succeeds.
                ModuleOperationStatus::CommittedHookPending,
                // Post-hook recovery resolves a committed-but-hook-failed
                // operation once the hook finally succeeds.
                ModuleOperationStatus::CommittedHookFailed,
            ],
            None,
        )
        .await
    }

    /// Terminal commit that also stores the exact serialized result the
    /// original caller received, in the same update as the status
    /// transition, so an idempotent replay can return operation-time
    /// evidence instead of reconstructing a response from current state.
    /// Accepts the same predecessor states as [`Self::mark_committed`].
    pub async fn mark_committed_with_result<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
        result_snapshot: &str,
    ) -> Result<(), ModuleOperationStoreError> {
        let allowed_from_sql = [
            ModuleOperationStatus::Validated,
            ModuleOperationStatus::Running,
            ModuleOperationStatus::CommittedHookPending,
            ModuleOperationStatus::CommittedHookFailed,
        ]
        .iter()
        .map(|status| format!("'{}'", status.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let sql = format!(
            "UPDATE module_operations SET status = {{1}}, result_snapshot = {{2}}, \
             updated_at = {{now}} WHERE id = {{3}} AND status IN ({allowed_from_sql})"
        );
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                render_parameters(&sql, backend),
                vec![
                    ModuleOperationStatus::Committed.as_str().into(),
                    result_snapshot.into(),
                    operation_id.into(),
                ],
            ))
            .await
            .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?;
        if result.rows_affected() != 1 {
            let current = Self::find(db, operation_id)
                .await?
                .ok_or(ModuleOperationStoreError::OperationNotFound(operation_id))?;
            return Err(ModuleOperationStoreError::InvalidStatusTransition {
                operation_id,
                current: current.status,
                requested: ModuleOperationStatus::Committed,
            });
        }
        Ok(())
    }

    /// Returns the stored serialized result for an operation, or `None` for
    /// rows committed before result snapshots existed (or through journal
    /// paths that do not produce a caller result). The payload shape is
    /// owned by the executor's result contract; the journal stores it
    /// opaquely.
    pub async fn result_snapshot<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<Option<String>, ModuleOperationStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                render_parameters(
                    "SELECT result_snapshot FROM module_operations WHERE id = {1}",
                    backend,
                ),
                vec![operation_id.into()],
            ))
            .await
            .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?
            .ok_or(ModuleOperationStoreError::OperationNotFound(operation_id))?;
        row.try_get("", "result_snapshot")
            .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))
    }

    /// Records that the operation's state commit is durable but its post-hook
    /// failed. It is reachable only from `CommittedHookPending`; terminal
    /// `Committed` operations are never rewritten after their state applied.
    pub async fn mark_committed_hook_failed<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
        error_message: &str,
    ) -> Result<(), ModuleOperationStoreError> {
        Self::mark_status(
            db,
            operation_id,
            ModuleOperationStatus::CommittedHookFailed,
            &[ModuleOperationStatus::CommittedHookPending],
            Some(error_message),
        )
        .await
    }

    /// Resolves an original operation whose post-hook has now succeeded via
    /// recovery. Accepts the typed `CommittedHookFailed` state and the
    /// legacy pre-split shape (`Failed` with a `post-hook:` message), which
    /// `mark_committed` deliberately does not: a plain `Failed` operation
    /// never committed its state and must never be rewritten to
    /// `Committed`. Returns `false` when the row exists but is in neither
    /// resolvable shape.
    pub async fn resolve_post_hook_failure<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<bool, ModuleOperationStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let sql_template = "UPDATE module_operations SET status = {1}, updated_at = {now} \
             WHERE id = {2} AND (status = {3} OR (status = {4} AND error_message LIKE 'post-hook:%'))";
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                render_parameters(sql_template, backend),
                vec![
                    ModuleOperationStatus::Committed.as_str().into(),
                    operation_id.into(),
                    ModuleOperationStatus::CommittedHookFailed.as_str().into(),
                    ModuleOperationStatus::Failed.as_str().into(),
                ],
            ))
            .await
            .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?;
        if result.rows_affected() == 1 {
            return Ok(true);
        }
        Self::find(db, operation_id)
            .await?
            .ok_or(ModuleOperationStoreError::OperationNotFound(operation_id))?;
        Ok(false)
    }

    pub async fn mark_failed<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
        error_message: &str,
    ) -> Result<(), ModuleOperationStoreError> {
        Self::mark_status(
            db,
            operation_id,
            ModuleOperationStatus::Failed,
            &[
                ModuleOperationStatus::Validated,
                ModuleOperationStatus::Running,
            ],
            Some(error_message),
        )
        .await
    }

    async fn mark_orphaned<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
        error_message: &str,
    ) -> Result<(), ModuleOperationStoreError> {
        Self::mark_status(
            db,
            operation_id,
            ModuleOperationStatus::Orphaned,
            &[
                ModuleOperationStatus::Validated,
                ModuleOperationStatus::Running,
            ],
            Some(error_message),
        )
        .await
    }

    /// Reconciles one bounded batch of lifecycle operations whose journal
    /// has not advanced within the aggregate claim lease.
    ///
    /// Journal age alone never proves abandonment while a matching claim is
    /// still live: long hooks heartbeat the claim without touching the
    /// operation row. A stale pre-commit operation is orphaned only when its
    /// claim is absent, belongs to another command, or has expired. A stale
    /// `CommittedHookPending` operation without a live matching claim has an
    /// unknown post-hook outcome, so it enters the existing committed-hook
    /// recovery path instead of being reported as successfully completed.
    /// Terminal `Committed` and `Failed` operations keep their status; only
    /// an expired matching claim left behind by an interrupted owner is
    /// released.
    pub async fn reap_stale(
        db: &DatabaseConnection,
        limit: usize,
    ) -> Result<StaleModuleOperationReapReport, ModuleOperationStoreError> {
        if limit == 0 || limit > MAX_STALE_MODULE_OPERATIONS_PER_REAP {
            return Err(ModuleOperationStoreError::Database(format!(
                "stale module operation reap limit must be between 1 and {MAX_STALE_MODULE_OPERATIONS_PER_REAP}"
            )));
        }
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let stale_predicate = match backend {
            SupportedBackend::Postgres => format!(
                "updated_at <= CURRENT_TIMESTAMP - INTERVAL '{STATIC_LIFECYCLE_CLAIM_LEASE_SECONDS} seconds'"
            ),
            SupportedBackend::Sqlite => format!(
                "datetime(updated_at) <= datetime('now', '-{STATIC_LIFECYCLE_CLAIM_LEASE_SECONDS} seconds')"
            ),
        };
        let transaction = db.begin().await.map_err(database_error)?;
        // On Postgres, lock candidate operation rows before locking their
        // lifecycle-claim rows below. This matches the executor's durable
        // transaction order (operation status first, claim second), avoiding
        // an opposite-order deadlock while making the two observations used
        // for a reap decision atomic with respect to heartbeat/finalization.
        // SQLite has no row-lock clause; guarded status transitions remain
        // the final fence there.
        let operation_lock_clause = match backend {
            SupportedBackend::Postgres => " FOR UPDATE SKIP LOCKED",
            SupportedBackend::Sqlite => "",
        };
        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "{} WHERE status IN ('validated', 'running', 'committed_hook_pending', \
                     'committed', 'committed_hook_failed', 'failed') AND {stale_predicate} \
                     ORDER BY CASE WHEN status IN ('validated', 'running', 'committed_hook_pending') \
                       THEN 0 ELSE 1 END ASC, updated_at ASC, id ASC LIMIT {limit}{operation_lock_clause}",
                    operation_select_sql()
                ),
                Vec::new(),
            ))
            .await
            .map_err(database_error)?;
        let candidates = rows
            .into_iter()
            .map(|row| operation_snapshot(backend, row))
            .collect::<Result<Vec<_>, _>>()?;

        let mut report = StaleModuleOperationReapReport::default();
        let mut validated_tenants = BTreeSet::new();
        for candidate in candidates {
            report.scanned += 1;
            if matches!(backend, SupportedBackend::Sqlite)
                && validated_tenants.insert(candidate.tenant_id)
            {
                validate_sqlite_journal_tenant(&transaction, candidate.tenant_id).await?;
            }
            let claim_state = StaticTenantLifecycleStore::claim_state_for_update(
                &transaction,
                candidate.tenant_id,
                &candidate.module_slug,
            )
            .await
            .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?;
            let matching_claim = match (candidate.idempotency_key, claim_state.as_ref()) {
                (Some(operation_key), Some(claim_state)) => {
                    claim_state.active_idempotency_key == Some(operation_key)
                }
                _ => false,
            };
            let matching_claim_live =
                matching_claim && claim_state.as_ref().is_some_and(|claim| !claim.expired);
            let matching_claim_expired =
                matching_claim && claim_state.as_ref().is_some_and(|claim| claim.expired);

            match candidate.status {
                ModuleOperationStatus::Validated | ModuleOperationStatus::Running => {
                    if matching_claim_live {
                        continue;
                    }
                    match Self::mark_orphaned(
                        &transaction,
                        candidate.id,
                        "module operation owner did not finish within the lifecycle claim lease; inspect current tenant enablement before repeating the command",
                    )
                    .await
                    {
                        Ok(()) => report.orphaned += 1,
                        Err(
                            ModuleOperationStoreError::InvalidStatusTransition { .. }
                            | ModuleOperationStoreError::OperationNotFound(_),
                        ) => continue,
                        Err(error) => return Err(error),
                    }
                    if matching_claim_expired {
                        let operation_key = candidate.idempotency_key.ok_or_else(|| {
                            ModuleOperationStoreError::Database(
                                "matching expired lifecycle claim has no operation idempotency key"
                                    .to_string(),
                            )
                        })?;
                        if StaticTenantLifecycleStore::release_expired_claim(
                            &transaction,
                            candidate.tenant_id,
                            &candidate.module_slug,
                            operation_key,
                        )
                        .await
                        .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?
                        {
                            report.released_claims += 1;
                        }
                    }
                }
                ModuleOperationStatus::CommittedHookPending => {
                    if matching_claim_live {
                        continue;
                    }
                    match Self::mark_committed_hook_failed(
                        &transaction,
                        candidate.id,
                        "module operation owner ended before the post-commit hook outcome was journaled; recovery must retry the hook",
                    )
                    .await
                    {
                        Ok(()) => report.committed_hook_unknown += 1,
                        Err(
                            ModuleOperationStoreError::InvalidStatusTransition { .. }
                            | ModuleOperationStoreError::OperationNotFound(_),
                        ) => continue,
                        Err(error) => return Err(error),
                    }
                    if matching_claim_expired {
                        let operation_key = candidate.idempotency_key.ok_or_else(|| {
                            ModuleOperationStoreError::Database(
                                "matching expired lifecycle claim has no operation idempotency key"
                                    .to_string(),
                            )
                        })?;
                        if StaticTenantLifecycleStore::release_expired_claim(
                            &transaction,
                            candidate.tenant_id,
                            &candidate.module_slug,
                            operation_key,
                        )
                        .await
                        .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?
                        {
                            report.released_claims += 1;
                        }
                    }
                }
                ModuleOperationStatus::Committed
                | ModuleOperationStatus::CommittedHookFailed
                | ModuleOperationStatus::Failed => {
                    if matching_claim_expired {
                        let operation_key = candidate.idempotency_key.ok_or_else(|| {
                            ModuleOperationStoreError::Database(
                                "matching expired lifecycle claim has no operation idempotency key"
                                    .to_string(),
                            )
                        })?;
                        if StaticTenantLifecycleStore::release_expired_claim(
                            &transaction,
                            candidate.tenant_id,
                            &candidate.module_slug,
                            operation_key,
                        )
                        .await
                        .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?
                        {
                            report.released_claims += 1;
                        }
                    }
                }
                ModuleOperationStatus::Orphaned => {}
            }
        }

        transaction.commit().await.map_err(database_error)?;
        Ok(report)
    }

    /// Guarded status transition: the update only applies from one of
    /// `allowed_from`, and a missing or already-advanced operation is an
    /// error rather than a silently ignored no-op.
    async fn mark_status<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
        status: ModuleOperationStatus,
        allowed_from: &[ModuleOperationStatus],
        error_message: Option<&str>,
    ) -> Result<(), ModuleOperationStoreError> {
        let allowed_from_sql = allowed_from
            .iter()
            .map(|status| format!("'{}'", status.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        let (sql_template, values) = match error_message {
            Some(error_message) => (
                format!(
                    "UPDATE module_operations SET status = {{1}}, error_message = {{2}}, \
                     updated_at = {{now}} WHERE id = {{3}} AND status IN ({allowed_from_sql})"
                ),
                vec![
                    status.as_str().into(),
                    error_message.into(),
                    operation_id.into(),
                ],
            ),
            None => (
                format!(
                    "UPDATE module_operations SET status = {{1}}, updated_at = {{now}} \
                     WHERE id = {{2}} AND status IN ({allowed_from_sql})"
                ),
                vec![status.as_str().into(), operation_id.into()],
            ),
        };
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                render_parameters(&sql_template, backend),
                values,
            ))
            .await
            .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?;
        if result.rows_affected() != 1 {
            let current = Self::find(db, operation_id)
                .await?
                .ok_or(ModuleOperationStoreError::OperationNotFound(operation_id))?;
            return Err(ModuleOperationStoreError::InvalidStatusTransition {
                operation_id,
                current: current.status,
                requested: status,
            });
        }
        Ok(())
    }
}

impl StaticTenantLifecycleStore {
    /// Loads the owner aggregate without creating a row for inherited/default
    /// tenant state. Callers submit revision zero for that initial state.
    pub async fn snapshot<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<StaticTenantLifecycleSnapshot, StaticTenantLifecycleStoreError> {
        Self::read_persisted(db, tenant_id, module_slug)
            .await?
            .map(Ok)
            .unwrap_or_else(|| {
                Ok(StaticTenantLifecycleSnapshot {
                    tenant_id,
                    module_slug: module_slug.to_string(),
                    revision: 0,
                    active_idempotency_key: None,
                })
            })
    }

    async fn read_persisted<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticTenantLifecycleSnapshot>, StaticTenantLifecycleStoreError> {
        let slugs = BTreeSet::from([module_slug.to_string()]);
        Ok(Self::read_persisted_many(db, tenant_id, &slugs)
            .await?
            .remove(module_slug))
    }

    async fn read_persisted_many<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slugs: &BTreeSet<String>,
    ) -> Result<BTreeMap<String, StaticTenantLifecycleSnapshot>, StaticTenantLifecycleStoreError>
    {
        if module_slugs.is_empty() {
            return Ok(BTreeMap::new());
        }
        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(static_lifecycle_database_error)?;
        let mut values = vec![backend.uuid_value(tenant_id)];
        if matches!(backend, SupportedBackend::Sqlite) {
            values.push(tenant_id.into());
        }
        let placeholders = module_slugs
            .iter()
            .map(|slug| {
                values.push(slug.clone().into());
                backend.placeholder(values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        let sql = match backend {
            SupportedBackend::Postgres => format!(
                "SELECT module_slug, revision, active_idempotency_key FROM module_static_tenant_lifecycle \
                 WHERE tenant_id = $1 AND module_slug IN ({placeholders})"
            ),
            SupportedBackend::Sqlite => format!(
                "WITH identity AS (SELECT COUNT(*) AS parent_count FROM tenant_identity_keys WHERE id IN (?1, ?2)) \
                 SELECT t.module_slug AS matched_module, t.module_slug, t.revision, t.active_idempotency_key, \
                 identity.parent_count, \
                 (t.tenant_id = (SELECT id FROM tenant_identity_keys WHERE id IN (?1, ?2) LIMIT 1)) AS matches_parent, \
                 (EXISTS(SELECT 1 FROM module_static_tenant_lifecycle WHERE tenant_id = ?1) AND \
                  EXISTS(SELECT 1 FROM module_static_tenant_lifecycle WHERE tenant_id = ?2)) AS mixed_identity, \
                 ((EXISTS(SELECT 1 FROM module_static_tenant_lifecycle WHERE tenant_id = ?1) AND \
                   NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = ?1)) OR \
                  (EXISTS(SELECT 1 FROM module_static_tenant_lifecycle WHERE tenant_id = ?2) AND \
                   NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = ?2))) AS orphaned_identity \
                 FROM identity LEFT JOIN module_static_tenant_lifecycle t \
                 ON t.tenant_id IN (?1, ?2) AND t.module_slug IN ({placeholders})"
            ),
        };
        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                sql,
                values,
            ))
            .await
            .map_err(static_lifecycle_database_error)?;
        let mut snapshots = BTreeMap::new();
        for row in rows {
            if matches!(backend, SupportedBackend::Sqlite) {
                let orphaned: bool = row
                    .try_get("", "orphaned_identity")
                    .map_err(static_lifecycle_database_error)?;
                let matched =
                    crate::backend::sqlite_tenant_module_identity(&row).map_err(|_| {
                        StaticTenantLifecycleStoreError::InconsistentState(
                            module_slugs.iter().next().expect("nonempty slugs").clone(),
                        )
                    })?;
                if orphaned {
                    return Err(StaticTenantLifecycleStoreError::InconsistentState(
                        module_slugs.iter().next().expect("nonempty slugs").clone(),
                    ));
                }
                if !matched {
                    continue;
                }
            }
            let slug: String = row
                .try_get("", "module_slug")
                .map_err(static_lifecycle_database_error)?;
            let snapshot = static_lifecycle_snapshot_from_row(backend, row, tenant_id, &slug)?;
            if snapshots.insert(slug.clone(), snapshot).is_some() {
                return Err(StaticTenantLifecycleStoreError::InconsistentState(slug));
            }
        }
        Ok(snapshots)
    }

    /// Reads one claim for a reap decision, locking the claim row on
    /// Postgres until the caller's transaction commits. Callers must lock
    /// the operation row first to preserve the executor's lock order.
    async fn claim_state_for_update<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticTenantLifecycleClaimState>, StaticTenantLifecycleStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(static_lifecycle_database_error)?;
        // The reaper calls this on its transaction. Validate both SQLite UUID
        // forms in that same snapshot before using either one for the claim.
        if matches!(backend, SupportedBackend::Sqlite)
            && Self::read_persisted(db, tenant_id, module_slug)
                .await?
                .is_none()
        {
            return Ok(None);
        }
        let lock_clause = match backend {
            SupportedBackend::Postgres => " FOR UPDATE",
            SupportedBackend::Sqlite => "",
        };
        let sql = match backend {
            SupportedBackend::Postgres => format!(
                "SELECT active_idempotency_key, \
                 CASE WHEN active_idempotency_key IS NOT NULL AND \
                 (active_claimed_at IS NULL OR active_claimed_at <= CURRENT_TIMESTAMP - INTERVAL '{STATIC_LIFECYCLE_CLAIM_LEASE_SECONDS} seconds') \
                 THEN 'expired' ELSE 'active' END AS claim_expiry \
                 FROM module_static_tenant_lifecycle \
                 WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1{lock_clause}"
            ),
            SupportedBackend::Sqlite => format!(
                "SELECT active_idempotency_key, \
                 CASE WHEN active_idempotency_key IS NOT NULL AND \
                 (active_claimed_at IS NULL OR datetime(active_claimed_at) <= datetime('now', '-{STATIC_LIFECYCLE_CLAIM_LEASE_SECONDS} seconds')) \
                 THEN 'expired' ELSE 'active' END AS claim_expiry \
                 FROM module_static_tenant_lifecycle \
                 WHERE tenant_id IN (?1, ?3) AND module_slug = ?2 LIMIT 1{lock_clause}"
            ),
        };
        let mut values = vec![backend.uuid_value(tenant_id), module_slug.into()];
        if matches!(backend, SupportedBackend::Sqlite) {
            values.push(tenant_id.into());
        }
        db.query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            sql,
            values,
        ))
        .await
        .map_err(static_lifecycle_database_error)?
        .map(|row| {
            let claim_expiry: String = row
                .try_get("", "claim_expiry")
                .map_err(static_lifecycle_database_error)?;
            Ok(StaticTenantLifecycleClaimState {
                active_idempotency_key: backend
                    .optional_uuid_from_row(&row, "active_idempotency_key")
                    .map_err(static_lifecycle_database_error)?,
                expired: match claim_expiry.as_str() {
                    "expired" => true,
                    "active" => false,
                    _ => {
                        return Err(StaticTenantLifecycleStoreError::InconsistentState(
                            module_slug.to_string(),
                        ));
                    }
                },
            })
        })
        .transpose()
    }

    async fn release_expired_claim<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
        idempotency_key: Uuid,
    ) -> Result<bool, StaticTenantLifecycleStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(static_lifecycle_database_error)?;
        let sql = match backend {
            SupportedBackend::Postgres => format!(
                "UPDATE module_static_tenant_lifecycle \
                 SET active_idempotency_key = NULL, active_claimed_at = NULL, updated_at = NOW() \
                 WHERE tenant_id = $1 AND module_slug = $2 AND active_idempotency_key = $3 \
                   AND (active_claimed_at IS NULL OR active_claimed_at <= CURRENT_TIMESTAMP - INTERVAL '{STATIC_LIFECYCLE_CLAIM_LEASE_SECONDS} seconds')"
            ),
            SupportedBackend::Sqlite => format!(
                "UPDATE module_static_tenant_lifecycle \
                 SET active_idempotency_key = NULL, active_claimed_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now') \
                 WHERE tenant_id IN (?1, ?4) AND module_slug = ?2 AND active_idempotency_key IN (?3, ?5) \
                   AND (active_claimed_at IS NULL OR datetime(active_claimed_at) <= datetime('now', '-{STATIC_LIFECYCLE_CLAIM_LEASE_SECONDS} seconds'))"
            ),
        };
        let mut values = vec![
            backend.uuid_value(tenant_id),
            module_slug.into(),
            backend.uuid_value(idempotency_key),
        ];
        if matches!(backend, SupportedBackend::Sqlite) {
            values.push(tenant_id.into());
            values.push(idempotency_key.into());
        }
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                sql,
                values,
            ))
            .await
            .map_err(static_lifecycle_database_error)?;
        Ok(result.rows_affected() == 1)
    }

    /// Returns a complete aggregate snapshot for every requested static slug
    /// in one owner query. Missing rows are inherited/default state at
    /// revision zero and are represented without creating an override row.
    pub async fn snapshots<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slugs: impl IntoIterator<Item = String>,
    ) -> Result<BTreeMap<String, StaticTenantLifecycleSnapshot>, StaticTenantLifecycleStoreError>
    {
        let module_slugs = module_slugs.into_iter().collect::<BTreeSet<_>>();
        if module_slugs.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut snapshots = module_slugs
            .iter()
            .map(|module_slug| {
                (
                    module_slug.clone(),
                    StaticTenantLifecycleSnapshot {
                        tenant_id,
                        module_slug: module_slug.clone(),
                        revision: 0,
                        active_idempotency_key: None,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        snapshots.extend(Self::read_persisted_many(db, tenant_id, &module_slugs).await?);
        Ok(snapshots)
    }

    /// Admits one command only when its reviewed aggregate revision is current.
    /// The claim is durable and is cleared by its owner at a terminal
    /// lifecycle boundary. As a crash-safety net the claim also carries a
    /// lease (`STATIC_LIFECYCLE_CLAIM_LEASE_SECONDS`): once it expires, a
    /// later command at the same expected revision may CAS-reclaim the
    /// aggregate, fencing out the previous holder (its advance/release match
    /// on its own idempotency key and will fail).
    pub async fn claim<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
        expected_revision: u64,
        idempotency_key: Uuid,
    ) -> Result<StaticTenantLifecycleClaim, StaticTenantLifecycleStoreError> {
        let persisted = Self::read_persisted(db, tenant_id, module_slug).await?;
        let current = persisted.clone().unwrap_or(StaticTenantLifecycleSnapshot {
            tenant_id,
            module_slug: module_slug.to_string(),
            revision: 0,
            active_idempotency_key: None,
        });
        if let Some(active_idempotency_key) = current.active_idempotency_key {
            if active_idempotency_key == idempotency_key {
                if current.revision != expected_revision {
                    return Err(StaticTenantLifecycleStoreError::RevisionConflict {
                        module_slug: module_slug.to_string(),
                        expected: expected_revision,
                        current: current.revision,
                    });
                }
                return Ok(StaticTenantLifecycleClaim::AlreadyHeld(current));
            }
            // The holder may have crashed without releasing. Reclaim is only
            // attempted at the exact expected revision and only when the
            // recorded claim lease has expired in database time.
            if current.revision == expected_revision
                && Self::reclaim_expired_claim(
                    db,
                    tenant_id,
                    module_slug,
                    expected_revision,
                    active_idempotency_key,
                    idempotency_key,
                )
                .await?
            {
                return Ok(StaticTenantLifecycleClaim::Acquired(
                    StaticTenantLifecycleSnapshot {
                        tenant_id,
                        module_slug: module_slug.to_string(),
                        revision: expected_revision,
                        active_idempotency_key: Some(idempotency_key),
                    },
                ));
            }
            return Err(StaticTenantLifecycleStoreError::OperationInProgress {
                module_slug: module_slug.to_string(),
            });
        }
        if current.revision != expected_revision {
            return Err(StaticTenantLifecycleStoreError::RevisionConflict {
                module_slug: module_slug.to_string(),
                expected: expected_revision,
                current: current.revision,
            });
        }

        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(static_lifecycle_database_error)?;
        if persisted.is_none() {
            let mut values = vec![
                backend.uuid_value(tenant_id),
                module_slug.into(),
                i64::try_from(expected_revision)
                    .map_err(|_| {
                        StaticTenantLifecycleStoreError::RevisionOverflow(module_slug.to_string())
                    })?
                    .into(),
                backend.uuid_value(idempotency_key),
            ];
            let tenant_key = static_tenant_key_sql(
                backend,
                1,
                tenant_id,
                &mut values,
                StaticTenantRows::Lifecycle,
            );
            let sql = match backend {
                SupportedBackend::Postgres => "INSERT INTO module_static_tenant_lifecycle \
                    (tenant_id, module_slug, revision, active_idempotency_key, active_claimed_at) \
                    VALUES ($1, $2, $3, $4, NOW())"
                    .to_string(),
                SupportedBackend::Sqlite => format!(
                    "INSERT INTO module_static_tenant_lifecycle \
                    (tenant_id, module_slug, revision, active_idempotency_key, active_claimed_at) \
                    SELECT {tenant_key}, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now') \
                    WHERE {tenant_key} IS NOT NULL"
                ),
            };
            let insert = db
                .execute_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    sql,
                    values,
                ))
                .await;
            match insert {
                Ok(result) if result.rows_affected() == 1 => {
                    return Ok(StaticTenantLifecycleClaim::Acquired(
                        StaticTenantLifecycleSnapshot {
                            tenant_id,
                            module_slug: module_slug.to_string(),
                            revision: expected_revision,
                            active_idempotency_key: Some(idempotency_key),
                        },
                    ));
                }
                Ok(_) => {
                    return Err(StaticTenantLifecycleStoreError::InconsistentState(
                        module_slug.to_string(),
                    ));
                }
                Err(error) if backend.is_unique_violation(&error) => {
                    // A concurrent first claim won the composite primary key.
                    // Reloading applies the same exact-key/revision decision as
                    // an already materialized aggregate.
                    let refreshed = Self::snapshot(db, tenant_id, module_slug).await?;
                    return if refreshed.active_idempotency_key == Some(idempotency_key)
                        && refreshed.revision != expected_revision
                    {
                        Err(StaticTenantLifecycleStoreError::RevisionConflict {
                            module_slug: module_slug.to_string(),
                            expected: expected_revision,
                            current: refreshed.revision,
                        })
                    } else if refreshed.active_idempotency_key == Some(idempotency_key) {
                        Ok(StaticTenantLifecycleClaim::AlreadyHeld(refreshed))
                    } else if refreshed.active_idempotency_key.is_some() {
                        Err(StaticTenantLifecycleStoreError::OperationInProgress {
                            module_slug: module_slug.to_string(),
                        })
                    } else if refreshed.revision != expected_revision {
                        Err(StaticTenantLifecycleStoreError::RevisionConflict {
                            module_slug: module_slug.to_string(),
                            expected: expected_revision,
                            current: refreshed.revision,
                        })
                    } else {
                        Err(StaticTenantLifecycleStoreError::InconsistentState(
                            module_slug.to_string(),
                        ))
                    };
                }
                Err(error) => return Err(static_lifecycle_database_error(error)),
            }
        }

        let mut values = vec![
            backend.uuid_value(idempotency_key),
            backend.uuid_value(tenant_id),
            module_slug.into(),
            i64::try_from(expected_revision)
                .map_err(|_| {
                    StaticTenantLifecycleStoreError::RevisionOverflow(module_slug.to_string())
                })?
                .into(),
        ];
        let tenant_key = static_tenant_key_sql(
            backend,
            2,
            tenant_id,
            &mut values,
            StaticTenantRows::Lifecycle,
        );
        let sql = "UPDATE module_static_tenant_lifecycle \
            SET active_idempotency_key = {1}, active_claimed_at = {now}, updated_at = {now} \
            WHERE tenant_id = {tenant} AND module_slug = {3} \
            AND revision = {4} AND active_idempotency_key IS NULL"
            .replace("{tenant}", &tenant_key);
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                render_parameters(&sql, backend),
                values,
            ))
            .await
            .map_err(static_lifecycle_database_error)?;
        if result.rows_affected() == 1 {
            return Ok(StaticTenantLifecycleClaim::Acquired(
                StaticTenantLifecycleSnapshot {
                    tenant_id,
                    module_slug: module_slug.to_string(),
                    revision: expected_revision,
                    active_idempotency_key: Some(idempotency_key),
                },
            ));
        }

        let refreshed = Self::snapshot(db, tenant_id, module_slug).await?;
        if refreshed.active_idempotency_key == Some(idempotency_key)
            && refreshed.revision != expected_revision
        {
            Err(StaticTenantLifecycleStoreError::RevisionConflict {
                module_slug: module_slug.to_string(),
                expected: expected_revision,
                current: refreshed.revision,
            })
        } else if refreshed.active_idempotency_key == Some(idempotency_key) {
            Ok(StaticTenantLifecycleClaim::AlreadyHeld(refreshed))
        } else if refreshed.active_idempotency_key.is_some() {
            Err(StaticTenantLifecycleStoreError::OperationInProgress {
                module_slug: module_slug.to_string(),
            })
        } else {
            Err(StaticTenantLifecycleStoreError::RevisionConflict {
                module_slug: module_slug.to_string(),
                expected: expected_revision,
                current: refreshed.revision,
            })
        }
    }

    /// Renews the finite claim lease while long-running pre/post-commit
    /// hooks execute. `false` means the exact key/revision no longer owns
    /// the aggregate (the lease lapsed and another command reclaimed it, or
    /// the revision advanced), so the caller must stop treating its claim
    /// as valid before any further durable write.
    pub async fn heartbeat<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
        expected_revision: u64,
        idempotency_key: Uuid,
    ) -> Result<bool, StaticTenantLifecycleStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(static_lifecycle_database_error)?;
        let mut values = vec![
            backend.uuid_value(tenant_id),
            module_slug.into(),
            i64::try_from(expected_revision)
                .map_err(|_| {
                    StaticTenantLifecycleStoreError::RevisionOverflow(module_slug.to_string())
                })?
                .into(),
            backend.uuid_value(idempotency_key),
        ];
        let tenant_key = static_tenant_key_sql(
            backend,
            1,
            tenant_id,
            &mut values,
            StaticTenantRows::Lifecycle,
        );
        let claim_key = lifecycle_claim_key_sql(backend, 4, idempotency_key, &mut values);
        let sql = "UPDATE module_static_tenant_lifecycle \
            SET active_claimed_at = {now}, updated_at = {now} \
            WHERE tenant_id = {tenant} AND module_slug = {2} AND revision = {3} AND {claim}"
            .replace("{tenant}", &tenant_key)
            .replace("{claim}", &claim_key);
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                render_parameters(&sql, backend),
                values,
            ))
            .await
            .map_err(static_lifecycle_database_error)?;
        Ok(result.rows_affected() == 1)
    }

    /// CAS-reclaims an aggregate whose recorded claim lease has expired.
    /// Returns `true` when this command now holds the claim. A claim recorded
    /// before the lease column existed (`NULL`) is treated as expired: fresh
    /// claims always record their timestamp.
    async fn reclaim_expired_claim<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
        expected_revision: u64,
        expired_idempotency_key: Uuid,
        idempotency_key: Uuid,
    ) -> Result<bool, StaticTenantLifecycleStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(static_lifecycle_database_error)?;
        let claim_expired = match backend {
            SupportedBackend::Postgres => format!(
                "(active_claimed_at IS NULL OR active_claimed_at <= CURRENT_TIMESTAMP - INTERVAL '{STATIC_LIFECYCLE_CLAIM_LEASE_SECONDS} seconds')"
            ),
            SupportedBackend::Sqlite => format!(
                "(active_claimed_at IS NULL OR datetime(active_claimed_at) <= datetime('now', '-{STATIC_LIFECYCLE_CLAIM_LEASE_SECONDS} seconds'))"
            ),
        };
        let mut values = vec![
            backend.uuid_value(idempotency_key),
            backend.uuid_value(tenant_id),
            module_slug.into(),
            i64::try_from(expected_revision)
                .map_err(|_| {
                    StaticTenantLifecycleStoreError::RevisionOverflow(module_slug.to_string())
                })?
                .into(),
            backend.uuid_value(expired_idempotency_key),
        ];
        let tenant_key = static_tenant_key_sql(
            backend,
            2,
            tenant_id,
            &mut values,
            StaticTenantRows::Lifecycle,
        );
        let claim_key = lifecycle_claim_key_sql(backend, 5, expired_idempotency_key, &mut values);
        let sql = format!(
            "UPDATE module_static_tenant_lifecycle \
            SET active_idempotency_key = {{1}}, active_claimed_at = {{now}}, updated_at = {{now}} \
            WHERE tenant_id = {tenant_key} AND module_slug = {{3}} AND revision = {{4}} \
            AND {claim_key} AND {claim_expired}"
        );
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                render_parameters(&sql, backend),
                values,
            ))
            .await
            .map_err(static_lifecycle_database_error)?;
        Ok(result.rows_affected() == 1)
    }

    /// Advances a claimed aggregate while deliberately retaining the claim for
    /// the subsequent post-hook. The caller must clear it after the hook's
    /// terminal journal state is durable.
    pub async fn advance<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
        expected_revision: u64,
        idempotency_key: Uuid,
    ) -> Result<u64, StaticTenantLifecycleStoreError> {
        let next_revision = expected_revision.checked_add(1).ok_or_else(|| {
            StaticTenantLifecycleStoreError::RevisionOverflow(module_slug.to_string())
        })?;
        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(static_lifecycle_database_error)?;
        let mut values = vec![
            i64::try_from(next_revision)
                .map_err(|_| {
                    StaticTenantLifecycleStoreError::RevisionOverflow(module_slug.to_string())
                })?
                .into(),
            backend.uuid_value(tenant_id),
            module_slug.into(),
            i64::try_from(expected_revision)
                .map_err(|_| {
                    StaticTenantLifecycleStoreError::RevisionOverflow(module_slug.to_string())
                })?
                .into(),
            backend.uuid_value(idempotency_key),
        ];
        let tenant_key = static_tenant_key_sql(
            backend,
            2,
            tenant_id,
            &mut values,
            StaticTenantRows::Lifecycle,
        );
        let claim_key = lifecycle_claim_key_sql(backend, 5, idempotency_key, &mut values);
        let sql = "UPDATE module_static_tenant_lifecycle SET revision = {1}, updated_at = {now} \
            WHERE tenant_id = {tenant} AND module_slug = {3} AND revision = {4} AND {claim}"
            .replace("{tenant}", &tenant_key)
            .replace("{claim}", &claim_key);
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                render_parameters(&sql, backend),
                values,
            ))
            .await
            .map_err(static_lifecycle_database_error)?;
        if result.rows_affected() == 1 {
            return Ok(next_revision);
        }
        Err(StaticTenantLifecycleStoreError::InconsistentState(
            module_slug.to_string(),
        ))
    }

    /// Clears an aggregate claim after its hook/journal operation reaches a
    /// terminal state. A different command can never clear this claim.
    pub async fn release<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
        idempotency_key: Uuid,
    ) -> Result<(), StaticTenantLifecycleStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(static_lifecycle_database_error)?;
        let mut values = vec![
            backend.uuid_value(tenant_id),
            module_slug.into(),
            backend.uuid_value(idempotency_key),
        ];
        let tenant_key = static_tenant_key_sql(
            backend,
            1,
            tenant_id,
            &mut values,
            StaticTenantRows::Lifecycle,
        );
        let claim_key = lifecycle_claim_key_sql(backend, 3, idempotency_key, &mut values);
        let sql = "UPDATE module_static_tenant_lifecycle \
            SET active_idempotency_key = NULL, active_claimed_at = NULL, updated_at = {now} \
            WHERE tenant_id = {tenant} AND module_slug = {2} AND {claim}"
            .replace("{tenant}", &tenant_key)
            .replace("{claim}", &claim_key);
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                render_parameters(&sql, backend),
                values,
            ))
            .await
            .map_err(static_lifecycle_database_error)?;
        if result.rows_affected() != 1 {
            return Err(StaticTenantLifecycleStoreError::InconsistentState(
                module_slug.to_string(),
            ));
        }
        Ok(())
    }
}

fn static_lifecycle_snapshot_from_row(
    backend: SupportedBackend,
    row: sea_orm::QueryResult,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<StaticTenantLifecycleSnapshot, StaticTenantLifecycleStoreError> {
    let revision: i64 = row
        .try_get("", "revision")
        .map_err(static_lifecycle_database_error)?;
    if revision < 0 {
        return Err(StaticTenantLifecycleStoreError::InconsistentState(
            module_slug.to_string(),
        ));
    }
    Ok(StaticTenantLifecycleSnapshot {
        tenant_id,
        module_slug: module_slug.to_string(),
        revision: u64::try_from(revision).map_err(|_| {
            StaticTenantLifecycleStoreError::InconsistentState(module_slug.to_string())
        })?,
        active_idempotency_key: backend
            .optional_uuid_from_row(&row, "active_idempotency_key")
            .map_err(static_lifecycle_database_error)?,
    })
}

fn static_lifecycle_database_error(
    error: impl std::fmt::Display,
) -> StaticTenantLifecycleStoreError {
    StaticTenantLifecycleStoreError::Database(error.to_string())
}

fn operation_select_sql() -> &'static str {
    "SELECT id, tenant_id, module_slug, requested_enabled, previous_effective_enabled, status, \
     requested_by, trace_id, correlation_id, idempotency_key, expected_revision, recovery_source_id, error_message, created_at, updated_at FROM module_operations"
}

/// Compares only the semantic lifecycle request under the already-matched
/// `(tenant_id, idempotency_key)`. Requester, trace, correlation, and the
/// previously observed effective state are audit/recovery evidence, not
/// replay identity, so a legitimate retry may carry fresh observability
/// context without turning into a conflict.
fn replay_or_conflict(
    request: &ModuleOperationRequest,
    existing: ModuleOperationSnapshot,
) -> Result<ModuleOperationRecordOutcome, ModuleOperationStoreError> {
    if existing.module_slug != request.module_slug
        || existing.requested_enabled != request.requested_enabled
        || existing.expected_revision != request.expected_revision
        || existing.recovery_source_id != request.recovery_source_id
    {
        return Err(ModuleOperationStoreError::IdempotencyConflict);
    }
    Ok(ModuleOperationRecordOutcome::Replayed(
        ModuleOperationRecord { id: existing.id },
    ))
}

fn operation_snapshot(
    backend: SupportedBackend,
    row: sea_orm::QueryResult,
) -> Result<ModuleOperationSnapshot, ModuleOperationStoreError> {
    let status: String = row.try_get("", "status").map_err(database_error)?;
    Ok(ModuleOperationSnapshot {
        id: backend.uuid_from_row(&row, "id").map_err(database_error)?,
        tenant_id: backend
            .uuid_from_row(&row, "tenant_id")
            .map_err(database_error)?,
        module_slug: row.try_get("", "module_slug").map_err(database_error)?,
        requested_enabled: row
            .try_get("", "requested_enabled")
            .map_err(database_error)?,
        previous_effective_enabled: row
            .try_get("", "previous_effective_enabled")
            .map_err(database_error)?,
        status: ModuleOperationStatus::parse(&status).ok_or_else(|| {
            ModuleOperationStoreError::Database(format!(
                "unknown module operation status `{status}`"
            ))
        })?,
        requested_by: row.try_get("", "requested_by").map_err(database_error)?,
        trace_id: row.try_get("", "trace_id").map_err(database_error)?,
        correlation_id: row.try_get("", "correlation_id").map_err(database_error)?,
        idempotency_key: backend
            .optional_uuid_from_row(&row, "idempotency_key")
            .map_err(database_error)?,
        recovery_source_id: row
            .try_get::<Option<String>>("", "recovery_source_id")
            .map_err(database_error)?
            .map(|value| Uuid::parse_str(&value).map_err(database_error))
            .transpose()?,
        expected_revision: row
            .try_get::<Option<i64>>("", "expected_revision")
            .map_err(database_error)?
            .map(|value| {
                u64::try_from(value).map_err(|_| {
                    ModuleOperationStoreError::Database(
                        "module operation has an invalid expected revision".to_string(),
                    )
                })
            })
            .transpose()?,
        error_message: row.try_get("", "error_message").map_err(database_error)?,
        created_at: row.try_get("", "created_at").map_err(database_error)?,
        updated_at: row.try_get("", "updated_at").map_err(database_error)?,
    })
}

fn database_error(error: impl std::fmt::Display) -> ModuleOperationStoreError {
    ModuleOperationStoreError::Database(error.to_string())
}

fn validate_sqlite_journal_identity(
    row: &sea_orm::QueryResult,
) -> Result<bool, ModuleOperationStoreError> {
    let parent_count: i64 = row.try_get("", "parent_count").map_err(database_error)?;
    let orphaned: bool = row
        .try_get("", "orphaned_identity")
        .map_err(database_error)?;
    let matched = crate::backend::sqlite_tenant_module_identity(row)
        .map_err(|_| ModuleOperationStoreError::UnverifiableSqliteIdentity)?;
    if parent_count != 1 || orphaned {
        return Err(ModuleOperationStoreError::UnverifiableSqliteIdentity);
    }
    Ok(matched)
}

async fn validate_sqlite_journal_tenant<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
) -> Result<(), ModuleOperationStoreError> {
    let row = db.query_one_raw(Statement::from_sql_and_values(
        sea_orm::DbBackend::Sqlite,
        "SELECT COUNT(*) AS parent_count, NULL AS matched_module, NULL AS matches_parent, \
         (EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?1) AND EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?2)) AS mixed_identity, \
         ((EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?1) AND NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = ?1)) OR \
          (EXISTS(SELECT 1 FROM module_operations WHERE tenant_id = ?2) AND NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = ?2))) AS orphaned_identity \
         FROM tenant_identity_keys WHERE id IN (?1, ?2)",
        vec![tenant_id.into(), tenant_id.to_string().into()],
    )).await.map_err(database_error)?.ok_or_else(|| database_error("tenant identity query returned no aggregate"))?;
    validate_sqlite_journal_identity(&row)?;
    Ok(())
}

impl TenantModuleStateStore {
    pub async fn read<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<TenantModuleStateRecord>, ModuleOperationStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let sql = match backend {
            SupportedBackend::Postgres => {
                "SELECT id, enabled FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
            }
            SupportedBackend::Sqlite => {
                "WITH identity AS (SELECT COUNT(*) AS parent_count FROM tenant_identity_keys WHERE id IN (?1, ?3)) \
                 SELECT t.module_slug AS matched_module, t.id, t.enabled, identity.parent_count, \
                 (t.tenant_id = (SELECT id FROM tenant_identity_keys WHERE id IN (?1, ?3) LIMIT 1)) AS matches_parent, \
                 (EXISTS(SELECT 1 FROM tenant_modules WHERE tenant_id = ?1) AND \
                  EXISTS(SELECT 1 FROM tenant_modules WHERE tenant_id = ?3)) AS mixed_identity, \
                 ((EXISTS(SELECT 1 FROM tenant_modules WHERE tenant_id = ?1) AND \
                   NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = ?1)) OR \
                  (EXISTS(SELECT 1 FROM tenant_modules WHERE tenant_id = ?3) AND \
                   NOT EXISTS(SELECT 1 FROM tenant_identity_keys WHERE id = ?3))) AS orphaned_identity \
                 FROM identity LEFT JOIN tenant_modules t ON t.tenant_id IN (?1, ?3) \
                 AND t.module_slug = ?2 LIMIT 2"
            }
        };
        let mut values = vec![backend.uuid_value(tenant_id), module_slug.into()];
        if matches!(backend, SupportedBackend::Sqlite) {
            values.push(tenant_id.into());
        }
        let mut rows = db
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                sql,
                values,
            ))
            .await
            .map_err(database_error)?;
        if rows.len() > 1 {
            return Err(database_error(
                "tenant module state has conflicting UUID representations",
            ));
        }
        let Some(row) = rows.pop() else {
            return Ok(None);
        };
        if matches!(backend, SupportedBackend::Sqlite) {
            let orphaned: bool = row
                .try_get("", "orphaned_identity")
                .map_err(database_error)?;
            let matched =
                crate::backend::sqlite_tenant_module_identity(&row).map_err(database_error)?;
            if orphaned {
                return Err(database_error(
                    "tenant module state has an orphaned tenant identity",
                ));
            }
            if !matched {
                return Ok(None);
            }
        }
        let enabled: Option<bool> = row.try_get("", "enabled").map_err(database_error)?;
        Ok(Some(TenantModuleStateRecord {
            id: backend.uuid_from_row(&row, "id").map_err(database_error)?,
            enabled,
            previous_enabled: enabled,
            changed: false,
        }))
    }

    pub async fn persist<C: ConnectionTrait>(
        db: &C,
        request: TenantModuleStateRequest,
    ) -> Result<TenantModuleStateRecord, ModuleOperationStoreError> {
        if let Some(state) = Self::read(db, request.tenant_id, &request.module_slug).await? {
            if state.enabled != Some(request.enabled) {
                Self::update_existing(
                    db,
                    request.tenant_id,
                    &request.module_slug,
                    &state,
                    OverrideUpdate::Enabled(Some(request.enabled)),
                )
                .await?;
            }
            return Ok(TenantModuleStateRecord {
                id: state.id,
                enabled: Some(request.enabled),
                previous_enabled: state.enabled,
                changed: state.enabled != Some(request.enabled),
            });
        }

        let id = rustok_core::generate_id();
        Self::insert(
            db,
            id,
            request.tenant_id,
            &request.module_slug,
            Some(request.enabled),
            serde_json::json!({}),
        )
        .await?;
        Ok(TenantModuleStateRecord {
            id,
            enabled: Some(request.enabled),
            // The row was just created: there is no predecessor state.
            previous_enabled: None,
            changed: true,
        })
    }

    /// Persists tenant module settings without creating or changing
    /// enablement intent.
    ///
    /// An existing row keeps its recorded intent untouched (the column is
    /// not part of the UPDATE). A newly created row stores NULL intent —
    /// "settings without intent" — so the platform default keeps applying
    /// until the tenant explicitly toggles the module; freezing a default
    /// (or, worse, derived effective availability such as a transient
    /// denial) into the column would fabricate tenant intent that outlives
    /// the condition that produced it (audit M-5).
    pub async fn persist_settings<C: ConnectionTrait>(
        db: &C,
        request: TenantModuleSettingsRequest,
    ) -> Result<TenantModuleSettingsRecord, ModuleOperationStoreError> {
        let module_slug = request.module_slug.clone();
        let settings = request.settings.clone();
        if let Some(state) = Self::read(db, request.tenant_id, &request.module_slug).await? {
            // A settings save never changes recorded enablement intent:
            // the column is not part of this UPDATE at all.
            Self::update_existing(
                db,
                request.tenant_id,
                &request.module_slug,
                &state,
                OverrideUpdate::Settings(request.settings),
            )
            .await?;
            return Ok(TenantModuleSettingsRecord {
                id: state.id,
                module_slug,
                enabled: state.enabled,
                settings,
            });
        }

        let id = rustok_core::generate_id();
        // A first row created by a settings save records no enablement
        // intent (NULL): the platform default keeps applying until the
        // tenant explicitly toggles the module.
        Self::insert(
            db,
            id,
            request.tenant_id,
            &request.module_slug,
            None,
            request.settings,
        )
        .await?;
        Ok(TenantModuleSettingsRecord {
            id,
            module_slug,
            enabled: None,
            settings,
        })
    }

    pub(crate) async fn clear<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<(), ModuleOperationStoreError> {
        if let Some(state) = Self::read(db, tenant_id, module_slug).await? {
            if state.enabled.is_some() {
                Self::update_existing(
                    db,
                    tenant_id,
                    module_slug,
                    &state,
                    OverrideUpdate::Enabled(None),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn insert<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        tenant_id: Uuid,
        slug: &str,
        enabled: Option<bool>,
        settings: serde_json::Value,
    ) -> Result<(), ModuleOperationStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let mut values = vec![
            id.into(),
            backend.uuid_value(tenant_id),
            slug.into(),
            enabled.into(),
            json_value(settings),
        ];
        let tenant_key = static_tenant_key_sql(
            backend,
            2,
            tenant_id,
            &mut values,
            StaticTenantRows::Overrides,
        );
        let sql = match backend {
            SupportedBackend::Postgres => "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings) VALUES ({1}, {2}, {3}, {4}, {5})".to_string(),
            SupportedBackend::Sqlite => "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings) \
                SELECT {1}, {tenant}, {3}, {4}, {5} WHERE {tenant} IS NOT NULL".replace("{tenant}", &tenant_key),
        };
        execute_one(db, &sql, values).await
    }

    async fn update_existing<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        slug: &str,
        state: &TenantModuleStateRecord,
        update: OverrideUpdate,
    ) -> Result<(), ModuleOperationStoreError> {
        let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
        let (column, value) = match update {
            OverrideUpdate::Enabled(value) => ("enabled", value.into()),
            OverrideUpdate::Settings(value) => ("settings", json_value(value)),
        };
        let mut values = vec![
            value,
            backend.uuid_value(tenant_id),
            slug.into(),
            state.enabled.into(),
            state.id.into(),
        ];
        let tenant_key = static_tenant_key_sql(
            backend,
            2,
            tenant_id,
            &mut values,
            StaticTenantRows::Overrides,
        );
        let id_match = static_override_id_sql(backend, 5, state.id, &mut values);
        let enabled_match = match backend {
            SupportedBackend::Postgres => "enabled IS NOT DISTINCT FROM {4}",
            SupportedBackend::Sqlite => "enabled IS {4}",
        };
        let sql = format!(
            "UPDATE tenant_modules SET {column} = {{1}}, updated_at = {{now}} \
            WHERE tenant_id = {tenant_key} AND module_slug = {{3}} AND {enabled_match} AND {id_match}"
        );
        execute_one(db, &sql, values).await
    }
}

enum OverrideUpdate {
    Enabled(Option<bool>),
    Settings(serde_json::Value),
}

fn json_value(value: serde_json::Value) -> sea_orm::Value {
    sea_orm::Value::Json(Some(Box::new(value)))
}

#[derive(Clone, Copy)]
enum StaticTenantRows {
    Lifecycle,
    Overrides,
}

/// Resolve the tenant key inside the mutating statement's SQLite snapshot.
/// The text form is already bound at `position`; the native form is appended.
/// A missing, ambiguous, or orphaned identity produces no writable key.
fn static_tenant_key_sql(
    backend: SupportedBackend,
    position: usize,
    tenant_id: Uuid,
    values: &mut Vec<sea_orm::Value>,
    rows: StaticTenantRows,
) -> String {
    let text = backend.placeholder(position);
    if backend == SupportedBackend::Postgres {
        return text;
    }
    values.push(tenant_id.into());
    let native = backend.placeholder(values.len());
    let table = match rows {
        StaticTenantRows::Lifecycle => "module_static_tenant_lifecycle",
        StaticTenantRows::Overrides => "tenant_modules",
    };
    format!(
        "(SELECT parent.id FROM tenant_identity_keys parent WHERE parent.id IN ({text}, {native}) \
         AND (SELECT COUNT(*) FROM tenant_identity_keys WHERE id IN ({text}, {native})) = 1 \
         AND NOT EXISTS (SELECT 1 FROM {table} existing \
         WHERE existing.tenant_id IN ({text}, {native}) AND existing.tenant_id != parent.id))"
    )
}

/// Compare legacy native and current text encodings of an aggregate claim key.
fn lifecycle_claim_key_sql(
    backend: SupportedBackend,
    position: usize,
    key: Uuid,
    values: &mut Vec<sea_orm::Value>,
) -> String {
    let text = backend.placeholder(position);
    if backend == SupportedBackend::Postgres {
        return format!("active_idempotency_key = {text}");
    }
    values.push(key.into());
    format!(
        "active_idempotency_key IN ({text}, {})",
        backend.placeholder(values.len())
    )
}

/// Restrict the CAS update to the same physical override row even if an old
/// SQLite row stored its UUID as canonical text instead of a native blob.
fn static_override_id_sql(
    backend: SupportedBackend,
    position: usize,
    id: Uuid,
    values: &mut Vec<sea_orm::Value>,
) -> String {
    let native = backend.placeholder(position);
    if backend == SupportedBackend::Postgres {
        return format!("id = {native}");
    }
    values.push(id.to_string().into());
    format!("id IN ({native}, {})", backend.placeholder(values.len()))
}

/// Executes a statement that must affect exactly one row.
///
/// The tenant-module writes select the target row first and then update it
/// by id; silently accepting zero affected rows would report a persisted
/// record for a row that no longer exists (concurrently purged), so the
/// affected-row count is part of the success contract.
async fn execute_one<C: ConnectionTrait>(
    db: &C,
    sql_template: &str,
    values: Vec<sea_orm::Value>,
) -> Result<(), ModuleOperationStoreError> {
    let backend = SupportedBackend::new(db.get_database_backend()).map_err(database_error)?;
    let affected = db
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            render_parameters(sql_template, backend),
            values,
        ))
        .await
        .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?
        .rows_affected();
    if affected != 1 {
        return Err(ModuleOperationStoreError::Database(format!(
            "expected statement to affect exactly one row, affected {affected}"
        )));
    }
    Ok(())
}

fn render_parameters(sql_template: &str, backend: SupportedBackend) -> String {
    let mut rendered = String::with_capacity(sql_template.len());
    let mut remaining = sql_template;
    while let Some(start) = remaining.find('{') {
        rendered.push_str(&remaining[..start]);
        remaining = &remaining[start..];
        let Some(end) = remaining.find('}') else {
            rendered.push_str(remaining);
            return rendered;
        };
        let key = &remaining[1..end];
        if key == "now" {
            rendered.push_str(backend.now_sql());
        } else if !key.is_empty()
            && key.bytes().all(|byte| byte.is_ascii_digit())
            && let Ok(index) = key.parse::<usize>()
            && index > 0
        {
            rendered.push_str(&backend.placeholder(index));
        } else {
            rendered.push_str(&remaining[..=end]);
        }
        remaining = &remaining[end + 1..];
    }
    rendered.push_str(remaining);
    rendered
}

#[cfg(test)]
mod tests {
    use rustok_api::StaticModuleSettingsReader;
    use sea_orm::{Database, DbBackend, Statement};
    use serde_json::json;

    use super::*;

    #[test]
    fn sql_parameter_rendering_supports_all_positions_without_touching_literals() {
        let template = "VALUES ({1}, {10}, {12}, {123}, '{}', {now})";
        for (backend, prefix) in [
            (SupportedBackend::Sqlite, "?"),
            (SupportedBackend::Postgres, "$"),
        ] {
            let expected = format!(
                "VALUES ({prefix}1, {prefix}10, {prefix}12, {prefix}123, '{{}}', {})",
                backend.now_sql()
            );
            assert_eq!(render_parameters(template, backend), expected);
        }
    }

    async fn database() -> sea_orm::DatabaseConnection {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute_unprepared("CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants")
            .await
            .expect("tenant table");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE tenant_modules (\
                    id TEXT PRIMARY KEY NOT NULL, \
                    tenant_id TEXT NOT NULL, \
                    module_slug TEXT NOT NULL, \
                    enabled BOOLEAN NULL, \
                    settings JSON NOT NULL, \
                    updated_at TEXT\
                 )"
                .to_string(),
            ))
            .await
            .expect("tenant modules table");
        database
            .execute_unprepared(
                "CREATE TRIGGER fixture_tenant AFTER INSERT ON tenant_modules BEGIN \
                 INSERT OR IGNORE INTO tenants (id) VALUES (NEW.tenant_id); END",
            )
            .await
            .expect("tenant fixture trigger");
        database
    }

    #[tokio::test]
    async fn override_writes_preserve_parent_identity_and_detect_stale_rows() {
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:");
        options
            .max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        let db = Database::connect(options).await.unwrap();
        for sql in [
            "PRAGMA foreign_keys = ON",
            "CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants",
            "CREATE TABLE tenant_modules (id BLOB PRIMARY KEY NOT NULL, tenant_id BLOB NOT NULL, module_slug TEXT NOT NULL, \
             enabled BOOLEAN, settings TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
             updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, module_slug), FOREIGN KEY (tenant_id) REFERENCES tenants(id))",
        ] {
            db.execute_unprepared(sql).await.unwrap();
        }
        let native = Uuid::new_v4();
        let text = Uuid::new_v4();
        for parent in [native.into(), text.to_string().into()] {
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?1)",
                vec![parent],
            ))
            .await
            .unwrap();
        }
        for tenant in [native, text] {
            let created = TenantModuleStateStore::persist_settings(
                &db,
                TenantModuleSettingsRequest {
                    tenant_id: tenant,
                    module_slug: "blog".into(),
                    settings: json!({"first": true}),
                },
            )
            .await
            .unwrap();
            assert_eq!(created.enabled, None);
            // Historical override IDs may be stored as text.
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE tenant_modules SET id = ?1 WHERE id = ?2",
                vec![created.id.to_string().into(), created.id.into()],
            ))
            .await
            .unwrap();
            let toggled = TenantModuleStateStore::persist(
                &db,
                TenantModuleStateRequest {
                    tenant_id: tenant,
                    module_slug: "blog".into(),
                    enabled: true,
                },
            )
            .await
            .unwrap();
            assert_eq!(toggled.previous_enabled, None);
            let changed = TenantModuleStateStore::persist_settings(
                &db,
                TenantModuleSettingsRequest {
                    tenant_id: tenant,
                    module_slug: "blog".into(),
                    settings: json!({"second": true}),
                },
            )
            .await
            .unwrap();
            assert_eq!(changed.id, created.id);
            assert_eq!(changed.enabled, Some(true));
            let stale = TenantModuleStateStore::read(&db, tenant, "blog")
                .await
                .unwrap()
                .unwrap();
            TenantModuleStateStore::clear(&db, tenant, "blog")
                .await
                .unwrap();
            assert!(
                TenantModuleStateStore::update_existing(
                    &db,
                    tenant,
                    "blog",
                    &stale,
                    OverrideUpdate::Enabled(Some(false))
                )
                .await
                .is_err()
            );
            let reader = crate::DatabaseStaticModuleSettingsReader::new(db.clone());
            let after = reader.settings(tenant, "blog").await.unwrap().unwrap();
            assert_eq!(after.enabled, None);
            assert_eq!(after.settings, json!({"second": true}));
        }
        let before_ambiguity = TenantModuleStateStore::read(&db, native, "blog")
            .await
            .unwrap()
            .unwrap();
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![native.to_string().into()],
        ))
        .await
        .unwrap();
        assert!(
            TenantModuleStateStore::update_existing(
                &db,
                native,
                "blog",
                &before_ambiguity,
                OverrideUpdate::Settings(json!({"must_not_persist": true}))
            )
            .await
            .is_err()
        );
        assert!(
            TenantModuleStateStore::persist_settings(
                &db,
                TenantModuleSettingsRequest {
                    tenant_id: native,
                    module_slug: "blog".into(),
                    settings: json!({"must_not_persist": true}),
                }
            )
            .await
            .is_err()
        );
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM tenants WHERE id = ?1",
            vec![native.to_string().into()],
        ))
        .await
        .unwrap();
        let reader = crate::DatabaseStaticModuleSettingsReader::new(db.clone());
        assert_eq!(
            reader
                .settings(native, "blog")
                .await
                .unwrap()
                .unwrap()
                .settings,
            json!({"second": true})
        );
    }

    #[tokio::test]
    async fn tenant_state_and_recovery_read_native_uuid_and_reject_ambiguity() {
        let database = database().await;
        let tenant_id = Uuid::new_v4();
        database.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings) VALUES (?1, ?2, 'blog', TRUE, '{}')",
            vec![Uuid::new_v4().to_string().into(), tenant_id.into()],
        )).await.unwrap();
        assert_eq!(
            TenantModuleStateStore::read(&database, tenant_id, "blog")
                .await
                .unwrap()
                .unwrap()
                .enabled,
            Some(true)
        );
        assert_eq!(
            crate::recovery::read_tenant_override_enabled(&database, tenant_id, "blog")
                .await
                .unwrap(),
            Some(true)
        );
        database.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings) VALUES (?1, ?2, 'other', FALSE, '{}')",
            vec![Uuid::new_v4().to_string().into(), tenant_id.to_string().into()],
        )).await.unwrap();
        assert!(
            TenantModuleStateStore::read(&database, tenant_id, "missing")
                .await
                .is_err()
        );
        assert!(
            crate::recovery::read_tenant_override_enabled(&database, tenant_id, "blog")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn settings_persistence_creates_no_intent_and_preserves_recorded_intent() {
        let database = database().await;
        let tenant_id = Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?1)",
                vec![tenant_id.to_string().into()],
            ))
            .await
            .unwrap();
        // A first settings save records no enablement intent: the row
        // carries settings only and the platform default keeps applying.
        let dormant = TenantModuleStateStore::persist_settings(
            &database,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: "optional_module".to_string(),
                settings: json!({ "value": 1 }),
            },
        )
        .await
        .expect("settings without intent");
        assert_eq!(dormant.module_slug, "optional_module");
        assert_eq!(dormant.enabled, None);
        assert_eq!(dormant.settings, json!({ "value": 1 }));

        // An explicit toggle records intent on the same row; the previous
        // state reports "no recorded intent", not a fabricated boolean.
        let toggled = TenantModuleStateStore::persist(
            &database,
            TenantModuleStateRequest {
                tenant_id,
                module_slug: "optional_module".to_string(),
                enabled: false,
            },
        )
        .await
        .expect("explicit disable");
        assert_eq!(toggled.id, dormant.id);
        assert_eq!(toggled.enabled, Some(false));
        assert_eq!(toggled.previous_enabled, None);
        assert!(toggled.changed);

        // A later settings save must not disturb the recorded intent.
        let refreshed = TenantModuleStateStore::persist_settings(
            &database,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: "optional_module".to_string(),
                settings: json!({ "value": 11 }),
            },
        )
        .await
        .expect("refreshed settings");
        assert_eq!(refreshed.id, dormant.id);
        assert_eq!(refreshed.enabled, Some(false));
        assert_eq!(refreshed.settings, json!({ "value": 11 }));

        // Re-recording the same intent is not a change.
        let repeated = TenantModuleStateStore::persist(
            &database,
            TenantModuleStateRequest {
                tenant_id,
                module_slug: "optional_module".to_string(),
                enabled: false,
            },
        )
        .await
        .expect("repeated disable");
        assert_eq!(repeated.enabled, Some(false));
        assert_eq!(repeated.previous_enabled, Some(false));
        assert!(!repeated.changed);
    }

    #[tokio::test]
    async fn disable_and_reenable_preserve_settings_for_the_runtime_reader() {
        let database = database().await;
        let tenant_id = Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?1)",
                vec![tenant_id.to_string().into()],
            ))
            .await
            .unwrap();
        TenantModuleStateStore::persist_settings(
            &database,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: "blog".to_string(),
                settings: json!({ "use_reactions": true }),
            },
        )
        .await
        .expect("enabled Blog settings");

        TenantModuleStateStore::persist(
            &database,
            TenantModuleStateRequest {
                tenant_id,
                module_slug: "blog".to_string(),
                enabled: false,
            },
        )
        .await
        .expect("disable Blog");

        let reader = crate::DatabaseStaticModuleSettingsReader::new(database.clone());
        let disabled = reader
            .settings(tenant_id, "blog")
            .await
            .expect("disabled settings read")
            .expect("disabled settings row");
        assert_eq!(disabled.enabled, Some(false));
        assert_eq!(disabled.settings, json!({ "use_reactions": true }));

        TenantModuleStateStore::persist(
            &database,
            TenantModuleStateRequest {
                tenant_id,
                module_slug: "blog".to_string(),
                enabled: true,
            },
        )
        .await
        .expect("re-enable Blog");

        let reenabled = reader
            .settings(tenant_id, "blog")
            .await
            .expect("re-enabled settings read")
            .expect("re-enabled settings row");
        assert_eq!(reenabled.enabled, Some(true));
        assert_eq!(reenabled.settings, json!({ "use_reactions": true }));
    }

    async fn operation_database() -> sea_orm::DatabaseConnection {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("operation database");
        database.execute_unprepared(
            "CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants",
        ).await.expect("tenant identity fixture");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE module_operations (\
                    id TEXT PRIMARY KEY NOT NULL, \
                    tenant_id TEXT NOT NULL, \
                    module_slug TEXT NOT NULL, \
                    requested_enabled BOOLEAN NOT NULL, \
                    previous_effective_enabled BOOLEAN NOT NULL, \
                    status TEXT NOT NULL, \
                    requested_by TEXT, \
                    trace_id TEXT, \
                    correlation_id TEXT, \
                    idempotency_key TEXT, \
                    recovery_source_id TEXT, \
                    expected_revision INTEGER, \
                    error_message TEXT, \
                    result_snapshot TEXT, \
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                 )"
                .to_string(),
            ))
            .await
            .expect("module operations table");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE UNIQUE INDEX uq_module_operations_tenant_idempotency_key \
                 ON module_operations (tenant_id, idempotency_key) \
                 WHERE idempotency_key IS NOT NULL"
                    .to_string(),
            ))
            .await
            .expect("module operations idempotency index");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE module_static_tenant_lifecycle (\
                    tenant_id TEXT NOT NULL, \
                    module_slug TEXT NOT NULL, \
                    revision INTEGER NOT NULL, \
                    active_idempotency_key TEXT, \
                    active_claimed_at TEXT, \
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    PRIMARY KEY (tenant_id, module_slug)\
                 )"
                .to_string(),
            ))
            .await
            .expect("module lifecycle aggregate table");
        database.execute_unprepared(
            "CREATE TRIGGER fixture_lifecycle_tenant AFTER INSERT ON module_static_tenant_lifecycle BEGIN \
             INSERT OR IGNORE INTO tenants (id) VALUES (NEW.tenant_id); END",
        ).await.expect("lifecycle tenant fixture");
        database
    }

    fn operation_request(
        tenant_id: Uuid,
        module_slug: &str,
        idempotency_key: Uuid,
    ) -> ModuleOperationRequest {
        ModuleOperationRequest {
            tenant_id,
            module_slug: module_slug.to_string(),
            requested_enabled: true,
            previous_effective_enabled: false,
            requested_by: Some("operator".to_string()),
            trace_id: Some("test:operation-store".to_string()),
            correlation_id: Uuid::new_v4().to_string(),
            idempotency_key: Some(idempotency_key),
            recovery_source_id: None,
            expected_revision: Some(0),
        }
    }

    async fn seed_native_tenant(database: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?1)",
                vec![tenant_id.into()],
            ))
            .await
            .unwrap();
    }

    async fn raw_journal_status(
        database: &sea_orm::DatabaseConnection,
        operation_id: Uuid,
    ) -> String {
        database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT status FROM module_operations WHERE id = ?1",
                vec![operation_id.into()],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "status")
            .unwrap()
    }

    async fn insert_stale_operation(
        database: &sea_orm::DatabaseConnection,
        operation_id: Uuid,
        tenant_id: Uuid,
        module_slug: &str,
        status: ModuleOperationStatus,
        idempotency_key: Uuid,
    ) {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_operations (\
                    id, tenant_id, module_slug, requested_enabled, \
                    previous_effective_enabled, status, requested_by, trace_id, \
                    correlation_id, idempotency_key, expected_revision, \
                    error_message, created_at, updated_at\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                vec![
                    operation_id.into(),
                    tenant_id.to_string().into(),
                    module_slug.into(),
                    true.into(),
                    false.into(),
                    status.as_str().into(),
                    "operator".into(),
                    "test:stale-operation".into(),
                    Uuid::new_v4().to_string().into(),
                    idempotency_key.into(),
                    0_i64.into(),
                    Option::<String>::None.into(),
                    "2000-01-01T00:00:00.000+00:00".into(),
                    "2000-01-01T00:00:00.000+00:00".into(),
                ],
            ))
            .await
            .expect("stale operation insert");
    }

    async fn insert_claim(
        database: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        module_slug: &str,
        revision: i64,
        idempotency_key: Option<Uuid>,
        claimed_at: Option<String>,
    ) {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_static_tenant_lifecycle (\
                    tenant_id, module_slug, revision, active_idempotency_key, \
                    active_claimed_at\
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                vec![
                    tenant_id.to_string().into(),
                    module_slug.into(),
                    revision.into(),
                    idempotency_key.map(|key| key.to_string()).into(),
                    claimed_at.into(),
                ],
            ))
            .await
            .expect("lifecycle claim insert");
    }

    #[tokio::test]
    async fn reaper_rejects_orphaned_or_ambiguous_journal_tenants_without_mutating_operations() {
        let database = operation_database().await;
        let tenant_id = Uuid::new_v4();
        seed_native_tenant(&database, tenant_id).await;
        let operation_id = Uuid::new_v4();
        insert_stale_operation(
            &database,
            operation_id,
            tenant_id,
            "orphan",
            ModuleOperationStatus::Running,
            Uuid::new_v4(),
        )
        .await;
        let healthy_tenant = Uuid::new_v4();
        let healthy_operation = Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?1)",
                vec![healthy_tenant.to_string().into()],
            ))
            .await
            .unwrap();
        insert_stale_operation(
            &database,
            healthy_operation,
            healthy_tenant,
            "healthy",
            ModuleOperationStatus::Running,
            Uuid::new_v4(),
        )
        .await;
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_operations SET updated_at = '1999-01-01T00:00:00Z' WHERE id = ?1",
                vec![healthy_operation.into()],
            ))
            .await
            .unwrap();
        assert!(matches!(
            ModuleOperationJournal::reap_stale(&database, 10).await,
            Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
        ));
        assert_eq!(
            ModuleOperationJournal::find(&database, healthy_operation)
                .await
                .unwrap()
                .unwrap()
                .status,
            ModuleOperationStatus::Running,
            "the earlier candidate's status update must roll back with the batch"
        );
        assert!(matches!(
            ModuleOperationJournal::find(&database, operation_id).await,
            Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
        ));
        assert_eq!(raw_journal_status(&database, operation_id).await, "running");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?1)",
                vec![tenant_id.to_string().into()],
            ))
            .await
            .unwrap();
        assert!(matches!(
            ModuleOperationJournal::reap_stale(&database, 10).await,
            Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
        ));
        assert!(matches!(
            ModuleOperationJournal::find(&database, operation_id).await,
            Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
        ));
        assert_eq!(raw_journal_status(&database, operation_id).await, "running");
    }

    #[tokio::test]
    async fn reaper_handles_native_sqlite_tenant_claims_without_orphaning_live_work() {
        let database = operation_database().await;
        let tenant_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        let key = Uuid::new_v4();
        insert_stale_operation(
            &database,
            operation_id,
            tenant_id,
            "native",
            ModuleOperationStatus::Running,
            key,
        )
        .await;
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_operations SET tenant_id = ?1 WHERE id = ?2",
                vec![tenant_id.into(), operation_id.into()],
            ))
            .await
            .unwrap();
        database.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_static_tenant_lifecycle (tenant_id, module_slug, revision, active_idempotency_key, active_claimed_at) VALUES (?1, 'native', 0, ?2, ?3)",
            vec![tenant_id.into(), key.to_string().into(), Utc::now().to_rfc3339().into()],
        )).await.unwrap();
        let live = ModuleOperationJournal::reap_stale(&database, 10)
            .await
            .unwrap();
        assert_eq!(live.orphaned, 0);
        assert_eq!(
            ModuleOperationJournal::find(&database, operation_id)
                .await
                .unwrap()
                .unwrap()
                .status,
            ModuleOperationStatus::Running
        );
        database.execute_unprepared("UPDATE module_static_tenant_lifecycle SET active_claimed_at = '2000-01-01T00:00:00Z'").await.unwrap();
        let expired = ModuleOperationJournal::reap_stale(&database, 10)
            .await
            .unwrap();
        assert_eq!(expired.orphaned, 1);
        assert_eq!(expired.released_claims, 1);
        assert_eq!(
            StaticTenantLifecycleStore::snapshot(&database, tenant_id, "native")
                .await
                .unwrap()
                .active_idempotency_key,
            None
        );
    }

    #[tokio::test]
    async fn reaper_orphans_only_operations_without_a_live_matching_claim() {
        let database = operation_database().await;
        let tenant_id = Uuid::new_v4();
        let stale_claimed_at = Some("2000-01-01T00:00:00.000+00:00".to_string());
        let live_claimed_at = Some(Utc::now().to_rfc3339());

        let orphan_expired_id = Uuid::new_v4();
        let orphan_expired_key = Uuid::new_v4();
        insert_stale_operation(
            &database,
            orphan_expired_id,
            tenant_id,
            "orphan-expired",
            ModuleOperationStatus::Running,
            orphan_expired_key,
        )
        .await;
        insert_claim(
            &database,
            tenant_id,
            "orphan-expired",
            0,
            Some(orphan_expired_key),
            stale_claimed_at.clone(),
        )
        .await;

        let orphan_mismatch_id = Uuid::new_v4();
        let orphan_mismatch_key = Uuid::new_v4();
        let other_claim_key = Uuid::new_v4();
        insert_stale_operation(
            &database,
            orphan_mismatch_id,
            tenant_id,
            "orphan-mismatch",
            ModuleOperationStatus::Validated,
            orphan_mismatch_key,
        )
        .await;
        insert_claim(
            &database,
            tenant_id,
            "orphan-mismatch",
            0,
            Some(other_claim_key),
            stale_claimed_at.clone(),
        )
        .await;

        let committed_unknown_id = Uuid::new_v4();
        let committed_unknown_key = Uuid::new_v4();
        insert_stale_operation(
            &database,
            committed_unknown_id,
            tenant_id,
            "committed-unknown",
            ModuleOperationStatus::CommittedHookPending,
            committed_unknown_key,
        )
        .await;
        insert_claim(
            &database,
            tenant_id,
            "committed-unknown",
            1,
            Some(committed_unknown_key),
            stale_claimed_at.clone(),
        )
        .await;

        let failed_claim_id = Uuid::new_v4();
        let failed_claim_key = Uuid::new_v4();
        insert_stale_operation(
            &database,
            failed_claim_id,
            tenant_id,
            "failed-claim",
            ModuleOperationStatus::Failed,
            failed_claim_key,
        )
        .await;
        insert_claim(
            &database,
            tenant_id,
            "failed-claim",
            0,
            Some(failed_claim_key),
            stale_claimed_at.clone(),
        )
        .await;

        let live_running_id = Uuid::new_v4();
        let live_running_key = Uuid::new_v4();
        insert_stale_operation(
            &database,
            live_running_id,
            tenant_id,
            "live-running",
            ModuleOperationStatus::Running,
            live_running_key,
        )
        .await;
        insert_claim(
            &database,
            tenant_id,
            "live-running",
            0,
            Some(live_running_key),
            live_claimed_at.clone(),
        )
        .await;

        let live_committed_id = Uuid::new_v4();
        let live_committed_key = Uuid::new_v4();
        insert_stale_operation(
            &database,
            live_committed_id,
            tenant_id,
            "live-committed",
            ModuleOperationStatus::CommittedHookPending,
            live_committed_key,
        )
        .await;
        insert_claim(
            &database,
            tenant_id,
            "live-committed",
            1,
            Some(live_committed_key),
            live_claimed_at,
        )
        .await;

        let terminal_committed_id = Uuid::new_v4();
        let terminal_committed_key = Uuid::new_v4();
        insert_stale_operation(
            &database,
            terminal_committed_id,
            tenant_id,
            "terminal-committed",
            ModuleOperationStatus::Committed,
            terminal_committed_key,
        )
        .await;
        insert_claim(&database, tenant_id, "terminal-committed", 1, None, None).await;

        let terminal_committed_claim_id = Uuid::new_v4();
        let terminal_committed_claim_key = Uuid::new_v4();
        insert_stale_operation(
            &database,
            terminal_committed_claim_id,
            tenant_id,
            "terminal-committed-claim",
            ModuleOperationStatus::Committed,
            terminal_committed_claim_key,
        )
        .await;
        insert_claim(
            &database,
            tenant_id,
            "terminal-committed-claim",
            1,
            Some(terminal_committed_claim_key),
            stale_claimed_at,
        )
        .await;

        let report =
            ModuleOperationJournal::reap_stale(&database, MAX_STALE_MODULE_OPERATIONS_PER_REAP)
                .await
                .expect("stale operation reap");
        assert_eq!(report.scanned, 8);
        assert_eq!(report.orphaned, 2);
        assert_eq!(report.committed_hook_unknown, 1);
        assert_eq!(report.released_claims, 4);
        assert!(report.has_changes());

        for (operation_id, expected_status) in [
            (orphan_expired_id, ModuleOperationStatus::Orphaned),
            (orphan_mismatch_id, ModuleOperationStatus::Orphaned),
            (
                committed_unknown_id,
                ModuleOperationStatus::CommittedHookFailed,
            ),
            (failed_claim_id, ModuleOperationStatus::Failed),
            (live_running_id, ModuleOperationStatus::Running),
            (
                live_committed_id,
                ModuleOperationStatus::CommittedHookPending,
            ),
            (terminal_committed_id, ModuleOperationStatus::Committed),
            (
                terminal_committed_claim_id,
                ModuleOperationStatus::Committed,
            ),
        ] {
            let operation = ModuleOperationJournal::find(&database, operation_id)
                .await
                .expect("reaped operation lookup")
                .expect("reaped operation exists");
            assert_eq!(operation.status, expected_status);
        }

        for (module_slug, expected_key) in [
            ("orphan-expired", None),
            ("orphan-mismatch", Some(other_claim_key)),
            ("committed-unknown", None),
            ("failed-claim", None),
            ("live-running", Some(live_running_key)),
            ("live-committed", Some(live_committed_key)),
            ("terminal-committed", None),
            ("terminal-committed-claim", None),
        ] {
            let claim = StaticTenantLifecycleStore::snapshot(&database, tenant_id, module_slug)
                .await
                .expect("lifecycle claim snapshot");
            assert_eq!(claim.active_idempotency_key, expected_key);
        }
    }

    #[tokio::test]
    async fn already_held_claim_still_requires_the_current_revision() {
        let database = operation_database().await;
        let tenant_id = Uuid::new_v4();
        let idempotency_key = Uuid::new_v4();
        insert_claim(
            &database,
            tenant_id,
            "already-held",
            3,
            Some(idempotency_key),
            Some(Utc::now().to_rfc3339()),
        )
        .await;

        let stale = StaticTenantLifecycleStore::claim(
            &database,
            tenant_id,
            "already-held",
            2,
            idempotency_key,
        )
        .await;
        assert!(matches!(
            stale,
            Err(StaticTenantLifecycleStoreError::RevisionConflict {
                expected: 2,
                current: 3,
                ..
            })
        ));

        let current = StaticTenantLifecycleStore::claim(
            &database,
            tenant_id,
            "already-held",
            3,
            idempotency_key,
        )
        .await
        .expect("current already-held claim");
        assert!(matches!(
            current,
            StaticTenantLifecycleClaim::AlreadyHeld(snapshot)
                if snapshot.revision == 3
                    && snapshot.active_idempotency_key == Some(idempotency_key)
        ));
    }

    #[tokio::test]
    async fn journal_refuses_legacy_or_duplicate_uuid_encodings_before_reexecution() {
        for column in ["tenant_id", "idempotency_key", "id"] {
            let database = operation_database().await;
            let tenant_id = Uuid::new_v4();
            seed_native_tenant(&database, tenant_id).await;
            let key = Uuid::new_v4();
            let request = operation_request(tenant_id, "journal", key);
            let recorded = ModuleOperationJournal::record(&database, request.clone())
                .await
                .unwrap();
            let legacy = match column {
                "tenant_id" => tenant_id,
                "idempotency_key" => key,
                "id" => recorded.id,
                _ => unreachable!(),
            };
            database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    format!("UPDATE module_operations SET {column} = ?1 WHERE id = ?2"),
                    vec![legacy.to_string().into(), recorded.id.into()],
                ))
                .await
                .unwrap();
            assert!(
                matches!(
                    ModuleOperationJournal::record_idempotent(&database, request.clone()).await,
                    Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
                ),
                "legacy {column} must fail before inserting a second operation"
            );
            assert!(matches!(
                ModuleOperationJournal::find(&database, recorded.id).await,
                Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
            ));
            let row = database
                .query_one_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    "SELECT COUNT(*) AS count FROM module_operations",
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<i64>("", "count").unwrap(), 1);
        }

        let database = operation_database().await;
        let tenant_id = Uuid::new_v4();
        seed_native_tenant(&database, tenant_id).await;
        let key = Uuid::new_v4();
        let request = operation_request(tenant_id, "journal", key);
        let original = ModuleOperationJournal::record(&database, request.clone())
            .await
            .unwrap();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_operations SET tenant_id = ?1 WHERE id = ?2",
                vec![tenant_id.to_string().into(), original.id.into()],
            ))
            .await
            .unwrap();
        database.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_operations (id, tenant_id, module_slug, requested_enabled, previous_effective_enabled, status, idempotency_key, expected_revision) VALUES (?1, ?2, 'journal', TRUE, FALSE, 'validated', ?3, 0)",
            vec![Uuid::new_v4().into(), tenant_id.into(), key.into()],
        )).await.unwrap();
        assert!(matches!(
            ModuleOperationJournal::record_idempotent(&database, request).await,
            Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
        ));

        let database = operation_database().await;
        let tenant_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        let unsupported_parent = format!("urn:uuid:{tenant_id}");
        database.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)", vec![unsupported_parent.clone().into()])).await.unwrap();
        database.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO module_operations (id, tenant_id, module_slug, requested_enabled, previous_effective_enabled, status, idempotency_key, expected_revision) VALUES (?1, ?2, 'journal', TRUE, FALSE, 'validated', ?3, 0)",
            vec![operation_id.into(), unsupported_parent.into(), Uuid::new_v4().into()])).await.unwrap();
        assert!(matches!(ModuleOperationJournal::find(&database, operation_id).await,
            Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)));
    }

    #[tokio::test]
    async fn journal_insert_narrows_only_unique_violations_and_types_transitions() {
        let database = operation_database().await;
        let tenant_id = Uuid::new_v4();
        seed_native_tenant(&database, tenant_id).await;
        let idempotency_key = Uuid::new_v4();
        let request = operation_request(tenant_id, "journal", idempotency_key);

        let recorded = match ModuleOperationJournal::record_idempotent(&database, request.clone())
            .await
            .expect("initial journal record")
        {
            ModuleOperationRecordOutcome::Recorded(record) => record,
            ModuleOperationRecordOutcome::Replayed(_) => {
                panic!("the first journal insert must be recorded")
            }
        };

        assert!(matches!(
            ModuleOperationJournal::record(&database, request.clone()).await,
            Err(ModuleOperationStoreError::UniqueViolation(_))
        ));

        let replayed = match ModuleOperationJournal::record_idempotent(&database, request.clone())
            .await
            .expect("journal replay")
        {
            ModuleOperationRecordOutcome::Replayed(record) => record,
            ModuleOperationRecordOutcome::Recorded(_) => {
                panic!("an exact retry must replay the existing journal row")
            }
        };
        assert_eq!(replayed.id, recorded.id);

        ModuleOperationJournal::mark_failed(&database, recorded.id, "failed")
            .await
            .expect("mark journal failed");
        assert!(matches!(
            ModuleOperationJournal::mark_running(&database, recorded.id).await,
            Err(ModuleOperationStoreError::InvalidStatusTransition {
                current: ModuleOperationStatus::Failed,
                requested: ModuleOperationStatus::Running,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn discard_validated_only_removes_an_unstarted_operation() {
        let database = operation_database().await;
        let tenant_id = Uuid::new_v4();
        seed_native_tenant(&database, tenant_id).await;
        let discarded = ModuleOperationJournal::record(
            &database,
            operation_request(tenant_id, "discarded", Uuid::new_v4()),
        )
        .await
        .expect("discarded operation record");
        assert!(
            ModuleOperationJournal::discard_validated(&database, discarded.id)
                .await
                .expect("discard validated operation")
        );
        assert!(
            ModuleOperationJournal::find(&database, discarded.id)
                .await
                .expect("discarded operation lookup")
                .is_none()
        );

        let running = ModuleOperationJournal::record(
            &database,
            operation_request(tenant_id, "running", Uuid::new_v4()),
        )
        .await
        .expect("running operation record");
        ModuleOperationJournal::mark_running(&database, running.id)
            .await
            .expect("mark operation running");
        assert!(
            !ModuleOperationJournal::discard_validated(&database, running.id)
                .await
                .expect("running operation cannot be discarded")
        );
    }
}
