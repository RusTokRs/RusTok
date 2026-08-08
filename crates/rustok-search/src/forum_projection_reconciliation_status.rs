use std::time::Instant;

use rustok_core::{Error, Result};
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    IsolationLevel, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::forum_reconciliation::{
    ForumProjectionOwnerRevisionRequest, SharedForumProjectionOwnerRevisionSourcePort,
    resolve_forum_projection_owner_revisions,
};

const FORUM_SOURCE_MODULE: &str = "forum";
const FORUM_SEARCH_RECONCILIATION_STATUS_OPERATION: &str =
    "search.forum_projection_reconciliation_status";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForumSearchProjectionDriftKind {
    CheckpointBehind,
    CheckpointAhead,
    CheckpointEventMismatch,
    NonTerminalInboxWork,
}

impl ForumSearchProjectionDriftKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CheckpointBehind => "checkpoint_behind",
            Self::CheckpointAhead => "checkpoint_ahead",
            Self::CheckpointEventMismatch => "checkpoint_event_mismatch",
            Self::NonTerminalInboxWork => "non_terminal_inbox_work",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumSearchProjectionDrift {
    pub kind: ForumSearchProjectionDriftKind,
    pub stored_revision: i64,
    pub expected_revision: i64,
    pub stored_event_id: Option<Uuid>,
    pub expected_event_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumSearchProjectionReconciliationStatus {
    pub tenant_id: Uuid,
    pub checkpoint_revision: i64,
    pub checkpoint_event_id: Option<Uuid>,
    pub checkpoint_outcome: Option<String>,
    pub owner_checkpoint_event_id: Option<Uuid>,
    pub next_owner_revision: Option<i64>,
    pub next_owner_event_id: Option<Uuid>,
    pub non_terminal_inbox_count: u64,
    pub drifts: Vec<ForumSearchProjectionDrift>,
}

impl ForumSearchProjectionReconciliationStatus {
    pub fn drift_count(&self) -> usize {
        self.drifts.len()
    }

    pub fn is_clean(&self) -> bool {
        self.drifts.is_empty()
    }
}

/// Read-only cross-owner FORUM-33 diagnostic for Forum -> Search projection convergence.
///
/// Forum remains authority for its projection revision ledger and is consulted only through the
/// existing neutral bounded owner-revision port. Search reads only its own checkpoint and inbox
/// persistence. The report never reads Forum-private tables, never rebuilds a projection and never
/// advances the durable Search checkpoint.
///
/// The Search side is one PostgreSQL `REPEATABLE READ READ ONLY` snapshot. Forum owner records are
/// independent public-owner observations, so this status is diagnostic evidence rather than a
/// cross-owner serializable repair fence.
pub struct ForumSearchProjectionReconciliationStatusService {
    db: DatabaseConnection,
    owner_source: SharedForumProjectionOwnerRevisionSourcePort,
}

impl ForumSearchProjectionReconciliationStatusService {
    pub fn new(
        db: DatabaseConnection,
        owner_source: SharedForumProjectionOwnerRevisionSourcePort,
    ) -> Self {
        Self { db, owner_source }
    }

    pub async fn report(
        &self,
        tenant_id: Uuid,
    ) -> Result<ForumSearchProjectionReconciliationStatus> {
        rustok_telemetry::metrics::record_module_entrypoint_call(
            "search",
            "forum_projection_reconciliation_status",
            "library",
        );
        let started_at = Instant::now();
        let result = self.report_inner(tenant_id).await;
        rustok_telemetry::metrics::record_span_duration(
            FORUM_SEARCH_RECONCILIATION_STATUS_OPERATION,
            started_at.elapsed().as_secs_f64(),
        );
        if result.is_err() {
            rustok_telemetry::metrics::record_span_error(
                FORUM_SEARCH_RECONCILIATION_STATUS_OPERATION,
                "owner_status",
            );
            rustok_telemetry::metrics::record_module_error(
                "search",
                "forum_projection_reconciliation_status",
                "error",
            );
        }
        result
    }

    async fn report_inner(
        &self,
        tenant_id: Uuid,
    ) -> Result<ForumSearchProjectionReconciliationStatus> {
        if tenant_id.is_nil() {
            return Err(Error::Validation(
                "Forum Search projection reconciliation requires a tenant".to_string(),
            ));
        }
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::External(
                "Forum Search projection owner checkpoints require PostgreSQL".to_string(),
            ));
        }

        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(Error::Database)?;
        let local = load_local_status(&transaction, tenant_id).await;
        let local = match local {
            Ok(local) => {
                transaction.commit().await.map_err(Error::Database)?;
                local
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                return Err(error);
            }
        };

        let owner_checkpoint_record = if local.checkpoint_revision > 0 {
            let after_owner_revision = local
                .checkpoint_revision
                .checked_sub(1)
                .ok_or_else(|| Error::External("Forum owner revision underflowed".to_string()))?;
            resolve_forum_projection_owner_revisions(
                Some(self.owner_source.clone()),
                ForumProjectionOwnerRevisionRequest {
                    tenant_id,
                    after_owner_revision,
                    limit: 1,
                },
            )
            .await
            .map_err(map_owner_port_error)?
            .into_iter()
            .next()
            .filter(|revision| revision.owner_revision == local.checkpoint_revision)
        } else {
            None
        };

        let next_owner_record = resolve_forum_projection_owner_revisions(
            Some(self.owner_source.clone()),
            ForumProjectionOwnerRevisionRequest {
                tenant_id,
                after_owner_revision: local.checkpoint_revision,
                limit: 1,
            },
        )
        .await
        .map_err(map_owner_port_error)?
        .into_iter()
        .next();

        let owner_checkpoint_event_id = owner_checkpoint_record
            .as_ref()
            .map(|revision| revision.event_id);
        let next_owner_revision = next_owner_record
            .as_ref()
            .map(|revision| revision.owner_revision);
        let next_owner_event_id = next_owner_record.as_ref().map(|revision| revision.event_id);

        let mut drifts = Vec::new();
        if let Some(next_revision) = next_owner_revision {
            drifts.push(ForumSearchProjectionDrift {
                kind: ForumSearchProjectionDriftKind::CheckpointBehind,
                stored_revision: local.checkpoint_revision,
                expected_revision: next_revision,
                stored_event_id: local.checkpoint_event_id,
                expected_event_id: next_owner_event_id,
            });
        } else if local.checkpoint_revision > 0 && owner_checkpoint_record.is_none() {
            drifts.push(ForumSearchProjectionDrift {
                kind: ForumSearchProjectionDriftKind::CheckpointAhead,
                stored_revision: local.checkpoint_revision,
                expected_revision: local.checkpoint_revision.saturating_sub(1),
                stored_event_id: local.checkpoint_event_id,
                expected_event_id: None,
            });
        }

        if local.checkpoint_revision > 0
            && owner_checkpoint_record.is_some()
            && owner_checkpoint_event_id != local.checkpoint_event_id
        {
            drifts.push(ForumSearchProjectionDrift {
                kind: ForumSearchProjectionDriftKind::CheckpointEventMismatch,
                stored_revision: local.checkpoint_revision,
                expected_revision: local.checkpoint_revision,
                stored_event_id: local.checkpoint_event_id,
                expected_event_id: owner_checkpoint_event_id,
            });
        }

        if local.non_terminal_inbox_count > 0 {
            drifts.push(ForumSearchProjectionDrift {
                kind: ForumSearchProjectionDriftKind::NonTerminalInboxWork,
                stored_revision: local.checkpoint_revision,
                expected_revision: next_owner_revision.unwrap_or(local.checkpoint_revision),
                stored_event_id: local.checkpoint_event_id,
                expected_event_id: next_owner_event_id.or(owner_checkpoint_event_id),
            });
        }

        Ok(ForumSearchProjectionReconciliationStatus {
            tenant_id,
            checkpoint_revision: local.checkpoint_revision,
            checkpoint_event_id: local.checkpoint_event_id,
            checkpoint_outcome: local.checkpoint_outcome,
            owner_checkpoint_event_id,
            next_owner_revision,
            next_owner_event_id,
            non_terminal_inbox_count: local.non_terminal_inbox_count,
            drifts,
        })
    }
}

