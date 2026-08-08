use std::time::Instant;

use rustok_api::{Action, Resource, normalize_locale_tag};
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
use crate::mentions::FORUM_MAX_MENTION_TARGETS_PER_REVISION;

const FORUM_MENTION_RECONCILIATION_OPERATION: &str = "forum.mention_reconciliation_report";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForumMentionDriftKind {
    SourceUnavailable,
    ChildSourceMismatch,
    TargetLimitExceeded,
    LocaleInvalid,
    ProjectionFingerprintInvalid,
}

impl ForumMentionDriftKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceUnavailable => "source_unavailable",
            Self::ChildSourceMismatch => "child_source_mismatch",
            Self::TargetLimitExceeded => "target_limit_exceeded",
            Self::LocaleInvalid => "locale_invalid",
            Self::ProjectionFingerprintInvalid => "projection_fingerprint_invalid",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumMentionDrift {
    pub kind: ForumMentionDriftKind,
    pub revision_id: i64,
    pub source_kind: String,
    pub source_id: Uuid,
    pub source_locale: String,
    pub stored: i64,
    pub expected: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumMentionReconciliationReport {
    pub requested_limit: Option<u64>,
    pub effective_limit: u64,
    pub inspected_relation_revisions: u64,
    pub inspected_mention_revisions: u64,
    pub has_more_relation_revisions: bool,
    pub relation_cursor: Option<i64>,
    pub drifts: Vec<ForumMentionDrift>,
}

impl ForumMentionReconciliationReport {
    pub fn drift_count(&self) -> usize {
        self.drifts.len()
    }

    pub fn is_clean(&self) -> bool {
        self.drifts.is_empty()
    }
}

/// Read-only FORUM-33 reconciliation for persisted Forum mention snapshots.
///
/// Relation revisions are the Forum-owned immutable source identity for user/audience mention
/// snapshots. This report does not re-resolve handles through Profiles and does not inspect
/// Notifications delivery state. It checks only persisted Forum-owned invariants: exact source
/// availability, child/source identity agreement, the established 32-target bound, canonical
/// locale identity and the owner projection fingerprint shape.
///
/// Traversal is bounded by relation revision ID with strict keyset continuation. Each page is one
/// database snapshot: PostgreSQL uses `REPEATABLE READ READ ONLY`; SQLite uses one transaction.
/// Multi-page scans are page-local diagnostics and deliberately perform no repair.
pub struct ForumMentionReconciliationService {
    db: DatabaseConnection,
}

impl ForumMentionReconciliationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn report_page(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        requested_limit: Option<u64>,
        relation_after: Option<i64>,
    ) -> ForumResult<ForumMentionReconciliationReport> {
        rustok_telemetry::metrics::record_module_entrypoint_call(
            "forum",
            "mention_reconciliation_report",
            "library",
        );
        let started_at = Instant::now();
        let result = match enforce_operations_scope(security) {
            Ok(()) => self.report_inner(tenant_id, requested_limit, relation_after).await,
            Err(error) => Err(error),
        };
        rustok_telemetry::metrics::record_span_duration(
            FORUM_MENTION_RECONCILIATION_OPERATION,
            started_at.elapsed().as_secs_f64(),
        );
        if result.is_err() {
            rustok_telemetry::metrics::record_span_error(
                FORUM_MENTION_RECONCILIATION_OPERATION,
                "owner_report",
            );
            rustok_telemetry::metrics::record_module_error(
                "forum",
                "mention_reconciliation",
                "error",
            );
        }
        result
    }

    async fn report_inner(
        &self,
        tenant_id: Uuid,
        requested_limit: Option<u64>,
        relation_after: Option<i64>,
    ) -> ForumResult<ForumMentionReconciliationReport> {
        if relation_after.is_some_and(|value| value <= 0) {
            return Err(ForumError::Validation(
                "Forum mention reconciliation cursor must be positive".to_string(),
            ));
        }
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
                    "Forum mention reconciliation does not support database backend {other:?}"
                )));
            }
        };

        let report = self
            .report_in_transaction(
                &transaction,
                backend,
                tenant_id,
                requested_limit,
                relation_after,
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
        relation_after: Option<i64>,
    ) -> ForumResult<ForumMentionReconciliationReport> {
        let effective_limit = requested_limit
            .unwrap_or(DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT)
            .clamp(1, MAX_FORUM_COUNTER_RECONCILIATION_LIMIT);
        let fetch_limit = effective_limit.saturating_add(1);
        let rows = transaction
            .query_all(mention_statement(
                backend,
                tenant_id,
                relation_after,
                fetch_limit,
            )?)
            .await?;

        let has_more_relation_revisions = rows.len() > effective_limit as usize;
        let mut relation_cursor = relation_after;
        let mut inspected_mention_revisions = 0_u64;
        let mut drifts = Vec::new();

        for row in rows.iter().take(effective_limit as usize) {
            let revision_id: i64 = row.try_get("", "revision_id")?;
            relation_cursor = Some(revision_id);
            let source_kind: String = row.try_get("", "target_kind")?;
            let source_id: Uuid = row.try_get("", "target_id")?;
            let source_locale: String = row.try_get("", "locale")?;
            let projection_fingerprint: String = row.try_get("", "projection_fingerprint")?;
            let source_exists: i64 = row.try_get("", "source_exists")?;
            let user_count: i64 = row.try_get("", "user_count")?;
            let audience_count: i64 = row.try_get("", "audience_count")?;
            let child_source_mismatch_count: i64 =
                row.try_get("", "child_source_mismatch_count")?;
            let mention_count = user_count.saturating_add(audience_count);

            if mention_count == 0 {
                continue;
            }
            inspected_mention_revisions = inspected_mention_revisions.saturating_add(1);

            if source_exists != 1 {
                drifts.push(ForumMentionDrift {
                    kind: ForumMentionDriftKind::SourceUnavailable,
                    revision_id,
                    source_kind: source_kind.clone(),
                    source_id,
                    source_locale: source_locale.clone(),
                    stored: source_exists,
                    expected: 1,
                });
            }
            if child_source_mismatch_count != 0 {
                drifts.push(ForumMentionDrift {
                    kind: ForumMentionDriftKind::ChildSourceMismatch,
                    revision_id,
                    source_kind: source_kind.clone(),
                    source_id,
                    source_locale: source_locale.clone(),
                    stored: child_source_mismatch_count,
                    expected: 0,
                });
            }
            if mention_count > FORUM_MAX_MENTION_TARGETS_PER_REVISION as i64 {
                drifts.push(ForumMentionDrift {
                    kind: ForumMentionDriftKind::TargetLimitExceeded,
                    revision_id,
                    source_kind: source_kind.clone(),
                    source_id,
                    source_locale: source_locale.clone(),
                    stored: mention_count,
                    expected: FORUM_MAX_MENTION_TARGETS_PER_REVISION as i64,
                });
            }
            if normalize_locale_tag(&source_locale).as_deref() != Some(source_locale.as_str()) {
                drifts.push(ForumMentionDrift {
                    kind: ForumMentionDriftKind::LocaleInvalid,
                    revision_id,
                    source_kind: source_kind.clone(),
                    source_id,
                    source_locale: source_locale.clone(),
                    stored: 0,
                    expected: 1,
                });
            }
            if !valid_projection_fingerprint(&projection_fingerprint) {
                drifts.push(ForumMentionDrift {
                    kind: ForumMentionDriftKind::ProjectionFingerprintInvalid,
                    revision_id,
                    source_kind,
                    source_id,
                    source_locale,
                    stored: projection_fingerprint.len() as i64,
                    expected: 64,
                });
            }
        }

        Ok(ForumMentionReconciliationReport {
            requested_limit,
            effective_limit,
            inspected_relation_revisions: rows.len().min(effective_limit as usize) as u64,
            inspected_mention_revisions,
            has_more_relation_revisions,
            relation_cursor,
            drifts,
        })
    }
}

