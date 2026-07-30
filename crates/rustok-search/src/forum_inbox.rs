use std::cmp::Ordering;

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use uuid::Uuid;

use rustok_core::{Error, Result};
use rustok_events::{DomainEvent, EventEnvelope};

const FORUM_SOURCE_MODULE: &str = "forum";
const FULL_SCOPE_KEY: &str = "forum";
const CATEGORY_SCOPE_PREFIX: &str = "forum_category:";
const AUTHOR_SCOPE_PREFIX: &str = "forum_author:";
const MAX_ERROR_CHARS: usize = 2_000;
const MAX_ATTEMPTS: u32 = 12;
const RETRY_BASE_SECONDS: i64 = 5;
const RETRY_MAX_SECONDS: i64 = 300;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ForumProjectionScope {
    Full,
    Category(Uuid),
    Author(Uuid),
}

impl ForumProjectionScope {
    pub(crate) fn for_event(event: &DomainEvent) -> Option<Self> {
        match event {
            DomainEvent::ForumTopicCreated { .. }
            | DomainEvent::ForumTopicReplied { .. }
            | DomainEvent::ForumTopicStatusChanged { .. }
            | DomainEvent::ForumTopicPinned { .. }
            | DomainEvent::ForumReplyStatusChanged { .. }
            | DomainEvent::LocaleEnabled { .. }
            | DomainEvent::LocaleDisabled { .. }
            | DomainEvent::TenantCreated { .. }
            | DomainEvent::TenantUpdated { .. } => Some(Self::Full),
            DomainEvent::ProfileUpdated { user_id, .. }
            | DomainEvent::UserDeleted { user_id } => Some(Self::Author(*user_id)),
            DomainEvent::TenantModuleToggled { module_slug, .. } if module_slug == "forum" => {
                Some(Self::Full)
            }
            DomainEvent::ReindexRequested {
                target_type,
                target_id,
            } => match (target_type.as_str(), target_id) {
                ("search", _) | ("forum", _) | ("forum_topic", Some(_)) => Some(Self::Full),
                ("forum_category", Some(category_id)) => Some(Self::Category(*category_id)),
                _ => None,
            },
            _ => None,
        }
    }