#[derive(Debug)]
struct LocalForumProjectionStatus {
    checkpoint_revision: i64,
    checkpoint_event_id: Option<Uuid>,
    checkpoint_outcome: Option<String>,
    non_terminal_inbox_count: u64,
}

async fn load_local_status(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
) -> Result<LocalForumProjectionStatus> {
    let checkpoint = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT owner_revision, event_id, outcome
            FROM search_projection_owner_checkpoints
            WHERE tenant_id = $1
              AND source_module = 'forum'
            "#,
            vec![tenant_id.into()],
        ))
        .await
        .map_err(Error::Database)?;

    let (checkpoint_revision, checkpoint_event_id, checkpoint_outcome) = match checkpoint {
        Some(row) => {
            let revision: i64 = row
                .try_get("", "owner_revision")
                .map_err(Error::Database)?;
            if revision <= 0 {
                return Err(Error::External(
                    "Forum Search projection checkpoint must be positive".to_string(),
                ));
            }
            (
                revision,
                Some(row.try_get("", "event_id").map_err(Error::Database)?),
                Some(row.try_get("", "outcome").map_err(Error::Database)?),
            )
        }
        None => (0, None, None),
    };

    let inbox = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT COUNT(*)::BIGINT AS non_terminal_inbox_count
            FROM search_projection_inbox
            WHERE tenant_id = $1
              AND source_module = 'forum'
              AND status IN ('pending', 'processing', 'retryable_error')
            "#,
            vec![tenant_id.into()],
        ))
        .await
        .map_err(Error::Database)?
        .ok_or_else(|| Error::External("Forum Search inbox count returned no row".to_string()))?;
    let non_terminal_inbox_count: i64 = inbox
        .try_get("", "non_terminal_inbox_count")
        .map_err(Error::Database)?;
    if non_terminal_inbox_count < 0 {
        return Err(Error::External(
            "Forum Search inbox count must not be negative".to_string(),
        ));
    }

    Ok(LocalForumProjectionStatus {
        checkpoint_revision,
        checkpoint_event_id,
        checkpoint_outcome,
        non_terminal_inbox_count: non_terminal_inbox_count as u64,
    })
}

fn map_owner_port_error(error: rustok_api::PortError) -> Error {
    Error::External(format!(
        "Forum projection owner revision source failed [{}]: {}",
        error.code, error.message
    ))
}
