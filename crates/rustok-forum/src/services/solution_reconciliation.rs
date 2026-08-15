use std::{collections::HashSet, time::Instant};

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    IsolationLevel, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    counter_reconciliation::{
        DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT, MAX_FORUM_COUNTER_RECONCILIATION_LIMIT,
    },
    rbac::enforce_scope,
};
use crate::error::{ForumError, ForumResult};

const FORUM_SOLUTION_RECONCILIATION_OPERATION: &str = "forum.solution_reconciliation_report";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForumSolutionDriftKind {
    AcceptedReplyEligibility,
    SolutionAuthorStatMissing,
    SolutionAuthorStatCount,
}

impl ForumSolutionDriftKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AcceptedReplyEligibility => "accepted_reply_eligibility",
            Self::SolutionAuthorStatMissing => "solution_author_stat_missing",
            Self::SolutionAuthorStatCount => "solution_author_stat_count",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumSolutionDrift {
    pub kind: ForumSolutionDriftKind,
    pub subject_id: Uuid,
    pub stored: i64,
    pub expected: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumSolutionReconciliationReport {
    pub requested_limit: Option<u64>,
    pub effective_limit: u64,
    pub inspected_solutions: u64,
    pub inspected_solution_stats: u64,
    pub has_more_solutions: bool,
    pub has_more_solution_stats: bool,
    pub solution_cursor: Option<Uuid>,
    pub solution_stat_cursor: Option<Uuid>,
    pub drifts: Vec<ForumSolutionDrift>,
}

impl ForumSolutionReconciliationReport {
    pub fn drift_count(&self) -> usize {
        self.drifts.len()
    }

    pub fn is_clean(&self) -> bool {
        self.drifts.is_empty()
    }
}

/// Read-only reconciliation for Forum accepted-solution authority and its user-stat projection.
///
/// The source of truth is `forum_solutions` plus the exact same-tenant/topic reply relation. A
/// solution is operationally eligible only while its reply is `approved`; non-public reply states
/// are not accepted solutions. `forum_user_stats.solution_count` is treated only as a projection of
/// approved solution rows authored by that user.
///
/// Solution rows and solution-stat rows have independent UUID keyset cursors. Every page is fenced
/// by one database snapshot, but a multi-page scan intentionally does not keep a transaction open
/// across requests. This diagnostic service performs no repair.
pub struct ForumSolutionReconciliationService {
    db: DatabaseConnection,
}

impl ForumSolutionReconciliationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn report_page(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        requested_limit: Option<u64>,
        solution_after: Option<Uuid>,
        solution_stat_after: Option<Uuid>,
    ) -> ForumResult<ForumSolutionReconciliationReport> {
        rustok_telemetry::metrics::record_module_entrypoint_call(
            "forum",
            "solution_reconciliation_report",
            "library",
        );
        let started_at = Instant::now();
        let result = match enforce_operations_scope(security) {
            Ok(()) => {
                self.report_inner(
                    tenant_id,
                    requested_limit,
                    solution_after,
                    solution_stat_after,
                )
                .await
            }
            Err(error) => Err(error),
        };
        rustok_telemetry::metrics::record_span_duration(
            FORUM_SOLUTION_RECONCILIATION_OPERATION,
            started_at.elapsed().as_secs_f64(),
        );
        if result.is_err() {
            rustok_telemetry::metrics::record_span_error(
                FORUM_SOLUTION_RECONCILIATION_OPERATION,
                "owner_report",
            );
            rustok_telemetry::metrics::record_module_error(
                "forum",
                "solution_reconciliation",
                "error",
            );
        }
        result
    }

    async fn report_inner(
        &self,
        tenant_id: Uuid,
        requested_limit: Option<u64>,
        solution_after: Option<Uuid>,
        solution_stat_after: Option<Uuid>,
    ) -> ForumResult<ForumSolutionReconciliationReport> {
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
                    "Forum solution reconciliation does not support database backend {other:?}"
                )));
            }
        };

        let report = self
            .report_in_transaction(
                &transaction,
                backend,
                tenant_id,
                requested_limit,
                solution_after,
                solution_stat_after,
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
        solution_after: Option<Uuid>,
        solution_stat_after: Option<Uuid>,
    ) -> ForumResult<ForumSolutionReconciliationReport> {
        let effective_limit = requested_limit
            .unwrap_or(DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT)
            .clamp(1, MAX_FORUM_COUNTER_RECONCILIATION_LIMIT);
        let fetch_limit = effective_limit.saturating_add(1);

        let solution_rows = transaction
            .query_all(solution_statement(
                backend,
                tenant_id,
                solution_after,
                fetch_limit,
            )?)
            .await?;
        let stat_rows = transaction
            .query_all(solution_stat_statement(
                backend,
                tenant_id,
                solution_stat_after,
                fetch_limit,
            )?)
            .await?;

        let has_more_solutions = solution_rows.len() > effective_limit as usize;
        let has_more_solution_stats = stat_rows.len() > effective_limit as usize;
        let mut solution_cursor = solution_after;
        let mut solution_stat_cursor = solution_stat_after;
        let mut drifts = Vec::new();
        let mut missing_stat_authors = HashSet::new();

        for row in solution_rows.iter().take(effective_limit as usize) {
            let topic_id: Uuid = row.try_get("", "id")?;
            solution_cursor = Some(topic_id);
            let eligible: i64 = row.try_get("", "eligible")?;
            if eligible != 1 {
                drifts.push(ForumSolutionDrift {
                    kind: ForumSolutionDriftKind::AcceptedReplyEligibility,
                    subject_id: topic_id,
                    stored: 1,
                    expected: eligible,
                });
            }

            let author_id: Option<Uuid> = row.try_get("", "author_id")?;
            let author_stat_exists: i64 = row.try_get("", "author_stat_exists")?;
            if eligible == 1
                && author_stat_exists == 0
                && let Some(author_id) = author_id.filter(|id| missing_stat_authors.insert(*id))
            {
                drifts.push(ForumSolutionDrift {
                    kind: ForumSolutionDriftKind::SolutionAuthorStatMissing,
                    subject_id: author_id,
                    stored: 0,
                    expected: 1,
                });
            }
        }

        for row in stat_rows.iter().take(effective_limit as usize) {
            let user_id: Uuid = row.try_get("", "id")?;
            solution_stat_cursor = Some(user_id);
            let stored: i64 = row.try_get("", "stored_solution_count")?;
            let expected: i64 = row.try_get("", "expected_solution_count")?;
            if stored != expected {
                drifts.push(ForumSolutionDrift {
                    kind: ForumSolutionDriftKind::SolutionAuthorStatCount,
                    subject_id: user_id,
                    stored,
                    expected,
                });
            }
        }

        Ok(ForumSolutionReconciliationReport {
            requested_limit,
            effective_limit,
            inspected_solutions: solution_rows.len().min(effective_limit as usize) as u64,
            inspected_solution_stats: stat_rows.len().min(effective_limit as usize) as u64,
            has_more_solutions,
            has_more_solution_stats,
            solution_cursor,
            solution_stat_cursor,
            drifts,
        })
    }
}

