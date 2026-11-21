use rustok_api::PortError;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use uuid::Uuid;

use rustok_core::{Error, Result};

use crate::forum_owner_contracts::{
    ForumProjectionOwnerRevisionImpact, ForumProjectionOwnerRevisionRequest,
    ForumProjectionOwnerTenantHead, ForumProjectionOwnerTenantPageRequest,
    MAX_FORUM_OWNER_TENANT_PAGE_LIMIT, SharedForumProjectionOwnerRevisionSourcePort,
    resolve_forum_projection_owner_revisions, resolve_forum_projection_owner_tenant_heads,
};
pub use crate::forum_owner_contracts::{
    ForumProjectionOwnerTenantHead, ForumProjectionOwnerTenantPageRequest,
    MAX_FORUM_OWNER_TENANT_PAGE_LIMIT, resolve_forum_projection_owner_tenant_heads,
};
use crate::projection_driver::SearchProjectionDriver;

const FORUM_SOURCE_MODULE: &str = "forum";
const FULL_SCOPE_KEY: &str = "forum";
const PROCESSING_LEASE_INTERVAL: &str = "1 hour";
const DELIVERY_COVERED_OUTCOME: &str = "delivery_covered";
const REBUILD_REPAIRED_OUTCOME: &str = "rebuild_repaired";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ForumOwnerCheckpointSweepReport {
    pub(crate) recovered_processing_events: usize,
    pub(crate) owner_tenants_scanned: usize,
    pub(crate) owner_tenants_reconciled: usize,
    pub(crate) owner_tenants_blocked: usize,
    pub(crate) owner_tenants_failed: usize,
    pub(crate) owner_revisions_checkpointed: usize,
    pub(crate) owner_rebuilds: usize,
}

#[derive(Clone)]
pub(crate) struct ForumOwnerCheckpointReconciler {
    db: DatabaseConnection,
    projection_driver: SearchProjectionDriver,
    owner_source: SharedForumProjectionOwnerRevisionSourcePort,
}

impl ForumOwnerCheckpointReconciler {
    pub(crate) fn new(
        db: DatabaseConnection,
        projection_driver: SearchProjectionDriver,
        owner_source: SharedForumProjectionOwnerRevisionSourcePort,
    ) -> Self {
        Self {
            db,
            projection_driver,
            owner_source,
        }
    }

    pub(crate) async fn sweep_due(
        &self,
        tenant_limit: usize,
        revision_limit: usize,
    ) -> Result<ForumOwnerCheckpointSweepReport> {
        let recovered_processing_events = self
            .recover_abandoned_processing(tenant_limit, revision_limit)
            .await? as usize;
        let mut active_cursor = self.load_scan_cursor().await?;
        let mut heads = self.list_tenant_heads(active_cursor, tenant_limit).await?;
        if heads.is_empty() && active_cursor.is_some() {
            if !self.store_scan_cursor(active_cursor, None).await? {
                return Ok(ForumOwnerCheckpointSweepReport {
                    recovered_processing_events,
                    ..ForumOwnerCheckpointSweepReport::default()
                });
            }
            active_cursor = None;
            heads = self.list_tenant_heads(active_cursor, tenant_limit).await?;
        }

        let mut report = ForumOwnerCheckpointSweepReport {
            recovered_processing_events,
            owner_tenants_scanned: heads.len(),
            ..ForumOwnerCheckpointSweepReport::default()
        };

        for head in &heads {
            match self.reconcile_tenant(*head, revision_limit).await {
                Ok(TenantCheckpointOutcome::CaughtUp) => {}
                Ok(TenantCheckpointOutcome::Blocked) => {
                    report.owner_tenants_blocked += 1;
                }
                Ok(TenantCheckpointOutcome::Advanced { revisions, rebuilt }) => {
                    report.owner_tenants_reconciled += 1;
                    report.owner_revisions_checkpointed += revisions;
                    if rebuilt {
                        report.owner_rebuilds += 1;
                    }
                }
                Err(error) => {
                    report.owner_tenants_failed += 1;
                    tracing::warn!(
                        tenant_id = %head.tenant_id,
                        latest_owner_revision = head.latest_owner_revision,
                        error = %error,
                        "Forum Search owner-revision reconciliation failed"
                    );
                }
            }
        }

        let next_cursor = if heads.len() < tenant_limit {
            None
        } else {
            heads.last().map(|head| head.tenant_id)
        };
        let updated = self.store_scan_cursor(active_cursor, next_cursor).await?;
        if !updated {
            tracing::debug!("Concurrent scan cursor update observed for forum projection owner");
        }
        Ok(report)
    }

