use std::time::Instant;

use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    IsolationLevel, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

pub const DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT: u64 = 100;
pub const MAX_FORUM_COUNTER_RECONCILIATION_LIMIT: u64 = 500;
const FORUM_COUNTER_RECONCILIATION_OPERATION: &str = "forum.counter_reconciliation_report";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForumCounterDriftKind {
    TopicReplyCount,
    CategoryTopicCount,
    CategoryReplyCount,
}

impl ForumCounterDriftKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TopicReplyCount => "topic_reply_count",
            Self::CategoryTopicCount => "category_topic_count",
            Self::CategoryReplyCount => "category_reply_count",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumCounterDrift {
    pub kind: ForumCounterDriftKind,
    pub subject_id: Uuid,
    pub stored: i64,
    pub expected: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumCounterReconciliationReport {
    pub requested_limit: Option<u64>,
    pub effective_limit: u64,
    pub inspected_topics: u64,
    pub inspected_categories: u64,
    pub has_more_topics: bool,
    pub has_more_categories: bool,
    pub drifts: Vec<ForumCounterDrift>,
}

impl ForumCounterReconciliationReport {
    pub fn drift_count(&self) -> usize {
        self.drifts.len()
    }

    pub fn is_clean(&self) -> bool {
        self.drifts.is_empty()
    }
}

/// Read-only Forum owner reconciliation for denormalized category/topic counters.
///
/// The report deliberately performs no repair. A future write path must add the FORUM-33
/// requirements for operator RBAC, dry-run, audit and durable idempotent job state before it can
/// mutate any owner counter. This service is tenant-scoped and bounded independently for topics and
/// categories so an operator request cannot turn into an unbounded table scan.
///
/// Both aggregate reads are fenced by one database snapshot. PostgreSQL uses `REPEATABLE READ`
/// with `READ ONLY`; SQLite uses one ordinary transaction whose first read establishes the snapshot.
pub struct ForumCounterReconciliationService {
    db: DatabaseConnection,
}

impl ForumCounterReconciliationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn report(
        &self,
        tenant_id: Uuid,
        requested_limit: Option<u64>,
    ) -> ForumResult<ForumCounterReconciliationReport> {
        rustok_telemetry::metrics::record_module_entrypoint_call(
            "forum",
            "counter_reconciliation_report",
            "library",
        );
        let started_at = Instant::now();
        let result = self.report_inner(tenant_id, requested_limit).await;
        rustok_telemetry::metrics::record_span_duration(
            FORUM_COUNTER_RECONCILIATION_OPERATION,
            started_at.elapsed().as_secs_f64(),
        );
        if result.is_err() {
            rustok_telemetry::metrics::record_span_error(
                FORUM_COUNTER_RECONCILIATION_OPERATION,
                "owner_report",
            );
            rustok_telemetry::metrics::record_module_error(
                "forum",
                "counter_reconciliation",
                "error",
            );
        }
        result
    }

    async fn report_inner(
        &self,
        tenant_id: Uuid,
        requested_limit: Option<u64>,
    ) -> ForumResult<ForumCounterReconciliationReport> {
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
                    "Forum counter reconciliation does not support database backend {other:?}"
                )));
            }
        };

        let report = self
            .report_in_transaction(&transaction, backend, tenant_id, requested_limit)
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
    ) -> ForumResult<ForumCounterReconciliationReport> {
        let effective_limit = requested_limit
            .unwrap_or(DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT)
            .clamp(1, MAX_FORUM_COUNTER_RECONCILIATION_LIMIT);
        let fetch_limit = effective_limit.saturating_add(1);
        let topic_rows = transaction
            .query_all(counter_statement(
                backend,
                TOPIC_COUNTER_SQLITE,
                TOPIC_COUNTER_POSTGRES,
                tenant_id,
                fetch_limit,
            )?)
            .await?;
        let category_rows = transaction
            .query_all(counter_statement(
                backend,
                CATEGORY_COUNTER_SQLITE,
                CATEGORY_COUNTER_POSTGRES,
                tenant_id,
                fetch_limit,
            )?)
            .await?;

        let has_more_topics = topic_rows.len() > effective_limit as usize;
        let has_more_categories = category_rows.len() > effective_limit as usize;
        let mut drifts = Vec::new();

        for row in topic_rows.iter().take(effective_limit as usize) {
            let subject_id: Uuid = row.try_get("", "id")?;
            let stored: i64 = row.try_get("", "stored_reply_count")?;
            let expected: i64 = row.try_get("", "expected_reply_count")?;
            if stored != expected {
                drifts.push(ForumCounterDrift {
                    kind: ForumCounterDriftKind::TopicReplyCount,
                    subject_id,
                    stored,
                    expected,
                });
            }
        }

        for row in category_rows.iter().take(effective_limit as usize) {
            let subject_id: Uuid = row.try_get("", "id")?;
            let stored_topics: i64 = row.try_get("", "stored_topic_count")?;
            let expected_topics: i64 = row.try_get("", "expected_topic_count")?;
            let stored_replies: i64 = row.try_get("", "stored_reply_count")?;
            let expected_replies: i64 = row.try_get("", "expected_reply_count")?;
            if stored_topics != expected_topics {
                drifts.push(ForumCounterDrift {
                    kind: ForumCounterDriftKind::CategoryTopicCount,
                    subject_id,
                    stored: stored_topics,
                    expected: expected_topics,
                });
            }
            if stored_replies != expected_replies {
                drifts.push(ForumCounterDrift {
                    kind: ForumCounterDriftKind::CategoryReplyCount,
                    subject_id,
                    stored: stored_replies,
                    expected: expected_replies,
                });
            }
        }

        Ok(ForumCounterReconciliationReport {
            requested_limit,
            effective_limit,
            inspected_topics: topic_rows.len().min(effective_limit as usize) as u64,
            inspected_categories: category_rows.len().min(effective_limit as usize) as u64,
            has_more_topics,
            has_more_categories,
            drifts,
        })
    }
}