fn enforce_operations_scope(security: &SecurityContext) -> ForumResult<()> {
    enforce_scope(security, Resource::ForumCategories, Action::Manage)?;
    enforce_scope(security, Resource::ForumTopics, Action::Manage)
}

fn valid_projection_fingerprint(value: &str) -> bool {
    value == "legacy"
        || (value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')))
}

fn mention_statement(
    backend: DatabaseBackend,
    tenant_id: Uuid,
    after: Option<i64>,
    limit: u64,
) -> ForumResult<Statement> {
    match (backend, after) {
        (DatabaseBackend::Sqlite, None) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            MENTIONS_SQLITE,
            vec![tenant_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Sqlite, Some(after)) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            MENTIONS_AFTER_SQLITE,
            vec![tenant_id.into(), after.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Postgres, None) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            MENTIONS_POSTGRES,
            vec![tenant_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Postgres, Some(after)) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            MENTIONS_AFTER_POSTGRES,
            vec![tenant_id.into(), after.into(), (limit as i64).into()],
        )),
        (other, _) => Err(ForumError::Validation(format!(
            "Forum mention reconciliation does not support database backend {other:?}"
        ))),
    }
}

const MENTIONS_SQLITE: &str = r#"
WITH bounded AS (
    SELECT revision_id, target_kind, target_id, locale, projection_fingerprint
    FROM forum_relation_revisions
    WHERE tenant_id = ?1
    ORDER BY revision_id
    LIMIT ?2
), user_summary AS (
    SELECT
        b.revision_id,
        COUNT(u.source_revision_id) AS user_count,
        COALESCE(SUM(CASE WHEN u.source_revision_id IS NOT NULL AND (
            u.source_kind <> b.target_kind
            OR u.source_id <> b.target_id
            OR u.source_locale <> b.locale
        ) THEN 1 ELSE 0 END), 0) AS mismatch_count
    FROM bounded b
    LEFT JOIN forum_user_mentions u
      ON u.tenant_id = ?1
     AND u.source_revision_id = b.revision_id
    GROUP BY b.revision_id
), audience_summary AS (
    SELECT
        b.revision_id,
        COUNT(a.source_revision_id) AS audience_count,
        COALESCE(SUM(CASE WHEN a.source_revision_id IS NOT NULL AND (
            a.source_kind <> b.target_kind
            OR a.source_id <> b.target_id
            OR a.source_locale <> b.locale
        ) THEN 1 ELSE 0 END), 0) AS mismatch_count
    FROM bounded b
    LEFT JOIN forum_audience_mentions a
      ON a.tenant_id = ?1
     AND a.source_revision_id = b.revision_id
    GROUP BY b.revision_id
)
SELECT
    b.revision_id,
    b.target_kind,
    b.target_id,
    b.locale,
    b.projection_fingerprint,
    CASE WHEN (
        b.target_kind = 'topic' AND EXISTS (
            SELECT 1 FROM forum_topic_translations t
            WHERE t.tenant_id = ?1 AND t.topic_id = b.target_id AND t.locale = b.locale
        )
    ) OR (
        b.target_kind = 'reply' AND EXISTS (
            SELECT 1 FROM forum_reply_bodies r
            WHERE r.tenant_id = ?1 AND r.reply_id = b.target_id AND r.locale = b.locale
        )
    ) THEN 1 ELSE 0 END AS source_exists,
    CAST(u.user_count AS INTEGER) AS user_count,
    CAST(a.audience_count AS INTEGER) AS audience_count,
    CAST(u.mismatch_count + a.mismatch_count AS INTEGER) AS child_source_mismatch_count
