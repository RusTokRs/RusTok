//! Garbage collection service for expired artifact data objects.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement, TransactionTrait,
};
use uuid::Uuid;

use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::objects_persistence::*;
use super::types::*;
use super::*;

/// Point-in-time set of artifact-data namespace instances protected by
/// active retention holds. It is deliberately just the held namespace
/// identities, not a general retention policy: which queued objects become
/// deletable, and when, is still derived per tenant by the GC service from
/// candidate queue timestamps and the fixed grace window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArtifactDataObjectLedgerRetentionSnapshot {
    held_namespace_instances: HashSet<Uuid>,
}

impl ArtifactDataObjectLedgerRetentionSnapshot {
    /// Number of namespace instances retained by this snapshot.
    pub fn held_namespace_count(&self) -> usize {
        self.held_namespace_instances.len()
    }
}

/// Deletes only object bytes that an owner transaction has explicitly marked
/// unreferenced and an external retention snapshot permits. It is tenant-scoped
/// so PostgreSQL RLS remains active during candidate discovery and completion.
///
/// Each deletion uses the durable order required for external effects:
///
/// 1. claim the candidate and commit that intent;
/// 2. delete the object bytes with no database transaction or namespace lock
///    held;
/// 3. delete exactly the claimed candidate revision in a completion
///    transaction.
///
/// A crash between those steps leaves a `deleting` candidate. Once its claim
/// lease expires, a later sweep performs the final snapshot-reference recheck
/// again, reclaims it, and retries the idempotent external delete.
#[derive(Clone)]
pub struct SeaOrmArtifactDataObjectGcService {
    db: DatabaseConnection,
    storage: StorageRuntime,
}

impl SeaOrmArtifactDataObjectGcService {
    pub fn new(db: DatabaseConnection, storage: StorageRuntime) -> Self {
        Self { db, storage }
    }