fn enforce_operations_scope(security: &SecurityContext) -> ForumResult<()> {
    enforce_scope(security, Resource::ForumCategories, Action::Manage)?;
    enforce_scope(security, Resource::ForumTopics, Action::Manage)
}

fn solution_statement(
    backend: DatabaseBackend,
    tenant_id: Uuid,
    after: Option<Uuid>,
    limit: u64,
) -> ForumResult<Statement> {
    match (backend, after) {
        (DatabaseBackend::Sqlite, None) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            SOLUTION_SQLITE,
            vec![tenant_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Sqlite, Some(after)) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            SOLUTION_AFTER_SQLITE,
            vec![tenant_id.into(), after.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Postgres, None) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            SOLUTION_POSTGRES,
            vec![tenant_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Postgres, Some(after)) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            SOLUTION_AFTER_POSTGRES,
            vec![tenant_id.into(), after.into(), (limit as i64).into()],
        )),
        (other, _) => Err(ForumError::Validation(format!(
            "Forum solution reconciliation does not support database backend {other:?}"
        ))),
    }
}

fn solution_stat_statement(
    backend: DatabaseBackend,
    tenant_id: Uuid,
    after: Option<Uuid>,
    limit: u64,
) -> ForumResult<Statement> {
    match (backend, after) {
        (DatabaseBackend::Sqlite, None) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            SOLUTION_STAT_SQLITE,
            vec![tenant_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Sqlite, Some(after)) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            SOLUTION_STAT_AFTER_SQLITE,
            vec![tenant_id.into(), after.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Postgres, None) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            SOLUTION_STAT_POSTGRES,
            vec![tenant_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Postgres, Some(after)) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            SOLUTION_STAT_AFTER_POSTGRES,
            vec![tenant_id.into(), after.into(), (limit as i64).into()],
        )),
        (other, _) => Err(ForumError::Validation(format!(
            "Forum solution reconciliation does not support database backend {other:?}"
        ))),
    }
}