FROM bounded b
JOIN user_summary u ON u.revision_id = b.revision_id
JOIN audience_summary a ON a.revision_id = b.revision_id
ORDER BY b.revision_id
"#;

const MENTIONS_AFTER_SQLITE: &str = r#"
WITH bounded AS (
    SELECT revision_id, target_kind, target_id, locale, projection_fingerprint
    FROM forum_relation_revisions
    WHERE tenant_id = ?1
      AND revision_id > ?2
    ORDER BY revision_id
    LIMIT ?3
), user_summary AS (
    SELECT
        b.revision_id,
        COUNT(u.source_revision_id) AS user_count,
        COALESCE(SUM(CASE WHEN u.source_revision_id IS NOT NULL AND (
            u.source_kind <> b.target_kind
            OR u.source_id <> b.target_id
            OR u.source_locale <> b.locale
        ) THEN 1 ELSE 0 END), 0) AS mismatch_count
    FROM bounded b
    LEFT JOIN forum_user_mentions u
      ON u.tenant_id = ?1
     AND u.source_revision_id = b.revision_id
    GROUP BY b.revision_id
), audience_summary AS (
    SELECT
        b.revision_id,
        COUNT(a.source_revision_id) AS audience_count,
        COALESCE(SUM(CASE WHEN a.source_revision_id IS NOT NULL AND (
            a.source_kind <> b.target_kind
            OR a.source_id <> b.target_id
            OR a.source_locale <> b.locale
        ) THEN 1 ELSE 0 END), 0) AS mismatch_count
    FROM bounded b
    LEFT JOIN forum_audience_mentions a
      ON a.tenant_id = ?1
     AND a.source_revision_id = b.revision_id
    GROUP BY b.revision_id
)
SELECT
    b.revision_id,
    b.target_kind,
    b.target_id,
    b.locale,
    b.projection_fingerprint,
    CASE WHEN (
        b.target_kind = 'topic' AND EXISTS (
            SELECT 1 FROM forum_topic_translations t
            WHERE t.tenant_id = ?1 AND t.topic_id = b.target_id AND t.locale = b.locale
        )
    ) OR (
        b.target_kind = 'reply' AND EXISTS (
            SELECT 1 FROM forum_reply_bodies r
            WHERE r.tenant_id = ?1 AND r.reply_id = b.target_id AND r.locale = b.locale
        )
    ) THEN 1 ELSE 0 END AS source_exists,
    CAST(u.user_count AS INTEGER) AS user_count,
    CAST(a.audience_count AS INTEGER) AS audience_count,
    CAST(u.mismatch_count + a.mismatch_count AS INTEGER) AS child_source_mismatch_count
