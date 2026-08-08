use std::sync::Arc;
use std::time::Instant;

use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_effective_permission,
};
use rustok_core::{Error, ModuleRuntimeExtensions};
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    IsolationLevel, Statement, TransactionTrait,
};
use uuid::Uuid;

use crate::{
    ForumProjectionOwnerRevisionRequest, SharedForumProjectionOwnerRevisionSourcePort,
    resolve_forum_projection_owner_revisions,
};

const SEARCH_MODULE_SLUG: &str = "search";
const FORUM_MODULE_SLUG: &str = "forum";
const FORUM_SEARCH_RECONCILIATION_STATUS_OPERATION: &str =
    "search.forum_projection_reconciliation_status";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForumSearchProjectionDriftKind {
    CheckpointBehind,
    CheckpointAhead,
    CheckpointEventMismatch,
    NonTerminalInboxWork,
}

impl ForumSearchProjectionDriftKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CheckpointBehind => "checkpoint_behind",
            Self::CheckpointAhead => "checkpoint_ahead",
            Self::CheckpointEventMismatch => "checkpoint_event_mismatch",
            Self::NonTerminalInboxWork => "non_terminal_inbox_work",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForumSearchProjectionDrift {
    kind: ForumSearchProjectionDriftKind,
    stored_revision: i64,
    expected_revision: i64,
    stored_event_id: Option<Uuid>,
    expected_event_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForumSearchProjectionReconciliationStatus {
    checkpoint_revision: i64,
    checkpoint_event_id: Option<Uuid>,
    checkpoint_outcome: Option<String>,
    owner_checkpoint_event_id: Option<Uuid>,
    next_owner_revision: Option<i64>,
    next_owner_event_id: Option<Uuid>,
    non_terminal_inbox_count: u64,
    drifts: Vec<ForumSearchProjectionDrift>,
}

impl ForumSearchProjectionReconciliationStatus {
    fn is_clean(&self) -> bool {
        self.drifts.is_empty()
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumSearchProjectionDrift {
    pub kind: String,
    pub stored_revision: String,
    pub expected_revision: String,
    pub stored_event_id: Option<Uuid>,
    pub expected_event_id: Option<Uuid>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumSearchProjectionReconciliationStatus {
    pub checkpoint_revision: String,
    pub checkpoint_event_id: Option<Uuid>,
    pub checkpoint_outcome: Option<String>,
    pub owner_checkpoint_event_id: Option<Uuid>,
    pub next_owner_revision: Option<String>,
    pub next_owner_event_id: Option<Uuid>,
    pub non_terminal_inbox_count: i32,
    pub drift_count: i32,
    pub clean: bool,
    pub drifts: Vec<GqlForumSearchProjectionDrift>,
}

#[derive(Default)]
pub struct ForumSearchProjectionReconciliationQuery;

#[Object]
impl ForumSearchProjectionReconciliationQuery {
    /// Read-only FORUM-33 cross-owner convergence status for the current tenant's Forum Search
    /// projection. Forum owner revisions are consumed only through the neutral bounded port;
    /// Search reads only its own checkpoint/inbox state and performs no repair.
    async fn forum_search_projection_reconciliation_status(
        &self,
        ctx: &Context<'_>,
    ) -> Result<GqlForumSearchProjectionReconciliationStatus> {
        require_module_enabled(ctx, SEARCH_MODULE_SLUG).await?;
        require_module_enabled(ctx, FORUM_MODULE_SLUG).await?;

        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        if auth.tenant_id != tenant.id {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Search reconciliation access is denied",
            ));
        }
        require_reconciliation_permissions(auth)?;

        let db = ctx.data::<DatabaseConnection>()?.clone();
        let extensions = ctx.data::<Arc<ModuleRuntimeExtensions>>()?;
        let owner_source = extensions
            .get::<SharedForumProjectionOwnerRevisionSourcePort>()
            .cloned()
            .ok_or_else(|| FieldError::new("Forum Search projection reconciliation is unavailable"))?;

        let status = ForumSearchProjectionReconciliationStatusService::new(db, owner_source)
            .report(tenant.id)
            .await
            .map_err(|error| <FieldError as GraphQLError>::internal_error(&error.to_string()))?;
        Ok(map_status(status))
    }
}

fn require_reconciliation_permissions(auth: &AuthContext) -> Result<()> {
    let settings_read = has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ);
    let categories_manage = has_effective_permission(
        &auth.permissions,
        &Permission::FORUM_CATEGORIES_MANAGE,
    );
    let topics_manage =
        has_effective_permission(&auth.permissions, &Permission::FORUM_TOPICS_MANAGE);
    if settings_read && categories_manage && topics_manage {
        Ok(())
    } else {
        Err(<FieldError as GraphQLError>::permission_denied(
            "settings:read, forum_categories:manage and forum_topics:manage required",
        ))
    }
}

fn map_status(
    status: ForumSearchProjectionReconciliationStatus,
) -> GqlForumSearchProjectionReconciliationStatus {
    GqlForumSearchProjectionReconciliationStatus {
        checkpoint_revision: status.checkpoint_revision.to_string(),
        checkpoint_event_id: status.checkpoint_event_id,
        checkpoint_outcome: status.checkpoint_outcome,
        owner_checkpoint_event_id: status.owner_checkpoint_event_id,
        next_owner_revision: status.next_owner_revision.map(|value| value.to_string()),
        next_owner_event_id: status.next_owner_event_id,
        non_terminal_inbox_count: status
            .non_terminal_inbox_count
            .min(i32::MAX as u64) as i32,
        drift_count: status.drifts.len().min(i32::MAX as usize) as i32,
        clean: status.is_clean(),
        drifts: status.drifts.into_iter().map(map_drift).collect(),
    }
}

fn map_drift(drift: ForumSearchProjectionDrift) -> GqlForumSearchProjectionDrift {
    GqlForumSearchProjectionDrift {
        kind: drift.kind.as_str().to_string(),
        stored_revision: drift.stored_revision.to_string(),
        expected_revision: drift.expected_revision.to_string(),
        stored_event_id: drift.stored_event_id,
        expected_event_id: drift.expected_event_id,
    }
}

struct ForumSearchProjectionReconciliationStatusService {
    db: DatabaseConnection,
    owner_source: SharedForumProjectionOwnerRevisionSourcePort,
}

impl ForumSearchProjectionReconciliationStatusService {
    fn new(
        db: DatabaseConnection,
        owner_source: SharedForumProjectionOwnerRevisionSourcePort,
    ) -> Self {
        Self { db, owner_source }
    }

    async fn report(
        &self,
        tenant_id: Uuid,
    ) -> rustok_core::Result<ForumSearchProjectionReconciliationStatus> {
        rustok_telemetry::metrics::record_module_entrypoint_call(
            "search",
            "forum_projection_reconciliation_status",
            "graphql",
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
                "status_report",
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
    ) -> rustok_core::Result<ForumSearchProjectionReconciliationStatus> {
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
) -> rustok_core::Result<LocalForumProjectionStatus> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_revision_fields_use_strings() {
        let drift = ForumSearchProjectionDrift {
            kind: ForumSearchProjectionDriftKind::CheckpointBehind,
            stored_revision: i64::MAX - 1,
            expected_revision: i64::MAX,
            stored_event_id: None,
            expected_event_id: None,
        };
        let mapped = map_drift(drift);
        assert_eq!(mapped.stored_revision, (i64::MAX - 1).to_string());
        assert_eq!(mapped.expected_revision, i64::MAX.to_string());
    }
}