    fn key(&self) -> String {
        match self {
            Self::Full => FULL_SCOPE_KEY.to_string(),
            Self::Category(category_id) => format!("{CATEGORY_SCOPE_PREFIX}{category_id}"),
            Self::Author(user_id) => format!("{AUTHOR_SCOPE_PREFIX}{user_id}"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ForumProjectionInbox {
    db: DatabaseConnection,
}

impl ForumProjectionInbox {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) async fn enqueue(
        &self,
        envelope: &EventEnvelope,
        scope: &ForumProjectionScope,
    ) -> Result<()> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                r#"
                INSERT INTO search_projection_inbox (
                    event_id, tenant_id, source_module, scope_key, event_type,
                    revision_at, envelope_json, status, attempt_count, created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ON CONFLICT (event_id) DO NOTHING
                "#
            }
            DbBackend::Sqlite => {
                r#"
                INSERT INTO search_projection_inbox (
                    event_id, tenant_id, source_module, scope_key, event_type,
                    revision_at, envelope_json, status, attempt_count, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ON CONFLICT (event_id) DO NOTHING
                "#
            }
            other => {
                return Err(Error::External(format!(
                    "Forum projection inbox does not support database backend {other:?}"
                )));
            }
        };
        let envelope_json = serde_json::to_value(envelope)?;
        self.db
            .execute(Statement::from_sql_and_values(
                backend,
                sql,
                vec![
                    envelope.id.into(),
                    envelope.tenant_id.into(),
                    FORUM_SOURCE_MODULE.into(),
                    scope.key().into(),
                    envelope.event_type.clone().into(),
                    envelope.timestamp.to_owned().into(),
                    SqlValue::Json(Some(Box::new(envelope_json))),
                ],
            ))
            .await
            .map_err(Error::Database)?;
        Ok(())
    }

    /// Claims the oldest non-terminal event. A retry backoff on that event
    /// blocks newer work, so a later projection cannot overtake it. Claim lock
    /// acquisition is non-blocking so competing dispatcher tasks do not occupy
    /// the whole connection pool while one projector operation is in flight.
    pub(crate) async fn claim_next(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<ForumProjectionInboxClaim>> {
        self.ensure_postgres()?;

        loop {
            let transaction = self.db.begin().await.map_err(Error::Database)?;
            if !try_acquire_tenant_lock(&transaction, tenant_id).await? {
                transaction.commit().await.map_err(Error::Database)?;
                return Ok(None);
            }
            let row = transaction
                .query_one(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"
                    SELECT event_id, scope_key, revision_at, envelope_json,
                           status, attempt_count, next_attempt_at
                    FROM search_projection_inbox
                    WHERE tenant_id = $1
                      AND source_module = 'forum'
                      AND status IN ('pending', 'retryable_error')
                    ORDER BY revision_at ASC, event_id ASC
                    LIMIT 1
                    FOR UPDATE
                    "#,
                    vec![tenant_id.into()],
                ))
                .await
                .map_err(Error::Database)?;
            let Some(row) = row else {
                transaction.commit().await.map_err(Error::Database)?;
                return Ok(None);
            };

            let event_id: Uuid = row.try_get("", "event_id").map_err(Error::Database)?;
            let scope_key: String = row.try_get("", "scope_key").map_err(Error::Database)?;
            let revision_at: DateTime<Utc> =
                row.try_get("", "revision_at").map_err(Error::Database)?;
            let envelope_json: serde_json::Value =
                row.try_get("", "envelope_json").map_err(Error::Database)?;
            let status: String = row.try_get("", "status").map_err(Error::Database)?;
            let attempt_count: i32 = row.try_get("", "attempt_count").map_err(Error::Database)?;
            let next_attempt_at: Option<DateTime<Utc>> = row
                .try_get("", "next_attempt_at")
                .map_err(Error::Database)?;

            if status == "retryable_error"
                && next_attempt_at.is_some_and(|due_at| due_at > Utc::now())
            {
                transaction.commit().await.map_err(Error::Database)?;
                return Ok(None);
            }

            if let Some((watermark_at, watermark_event_id)) =
                load_effective_watermark(&transaction, tenant_id, &scope_key).await?
                && !is_newer_revision(
                    &revision_at,
                    event_id,
                    &watermark_at,
                    watermark_event_id,
                )
            {
                mark_terminal(
                    &transaction,
                    event_id,
                    "skipped",
                    Some("stale_revision"),
                )
                .await?;
                transaction.commit().await.map_err(Error::Database)?;
                continue;
            }

            let envelope: EventEnvelope = match serde_json::from_value(envelope_json) {
                Ok(envelope) => envelope,
                Err(error) => {
                    let message = bounded_error(&error.to_string());
                    mark_terminal(&transaction, event_id, "dead_letter", Some(&message)).await?;
                    transaction.commit().await.map_err(Error::Database)?;
                    continue;
                }
            };
            let scope_matches = ForumProjectionScope::for_event(&envelope.event)
                .map(|scope| scope.key() == scope_key)
                .unwrap_or(false);
            if envelope.id != event_id
                || envelope.tenant_id != tenant_id
                || envelope.timestamp != revision_at
                || !scope_matches
            {
                mark_terminal(
                    &transaction,
                    event_id,
                    "dead_letter",
                    Some("stored envelope identity does not match inbox identity"),
                )
                .await?;
                transaction.commit().await.map_err(Error::Database)?;
                continue;
            }

            transaction
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"
                    UPDATE search_projection_inbox
                    SET status = 'processing',
                        attempt_count = attempt_count + 1,
                        next_attempt_at = NULL,
                        last_error = NULL,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE event_id = $1
                    "#,
                    vec![event_id.into()],
                ))
                .await
                .map_err(Error::Database)?;

            return Ok(Some(ForumProjectionInboxClaim {
                transaction,
                event_id,
                tenant_id,
                scope_key,
                revision_at,
                envelope,
                attempt: attempt_count.saturating_add(1).max(1) as u32,
            }));
        }
    }

    fn ensure_postgres(&self) -> Result<()> {
        if self.db.get_database_backend() == DbBackend::Postgres {
            Ok(())
        } else {
            Err(Error::External(
                "Forum projection inbox reconciliation requires PostgreSQL".to_string(),
            ))
        }
    }
}

pub(crate) struct ForumProjectionInboxClaim {
    transaction: DatabaseTransaction,
    event_id: Uuid,
    tenant_id: Uuid,
    scope_key: String,
    revision_at: DateTime<Utc>,
    envelope: EventEnvelope,
    attempt: u32,
}

impl ForumProjectionInboxClaim {
    pub(crate) fn envelope(&self) -> &EventEnvelope {
        &self.envelope
    }