FROM bounded b
JOIN user_summary u ON u.revision_id = b.revision_id
JOIN audience_summary a ON a.revision_id = b.revision_id
ORDER BY b.revision_id
"#;

const MENTIONS_POSTGRES: &str = r#"
WITH bounded AS (
    SELECT revision_id, target_kind, target_id, locale, projection_fingerprint
    FROM forum_relation_revisions
    WHERE tenant_id = $1
    ORDER BY revision_id
    LIMIT $2
), user_summary AS (
    SELECT
        b.revision_id,
        COUNT(u.source_revision_id)::BIGINT AS user_count,
        COALESCE(SUM(CASE WHEN u.source_revision_id IS NOT NULL AND (
            u.source_kind <> b.target_kind
            OR u.source_id <> b.target_id
            OR u.source_locale <> b.locale
        ) THEN 1 ELSE 0 END), 0)::BIGINT AS mismatch_count
    FROM bounded b
    LEFT JOIN forum_user_mentions u
      ON u.tenant_id = $1
     AND u.source_revision_id = b.revision_id
    GROUP BY b.revision_id
), audience_summary AS (
    SELECT
        b.revision_id,
        COUNT(a.source_revision_id)::BIGINT AS audience_count,
        COALESCE(SUM(CASE WHEN a.source_revision_id IS NOT NULL AND (
            a.source_kind <> b.target_kind
            OR a.source_id <> b.target_id
            OR a.source_locale <> b.locale
        ) THEN 1 ELSE 0 END), 0)::BIGINT AS mismatch_count
    FROM bounded b
    LEFT JOIN forum_audience_mentions a
      ON a.tenant_id = $1
     AND a.source_revision_id = b.revision_id
    GROUP BY b.revision_id
)
SELECT
    b.revision_id,
    b.target_kind,
    b.target_id,
    b.locale,
    b.projection_fingerprint,
    CASE WHEN (
        b.target_kind = 'topic' AND EXISTS (
            SELECT 1 FROM forum_topic_translations t
            WHERE t.tenant_id = $1 AND t.topic_id = b.target_id AND t.locale = b.locale
        )
    ) OR (
        b.target_kind = 'reply' AND EXISTS (
            SELECT 1 FROM forum_reply_bodies r
            WHERE r.tenant_id = $1 AND r.reply_id = b.target_id AND r.locale = b.locale
        )
    ) THEN 1::BIGINT ELSE 0::BIGINT END AS source_exists,
    u.user_count,
    a.audience_count,
    (u.mismatch_count + a.mismatch_count)::BIGINT AS child_source_mismatch_count