    pub async fn sweep_tenant(
        &self,
        tenant_id: Uuid,
        limit: u32,
        retention: &dyn ArtifactDataObjectRetentionPolicy,
    ) -> Result<ArtifactDataObjectGcResult, ArtifactDataError> {
        if tenant_id.is_nil() || limit == 0 || limit > MAX_ARTIFACT_OBJECT_GC_BATCH_SIZE {
            return Err(ArtifactDataError::InvalidObject);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let candidates = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT candidate.candidate_id, candidate.tenant_id, candidate.data_owner_id,
                            candidate.namespace_instance_id, candidate.policy_revision, candidate.storage_key,
                            namespace.module_slug, namespace.data_contract_revision, namespace.data_contract_digest
                     FROM module_artifact_data_object_gc_candidates candidate
                     JOIN module_artifact_data_namespaces namespace
                       ON namespace.tenant_id = candidate.tenant_id AND namespace.data_owner_id = candidate.data_owner_id
                      AND namespace.namespace_instance_id = candidate.namespace_instance_id
                     WHERE candidate.tenant_id = {} ORDER BY candidate.queued_at ASC, candidate.candidate_id ASC LIMIT {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                ),
                vec![backend.uuid_value(tenant_id), i64::from(limit).into()],
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;

        let mut result = ArtifactDataObjectGcResult::default();
        for row in candidates {
            let candidate = artifact_data_object_gc_candidate_from_row(row, backend)?;
            if !retention
                .may_delete(&candidate.scope, &candidate.storage_key)
                .await?
            {
                result.retained = result
                    .retained
                    .checked_add(1)
                    .ok_or(ArtifactDataError::Storage("GC result overflow".to_string()))?;
                continue;
            }

            let Some(claimed_revision) = self
                .claim_candidate(tenant_id, &candidate, Utc::now())
                .await?
            else {
                result.retained = result
                    .retained
                    .checked_add(1)
                    .ok_or(ArtifactDataError::Storage("GC result overflow".to_string()))?;
                continue;
            };

            // The claim transaction has committed and released the namespace
            // lock. Only now may the sweep cross the external storage
            // boundary; uncertainty leaves the durable claim for lease-based
            // retry instead of rolling back an already-deleted object.
            self.storage
                .objects
                .delete(&Path::from(candidate.storage_key.as_str()))
                .await
                .map_err(storage_error)?;

            self.complete_candidate(tenant_id, &candidate, claimed_revision)
                .await?;
            result.deleted = result
                .deleted
                .checked_add(1)
                .ok_or(ArtifactDataError::Storage("GC result overflow".to_string()))?;
        }
        Ok(result)
    }

    /// Lists tenants that currently hold queued object-GC candidates, for
    /// the platform maintenance worker that sweeps tenant-by-tenant. The
    /// scan is global (platform-operated maintenance needs the tenant
    /// inventory before any tenant scope exists) but returns only tenant
    /// identifiers; every subsequent candidate read happens inside the
    /// tenant-scoped sweep transaction. `limit` must be in
    /// `1..=MAX_ARTIFACT_OBJECT_GC_TENANT_SCAN`.
    pub async fn tenants_with_candidates(
        &self,
        limit: u32,
    ) -> Result<Vec<Uuid>, ArtifactDataError> {
        if limit == 0 || limit > MAX_ARTIFACT_OBJECT_GC_TENANT_SCAN {
            return Err(ArtifactDataError::InvalidObject);
        }
        // The candidates table is tenant-RLS-scoped on PostgreSQL; the
        // inventory scan runs under the transaction-local platform owner
        // scope (policy bypass) and returns tenant identifiers only.
        let transaction = self.db.begin().await.map_err(storage_error)?;
        crate::backend::set_platform_scope(&transaction)
            .await
            .map_err(storage_error)?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT DISTINCT tenant_id FROM module_artifact_data_object_gc_candidates \
                     ORDER BY tenant_id LIMIT {}",
                    backend.placeholder(1),
                ),
                vec![i64::from(limit).into()],
            ))
            .await
            .map_err(storage_error)?;
        let mut tenant_ids = Vec::with_capacity(rows.len());
        for row in rows {
            let tenant_id = uuid_from_row(&row, "tenant_id", backend)?;
            if tenant_id.is_nil() {
                return Err(ArtifactDataError::Storage(
                    "artifact data object GC candidate carries the nil tenant id".to_string(),
                ));
            }
            tenant_ids.push(tenant_id);
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(tenant_ids)
    }

    /// Loads the namespace instances protected by active data-object
    /// retention holds. Production callers that sweep several tenants in
    /// one maintenance pass load this snapshot once and reuse it for each
    /// tenant, rather than reloading and reparsing the whole durable hold
    /// ledger per tenant. A load failure is an error: callers must skip
    /// deletion for the pass instead of treating unknown hold state as no
    /// holds.
    pub async fn load_ledger_retention_snapshot(
        &self,
    ) -> Result<ArtifactDataObjectLedgerRetentionSnapshot, ArtifactDataError> {
        let hold_records = crate::RetentionHoldStore::list_active_holds(&self.db)
            .await
            .map_err(storage_error)?;
        let mut held_namespace_instances = HashSet::new();
        for record in hold_records {
            if let crate::RetentionTarget::ArtifactDataObject {
                namespace_instance_id,
                ..
            } = record.target
            {
                held_namespace_instances.insert(namespace_instance_id);
            }
        }
        Ok(ArtifactDataObjectLedgerRetentionSnapshot {
            held_namespace_instances,
        })
    }

    /// Convenience one-tenant production sweep that loads a fresh
    /// retention-hold snapshot for this call. Multi-tenant maintenance
    /// passes should use [`Self::load_ledger_retention_snapshot`] plus
    /// [`Self::sweep_tenant_with_ledger_retention_snapshot`] so the hold
    /// ledger is read once per pass, not once per tenant.
    pub async fn sweep_tenant_with_ledger_retention(
        &self,
        tenant_id: Uuid,
        limit: u32,
    ) -> Result<ArtifactDataObjectGcResult, ArtifactDataError> {
        let snapshot = self.load_ledger_retention_snapshot().await?;
        self.sweep_tenant_with_ledger_retention_snapshot(tenant_id, limit, &snapshot)
            .await
    }

    /// Production sweep: derives per-object retention from a loaded
    /// retention-hold snapshot and a fixed post-queue grace window, then
    /// runs the claim/complete pipeline of [`Self::sweep_tenant`].
    ///
    /// A candidate becomes deletable
    /// `ARTIFACT_DATA_OBJECT_GC_RETENTION_WINDOW_SECONDS` after it was
    /// queued (its bytes became unreferenced before queueing, so the
    /// window is pure operator/undo grace). Any namespace with an active
    /// data-object hold is retained wholesale for the pass: hold identity
    /// is object-scoped while queued candidates address stored bytes, and
    /// over-retaining a namespace under investigation is the safe failure
    /// direction. Candidates queued after the snapshot simply get no rule
    /// and are retained until the next pass.
    pub async fn sweep_tenant_with_ledger_retention_snapshot(
        &self,
        tenant_id: Uuid,
        limit: u32,
        retention: &ArtifactDataObjectLedgerRetentionSnapshot,
    ) -> Result<ArtifactDataObjectGcResult, ArtifactDataError> {
        if tenant_id.is_nil() || limit == 0 || limit > MAX_ARTIFACT_OBJECT_GC_BATCH_SIZE {
            return Err(ArtifactDataError::InvalidObject);
        }
        let held_namespace_instances = &retention.held_namespace_instances;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT candidate.storage_key AS storage_key, \
                     namespace.namespace_instance_id AS namespace_instance_id, \
                     candidate.queued_at AS queued_at \
                     FROM module_artifact_data_object_gc_candidates candidate \
                     JOIN module_artifact_data_namespaces namespace \
                       ON namespace.tenant_id = candidate.tenant_id \
                      AND namespace.data_owner_id = candidate.data_owner_id \
                      AND namespace.namespace_instance_id = candidate.namespace_instance_id \
                     WHERE candidate.tenant_id = {} \
                     ORDER BY candidate.queued_at, candidate.candidate_id \
                     LIMIT {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                ),
                vec![
                    backend.uuid_value(tenant_id),
                    i64::from(limit).into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;

        let mut rules: HashMap<String, ArtifactDataObjectRetentionRule> = HashMap::new();
        for row in rows {
            let namespace_instance_id =
                uuid_from_row(&row, "namespace_instance_id", backend)?;
            if held_namespace_instances.contains(&namespace_instance_id) {
                continue;
            }
            let storage_key: String = row.try_get("", "storage_key").map_err(storage_error)?;
            let queued_at = backend
                .datetime_from_row(&row, "queued_at")
                .map_err(storage_error)?;
            let Some(delete_after) = queued_at.checked_add_signed(Duration::seconds(
                ARTIFACT_DATA_OBJECT_GC_RETENTION_WINDOW_SECONDS,
            )) else {
                // An out-of-range eligibility instant retains the object;
                // the collector never guesses a deletion time.
                continue;
            };
            rules.insert(
                storage_key,
                ArtifactDataObjectRetentionRule {
                    delete_after,
                    legal_hold: false,
                    audit_hold: false,
                    rollback_hold: false,
                },
            );
        }

        let policy = SnapshotArtifactDataObjectRetentionPolicy::new(Utc::now(), rules);
        self.sweep_tenant(tenant_id, limit, &policy).await
    }

    async fn claim_candidate(
        &self,
        tenant_id: Uuid,
        candidate: &ArtifactDataObjectGcCandidate,
        claimed_at: DateTime<Utc>,
    ) -> Result<Option<u64>, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;

        transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT namespace_revision FROM module_artifact_data_namespaces
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}{}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    namespace_lock_clause(backend),
                ),
                namespace_values(&candidate.scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;

        let Some(candidate_row) = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT state, revision, delete_started_at
                     FROM module_artifact_data_object_gc_candidates
                     WHERE tenant_id = {} AND candidate_id = {} AND storage_key = {}{}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    namespace_lock_clause(backend),
                ),
                vec![
                    backend.uuid_value(tenant_id),
                    backend.uuid_value(candidate.candidate_id),
                    candidate.storage_key.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?
        else {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(None);
        };

        let state: String = candidate_row
            .try_get("", "state")
            .map_err(storage_error)?;
        let revision: i64 = candidate_row
            .try_get("", "revision")
            .map_err(storage_error)?;
        let revision = u64::try_from(revision).map_err(|_| {
            ArtifactDataError::Storage(
                "artifact data object GC candidate revision is outside the supported range"
                    .to_string(),
            )
        })?;
        if revision == 0 {
            return Err(ArtifactDataError::Storage(
                "artifact data object GC candidate revision must be positive".to_string(),
            ));
        }
        let delete_started_at = backend
            .optional_datetime_from_row(&candidate_row, "delete_started_at")
            .map_err(storage_error)?;

        match state.as_str() {
            "queued" => {
                if delete_started_at.is_some() {
                    return Err(ArtifactDataError::Storage(
                        "queued artifact data object GC candidate has a delete start timestamp"
                            .to_string(),
                    ));
                }
            }
            "deleting" => {
                let delete_started_at = delete_started_at.ok_or_else(|| {
                    ArtifactDataError::Storage(
                        "deleting artifact data object GC candidate has no delete start timestamp"
                            .to_string(),
                    )
                })?;
                let stale_at = delete_started_at
                    .checked_add_signed(Duration::seconds(
                        ARTIFACT_OBJECT_GC_CLAIM_LEASE_SECONDS,
                    ))
                    .ok_or_else(|| {
                        ArtifactDataError::Storage(
                            "artifact data object GC claim lease exceeds the supported timestamp range"
                                .to_string(),
                        )
                    })?;
                if claimed_at < stale_at {
                    transaction.commit().await.map_err(storage_error)?;
                    return Ok(None);
                }
            }
            other => {
                return Err(ArtifactDataError::Storage(format!(
                    "unknown artifact data object GC candidate state `{other}`"
                )));
            }
        }

        let snapshot_reference = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT COUNT(*) AS reference_count
                     FROM module_artifact_data_snapshot_objects snapshot_object
                     JOIN module_artifact_data_snapshots snapshot
                       ON snapshot.snapshot_id = snapshot_object.snapshot_id
                     WHERE snapshot_object.tenant_id = {}
                       AND snapshot_object.source_storage_key = {}
                       AND snapshot.status = 'staging'",
                    backend.placeholder(1),
                    backend.placeholder(2),
                ),
                vec![
                    backend.uuid_value(tenant_id),
                    candidate.storage_key.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                ArtifactDataError::Storage(
                    "snapshot reference query returned no row".to_string(),
                )
            })?;
        let reference_count: i64 = snapshot_reference
            .try_get("", "reference_count")
            .map_err(storage_error)?;
        if reference_count != 0 {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(None);
        }

        let claimed_revision = revision.checked_add(1).ok_or_else(|| {
            ArtifactDataError::Storage(
                "artifact data object GC candidate revision overflow".to_string(),
            )
        })?;
        let claimed = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_data_object_gc_candidates
                     SET state = 'deleting', revision = {}, delete_started_at = {}
                     WHERE tenant_id = {} AND candidate_id = {} AND storage_key = {}
                       AND state = {} AND revision = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                    backend.placeholder(7),
                ),
                vec![
                    revision_value(claimed_revision)?,
                    backend.datetime_value(claimed_at),
                    backend.uuid_value(tenant_id),
                    backend.uuid_value(candidate.candidate_id),
                    candidate.storage_key.clone().into(),
                    state.into(),
                    revision_value(revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if claimed.rows_affected() != 1 {
            return Err(ArtifactDataError::Storage(
                "artifact data object GC candidate changed while its collection claim was recorded"
                    .to_string(),
            ));
        }

        transaction.commit().await.map_err(storage_error)?;
        Ok(Some(claimed_revision))
    }

    async fn complete_candidate(
        &self,
        tenant_id: Uuid,
        candidate: &ArtifactDataObjectGcCandidate,
        claimed_revision: u64,
    ) -> Result<(), ArtifactDataError> {
        let transaction: DatabaseTransaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let deleted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "DELETE FROM module_artifact_data_object_gc_candidates
                     WHERE tenant_id = {} AND candidate_id = {} AND storage_key = {}
                       AND state = 'deleting' AND revision = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                ),
                vec![
                    backend.uuid_value(tenant_id),
                    backend.uuid_value(candidate.candidate_id),
                    candidate.storage_key.clone().into(),
                    revision_value(claimed_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if deleted.rows_affected() != 1 {
            return Err(ArtifactDataError::Storage(
                "artifact data object GC candidate changed after its claimed external delete; completion was not recorded"
                    .to_string(),
            ));
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }
}

struct ArtifactDataObjectGcCandidate {
    candidate_id: Uuid,
    scope: ArtifactDataScope,
    storage_key: String,
}

fn artifact_data_object_gc_candidate_from_row(
    row: sea_orm::QueryResult,
    backend: SupportedBackend,
) -> Result<ArtifactDataObjectGcCandidate, ArtifactDataError> {
    let data_contract_revision: i64 = row
        .try_get("", "data_contract_revision")
        .map_err(storage_error)?;
    let policy_revision: i64 = row.try_get("", "policy_revision").map_err(storage_error)?;
    let scope = ArtifactDataScope {
        tenant_id: uuid_from_row(&row, "tenant_id", backend)?,
        data_owner_id: uuid_from_row(&row, "data_owner_id", backend)?,
        namespace_instance_id: uuid_from_row(&row, "namespace_instance_id", backend)?,
        data_contract_digest: row
            .try_get("", "data_contract_digest")
            .map_err(storage_error)?,
        module_slug: row.try_get("", "module_slug").map_err(storage_error)?,
        data_contract_revision: u64::try_from(data_contract_revision)
            .map_err(|_| ArtifactDataError::InvalidObject)?,
        policy_revision: u64::try_from(policy_revision)
            .map_err(|_| ArtifactDataError::InvalidObject)?,
    };
    scope.validate()?;
    Ok(ArtifactDataObjectGcCandidate {
        candidate_id: uuid_from_row(&row, "candidate_id", backend)?,
        scope,
        storage_key: row.try_get("", "storage_key").map_err(storage_error)?,
    })
}