    pub(crate) async fn complete(self) -> Result<()> {
        self.transaction
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                INSERT INTO search_projection_watermarks (
                    tenant_id, source_module, scope_key, revision_at, event_id, updated_at
                ) VALUES ($1, 'forum', $2, $3, $4, CURRENT_TIMESTAMP)
                ON CONFLICT (tenant_id, source_module, scope_key)
                DO UPDATE SET
                    revision_at = EXCLUDED.revision_at,
                    event_id = EXCLUDED.event_id,
                    updated_at = CURRENT_TIMESTAMP
                WHERE search_projection_watermarks.revision_at < EXCLUDED.revision_at
                   OR (
                        search_projection_watermarks.revision_at = EXCLUDED.revision_at
                        AND search_projection_watermarks.event_id < EXCLUDED.event_id
                   )
                "#,
                vec![
                    self.tenant_id.into(),
                    self.scope_key.into(),
                    self.revision_at.into(),
                    self.event_id.into(),
                ],
            ))
            .await
            .map_err(Error::Database)?;
        mark_terminal(&self.transaction, self.event_id, "completed", None).await?;
        self.transaction.commit().await.map_err(Error::Database)?;
        Ok(())
    }

    pub(crate) async fn retry(self, error: &Error) -> Result<()> {
        let message = bounded_error(&error.to_string());
        if self.attempt >= MAX_ATTEMPTS {
            let exhausted = bounded_error(&format!("retry_exhausted: {message}"));
            mark_terminal(
                &self.transaction,
                self.event_id,
                "dead_letter",
                Some(&exhausted),
            )
            .await?;
            self.transaction.commit().await.map_err(Error::Database)?;
            return Ok(());
        }

        let next_attempt_at = Utc::now() + Duration::seconds(retry_delay_seconds(self.attempt));
        self.transaction
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                UPDATE search_projection_inbox
                SET status = 'retryable_error',
                    next_attempt_at = $2,
                    last_error = $3,
                    completed_at = NULL,
                    updated_at = CURRENT_TIMESTAMP
                WHERE event_id = $1
                "#,
                vec![self.event_id.into(), next_attempt_at.into(), message.into()],
            ))
            .await
            .map_err(Error::Database)?;
        self.transaction.commit().await.map_err(Error::Database)?;
        Ok(())
    }
}

async fn try_acquire_tenant_lock(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
) -> Result<bool> {
    let lock_key = format!("search:{FORUM_SOURCE_MODULE}:{tenant_id}:{FULL_SCOPE_KEY}");
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0)) AS acquired",
            vec![lock_key.into()],
        ))
        .await
        .map_err(Error::Database)?
        .ok_or_else(|| Error::External("PostgreSQL advisory lock returned no row".to_string()))?;
    row.try_get("", "acquired").map_err(Error::Database)
}

async fn load_effective_watermark(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    scope_key: &str,
) -> Result<Option<(DateTime<Utc>, Uuid)>> {
    if scope_key.starts_with(AUTHOR_SCOPE_PREFIX) {
        // Profile privacy changes are redaction barriers. They always rebuild from
        // current owner state and must not be discarded because an unrelated Forum
        // producer emitted a later wall-clock timestamp.
        return Ok(None);
    }

    let scope_watermark = load_watermark(transaction, tenant_id, scope_key).await?;
    if scope_key == FULL_SCOPE_KEY {
        return Ok(scope_watermark);
    }
    if scope_key.starts_with(CATEGORY_SCOPE_PREFIX) {
        let full_scope_watermark = load_watermark(transaction, tenant_id, FULL_SCOPE_KEY).await?;
        return Ok(max_watermark(scope_watermark, full_scope_watermark));
    }
    Err(Error::Validation(format!(
        "Unsupported Forum projection watermark scope `{scope_key}`"
    )))
}

async fn load_watermark(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    scope_key: &str,
) -> Result<Option<(DateTime<Utc>, Uuid)>> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT revision_at, event_id
            FROM search_projection_watermarks
            WHERE tenant_id = $1
              AND source_module = 'forum'
              AND scope_key = $2
            "#,
            vec![tenant_id.into(), scope_key.to_string().into()],
        ))
        .await
        .map_err(Error::Database)?;
    row.map(|row| {
        Ok((
            row.try_get("", "revision_at").map_err(Error::Database)?,
            row.try_get("", "event_id").map_err(Error::Database)?,
        ))
    })
    .transpose()
}

