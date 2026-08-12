use std::sync::Arc;

use chrono::{DateTime, FixedOffset};
use rustok_notifications_api::{
    NotificationPriority, NotificationSourceRegistry, NotificationSourceSlug,
    NotificationTemplateData, NotificationTemplateKey, NotificationTypeKey,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, EntityTrait,
    FromQueryResult, QueryFilter, Statement,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::candidate::NotificationRecipientPolicy;
use crate::entities::notification;
use crate::error::{NotificationError, NotificationResult};
use crate::inbox::{
    DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE, MAX_NOTIFICATION_INBOX_PAGE_SIZE, NotificationInboxItem,
    NotificationInboxOpenDecision, NotificationInboxOpenRequest, NotificationInboxOpenService,
    decode_inbox_cursor, encode_inbox_position_cursor,
};
use crate::inbox_group::MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES;
use crate::model::{NotificationPriorityValue, NotificationState};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxGroupSummaryRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

impl NotificationInboxGroupSummaryRequest {
    pub fn bounded_limit(&self) -> u64 {
        let requested = if self.limit == 0 {
            DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE
        } else {
            self.limit
        };
        u64::from(requested.min(MAX_NOTIFICATION_INBOX_PAGE_SIZE))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxGroupSummary {
    pub group_key: String,
    pub item_count: u64,
    pub unread_count: u64,
    pub latest_item: NotificationInboxItem,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxGroupSummaryPage {
    pub groups: Vec<NotificationInboxGroupSummary>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, FromQueryResult)]
struct StoredGroupSummary {
    latest_id: Uuid,
    group_key: String,
    latest_created_at: DateTime<FixedOffset>,
    item_count: i64,
    unread_count: i64,
}

/// Lists one bounded exact-recipient page of stored notification group summaries.
///
/// Only groups with at least one non-archived row are selected. Raw groups are ordered by their
/// latest non-archived row and paged through the shared versioned inbox cursor. Counts reflect
/// stored owner state: `item_count` includes non-archived rows and `unread_count` includes unread
/// rows. The latest row must still pass current recipient privacy and source target authorization;
/// suppressed groups are omitted while the raw group cursor continues to advance. This read
/// mutates no inbox state and creates no delivery attempt.
#[derive(Clone)]
pub struct NotificationInboxGroupSummaryService {
    db: DatabaseConnection,
    open: NotificationInboxOpenService,
}

impl NotificationInboxGroupSummaryService {
    pub fn new(
        db: DatabaseConnection,
        registry: Arc<NotificationSourceRegistry>,
        policy: Arc<dyn NotificationRecipientPolicy>,
    ) -> Self {
        Self {
            db: db.clone(),
            open: NotificationInboxOpenService::new(db, registry, policy),
        }
    }

    pub async fn list_page(
        &self,
        request: NotificationInboxGroupSummaryRequest,
    ) -> NotificationResult<NotificationInboxGroupSummaryPage> {
        validate_request(&request)?;
        let cursor = request
            .cursor
            .as_deref()
            .map(decode_inbox_cursor)
            .transpose()?;
        let limit = request.bounded_limit();
        let query_limit =
            i64::try_from(limit + 1).map_err(|_| NotificationError::InvalidDescriptor)?;
        let backend = self.db.get_database_backend();
        let statement = match (backend, cursor) {
            (DatabaseBackend::Postgres, None) => Statement::from_sql_and_values(
                backend,
                POSTGRES_FIRST_PAGE,
                vec![
                    request.tenant_id.into(),
                    request.recipient_id.into(),
                    query_limit.into(),
                ],
            ),
            (DatabaseBackend::Postgres, Some(cursor)) => Statement::from_sql_and_values(
                backend,
                POSTGRES_CURSOR_PAGE,
                vec![
                    request.tenant_id.into(),
                    request.recipient_id.into(),
                    cursor.created_at.into(),
                    cursor.id.into(),
                    query_limit.into(),
                ],
            ),
            (DatabaseBackend::Sqlite, None) => Statement::from_sql_and_values(
                backend,
                SQLITE_FIRST_PAGE,
                vec![
                    request.tenant_id.into(),
                    request.recipient_id.into(),
                    query_limit.into(),
                ],
            ),
            (DatabaseBackend::Sqlite, Some(cursor)) => Statement::from_sql_and_values(
                backend,
                SQLITE_CURSOR_PAGE,
                vec![
                    request.tenant_id.into(),
                    request.recipient_id.into(),
                    cursor.created_at.to_owned().into(),
                    cursor.created_at.into(),
                    cursor.id.into(),
                    query_limit.into(),
                ],
            ),
            (backend, _) => {
                return Err(DbErr::Custom(format!(
                    "notification group summaries do not support database backend {backend:?}"
                ))
                .into());
            }
        };

        let query_rows = self.db.query_all(statement).await?;
        let mut rows = query_rows
            .iter()
            .map(|row| StoredGroupSummary::from_query_result(row, ""))
            .collect::<Result<Vec<_>, DbErr>>()?;
        let has_more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let next_cursor = has_more
            .then(|| {
                rows.last()
                    .map(|row| encode_summary_cursor(&row.latest_created_at, row.latest_id))
            })
            .flatten();

        let mut groups = Vec::with_capacity(rows.len());
        for row in rows {
            validate_stored_summary(&row)?;
            let stored = notification::Entity::find_by_id(row.latest_id)
                .filter(notification::Column::TenantId.eq(request.tenant_id))
                .filter(notification::Column::RecipientId.eq(request.recipient_id))
                .filter(notification::Column::GroupKey.eq(row.group_key.as_str()))
                .filter(notification::Column::State.ne(NotificationState::Archived))
                .one(&self.db)
                .await?;
            let Some(stored) = stored else {
                continue;
            };

            let decision = self
                .open
                .authorize_open(NotificationInboxOpenRequest {
                    tenant_id: request.tenant_id,
                    recipient_id: request.recipient_id,
                    notification_id: stored.id,
                })
                .await?;
            if matches!(decision, NotificationInboxOpenDecision::Allowed { .. }) {
                groups.push(NotificationInboxGroupSummary {
                    group_key: row.group_key,
                    item_count: u64::try_from(row.item_count)
                        .map_err(|_| NotificationError::InvalidDescriptor)?,
                    unread_count: u64::try_from(row.unread_count)
                        .map_err(|_| NotificationError::InvalidDescriptor)?,
                    latest_item: materialize_item(stored)?,
                });
            }
        }

        Ok(NotificationInboxGroupSummaryPage {
            groups,
            next_cursor,
            has_more,
        })
    }
}

fn validate_request(request: &NotificationInboxGroupSummaryRequest) -> NotificationResult<()> {
    if request.tenant_id.is_nil() || request.recipient_id.is_nil() {
        return Err(NotificationError::Validation(
            "notification inbox group summary identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn validate_stored_summary(row: &StoredGroupSummary) -> NotificationResult<()> {
    if row.latest_id.is_nil()
        || row.group_key.is_empty()
        || row.group_key != row.group_key.trim()
        || row.group_key.len() > MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES
        || row.group_key.chars().any(char::is_control)
        || row.item_count <= 0
        || row.unread_count < 0
        || row.unread_count > row.item_count
    {
        return Err(NotificationError::InvalidDescriptor);
    }
    Ok(())
}

fn encode_summary_cursor(created_at: &DateTime<FixedOffset>, id: Uuid) -> String {
    encode_inbox_position_cursor(created_at, id)
}

fn materialize_item(stored: notification::Model) -> NotificationResult<NotificationInboxItem> {
    Ok(NotificationInboxItem {
        id: stored.id,
        source: NotificationSourceSlug::new(stored.source_slug)
            .map_err(|_| NotificationError::InvalidDescriptor)?,
        notification_type: NotificationTypeKey::new(stored.notification_type)
            .map_err(|_| NotificationError::InvalidDescriptor)?,
        template_key: NotificationTemplateKey::new(stored.template_key)
            .map_err(|_| NotificationError::InvalidDescriptor)?,
        actor_id: stored.actor_id,
        priority: priority_from_value(stored.priority),
        state: stored.state,
        template_data: serde_json::from_value::<NotificationTemplateData>(
            stored.template_data_json,
        )?,
        seen_at: stored.seen_at,
        read_at: stored.read_at,
        archived_at: stored.archived_at,
        created_at: stored.created_at,
    })
}

const fn priority_from_value(value: NotificationPriorityValue) -> NotificationPriority {
    match value {
        NotificationPriorityValue::Low => NotificationPriority::Low,
        NotificationPriorityValue::Normal => NotificationPriority::Normal,
        NotificationPriorityValue::High => NotificationPriority::High,
        NotificationPriorityValue::Urgent => NotificationPriority::Urgent,
    }
}

const POSTGRES_FIRST_PAGE: &str = r#"
SELECT
    latest.id AS latest_id,
    latest.group_key AS group_key,
    latest.created_at AS latest_created_at,
    (
        SELECT COUNT(*)
        FROM notifications counted
        WHERE counted.tenant_id = latest.tenant_id
          AND counted.recipient_id = latest.recipient_id
          AND counted.group_key = latest.group_key
          AND counted.state <> 'archived'
    ) AS item_count,
    (
        SELECT COUNT(*)
        FROM notifications unread
        WHERE unread.tenant_id = latest.tenant_id
          AND unread.recipient_id = latest.recipient_id
          AND unread.group_key = latest.group_key
          AND unread.state = 'unread'
    ) AS unread_count
FROM notifications latest
WHERE latest.tenant_id = $1
  AND latest.recipient_id = $2
  AND latest.group_key IS NOT NULL
  AND latest.state <> 'archived'
  AND NOT EXISTS (
      SELECT 1
      FROM notifications newer
      WHERE newer.tenant_id = latest.tenant_id
        AND newer.recipient_id = latest.recipient_id
        AND newer.group_key = latest.group_key
        AND newer.state <> 'archived'
        AND (
            newer.created_at > latest.created_at
            OR (newer.created_at = latest.created_at AND newer.id > latest.id)
        )
  )
ORDER BY latest.created_at DESC, latest.id DESC
LIMIT $3
"#;

const POSTGRES_CURSOR_PAGE: &str = r#"
SELECT
    latest.id AS latest_id,
    latest.group_key AS group_key,
    latest.created_at AS latest_created_at,
    (
        SELECT COUNT(*)
        FROM notifications counted
        WHERE counted.tenant_id = latest.tenant_id
          AND counted.recipient_id = latest.recipient_id
          AND counted.group_key = latest.group_key
          AND counted.state <> 'archived'
    ) AS item_count,
    (
        SELECT COUNT(*)
        FROM notifications unread
        WHERE unread.tenant_id = latest.tenant_id
          AND unread.recipient_id = latest.recipient_id
          AND unread.group_key = latest.group_key
          AND unread.state = 'unread'
    ) AS unread_count
FROM notifications latest
WHERE latest.tenant_id = $1
  AND latest.recipient_id = $2
  AND latest.group_key IS NOT NULL
  AND latest.state <> 'archived'
  AND (
      latest.created_at < $3
      OR (latest.created_at = $3 AND latest.id < $4)
  )
  AND NOT EXISTS (
      SELECT 1
      FROM notifications newer
      WHERE newer.tenant_id = latest.tenant_id
        AND newer.recipient_id = latest.recipient_id
        AND newer.group_key = latest.group_key
        AND newer.state <> 'archived'
        AND (
            newer.created_at > latest.created_at
            OR (newer.created_at = latest.created_at AND newer.id > latest.id)
        )
  )
ORDER BY latest.created_at DESC, latest.id DESC
LIMIT $5
"#;

const SQLITE_FIRST_PAGE: &str = r#"
SELECT
    latest.id AS latest_id,
    latest.group_key AS group_key,
    latest.created_at AS latest_created_at,
    (
        SELECT COUNT(*)
        FROM notifications counted
        WHERE counted.tenant_id = latest.tenant_id
          AND counted.recipient_id = latest.recipient_id
          AND counted.group_key = latest.group_key
          AND counted.state <> 'archived'
    ) AS item_count,
    (
        SELECT COUNT(*)
        FROM notifications unread
        WHERE unread.tenant_id = latest.tenant_id
          AND unread.recipient_id = latest.recipient_id
          AND unread.group_key = latest.group_key
          AND unread.state = 'unread'
    ) AS unread_count
FROM notifications latest
WHERE latest.tenant_id = ?
  AND latest.recipient_id = ?
  AND latest.group_key IS NOT NULL
  AND latest.state <> 'archived'
  AND NOT EXISTS (
      SELECT 1
      FROM notifications newer
      WHERE newer.tenant_id = latest.tenant_id
        AND newer.recipient_id = latest.recipient_id
        AND newer.group_key = latest.group_key
        AND newer.state <> 'archived'
        AND (
            newer.created_at > latest.created_at
            OR (newer.created_at = latest.created_at AND newer.id > latest.id)
        )
  )
ORDER BY latest.created_at DESC, latest.id DESC
LIMIT ?
"#;

const SQLITE_CURSOR_PAGE: &str = r#"
SELECT
    latest.id AS latest_id,
    latest.group_key AS group_key,
    latest.created_at AS latest_created_at,
    (
        SELECT COUNT(*)
        FROM notifications counted
        WHERE counted.tenant_id = latest.tenant_id
          AND counted.recipient_id = latest.recipient_id
          AND counted.group_key = latest.group_key
          AND counted.state <> 'archived'
    ) AS item_count,
    (
        SELECT COUNT(*)
        FROM notifications unread
        WHERE unread.tenant_id = latest.tenant_id
          AND unread.recipient_id = latest.recipient_id
          AND unread.group_key = latest.group_key
          AND unread.state = 'unread'
    ) AS unread_count
FROM notifications latest
WHERE latest.tenant_id = ?
  AND latest.recipient_id = ?
  AND latest.group_key IS NOT NULL
  AND latest.state <> 'archived'
  AND (
      latest.created_at < ?
      OR (latest.created_at = ? AND latest.id < ?)
  )
  AND NOT EXISTS (
      SELECT 1
      FROM notifications newer
      WHERE newer.tenant_id = latest.tenant_id
        AND newer.recipient_id = latest.recipient_id
        AND newer.group_key = latest.group_key
        AND newer.state <> 'archived'
        AND (
            newer.created_at > latest.created_at
            OR (newer.created_at = latest.created_at AND newer.id > latest.id)
        )
  )
ORDER BY latest.created_at DESC, latest.id DESC
LIMIT ?
"#;