const SOLUTION_SQLITE: &str = r#"
SELECT
    s.topic_id AS id,
    CASE WHEN t.id IS NOT NULL AND r.id IS NOT NULL AND r.status = 'approved' THEN 1 ELSE 0 END
        AS eligible,
    r.author_id AS author_id,
    CASE WHEN r.author_id IS NULL OR us.user_id IS NOT NULL THEN 1 ELSE 0 END AS author_stat_exists
FROM forum_solutions s
LEFT JOIN forum_topics t
    ON t.tenant_id = s.tenant_id
   AND t.id = s.topic_id
LEFT JOIN forum_replies r
    ON r.tenant_id = s.tenant_id
   AND r.topic_id = s.topic_id
   AND r.id = s.reply_id
LEFT JOIN forum_user_stats us
    ON us.tenant_id = s.tenant_id
   AND us.user_id = r.author_id
WHERE s.tenant_id = ?1
ORDER BY s.topic_id
LIMIT ?2
"#;

const SOLUTION_AFTER_SQLITE: &str = r#"
SELECT
    s.topic_id AS id,
    CASE WHEN t.id IS NOT NULL AND r.id IS NOT NULL AND r.status = 'approved' THEN 1 ELSE 0 END
        AS eligible,
    r.author_id AS author_id,
    CASE WHEN r.author_id IS NULL OR us.user_id IS NOT NULL THEN 1 ELSE 0 END AS author_stat_exists
FROM forum_solutions s
LEFT JOIN forum_topics t
    ON t.tenant_id = s.tenant_id
   AND t.id = s.topic_id
LEFT JOIN forum_replies r
    ON r.tenant_id = s.tenant_id
   AND r.topic_id = s.topic_id
   AND r.id = s.reply_id
LEFT JOIN forum_user_stats us
    ON us.tenant_id = s.tenant_id
   AND us.user_id = r.author_id
WHERE s.tenant_id = ?1
  AND s.topic_id > ?2
ORDER BY s.topic_id
LIMIT ?3
"#;

const SOLUTION_POSTGRES: &str = r#"
SELECT
    s.topic_id AS id,
    CASE WHEN t.id IS NOT NULL AND r.id IS NOT NULL AND r.status = 'approved' THEN 1::BIGINT ELSE 0::BIGINT END
        AS eligible,
    r.author_id AS author_id,
    CASE WHEN r.author_id IS NULL OR us.user_id IS NOT NULL THEN 1::BIGINT ELSE 0::BIGINT END
        AS author_stat_exists