async fn mark_terminal(
    transaction: &DatabaseTransaction,
    event_id: Uuid,
    status: &str,
    last_error: Option<&str>,
) -> Result<()> {
    transaction
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            UPDATE search_projection_inbox
            SET status = $2,
                next_attempt_at = NULL,
                last_error = $3,
                completed_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE event_id = $1
            "#,
            vec![
                event_id.into(),
                status.to_string().into(),
                last_error.map(str::to_string).into(),
            ],
        ))
        .await
        .map_err(Error::Database)?;
    Ok(())
}

fn max_watermark(
    left: Option<(DateTime<Utc>, Uuid)>,
    right: Option<(DateTime<Utc>, Uuid)>,
) -> Option<(DateTime<Utc>, Uuid)> {
    match (left, right) {
        (Some(left), Some(right)) => {
            if is_newer_revision(&left.0, left.1, &right.0, right.1) {
                Some(left)
            } else {
                Some(right)
            }
        }
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn is_newer_revision(
    incoming_at: &DateTime<Utc>,
    incoming_event_id: Uuid,
    watermark_at: &DateTime<Utc>,
    watermark_event_id: Uuid,
) -> bool {
    match incoming_at.cmp(watermark_at) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => incoming_event_id.as_bytes() > watermark_event_id.as_bytes(),
    }
}

fn retry_delay_seconds(attempt: u32) -> i64 {
    let exponent = attempt.saturating_sub(1).min(16);
    RETRY_BASE_SECONDS
        .saturating_mul(2_i64.saturating_pow(exponent))
        .min(RETRY_MAX_SECONDS)
}

fn bounded_error(value: &str) -> String {
    value.chars().take(MAX_ERROR_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forum_scope_groups_full_rebuild_events() {
        let topic_id = Uuid::new_v4();
        let category_id = Uuid::new_v4();
        for event in [
            DomainEvent::ForumTopicCreated {
                topic_id,
                category_id,
                author_id: None,
                locale: "en".to_string(),
            },
            DomainEvent::ForumReplyStatusChanged {
                reply_id: Uuid::new_v4(),
                topic_id,
                old_status: "pending".to_string(),
                new_status: "approved".to_string(),
                moderator_id: None,
            },
            DomainEvent::TenantModuleToggled {
                tenant_id: Uuid::new_v4(),
                module_slug: "forum".to_string(),
                enabled: false,
            },
            DomainEvent::ReindexRequested {
                target_type: "search".to_string(),
                target_id: None,
            },
        ] {
            assert_eq!(
                ForumProjectionScope::for_event(&event),
                Some(ForumProjectionScope::Full)
            );
        }
    }

    #[test]
    fn category_reindex_has_independent_watermark_scope() {
        let category_id = Uuid::new_v4();
        assert_eq!(
            ForumProjectionScope::for_event(&DomainEvent::ReindexRequested {
                target_type: "forum_category".to_string(),
                target_id: Some(category_id),
            }),
            Some(ForumProjectionScope::Category(category_id))
        );
    }

    #[test]
    fn profile_and_account_changes_have_redaction_barrier_scope() {
        let user_id = Uuid::new_v4();
        for event in [
            DomainEvent::ProfileUpdated {
                user_id,
                handle: "safe-author".to_string(),
                locale: Some("en".to_string()),
            },
            DomainEvent::UserDeleted { user_id },
        ] {
            assert_eq!(
                ForumProjectionScope::for_event(&event),
                Some(ForumProjectionScope::Author(user_id))
            );
        }
        assert!(ForumProjectionScope::Author(user_id)
            .key()
            .starts_with(AUTHOR_SCOPE_PREFIX));
    }

    #[test]
    fn revision_order_uses_timestamp_then_event_identity() {
        let timestamp = Utc::now();
        let low = Uuid::from_u128(1);
        let high = Uuid::from_u128(2);
        assert!(is_newer_revision(&timestamp, high, &timestamp, low));
        assert!(!is_newer_revision(&timestamp, low, &timestamp, high));
        assert!(is_newer_revision(
            &(timestamp.to_owned() + Duration::microseconds(1)),
            low,
            &timestamp,
            high
        ));
    }

    #[test]
    fn effective_watermark_prefers_newest_revision() {
        let timestamp = Utc::now();
        let low = (timestamp.to_owned(), Uuid::from_u128(1));
        let high = (
            timestamp.to_owned() + Duration::microseconds(1),
            Uuid::from_u128(2),
        );
        assert_eq!(max_watermark(Some(low), Some(high.to_owned())), Some(high));
    }

    #[test]
    fn retry_backoff_is_bounded() {
        assert_eq!(retry_delay_seconds(1), RETRY_BASE_SECONDS);
        assert_eq!(retry_delay_seconds(100), RETRY_MAX_SECONDS);
    }
}