fn counter_statement(
    backend: DatabaseBackend,
    sqlite_sql: &str,
    postgres_sql: &str,
    tenant_id: Uuid,
    limit: u64,
) -> ForumResult<Statement> {
    let values = vec![tenant_id.into(), (limit as i64).into()];
    match backend {
        DatabaseBackend::Sqlite => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sqlite_sql,
            values,
        )),
        DatabaseBackend::Postgres => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            postgres_sql,
            values,
        )),
        other => Err(ForumError::Validation(format!(
            "Forum counter reconciliation does not support database backend {other:?}"
        ))),
    }
}

const TOPIC_COUNTER_SQLITE: &str = r#"
SELECT
    t.id AS id,
    CAST(t.reply_count AS INTEGER) AS stored_reply_count,
    CAST(COUNT(r.id) AS INTEGER) AS expected_reply_count
FROM forum_topics t
LEFT JOIN forum_replies r
    ON r.tenant_id = t.tenant_id
   AND r.topic_id = t.id
   AND r.status = 'approved'
WHERE t.tenant_id = ?1
GROUP BY t.id, t.reply_count
ORDER BY t.id
LIMIT ?2
"#;

const TOPIC_COUNTER_POSTGRES: &str = r#"
SELECT
    t.id AS id,
    t.reply_count::BIGINT AS stored_reply_count,
    COUNT(r.id)::BIGINT AS expected_reply_count
FROM forum_topics t
LEFT JOIN forum_replies r
    ON r.tenant_id = t.tenant_id
   AND r.topic_id = t.id
   AND r.status = 'approved'
WHERE t.tenant_id = $1
GROUP BY t.id, t.reply_count
ORDER BY t.id
LIMIT $2
"#;

const CATEGORY_COUNTER_SQLITE: &str = r#"
SELECT
    c.id AS id,
    CAST(c.topic_count AS INTEGER) AS stored_topic_count,
    CAST(c.reply_count AS INTEGER) AS stored_reply_count,
    CAST(COUNT(DISTINCT t.id) AS INTEGER) AS expected_topic_count,
    CAST(COALESCE(SUM(CASE WHEN r.status = 'approved' THEN 1 ELSE 0 END), 0) AS INTEGER)
        AS expected_reply_count
FROM forum_categories c
LEFT JOIN forum_topics t
    ON t.tenant_id = c.tenant_id
   AND t.category_id = c.id
LEFT JOIN forum_replies r
    ON r.tenant_id = t.tenant_id
   AND r.topic_id = t.id
WHERE c.tenant_id = ?1
GROUP BY c.id, c.topic_count, c.reply_count
ORDER BY c.id
LIMIT ?2
"#;

const CATEGORY_COUNTER_POSTGRES: &str = r#"
SELECT
    c.id AS id,
    c.topic_count::BIGINT AS stored_topic_count,
    c.reply_count::BIGINT AS stored_reply_count,
    COUNT(DISTINCT t.id)::BIGINT AS expected_topic_count,
    COALESCE(SUM(CASE WHEN r.status = 'approved' THEN 1 ELSE 0 END), 0)::BIGINT
        AS expected_reply_count
FROM forum_categories c
LEFT JOIN forum_topics t
    ON t.tenant_id = c.tenant_id
   AND t.category_id = c.id
LEFT JOIN forum_replies r
    ON r.tenant_id = t.tenant_id
   AND r.topic_id = t.id
WHERE c.tenant_id = $1
GROUP BY c.id, c.topic_count, c.reply_count
ORDER BY c.id
LIMIT $2
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconciliation_limit_is_bounded() {
        assert_eq!(
            None.unwrap_or(DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT),
            DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT
        );
        assert_eq!(
            Some(10_000_u64)
                .unwrap_or(DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT)
                .clamp(1, MAX_FORUM_COUNTER_RECONCILIATION_LIMIT),
            MAX_FORUM_COUNTER_RECONCILIATION_LIMIT
        );
    }

    #[test]
    fn drift_kind_names_are_stable() {
        assert_eq!(
            ForumCounterDriftKind::TopicReplyCount.as_str(),
            "topic_reply_count"
        );
        assert_eq!(
            ForumCounterDriftKind::CategoryTopicCount.as_str(),
            "category_topic_count"
        );
        assert_eq!(
            ForumCounterDriftKind::CategoryReplyCount.as_str(),
            "category_reply_count"
        );
    }
}