    async fn list_tenant_heads(
        &self,
        after_tenant_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Vec<ForumProjectionOwnerTenantHead>> {
        resolve_forum_projection_owner_tenant_heads(
            Some(self.owner_source.clone()),
            ForumProjectionOwnerTenantPageRequest {
                after_tenant_id,
                limit,
            },
        )
        .await
        .map_err(map_owner_port_error)
    }

    async fn reconcile_tenant(
        &self,
        head: ForumProjectionOwnerTenantHead,
        revision_limit: usize,
    ) -> Result<TenantCheckpointOutcome> {
        let transaction = self.db.begin().await.map_err(Error::Database)?;
        if !try_acquire_tenant_lock(&transaction, head.tenant_id).await? {
            transaction.commit().await.map_err(Error::Database)?;
            return Ok(TenantCheckpointOutcome::Blocked);
        }

        let checkpoint = load_checkpoint(&transaction, head.tenant_id).await?;
        if checkpoint > head.latest_owner_revision {
            return Err(Error::External(
                "Forum owner checkpoint is ahead of the owner ledger head".to_string(),
            ));
        }
        if checkpoint == head.latest_owner_revision {
            transaction.commit().await.map_err(Error::Database)?;
            return Ok(TenantCheckpointOutcome::CaughtUp);
        }
        if has_non_terminal_inbox_work(&transaction, head.tenant_id).await? {
            transaction.commit().await.map_err(Error::Database)?;
            return Ok(TenantCheckpointOutcome::Blocked);
        }

        let revisions = resolve_forum_projection_owner_revisions(
            Some(self.owner_source.clone()),
            ForumProjectionOwnerRevisionRequest {
                tenant_id: head.tenant_id,
                after_owner_revision: checkpoint,
                limit: revision_limit,
            },
        )
        .await
        .map_err(map_owner_port_error)?;
        if revisions.is_empty() {
            return Err(Error::External(
                "Forum owner ledger head advanced but returned no revision page".to_string(),
            ));
        }

        let event_ids = revisions.iter().map(|r| r.event_id).collect::<Vec<_>>();
        let coverage_map =
            load_batch_delivery_coverage(&transaction, head.tenant_id, &event_ids).await?;

        let mut rebuild_required = false;
        for revision in &revisions {
            let coverage = coverage_map
                .get(&revision.event_id)
                .copied()
                .unwrap_or(DeliveryCoverage::Missing);
            match coverage {
                DeliveryCoverage::Covered => {}
                DeliveryCoverage::Missing => rebuild_required = true,
                DeliveryCoverage::Pending => {
                    transaction.commit().await.map_err(Error::Database)?;
                    return Ok(TenantCheckpointOutcome::Blocked);
                }
            }
        }

        if rebuild_required {
            self.projection_driver
                .rebuild_tenant(head.tenant_id)
                .await?;
        }

        let outcome = if rebuild_required {
            REBUILD_REPAIRED_OUTCOME
        } else {
            DELIVERY_COVERED_OUTCOME
        };
        let mut previous_revision = checkpoint;
        for revision in &revisions {
            if revision.impact != ForumProjectionOwnerRevisionImpact::FullRebuild {
                return Err(Error::External(
                    "Forum owner revision does not require projection reconciliation".to_string(),
                ));
            }
            advance_checkpoint(
                &transaction,
                head.tenant_id,
                previous_revision,
                revision.owner_revision,
                revision.event_id,
                outcome,
            )
            .await?;
            previous_revision = revision.owner_revision;
        }
        transaction.commit().await.map_err(Error::Database)?;

        Ok(TenantCheckpointOutcome::Advanced {
            revisions: revisions.len(),
            rebuilt: rebuild_required,
        })
    }

    async fn recover_abandoned_processing(
        &self,
        tenant_limit: usize,
        event_limit: usize,
    ) -> Result<u64> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                format!(
                    r#"
                    SELECT DISTINCT tenant_id
                    FROM search_projection_inbox
                    WHERE source_module = 'forum'
                      AND status = 'processing'
                      AND updated_at <= CURRENT_TIMESTAMP - INTERVAL '{PROCESSING_LEASE_INTERVAL}'
                    ORDER BY tenant_id ASC
                    LIMIT $1
                    "#
                ),
                vec![(tenant_limit as i64).into()],
            ))
            .await
            .map_err(Error::Database)?;

        let mut recovered = 0;
        for row in rows {
            let tenant_id: Uuid = row.try_get("", "tenant_id").map_err(Error::Database)?;
            let transaction = self.db.begin().await.map_err(Error::Database)?;
            if !try_acquire_tenant_lock(&transaction, tenant_id).await? {
                transaction.commit().await.map_err(Error::Database)?;
                continue;
            }
            let result = transaction
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    format!(
                        r#"
                        WITH abandoned AS (
                            SELECT event_id
                            FROM search_projection_inbox
                            WHERE tenant_id = $1
                              AND source_module = 'forum'
                              AND status = 'processing'
                              AND updated_at <= CURRENT_TIMESTAMP - INTERVAL '{PROCESSING_LEASE_INTERVAL}'
                            ORDER BY ingest_sequence ASC
                            LIMIT $2
                            FOR UPDATE
                        )
                        UPDATE search_projection_inbox AS inbox
                        SET status = 'retryable_error',
                            next_attempt_at = CURRENT_TIMESTAMP,
                            last_error = 'processing_lease_expired',
                            completed_at = NULL,
                            updated_at = CURRENT_TIMESTAMP
                        FROM abandoned
                        WHERE inbox.event_id = abandoned.event_id
                        "#
                    ),
                    vec![tenant_id.into(), (event_limit as i64).into()],
                ))
                .await
                .map_err(Error::Database)?;
            recovered += result.rows_affected();
            transaction.commit().await.map_err(Error::Database)?;
        }
        Ok(recovered)
    }

    async fn load_scan_cursor(&self) -> Result<Option<Uuid>> {
        let row = self
            .db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT after_tenant_id FROM search_projection_owner_scan_cursors WHERE source_module = 'forum'"
                    .to_string(),
            ))
            .await
            .map_err(Error::Database)?;
        match row {
            Some(row) => row
                .try_get::<Option<Uuid>>("", "after_tenant_id")
                .map_err(Error::Database),
            None => Ok(None),
        }
    }

    async fn store_scan_cursor(
        &self,
        expected_cursor: Option<Uuid>,
        next_cursor: Option<Uuid>,
    ) -> Result<bool> {
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                INSERT INTO search_projection_owner_scan_cursors (
                    source_module, after_tenant_id, updated_at
                ) VALUES ('forum', $1, CURRENT_TIMESTAMP)
                ON CONFLICT (source_module)
                DO UPDATE SET
                    after_tenant_id = EXCLUDED.after_tenant_id,
                    updated_at = CURRENT_TIMESTAMP
                WHERE search_projection_owner_scan_cursors.after_tenant_id
                      IS NOT DISTINCT FROM $2
                RETURNING source_module
                "#,
                vec![next_cursor.into(), expected_cursor.into()],
            ))
            .await
            .map_err(Error::Database)?;
        Ok(row.is_some())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TenantCheckpointOutcome {
    CaughtUp,
    Blocked,
    Advanced { revisions: usize, rebuilt: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeliveryCoverage {
    Covered,
    Missing,
    Pending,
}

async fn try_acquire_tenant_lock(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
) -> Result<bool> {
    let lock_key = format!("search:{FORUM_SOURCE_MODULE}:{tenant_id}:{FULL_SCOPE_KEY}");
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0)) AS acquired",
            vec![lock_key.into()],
        ))
        .await
        .map_err(Error::Database)?
        .ok_or_else(|| Error::External("PostgreSQL advisory lock returned no row".to_string()))?;
    row.try_get("", "acquired").map_err(Error::Database)
}

