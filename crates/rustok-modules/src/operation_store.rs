use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

use crate::ModuleOperationStatus;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleOperationRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub requested_enabled: bool,
    pub previous_effective_enabled: bool,
    pub requested_by: Option<String>,
    pub correlation_id: String,
    pub idempotency_key: Option<Uuid>,
    /// Static lifecycle commands bind this precondition into the durable
    /// idempotency identity. Artifact paths retain no value here because they
    /// use their own lifecycle aggregate.
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
    pub correlation_id: Option<String>,
    pub idempotency_key: Option<Uuid>,
    pub expected_revision: Option<u64>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum ModuleOperationStoreError {
    #[error("module operation store database error: {0}")]
    Database(String),
    #[error("module `{0}` is not enabled for this tenant")]
    ModuleNotEnabled(String),
    #[error("module operation idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("module operation idempotency key is required")]
    MissingIdempotencyKey,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TenantModuleStateRecord {
    pub id: Uuid,
    /// Explicit enablement persisted by this operation.
    pub enabled: bool,
    pub previous_enabled: bool,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TenantModuleSettingsRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub settings: serde_json::Value,
    pub is_core: bool,
    pub is_effectively_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TenantModuleSettingsRecord {
    pub id: Uuid,
    pub module_slug: String,
    pub enabled: bool,
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
        let backend = db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => format!("{} WHERE id = $1", operation_select_sql()),
            _ => format!("{} WHERE id = ?1", operation_select_sql()),
        };
        db.query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![operation_id.into()],
        ))
        .await
        .map_err(database_error)?
        .map(operation_snapshot)
        .transpose()
    }

    pub async fn failed_for_tenant<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: Option<&str>,
    ) -> Result<Vec<ModuleOperationSnapshot>, ModuleOperationStoreError> {
        let backend = db.get_database_backend();
        let (sql, values) = match (backend, module_slug) {
            (DbBackend::Postgres, Some(module_slug)) => (
                format!(
                    "{} WHERE tenant_id = $1 AND status = $2 AND module_slug = $3 ORDER BY created_at DESC",
                    operation_select_sql()
                ),
                vec![
                    tenant_id.into(),
                    ModuleOperationStatus::Failed.as_str().into(),
                    module_slug.into(),
                ],
            ),
            (DbBackend::Postgres, None) => (
                format!(
                    "{} WHERE tenant_id = $1 AND status = $2 ORDER BY created_at DESC",
                    operation_select_sql()
                ),
                vec![
                    tenant_id.into(),
                    ModuleOperationStatus::Failed.as_str().into(),
                ],
            ),
            (_, Some(module_slug)) => (
                format!(
                    "{} WHERE tenant_id = ?1 AND status = ?2 AND module_slug = ?3 ORDER BY created_at DESC",
                    operation_select_sql()
                ),
                vec![
                    tenant_id.into(),
                    ModuleOperationStatus::Failed.as_str().into(),
                    module_slug.into(),
                ],
            ),
            (_, None) => (
                format!(
                    "{} WHERE tenant_id = ?1 AND status = ?2 ORDER BY created_at DESC",
                    operation_select_sql()
                ),
                vec![
                    tenant_id.into(),
                    ModuleOperationStatus::Failed.as_str().into(),
                ],
            ),
        };
        db.query_all(Statement::from_sql_and_values(backend, sql, values))
            .await
            .map_err(database_error)?
            .into_iter()
            .map(operation_snapshot)
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
            Err(error) => {
                if let Some(existing) =
                    Self::find_by_idempotency_key(db, request.tenant_id, idempotency_key).await?
                {
                    replay_or_conflict(&request, existing)
                } else {
                    Err(error)
                }
            }
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
            || existing.requested_by != request.requested_by
            || existing.correlation_id.as_deref() != Some(request.correlation_id.as_str())
            || existing.expected_revision != request.expected_revision
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
        execute(
            db,
            "INSERT INTO module_operations (id, tenant_id, module_slug, requested_enabled, previous_effective_enabled, status, requested_by, correlation_id, idempotency_key, expected_revision, error_message) VALUES ({1}, {2}, {3}, {4}, {5}, {6}, {7}, {8}, {9}, {10}, NULL)",
            vec![
                id.into(),
                request.tenant_id.into(),
                request.module_slug.into(),
                request.requested_enabled.into(),
                request.previous_effective_enabled.into(),
                ModuleOperationStatus::Validated.as_str().into(),
                request.requested_by.into(),
                request.correlation_id.into(),
                request.idempotency_key.into(),
                request
                    .expected_revision
                    .map(|value| i64::try_from(value))
                    .transpose()
                    .map_err(|_| ModuleOperationStoreError::Database(
                        "static lifecycle expected revision exceeds storage range".to_string(),
                    ))?
                    .into(),
            ],
        )
        .await?;
        Ok(ModuleOperationRecord { id })
    }

    async fn find_by_idempotency_key<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        idempotency_key: Uuid,
    ) -> Result<Option<ModuleOperationSnapshot>, ModuleOperationStoreError> {
        let backend = db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => format!(
                "{} WHERE tenant_id = $1 AND idempotency_key = $2 LIMIT 1",
                operation_select_sql()
            ),
            _ => format!(
                "{} WHERE tenant_id = ?1 AND idempotency_key = ?2 LIMIT 1",
                operation_select_sql()
            ),
        };
        db.query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into(), idempotency_key.into()],
        ))
        .await
        .map_err(database_error)?
        .map(operation_snapshot)
        .transpose()
    }

    pub async fn mark_running<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<(), ModuleOperationStoreError> {
        Self::mark_status(db, operation_id, ModuleOperationStatus::Running, None).await
    }

    pub async fn mark_committed<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<(), ModuleOperationStoreError> {
        Self::mark_status(db, operation_id, ModuleOperationStatus::Committed, None).await
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
            Some(error_message),
        )
        .await
    }

    async fn mark_status<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
        status: ModuleOperationStatus,
        error_message: Option<&str>,
    ) -> Result<(), ModuleOperationStoreError> {
        match error_message {
            Some(error_message) => execute(
                db,
                "UPDATE module_operations SET status = {1}, error_message = {2}, updated_at = CURRENT_TIMESTAMP WHERE id = {3}",
                vec![status.as_str().into(), error_message.into(), operation_id.into()],
            )
            .await,
            None => execute(
                db,
                "UPDATE module_operations SET status = {1}, updated_at = CURRENT_TIMESTAMP WHERE id = {2}",
                vec![status.as_str().into(), operation_id.into()],
            )
            .await,
        }
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
        let backend = db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                "SELECT revision, active_idempotency_key FROM module_static_tenant_lifecycle \
                 WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
            }
            _ => {
                "SELECT revision, active_idempotency_key FROM module_static_tenant_lifecycle \
                 WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1"
            }
        };
        db.query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into(), module_slug.into()],
        ))
        .await
        .map_err(static_lifecycle_database_error)?
        .map(|row| static_lifecycle_snapshot_from_row(row, tenant_id, module_slug))
        .transpose()
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
        let backend = db.get_database_backend();
        let mut values = vec![tenant_id.into()];
        let placeholders = module_slugs
            .iter()
            .enumerate()
            .map(|(index, module_slug)| {
                values.push(module_slug.clone().into());
                match backend {
                    DbBackend::Postgres => format!("${}", index + 2),
                    _ => format!("?{}", index + 2),
                }
            })
            .collect::<Vec<_>>();
        let tenant_placeholder = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };
        let rows = db
            .query_all(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT module_slug, revision, active_idempotency_key \
                     FROM module_static_tenant_lifecycle WHERE tenant_id = {tenant_placeholder} \
                     AND module_slug IN ({})",
                    placeholders.join(", ")
                ),
                values,
            ))
            .await
            .map_err(static_lifecycle_database_error)?;
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
        for row in rows {
            let module_slug: String = row
                .try_get("", "module_slug")
                .map_err(static_lifecycle_database_error)?;
            let snapshot = static_lifecycle_snapshot_from_row(row, tenant_id, &module_slug)?;
            snapshots.insert(module_slug, snapshot);
        }
        Ok(snapshots)
    }

    /// Admits one command only when its reviewed aggregate revision is current.
    /// The claim is intentionally durable and is cleared only by the owner when
    /// that same operation reaches a terminal lifecycle boundary.
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
            return if active_idempotency_key == idempotency_key {
                Ok(StaticTenantLifecycleClaim::AlreadyHeld(current))
            } else {
                Err(StaticTenantLifecycleStoreError::OperationInProgress {
                    module_slug: module_slug.to_string(),
                })
            };
        }
        if current.revision != expected_revision {
            return Err(StaticTenantLifecycleStoreError::RevisionConflict {
                module_slug: module_slug.to_string(),
                expected: expected_revision,
                current: current.revision,
            });
        }

        let backend = db.get_database_backend();
        if persisted.is_none() {
            let insert = db
                .execute(Statement::from_sql_and_values(
                    backend,
                    match backend {
                        DbBackend::Postgres => {
                            "INSERT INTO module_static_tenant_lifecycle \
                             (tenant_id, module_slug, revision, active_idempotency_key) \
                             VALUES ($1, $2, $3, $4)"
                        }
                        _ => {
                            "INSERT INTO module_static_tenant_lifecycle \
                             (tenant_id, module_slug, revision, active_idempotency_key) \
                             VALUES (?1, ?2, ?3, ?4)"
                        }
                    },
                    vec![
                        tenant_id.into(),
                        module_slug.into(),
                        i64::try_from(expected_revision)
                            .map_err(|_| {
                                StaticTenantLifecycleStoreError::RevisionOverflow(
                                    module_slug.to_string(),
                                )
                            })?
                            .into(),
                        idempotency_key.into(),
                    ],
                ))
                .await;
            match insert {
                Ok(_) => {
                    return Ok(StaticTenantLifecycleClaim::Acquired(
                        StaticTenantLifecycleSnapshot {
                            tenant_id,
                            module_slug: module_slug.to_string(),
                            revision: expected_revision,
                            active_idempotency_key: Some(idempotency_key),
                        },
                    ));
                }
                Err(_) => {
                    // A concurrent first claim won the composite primary key.
                    // Reloading applies the same exact-key/revision decision as
                    // an already materialized aggregate.
                    let refreshed = Self::snapshot(db, tenant_id, module_slug).await?;
                    return if refreshed.active_idempotency_key == Some(idempotency_key) {
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
            }
        }

        let result = db
            .execute(Statement::from_sql_and_values(
                backend,
                render_parameters(
                    "UPDATE module_static_tenant_lifecycle \
                     SET active_idempotency_key = {1}, updated_at = CURRENT_TIMESTAMP \
                     WHERE tenant_id = {2} AND module_slug = {3} \
                       AND revision = {4} AND active_idempotency_key IS NULL",
                    backend,
                ),
                vec![
                    idempotency_key.into(),
                    tenant_id.into(),
                    module_slug.into(),
                    i64::try_from(expected_revision)
                        .map_err(|_| {
                            StaticTenantLifecycleStoreError::RevisionOverflow(
                                module_slug.to_string(),
                            )
                        })?
                        .into(),
                ],
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
        if refreshed.active_idempotency_key == Some(idempotency_key) {
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
        let backend = db.get_database_backend();
        let result = db
            .execute(Statement::from_sql_and_values(
                backend,
                render_parameters(
                    "UPDATE module_static_tenant_lifecycle \
                     SET revision = {1}, updated_at = CURRENT_TIMESTAMP \
                     WHERE tenant_id = {2} AND module_slug = {3} \
                       AND revision = {4} AND active_idempotency_key = {5}",
                    backend,
                ),
                vec![
                    i64::try_from(next_revision)
                        .map_err(|_| {
                            StaticTenantLifecycleStoreError::RevisionOverflow(
                                module_slug.to_string(),
                            )
                        })?
                        .into(),
                    tenant_id.into(),
                    module_slug.into(),
                    i64::try_from(expected_revision)
                        .map_err(|_| {
                            StaticTenantLifecycleStoreError::RevisionOverflow(
                                module_slug.to_string(),
                            )
                        })?
                        .into(),
                    idempotency_key.into(),
                ],
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
        let backend = db.get_database_backend();
        let result = db
            .execute(Statement::from_sql_and_values(
                backend,
                render_parameters(
                    "UPDATE module_static_tenant_lifecycle \
                     SET active_idempotency_key = NULL, updated_at = CURRENT_TIMESTAMP \
                     WHERE tenant_id = {1} AND module_slug = {2} \
                       AND active_idempotency_key = {3}",
                    backend,
                ),
                vec![tenant_id.into(), module_slug.into(), idempotency_key.into()],
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
        active_idempotency_key: row
            .try_get("", "active_idempotency_key")
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
     requested_by, correlation_id, idempotency_key, expected_revision, error_message, created_at FROM module_operations"
}

fn replay_or_conflict(
    request: &ModuleOperationRequest,
    existing: ModuleOperationSnapshot,
) -> Result<ModuleOperationRecordOutcome, ModuleOperationStoreError> {
    if existing.module_slug != request.module_slug
        || existing.requested_enabled != request.requested_enabled
        || existing.previous_effective_enabled != request.previous_effective_enabled
        || existing.requested_by != request.requested_by
        || existing.correlation_id.as_deref() != Some(request.correlation_id.as_str())
        || existing.expected_revision != request.expected_revision
    {
        return Err(ModuleOperationStoreError::IdempotencyConflict);
    }
    Ok(ModuleOperationRecordOutcome::Replayed(
        ModuleOperationRecord { id: existing.id },
    ))
}

fn operation_snapshot(
    row: sea_orm::QueryResult,
) -> Result<ModuleOperationSnapshot, ModuleOperationStoreError> {
    let status: String = row.try_get("", "status").map_err(database_error)?;
    Ok(ModuleOperationSnapshot {
        id: row.try_get("", "id").map_err(database_error)?,
        tenant_id: row.try_get("", "tenant_id").map_err(database_error)?,
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
        correlation_id: row.try_get("", "correlation_id").map_err(database_error)?,
        idempotency_key: row.try_get("", "idempotency_key").map_err(database_error)?,
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
    })
}

fn database_error(error: impl std::fmt::Display) -> ModuleOperationStoreError {
    ModuleOperationStoreError::Database(error.to_string())
}

impl TenantModuleStateStore {
    pub async fn read<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<TenantModuleStateRecord>, ModuleOperationStoreError> {
        let backend = db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                "SELECT id, enabled FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
            }
            _ => {
                "SELECT id, enabled FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1"
            }
        };
        db.query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into(), module_slug.into()],
        ))
        .await
        .map_err(database_error)?
        .map(|row| {
            let enabled: bool = row.try_get("", "enabled").map_err(database_error)?;
            Ok(TenantModuleStateRecord {
                id: row.try_get("", "id").map_err(database_error)?,
                enabled,
                previous_enabled: enabled,
                changed: false,
            })
        })
        .transpose()
    }

    pub async fn persist<C: ConnectionTrait>(
        db: &C,
        request: TenantModuleStateRequest,
    ) -> Result<TenantModuleStateRecord, ModuleOperationStoreError> {
        let backend = db.get_database_backend();
        let select = match backend {
            DbBackend::Postgres => {
                "SELECT id, enabled FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
            }
            _ => {
                "SELECT id, enabled FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1"
            }
        };
        if let Some(row) = db
            .query_one(Statement::from_sql_and_values(
                backend,
                select,
                vec![request.tenant_id.into(), request.module_slug.clone().into()],
            ))
            .await
            .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?
        {
            let id: Uuid = row
                .try_get("", "id")
                .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?;
            let previous_enabled = row
                .try_get("", "enabled")
                .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?;
            if previous_enabled != request.enabled {
                execute(
                    db,
                    "UPDATE tenant_modules SET enabled = {1}, updated_at = CURRENT_TIMESTAMP WHERE id = {2}",
                    vec![request.enabled.into(), id.into()],
                )
                .await?;
            }
            return Ok(TenantModuleStateRecord {
                id,
                enabled: request.enabled,
                previous_enabled,
                changed: previous_enabled != request.enabled,
            });
        }

        let id = rustok_core::generate_id();
        execute(
            db,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings) VALUES ({1}, {2}, {3}, {4}, '{}')",
            vec![
                id.into(),
                request.tenant_id.into(),
                request.module_slug.into(),
                request.enabled.into(),
            ],
        )
        .await?;
        Ok(TenantModuleStateRecord {
            id,
            enabled: request.enabled,
            previous_enabled: !request.enabled,
            changed: true,
        })
    }

    pub async fn persist_settings<C: ConnectionTrait>(
        db: &C,
        request: TenantModuleSettingsRequest,
    ) -> Result<TenantModuleSettingsRecord, ModuleOperationStoreError> {
        let module_slug = request.module_slug.clone();
        let settings = request.settings.clone();
        let backend = db.get_database_backend();
        let select = match backend {
            DbBackend::Postgres => {
                "SELECT id, enabled FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
            }
            _ => {
                "SELECT id, enabled FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1"
            }
        };
        let existing = db
            .query_one(Statement::from_sql_and_values(
                backend,
                select,
                vec![request.tenant_id.into(), request.module_slug.clone().into()],
            ))
            .await
            .map_err(database_error)?;
        if let Some(row) = existing {
            if !request.is_core && !request.is_effectively_enabled {
                return Err(ModuleOperationStoreError::ModuleNotEnabled(
                    request.module_slug,
                ));
            }
            let id: Uuid = row.try_get("", "id").map_err(database_error)?;
            let previous_enabled: bool = row.try_get("", "enabled").map_err(database_error)?;
            let enabled = request.is_core || previous_enabled;
            execute(
                db,
                "UPDATE tenant_modules SET enabled = {1}, settings = {2}, updated_at = CURRENT_TIMESTAMP WHERE id = {3}",
                vec![enabled.into(), json_value(request.settings), id.into()],
            )
            .await?;
            return Ok(TenantModuleSettingsRecord {
                id,
                module_slug,
                enabled,
                settings,
            });
        }

        if !request.is_core && !request.is_effectively_enabled {
            return Err(ModuleOperationStoreError::ModuleNotEnabled(
                request.module_slug,
            ));
        }
        let id = rustok_core::generate_id();
        let enabled = request.is_core || request.is_effectively_enabled;
        execute(
            db,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings) VALUES ({1}, {2}, {3}, {4}, {5})",
            vec![
                id.into(),
                request.tenant_id.into(),
                request.module_slug.into(),
                enabled.into(),
                json_value(request.settings),
            ],
        )
        .await?;
        Ok(TenantModuleSettingsRecord {
            id,
            module_slug,
            enabled,
            settings,
        })
    }
}

fn json_value(value: serde_json::Value) -> sea_orm::Value {
    sea_orm::Value::Json(Some(Box::new(value)))
}

async fn execute<C: ConnectionTrait>(
    db: &C,
    sql_template: &str,
    values: Vec<sea_orm::Value>,
) -> Result<(), ModuleOperationStoreError> {
    let backend = db.get_database_backend();
    db.execute(Statement::from_sql_and_values(
        backend,
        render_parameters(sql_template, backend),
        values,
    ))
    .await
    .map_err(|error| ModuleOperationStoreError::Database(error.to_string()))?;
    Ok(())
}

fn render_parameters(sql_template: &str, backend: DbBackend) -> String {
    (1..=10).fold(sql_template.to_string(), |sql, index| {
        let parameter = match backend {
            DbBackend::Postgres => format!("${index}"),
            _ => format!("?{index}"),
        };
        sql.replace(format!("{{{index}}}").as_str(), parameter.as_str())
    })
}

#[cfg(test)]
mod tests {
    use sea_orm::{Database, Statement};
    use serde_json::json;

    use super::*;

    async fn database() -> sea_orm::DatabaseConnection {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE tenant_modules (\
                    id TEXT PRIMARY KEY NOT NULL, \
                    tenant_id TEXT NOT NULL, \
                    module_slug TEXT NOT NULL, \
                    enabled BOOLEAN NOT NULL, \
                    settings JSON NOT NULL, \
                    updated_at TEXT\
                 )"
                .to_string(),
            ))
            .await
            .expect("tenant modules table");
        database
    }

    async fn stored_settings(
        database: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> serde_json::Value {
        database
            .query_one(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT settings FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2",
                vec![tenant_id.into(), module_slug.into()],
            ))
            .await
            .expect("read settings")
            .expect("settings row")
            .try_get("", "settings")
            .expect("settings json")
    }

    #[tokio::test]
    async fn settings_persistence_enforces_effective_enablement_and_keeps_core_enabled() {
        let database = database().await;
        let tenant_id = Uuid::new_v4();
        let disabled = TenantModuleStateStore::persist_settings(
            &database,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: "optional_module".to_string(),
                settings: json!({ "value": 1 }),
                is_core: false,
                is_effectively_enabled: false,
            },
        )
        .await;
        assert!(matches!(
            disabled,
            Err(ModuleOperationStoreError::ModuleNotEnabled(module_slug))
                if module_slug == "optional_module"
        ));

        let core = TenantModuleStateStore::persist_settings(
            &database,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: "modules".to_string(),
                settings: json!({ "value": 2 }),
                is_core: true,
                is_effectively_enabled: false,
            },
        )
        .await
        .expect("core settings");
        assert_eq!(core.module_slug, "modules");
        assert!(core.enabled);
        assert_eq!(core.settings, json!({ "value": 2 }));

        let updated = TenantModuleStateStore::persist_settings(
            &database,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: "modules".to_string(),
                settings: json!({ "value": 3 }),
                is_core: true,
                is_effectively_enabled: false,
            },
        )
        .await
        .expect("updated core settings");
        assert_eq!(updated.id, core.id);
        assert_eq!(updated.module_slug, "modules");
        assert!(updated.enabled);
        assert_eq!(updated.settings, json!({ "value": 3 }));
    }
}