FROM forum_solutions s
LEFT JOIN forum_topics t
    ON t.tenant_id = s.tenant_id
   AND t.id = s.topic_id
LEFT JOIN forum_replies r
    ON r.tenant_id = s.tenant_id
   AND r.topic_id = s.topic_id
   AND r.id = s.reply_id
LEFT JOIN forum_user_stats us
    ON us.tenant_id = s.tenant_id
   AND us.user_id = r.author_id
WHERE s.tenant_id = $1
ORDER BY s.topic_id
LIMIT $2
"#;

const SOLUTION_AFTER_POSTGRES: &str = r#"
SELECT
    s.topic_id AS id,
    CASE WHEN t.id IS NOT NULL AND r.id IS NOT NULL AND r.status = 'approved' THEN 1::BIGINT ELSE 0::BIGINT END
        AS eligible,
    r.author_id AS author_id,
    CASE WHEN r.author_id IS NULL OR us.user_id IS NOT NULL THEN 1::BIGINT ELSE 0::BIGINT END
        AS author_stat_exists
FROM forum_solutions s
LEFT JOIN forum_topics t
    ON t.tenant_id = s.tenant_id
   AND t.id = s.topic_id
LEFT JOIN forum_replies r
    ON r.tenant_id = s.tenant_id
   AND r.topic_id = s.topic_id
   AND r.id = s.reply_id
LEFT JOIN forum_user_stats us
    ON us.tenant_id = s.tenant_id
   AND us.user_id = r.author_id
WHERE s.tenant_id = $1
  AND s.topic_id > $2
ORDER BY s.topic_id
LIMIT $3
"#;

const SOLUTION_STAT_SQLITE: &str = r#"
WITH bounded_stats AS (
    SELECT user_id, solution_count
    FROM forum_user_stats
    WHERE tenant_id = ?1
    ORDER BY user_id
    LIMIT ?2
), expected_stats AS (
    SELECT r.author_id AS user_id, COUNT(s.topic_id) AS expected_solution_count
    FROM forum_solutions s
    JOIN forum_replies r
      ON r.tenant_id = s.tenant_id
     AND r.topic_id = s.topic_id
     AND r.id = s.reply_id
     AND r.status = 'approved'
    WHERE s.tenant_id = ?1
      AND r.author_id IN (SELECT user_id FROM bounded_stats)
    GROUP BY r.author_id
)
SELECT
    stats.user_id AS id,
    CAST(stats.solution_count AS INTEGER) AS stored_solution_count,
    CAST(COALESCE(expected.expected_solution_count, 0) AS INTEGER) AS expected_solution_count
FROM bounded_stats stats
LEFT JOIN expected_stats expected ON expected.user_id = stats.user_id
ORDER BY stats.user_id
"#;

const SOLUTION_STAT_AFTER_SQLITE: &str = r#"
WITH bounded_stats AS (
    SELECT user_id, solution_count
    FROM forum_user_stats
    WHERE tenant_id = ?1
      AND user_id > ?2
    ORDER BY user_id
    LIMIT ?3
), expected_stats AS (
    SELECT r.author_id AS user_id, COUNT(s.topic_id) AS expected_solution_count
    FROM forum_solutions s
    JOIN forum_replies r
      ON r.tenant_id = s.tenant_id
     AND r.topic_id = s.topic_id
     AND r.id = s.reply_id
     AND r.status = 'approved'
    WHERE s.tenant_id = ?1
      AND r.author_id IN (SELECT user_id FROM bounded_stats)
    GROUP BY r.author_id
)
SELECT
    stats.user_id AS id,
    CAST(stats.solution_count AS INTEGER) AS stored_solution_count,
    CAST(COALESCE(expected.expected_solution_count, 0) AS INTEGER) AS expected_solution_count
FROM bounded_stats stats
LEFT JOIN expected_stats expected ON expected.user_id = stats.user_id
ORDER BY stats.user_id
"#;