FROM bounded b
JOIN user_summary u ON u.revision_id = b.revision_id
JOIN audience_summary a ON a.revision_id = b.revision_id
ORDER BY b.revision_id
"#;

const MENTIONS_AFTER_POSTGRES: &str = r#"
WITH bounded AS (
    SELECT revision_id, target_kind, target_id, locale, projection_fingerprint
    FROM forum_relation_revisions
    WHERE tenant_id = $1
      AND revision_id > $2
    ORDER BY revision_id
    LIMIT $3
), user_summary AS (
    SELECT
        b.revision_id,
        COUNT(u.source_revision_id)::BIGINT AS user_count,
        COALESCE(SUM(CASE WHEN u.source_revision_id IS NOT NULL AND (
            u.source_kind <> b.target_kind
            OR u.source_id <> b.target_id
            OR u.source_locale <> b.locale
        ) THEN 1 ELSE 0 END), 0)::BIGINT AS mismatch_count
    FROM bounded b
    LEFT JOIN forum_user_mentions u
      ON u.tenant_id = $1
     AND u.source_revision_id = b.revision_id
    GROUP BY b.revision_id
), audience_summary AS (
    SELECT
        b.revision_id,
        COUNT(a.source_revision_id)::BIGINT AS audience_count,
        COALESCE(SUM(CASE WHEN a.source_revision_id IS NOT NULL AND (
            a.source_kind <> b.target_kind
            OR a.source_id <> b.target_id
            OR a.source_locale <> b.locale
        ) THEN 1 ELSE 0 END), 0)::BIGINT AS mismatch_count
    FROM bounded b
    LEFT JOIN forum_audience_mentions a
      ON a.tenant_id = $1
     AND a.source_revision_id = b.revision_id
    GROUP BY b.revision_id
)
SELECT
    b.revision_id,
    b.target_kind,
    b.target_id,
    b.locale,
    b.projection_fingerprint,
    CASE WHEN (
        b.target_kind = 'topic' AND EXISTS (
            SELECT 1 FROM forum_topic_translations t
            WHERE t.tenant_id = $1 AND t.topic_id = b.target_id AND t.locale = b.locale
        )
    ) OR (
        b.target_kind = 'reply' AND EXISTS (
            SELECT 1 FROM forum_reply_bodies r
            WHERE r.tenant_id = $1 AND r.reply_id = b.target_id AND r.locale = b.locale
        )
    ) THEN 1::BIGINT ELSE 0::BIGINT END AS source_exists,
    u.user_count,
    a.audience_count,
    (u.mismatch_count + a.mismatch_count)::BIGINT AS child_source_mismatch_count
FROM bounded b
JOIN user_summary u ON u.revision_id = b.revision_id
JOIN audience_summary a ON a.revision_id = b.revision_id
ORDER BY b.revision_id
"#;