async fn load_checkpoint(transaction: &DatabaseTransaction, tenant_id: Uuid) -> Result<i64> {
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT owner_revision
            FROM search_projection_owner_checkpoints
            WHERE tenant_id = $1
              AND source_module = 'forum'
            FOR UPDATE
            "#,
            vec![tenant_id.into()],
        ))
        .await
        .map_err(Error::Database)?;
    let revision = row
        .map(|row| {
            row.try_get::<i64>("", "owner_revision")
                .map_err(Error::Database)
        })
        .transpose()?
        .unwrap_or(0);
    if revision < 0 {
        return Err(Error::External(
            "Forum owner checkpoint returned a negative revision".to_string(),
        ));
    }
    Ok(revision)
}

async fn has_non_terminal_inbox_work(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
) -> Result<bool> {
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM search_projection_inbox
                WHERE tenant_id = $1
                  AND source_module = 'forum'
                  AND status IN ('pending', 'processing', 'retryable_error')
            ) AS has_work
            "#,
            vec![tenant_id.into()],
        ))
        .await
        .map_err(Error::Database)?
        .ok_or_else(|| Error::External("Forum inbox work check returned no row".to_string()))?;
    row.try_get("", "has_work").map_err(Error::Database)
}

async fn load_batch_delivery_coverage(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    event_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, DeliveryCoverage>> {
    if event_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders = (2..=event_ids.len() + 1)
        .map(|i| format!("${i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut values: Vec<sea_orm::Value> = Vec::with_capacity(event_ids.len() + 1);
    values.push(tenant_id.into());
    for id in event_ids {
        values.push((*id).into());
    }
    let sql = format!(
        r#"
        SELECT event_id, status
        FROM search_projection_inbox
        WHERE tenant_id = $1
          AND source_module = 'forum'
          AND event_id IN ({placeholders})
        "#
    );
    let rows = transaction
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
        .await
        .map_err(Error::Database)?;

    let mut coverage_map = std::collections::HashMap::new();
    for row in rows {
        let event_id: Uuid = row.try_get("", "event_id").map_err(Error::Database)?;
        let status: String = row.try_get("", "status").map_err(Error::Database)?;
        let coverage = match status.as_str() {
            "completed" | "skipped" => DeliveryCoverage::Covered,
            "dead_letter" => DeliveryCoverage::Missing,
            "pending" | "processing" | "retryable_error" => DeliveryCoverage::Pending,
            other => {
                return Err(Error::External(format!(
                    "Forum projection inbox returned unsupported status `{other}`"
                )));
            }
        };
        coverage_map.insert(event_id, coverage);
    }
    Ok(coverage_map)
}

async fn advance_checkpoint(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    expected_previous_revision: i64,
    owner_revision: i64,
    event_id: Uuid,
    outcome: &str,
) -> Result<()> {
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO search_projection_owner_checkpoints (
                tenant_id, source_module, owner_revision, event_id, outcome, updated_at
            ) VALUES ($1, 'forum', $2, $3, $4, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, source_module)
            DO UPDATE SET
                owner_revision = EXCLUDED.owner_revision,
                event_id = EXCLUDED.event_id,
                outcome = EXCLUDED.outcome,
                updated_at = CURRENT_TIMESTAMP
            WHERE search_projection_owner_checkpoints.owner_revision = $5
            RETURNING owner_revision
            "#,
            vec![
                tenant_id.into(),
                owner_revision.into(),
                event_id.into(),
                outcome.to_string().into(),
                expected_previous_revision.into(),
            ],
        ))
        .await
        .map_err(Error::Database)?;
    let Some(row) = row else {
        return Err(Error::External(
            "Forum owner checkpoint did not advance from the expected revision".to_string(),
        ));
    };
    let stored_revision: i64 = row.try_get("", "owner_revision").map_err(Error::Database)?;
    if stored_revision != owner_revision {
        return Err(Error::External(
            "Forum owner checkpoint stored an unexpected revision".to_string(),
        ));
    }
    Ok(())
}

fn map_owner_port_error(error: PortError) -> Error {
    Error::External(format!(
        "Forum owner revision source failed with stable code `{}`",
        error.code
    ))
}
