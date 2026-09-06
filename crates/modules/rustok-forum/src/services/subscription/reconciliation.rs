use std::time::Instant;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    IsolationLevel, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::services::{
    DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT, MAX_FORUM_COUNTER_RECONCILIATION_LIMIT,
};

const FORUM_SUBSCRIPTION_RECONCILIATION_OPERATION: &str =
    "forum.subscription_reconciliation_report";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForumSubscriptionTargetKind {
    Topic,
    Category,
}

impl ForumSubscriptionTargetKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Topic => "topic",
            Self::Category => "category",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForumSubscriptionDriftKind {
    TargetMissing,
    MergedTopicSourceSubscription,
    MutedPreferencesInvalid,
    RevisionInvalid,
}

impl ForumSubscriptionDriftKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TargetMissing => "target_missing",
            Self::MergedTopicSourceSubscription => "merged_topic_source_subscription",
            Self::MutedPreferencesInvalid => "muted_preferences_invalid",
            Self::RevisionInvalid => "revision_invalid",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumSubscriptionCursor {
    pub target_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumSubscriptionDrift {
    pub kind: ForumSubscriptionDriftKind,
    pub target_kind: ForumSubscriptionTargetKind,
    pub target_id: Uuid,
    pub user_id: Uuid,
    pub stored: i64,
    pub expected: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumSubscriptionReconciliationReport {
    pub requested_limit: Option<u64>,
    pub effective_limit: u64,
    pub inspected_topic_subscriptions: u64,
    pub inspected_category_subscriptions: u64,
    pub has_more_topic_subscriptions: bool,
    pub has_more_category_subscriptions: bool,
    pub topic_cursor: Option<ForumSubscriptionCursor>,
    pub category_cursor: Option<ForumSubscriptionCursor>,
    pub drifts: Vec<ForumSubscriptionDrift>,
}

impl ForumSubscriptionReconciliationReport {
    pub fn drift_count(&self) -> usize {
        self.drifts.len()
    }

    pub fn is_clean(&self) -> bool {
        self.drifts.is_empty()
    }
}

/// Read-only FORUM-33 reconciliation for persisted Forum subscription owner rows.
///
/// Topic and category subscriptions are independent shapes and use composite `(target_id, user_id)`
/// keyset cursors because neither table has a single row UUID. The report checks only invariants
/// already owned by Forum: target referential integrity, the schema-enforced muted preference shape,
/// positive optimistic revisions, and source-topic rows that remain after an immutable topic merge.
/// It deliberately does not infer missing subscriptions from participation policy and does not read
/// Profiles/Notifications-owned user or delivery state.
///
/// Every page is read-only and snapshot-consistent. PostgreSQL uses `REPEATABLE READ READ ONLY`;
/// SQLite uses one transaction whose first read establishes the page snapshot. Multi-page scans are
/// page-local diagnostics, not a serializable repair fence.
pub struct ForumSubscriptionReconciliationService {
    db: DatabaseConnection,
}

impl ForumSubscriptionReconciliationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn report_page(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        requested_limit: Option<u64>,
        topic_after_target: Option<Uuid>,
        topic_after_user: Option<Uuid>,
        category_after_target: Option<Uuid>,
        category_after_user: Option<Uuid>,
    ) -> ForumResult<ForumSubscriptionReconciliationReport> {
        rustok_telemetry::metrics::record_module_entrypoint_call(
            "forum",
            "subscription_reconciliation_report",
            "library",
        );
        let started_at = Instant::now();
        let result = match enforce_operations_scope(security) {
            Ok(()) => {
                self.report_inner(
                    tenant_id,
                    requested_limit,
                    topic_after_target,
                    topic_after_user,
                    category_after_target,
                    category_after_user,
                )
                .await
            }
            Err(error) => Err(error),
        };
        rustok_telemetry::metrics::record_span_duration(
            FORUM_SUBSCRIPTION_RECONCILIATION_OPERATION,
            started_at.elapsed().as_secs_f64(),
        );
        if result.is_err() {
            rustok_telemetry::metrics::record_span_error(
                FORUM_SUBSCRIPTION_RECONCILIATION_OPERATION,
                "owner_report",
            );
            rustok_telemetry::metrics::record_module_error(
                "forum",
                "subscription_reconciliation",
                "error",
            );
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    async fn report_inner(
        &self,
        tenant_id: Uuid,
        requested_limit: Option<u64>,
        topic_after_target: Option<Uuid>,
        topic_after_user: Option<Uuid>,
        category_after_target: Option<Uuid>,
        category_after_user: Option<Uuid>,
    ) -> ForumResult<ForumSubscriptionReconciliationReport> {
        let topic_after =
            subscription_cursor(topic_after_target, topic_after_user, "topic subscription")?;
        let category_after = subscription_cursor(
            category_after_target,
            category_after_user,
            "category subscription",
        )?;

        let backend = self.db.get_database_backend();
        let transaction = match backend {
            DatabaseBackend::Postgres => {
                self.db
                    .begin_with_config(
                        Some(IsolationLevel::RepeatableRead),
                        Some(AccessMode::ReadOnly),
                    )
                    .await?
            }
            DatabaseBackend::Sqlite => self.db.begin().await?,
            other => {
                return Err(ForumError::Validation(format!(
                    "Forum subscription reconciliation does not support database backend {other:?}"
                )));
            }
        };

        let report = self
            .report_in_transaction(
                &transaction,
                backend,
                tenant_id,
                requested_limit,
                topic_after,
                category_after,
            )
            .await;
        match report {
            Ok(report) => {
                transaction.commit().await?;
                Ok(report)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }

    async fn report_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        backend: DatabaseBackend,
        tenant_id: Uuid,
        requested_limit: Option<u64>,
        topic_after: Option<ForumSubscriptionCursor>,
        category_after: Option<ForumSubscriptionCursor>,
    ) -> ForumResult<ForumSubscriptionReconciliationReport> {
        let effective_limit = requested_limit
            .unwrap_or(DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT)
            .clamp(1, MAX_FORUM_COUNTER_RECONCILIATION_LIMIT);
        let fetch_limit = effective_limit.saturating_add(1);

        let topic_rows = transaction
            .query_all_raw(subscription_statement(
                backend,
                TOPIC_SUBSCRIPTIONS_SQLITE,
                TOPIC_SUBSCRIPTIONS_AFTER_SQLITE,
                TOPIC_SUBSCRIPTIONS_POSTGRES,
                TOPIC_SUBSCRIPTIONS_AFTER_POSTGRES,
                tenant_id,
                topic_after,
                fetch_limit,
            )?)
            .await?;
        let category_rows = transaction
            .query_all_raw(subscription_statement(
                backend,
                CATEGORY_SUBSCRIPTIONS_SQLITE,
                CATEGORY_SUBSCRIPTIONS_AFTER_SQLITE,
                CATEGORY_SUBSCRIPTIONS_POSTGRES,
                CATEGORY_SUBSCRIPTIONS_AFTER_POSTGRES,
                tenant_id,
                category_after,
                fetch_limit,
            )?)
            .await?;

        let has_more_topic_subscriptions = topic_rows.len() > effective_limit as usize;
        let has_more_category_subscriptions = category_rows.len() > effective_limit as usize;
        let mut topic_cursor = topic_after;
        let mut category_cursor = category_after;
        let mut drifts = Vec::new();

        for row in topic_rows.iter().take(effective_limit as usize) {
            let target_id: Uuid = row.try_get("", "target_id")?;
            let user_id: Uuid = row.try_get("", "user_id")?;
            topic_cursor = Some(ForumSubscriptionCursor { target_id, user_id });
            collect_common_drifts(
                row,
                ForumSubscriptionTargetKind::Topic,
                target_id,
                user_id,
                &mut drifts,
            )?;
            let merged_source: i64 = row.try_get("", "merged_source")?;
            if merged_source != 0 {
                drifts.push(ForumSubscriptionDrift {
                    kind: ForumSubscriptionDriftKind::MergedTopicSourceSubscription,
                    target_kind: ForumSubscriptionTargetKind::Topic,
                    target_id,
                    user_id,
                    stored: 1,
                    expected: 0,
                });
            }
        }

        for row in category_rows.iter().take(effective_limit as usize) {
            let target_id: Uuid = row.try_get("", "target_id")?;
            let user_id: Uuid = row.try_get("", "user_id")?;
            category_cursor = Some(ForumSubscriptionCursor { target_id, user_id });
            collect_common_drifts(
                row,
                ForumSubscriptionTargetKind::Category,
                target_id,
                user_id,
                &mut drifts,
            )?;
        }

        Ok(ForumSubscriptionReconciliationReport {
            requested_limit,
            effective_limit,
            inspected_topic_subscriptions: topic_rows.len().min(effective_limit as usize) as u64,
            inspected_category_subscriptions: category_rows.len().min(effective_limit as usize)
                as u64,
            has_more_topic_subscriptions,
            has_more_category_subscriptions,
            topic_cursor,
            category_cursor,
            drifts,
        })
    }
}

fn collect_common_drifts(
    row: &sea_orm::QueryResult,
    target_kind: ForumSubscriptionTargetKind,
    target_id: Uuid,
    user_id: Uuid,
    drifts: &mut Vec<ForumSubscriptionDrift>,
) -> ForumResult<()> {
    let target_exists: i64 = row.try_get("", "target_exists")?;
    if target_exists != 1 {
        drifts.push(ForumSubscriptionDrift {
            kind: ForumSubscriptionDriftKind::TargetMissing,
            target_kind,
            target_id,
            user_id,
            stored: target_exists,
            expected: 1,
        });
    }

    let muted_preferences_valid: i64 = row.try_get("", "muted_preferences_valid")?;
    if muted_preferences_valid != 1 {
        drifts.push(ForumSubscriptionDrift {
            kind: ForumSubscriptionDriftKind::MutedPreferencesInvalid,
            target_kind,
            target_id,
            user_id,
            stored: muted_preferences_valid,
            expected: 1,
        });
    }

    let revision: i64 = row.try_get("", "revision")?;
    if revision <= 0 {
        drifts.push(ForumSubscriptionDrift {
            kind: ForumSubscriptionDriftKind::RevisionInvalid,
            target_kind,
            target_id,
            user_id,
            stored: revision,
            expected: 1,
        });
    }
    Ok(())
}

fn enforce_operations_scope(security: &SecurityContext) -> ForumResult<()> {
    enforce_scope(security, Resource::ForumCategories, Action::Manage)?;
    enforce_scope(security, Resource::ForumTopics, Action::Manage)
}

fn subscription_cursor(
    target_after: Option<Uuid>,
    user_after: Option<Uuid>,
    label: &str,
) -> ForumResult<Option<ForumSubscriptionCursor>> {
    match (target_after, user_after) {
        (None, None) => Ok(None),
        (Some(target_id), Some(user_id)) => {
            Ok(Some(ForumSubscriptionCursor { target_id, user_id }))
        }
        _ => Err(ForumError::Validation(format!(
            "Forum {label} cursor requires both target and user components"
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
fn subscription_statement(
    backend: DatabaseBackend,
    initial_sqlite: &str,
    after_sqlite: &str,
    initial_postgres: &str,
    after_postgres: &str,
    tenant_id: Uuid,
    after: Option<ForumSubscriptionCursor>,
    limit: u64,
) -> ForumResult<Statement> {
    match (backend, after) {
        (DatabaseBackend::Sqlite, None) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            initial_sqlite,
            vec![tenant_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Sqlite, Some(after)) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            after_sqlite,
            vec![
                tenant_id.into(),
                after.target_id.into(),
                after.user_id.into(),
                (limit as i64).into(),
            ],
        )),
        (DatabaseBackend::Postgres, None) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            initial_postgres,
            vec![tenant_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Postgres, Some(after)) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            after_postgres,
            vec![
                tenant_id.into(),
                after.target_id.into(),
                after.user_id.into(),
                (limit as i64).into(),
            ],
        )),
        (other, _) => Err(ForumError::Validation(format!(
            "Forum subscription reconciliation does not support database backend {other:?}"
        ))),
    }
}

const TOPIC_SUBSCRIPTIONS_SQLITE: &str = r#"
SELECT
    s.topic_id AS target_id,
    s.user_id AS user_id,
    CASE WHEN t.id IS NULL THEN 0 ELSE 1 END AS target_exists,
    CASE WHEN EXISTS (
        SELECT 1
        FROM forum_topic_merge_operations merge_operation
        WHERE merge_operation.tenant_id = s.tenant_id
          AND merge_operation.source_topic_id = s.topic_id
    ) THEN 1 ELSE 0 END AS merged_source,
    CASE WHEN s.level <> 'muted' OR (
        NOT s.notify_mentions
        AND NOT s.notify_replies
        AND NOT s.notify_new_topics
        AND s.digest_mode = 'disabled'
    ) THEN 1 ELSE 0 END AS muted_preferences_valid,
    CAST(s.revision AS INTEGER) AS revision
FROM forum_topic_subscriptions s
LEFT JOIN forum_topics t
    ON t.tenant_id = s.tenant_id
   AND t.id = s.topic_id
WHERE s.tenant_id = ?1
ORDER BY s.topic_id, s.user_id
LIMIT ?2
"#;

const TOPIC_SUBSCRIPTIONS_AFTER_SQLITE: &str = r#"
SELECT
    s.topic_id AS target_id,
    s.user_id AS user_id,
    CASE WHEN t.id IS NULL THEN 0 ELSE 1 END AS target_exists,
    CASE WHEN EXISTS (
        SELECT 1
        FROM forum_topic_merge_operations merge_operation
        WHERE merge_operation.tenant_id = s.tenant_id
          AND merge_operation.source_topic_id = s.topic_id
    ) THEN 1 ELSE 0 END AS merged_source,
    CASE WHEN s.level <> 'muted' OR (
        NOT s.notify_mentions
        AND NOT s.notify_replies
        AND NOT s.notify_new_topics
        AND s.digest_mode = 'disabled'
    ) THEN 1 ELSE 0 END AS muted_preferences_valid,
    CAST(s.revision AS INTEGER) AS revision
FROM forum_topic_subscriptions s
LEFT JOIN forum_topics t
    ON t.tenant_id = s.tenant_id
   AND t.id = s.topic_id
WHERE s.tenant_id = ?1
  AND (s.topic_id > ?2 OR (s.topic_id = ?2 AND s.user_id > ?3))
ORDER BY s.topic_id, s.user_id
LIMIT ?4
"#;

const TOPIC_SUBSCRIPTIONS_POSTGRES: &str = r#"
SELECT
    s.topic_id AS target_id,
    s.user_id AS user_id,
    CASE WHEN t.id IS NULL THEN 0::BIGINT ELSE 1::BIGINT END AS target_exists,
    CASE WHEN EXISTS (
        SELECT 1
        FROM forum_topic_merge_operations merge_operation
        WHERE merge_operation.tenant_id = s.tenant_id
          AND merge_operation.source_topic_id = s.topic_id
    ) THEN 1::BIGINT ELSE 0::BIGINT END AS merged_source,
    CASE WHEN s.level <> 'muted' OR (
        NOT s.notify_mentions
        AND NOT s.notify_replies
        AND NOT s.notify_new_topics
        AND s.digest_mode = 'disabled'
    ) THEN 1::BIGINT ELSE 0::BIGINT END AS muted_preferences_valid,
    s.revision::BIGINT AS revision
FROM forum_topic_subscriptions s
LEFT JOIN forum_topics t
    ON t.tenant_id = s.tenant_id
   AND t.id = s.topic_id
WHERE s.tenant_id = $1
ORDER BY s.topic_id, s.user_id
LIMIT $2
"#;

const TOPIC_SUBSCRIPTIONS_AFTER_POSTGRES: &str = r#"
SELECT
    s.topic_id AS target_id,
    s.user_id AS user_id,
    CASE WHEN t.id IS NULL THEN 0::BIGINT ELSE 1::BIGINT END AS target_exists,
    CASE WHEN EXISTS (
        SELECT 1
        FROM forum_topic_merge_operations merge_operation
        WHERE merge_operation.tenant_id = s.tenant_id
          AND merge_operation.source_topic_id = s.topic_id
    ) THEN 1::BIGINT ELSE 0::BIGINT END AS merged_source,
    CASE WHEN s.level <> 'muted' OR (
        NOT s.notify_mentions
        AND NOT s.notify_replies
        AND NOT s.notify_new_topics
        AND s.digest_mode = 'disabled'
    ) THEN 1::BIGINT ELSE 0::BIGINT END AS muted_preferences_valid,
    s.revision::BIGINT AS revision
FROM forum_topic_subscriptions s
LEFT JOIN forum_topics t
    ON t.tenant_id = s.tenant_id
   AND t.id = s.topic_id
WHERE s.tenant_id = $1
  AND (s.topic_id > $2 OR (s.topic_id = $2 AND s.user_id > $3))
ORDER BY s.topic_id, s.user_id
LIMIT $4
"#;

const CATEGORY_SUBSCRIPTIONS_SQLITE: &str = r#"
SELECT
    s.category_id AS target_id,
    s.user_id AS user_id,
    CASE WHEN c.id IS NULL THEN 0 ELSE 1 END AS target_exists,
    CASE WHEN s.level <> 'muted' OR (
        NOT s.notify_mentions
        AND NOT s.notify_replies
        AND NOT s.notify_new_topics
        AND s.digest_mode = 'disabled'
    ) THEN 1 ELSE 0 END AS muted_preferences_valid,
    CAST(s.revision AS INTEGER) AS revision
FROM forum_category_subscriptions s
LEFT JOIN forum_categories c
    ON c.tenant_id = s.tenant_id
   AND c.id = s.category_id
WHERE s.tenant_id = ?1
ORDER BY s.category_id, s.user_id
LIMIT ?2
"#;

const CATEGORY_SUBSCRIPTIONS_AFTER_SQLITE: &str = r#"
SELECT
    s.category_id AS target_id,
    s.user_id AS user_id,
    CASE WHEN c.id IS NULL THEN 0 ELSE 1 END AS target_exists,
    CASE WHEN s.level <> 'muted' OR (
        NOT s.notify_mentions
        AND NOT s.notify_replies
        AND NOT s.notify_new_topics
        AND s.digest_mode = 'disabled'
    ) THEN 1 ELSE 0 END AS muted_preferences_valid,
    CAST(s.revision AS INTEGER) AS revision
FROM forum_category_subscriptions s
LEFT JOIN forum_categories c
    ON c.tenant_id = s.tenant_id
   AND c.id = s.category_id
WHERE s.tenant_id = ?1
  AND (s.category_id > ?2 OR (s.category_id = ?2 AND s.user_id > ?3))
ORDER BY s.category_id, s.user_id
LIMIT ?4
"#;

const CATEGORY_SUBSCRIPTIONS_POSTGRES: &str = r#"
SELECT
    s.category_id AS target_id,
    s.user_id AS user_id,
    CASE WHEN c.id IS NULL THEN 0::BIGINT ELSE 1::BIGINT END AS target_exists,
    CASE WHEN s.level <> 'muted' OR (
        NOT s.notify_mentions
        AND NOT s.notify_replies
        AND NOT s.notify_new_topics
        AND s.digest_mode = 'disabled'
    ) THEN 1::BIGINT ELSE 0::BIGINT END AS muted_preferences_valid,
    s.revision::BIGINT AS revision
FROM forum_category_subscriptions s
LEFT JOIN forum_categories c
    ON c.tenant_id = s.tenant_id
   AND c.id = s.category_id
WHERE s.tenant_id = $1
ORDER BY s.category_id, s.user_id
LIMIT $2
"#;

const CATEGORY_SUBSCRIPTIONS_AFTER_POSTGRES: &str = r#"
SELECT
    s.category_id AS target_id,
    s.user_id AS user_id,
    CASE WHEN c.id IS NULL THEN 0::BIGINT ELSE 1::BIGINT END AS target_exists,
    CASE WHEN s.level <> 'muted' OR (
        NOT s.notify_mentions
        AND NOT s.notify_replies
        AND NOT s.notify_new_topics
        AND s.digest_mode = 'disabled'
    ) THEN 1::BIGINT ELSE 0::BIGINT END AS muted_preferences_valid,
    s.revision::BIGINT AS revision
FROM forum_category_subscriptions s
LEFT JOIN forum_categories c
    ON c.tenant_id = s.tenant_id
   AND c.id = s.category_id
WHERE s.tenant_id = $1
  AND (s.category_id > $2 OR (s.category_id = $2 AND s.user_id > $3))
ORDER BY s.category_id, s.user_id
LIMIT $4
"#;