const SOLUTION_STAT_POSTGRES: &str = r#"
WITH bounded_stats AS (
    SELECT user_id, solution_count
    FROM forum_user_stats
    WHERE tenant_id = $1
    ORDER BY user_id
    LIMIT $2
), expected_stats AS (
    SELECT r.author_id AS user_id, COUNT(s.topic_id)::BIGINT AS expected_solution_count
    FROM forum_solutions s
    JOIN forum_replies r
      ON r.tenant_id = s.tenant_id
     AND r.topic_id = s.topic_id
     AND r.id = s.reply_id
     AND r.status = 'approved'
    WHERE s.tenant_id = $1
      AND r.author_id IN (SELECT user_id FROM bounded_stats)
    GROUP BY r.author_id
)
SELECT
    stats.user_id AS id,
    stats.solution_count::BIGINT AS stored_solution_count,
    COALESCE(expected.expected_solution_count, 0)::BIGINT AS expected_solution_count
FROM bounded_stats stats
LEFT JOIN expected_stats expected ON expected.user_id = stats.user_id
ORDER BY stats.user_id
"#;

const SOLUTION_STAT_AFTER_POSTGRES: &str = r#"
WITH bounded_stats AS (
    SELECT user_id, solution_count
    FROM forum_user_stats
    WHERE tenant_id = $1
      AND user_id > $2
    ORDER BY user_id
    LIMIT $3
), expected_stats AS (
    SELECT r.author_id AS user_id, COUNT(s.topic_id)::BIGINT AS expected_solution_count
    FROM forum_solutions s
    JOIN forum_replies r
      ON r.tenant_id = s.tenant_id
     AND r.topic_id = s.topic_id
     AND r.id = s.reply_id
     AND r.status = 'approved'
    WHERE s.tenant_id = $1
      AND r.author_id IN (SELECT user_id FROM bounded_stats)
    GROUP BY r.author_id
)
SELECT
    stats.user_id AS id,
    stats.solution_count::BIGINT AS stored_solution_count,
    COALESCE(expected.expected_solution_count, 0)::BIGINT AS expected_solution_count
FROM bounded_stats stats
LEFT JOIN expected_stats expected ON expected.user_id = stats.user_id
ORDER BY stats.user_id
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::Permission;

    #[test]
    fn solution_reconciliation_requires_both_manage_scopes() {
        let both = SecurityContext::from_permission_snapshot(
            Some(Uuid::new_v4()),
            &[
                Permission::FORUM_CATEGORIES_MANAGE,
                Permission::FORUM_TOPICS_MANAGE,
            ],
        );
        assert!(enforce_operations_scope(&both).is_ok());

        let topics_only = SecurityContext::from_permission_snapshot(
            Some(Uuid::new_v4()),
            &[Permission::FORUM_TOPICS_MANAGE],
        );
        assert!(enforce_operations_scope(&topics_only).is_err());
    }

    #[test]
    fn solution_keysets_are_strictly_forward() {
        assert!(SOLUTION_AFTER_SQLITE.contains("s.topic_id > ?2"));
        assert!(SOLUTION_AFTER_POSTGRES.contains("s.topic_id > $2"));
        assert!(SOLUTION_STAT_AFTER_SQLITE.contains("user_id > ?2"));
        assert!(SOLUTION_STAT_AFTER_POSTGRES.contains("user_id > $2"));
    }

    #[test]
    fn accepted_solution_requires_approved_reply() {
        assert!(SOLUTION_SQLITE.contains("r.status = 'approved'"));
        assert!(SOLUTION_POSTGRES.contains("r.status = 'approved'"));
        assert!(SOLUTION_STAT_SQLITE.contains("r.status = 'approved'"));
        assert!(SOLUTION_STAT_POSTGRES.contains("r.status = 'approved'"));
    }
}
