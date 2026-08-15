use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait, prelude::DateTimeWithTimeZone,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::entities::forum_topic_merge_operation;
use crate::error::{ForumError, ForumResult};

use super::rbac::enforce_scope;
use super::topic_route::ForumTopicRouteService;

pub const MAX_FORUM_TOPIC_MERGE_ROUTE_BACKFILL_OPERATIONS: u32 = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumTopicMergeRouteBackfillCursor {
    pub merged_at: DateTime<Utc>,
    pub operation_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackfillForumTopicMergeRouteAliasesInput {
    pub cursor: Option<ForumTopicMergeRouteBackfillCursor>,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumTopicMergeRouteBackfillResult {
    pub processed_operation_count: u32,
    pub ensured_alias_count: u32,
    pub next_cursor: Option<ForumTopicMergeRouteBackfillCursor>,
    pub exhausted: bool,
}

/// Bounded, resumable repair for merge receipts created before route aliases were composed.
///
/// New merges already write aliases in their owner transaction. This service scans the immutable
/// historical receipt order and delegates every route write to `ForumTopicRouteService`, so exact
/// replay verifies the existing alias payload while ownership or target drift fails closed.
pub struct ForumTopicMergeRouteBackfillService {
    db: DatabaseConnection,
}

impl ForumTopicMergeRouteBackfillService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security, input))]
    pub async fn backfill_merge_route_aliases(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: BackfillForumTopicMergeRouteAliasesInput,
    ) -> ForumResult<ForumTopicMergeRouteBackfillResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        validate_input(tenant_id, &input)?;

        let txn = self.db.begin().await?;
        let mut query = forum_topic_merge_operation::Entity::find()
            .filter(forum_topic_merge_operation::Column::TenantId.eq(tenant_id));

        if let Some(cursor) = input.cursor.as_ref() {
            let merged_at: DateTimeWithTimeZone = cursor.merged_at.fixed_offset();
            query = query.filter(
                Condition::any()
                    .add(forum_topic_merge_operation::Column::MergedAt.gt(merged_at))
                    .add(
                        Condition::all()
                            .add(forum_topic_merge_operation::Column::MergedAt.eq(merged_at))
                            .add(
                                forum_topic_merge_operation::Column::OperationId
                                    .gt(cursor.operation_id),
                            ),
                    ),
            );
        }

        let fetch_limit = u64::from(input.limit).checked_add(1).ok_or_else(|| {
            ForumError::Validation("Forum topic merge route backfill limit overflow".to_string())
        })?;
        let mut operations = query
            .order_by_asc(forum_topic_merge_operation::Column::MergedAt)
            .order_by_asc(forum_topic_merge_operation::Column::OperationId)
            .limit(fetch_limit)
            .all(&txn)
            .await?;
        let exhausted = operations.len() <= input.limit as usize;
        operations.truncate(input.limit as usize);

        let mut ensured_alias_count = 0_u32;
        for operation in &operations {
            let operation_alias_count =
                ForumTopicRouteService::record_merge_redirect_aliases_in_tx(
                    &txn,
                    tenant_id,
                    operation.source_topic_id,
                    operation.target_topic_id,
                    &operation.reason,
                )
                .await?;
            ensured_alias_count = ensured_alias_count
                .checked_add(operation_alias_count)
                .ok_or_else(|| {
                    ForumError::Validation(
                        "Forum topic merge route backfill alias count overflow".to_string(),
                    )
                })?;
        }

        let processed_operation_count = u32::try_from(operations.len()).map_err(|_| {
            ForumError::Validation(
                "Forum topic merge route backfill operation count overflow".to_string(),
            )
        })?;
        let next_cursor = if exhausted {
            None
        } else {
            operations.last().map(cursor_from_operation)
        };

        txn.commit().await?;
        Ok(ForumTopicMergeRouteBackfillResult {
            processed_operation_count,
            ensured_alias_count,
            next_cursor,
            exhausted,
        })
    }
}

fn validate_input(
    tenant_id: Uuid,
    input: &BackfillForumTopicMergeRouteAliasesInput,
) -> ForumResult<()> {
    if tenant_id.is_nil() {
        return Err(ForumError::Validation(
            "Forum topic merge route backfill tenant must not be nil".to_string(),
        ));
    }
    if input.limit == 0 || input.limit > MAX_FORUM_TOPIC_MERGE_ROUTE_BACKFILL_OPERATIONS {
        return Err(ForumError::Validation(format!(
            "Forum topic merge route backfill limit must be between 1 and {MAX_FORUM_TOPIC_MERGE_ROUTE_BACKFILL_OPERATIONS}"
        )));
    }
    if input
        .cursor
        .as_ref()
        .is_some_and(|cursor| cursor.operation_id.is_nil())
    {
        return Err(ForumError::Validation(
            "Forum topic merge route backfill cursor operation must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn cursor_from_operation(
    operation: &forum_topic_merge_operation::Model,
) -> ForumTopicMergeRouteBackfillCursor {
    ForumTopicMergeRouteBackfillCursor {
        merged_at: operation.merged_at.with_timezone(&Utc),
        operation_id: operation.operation_id,
    }
}
