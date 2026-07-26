use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::notification;
use crate::error::{NotificationError, NotificationResult};
use crate::inbox::{
    DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE, MAX_NOTIFICATION_INBOX_PAGE_SIZE, decode_inbox_cursor,
    encode_inbox_cursor,
};
use crate::inbox_state::{
    NotificationInboxStateDecision, NotificationInboxStateRequest, NotificationInboxStateService,
};
use crate::model::NotificationState;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxMarkAllReadRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

impl NotificationInboxMarkAllReadRequest {
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
pub struct NotificationInboxMarkAllReadPage {
    pub scanned: u16,
    pub marked_read: u16,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// Marks one bounded exact-recipient page of unread or seen notifications as read.
///
/// Selection is stable and cursor-based. Every selected row is delegated to the exact state owner,
/// preserving direct unread-to-read and seen-to-read timestamp invariants. Already-read and archived
/// rows are outside selection. Empty and foreign scopes return an empty page without exposing any
/// notification identity. No recipient-policy, source, target, or delivery owner is invoked.
#[derive(Clone)]
pub struct NotificationInboxMarkAllReadService {
    db: DatabaseConnection,
    state: NotificationInboxStateService,
}

impl NotificationInboxMarkAllReadService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db: db.clone(),
            state: NotificationInboxStateService::new(db),
        }
    }

    pub async fn mark_page(
        &self,
        request: NotificationInboxMarkAllReadRequest,
    ) -> NotificationResult<NotificationInboxMarkAllReadPage> {
        validate_request(&request)?;
        let cursor = request
            .cursor
            .as_deref()
            .map(decode_inbox_cursor)
            .transpose()?;
        let limit = request.bounded_limit();

        let mut select = notification::Entity::find()
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .filter(
                Condition::any()
                    .add(notification::Column::State.eq(NotificationState::Unread))
                    .add(notification::Column::State.eq(NotificationState::Seen)),
            );
        if let Some(cursor) = cursor {
            select = select.filter(
                Condition::any()
                    .add(notification::Column::CreatedAt.lt(cursor.created_at.to_owned()))
                    .add(
                        Condition::all()
                            .add(notification::Column::CreatedAt.eq(cursor.created_at))
                            .add(notification::Column::Id.lt(cursor.id)),
                    ),
            );
        }

        let mut rows = select
            .order_by_desc(notification::Column::CreatedAt)
            .order_by_desc(notification::Column::Id)
            .limit(limit + 1)
            .all(&self.db)
            .await?;
        let has_more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let next_cursor = has_more
            .then(|| rows.last().map(encode_inbox_cursor))
            .flatten();
        let scanned = rows.len() as u16;
        let mut marked_read = 0_u16;

        for stored in rows {
            if matches!(
                self.state
                    .mark_read(NotificationInboxStateRequest {
                        tenant_id: request.tenant_id,
                        recipient_id: request.recipient_id,
                        notification_id: stored.id,
                    })
                    .await?,
                NotificationInboxStateDecision::Available { changed: true, .. }
            ) {
                marked_read += 1;
            }
        }

        Ok(NotificationInboxMarkAllReadPage {
            scanned,
            marked_read,
            next_cursor,
            has_more,
        })
    }
}

fn validate_request(request: &NotificationInboxMarkAllReadRequest) -> NotificationResult<()> {
    if request.tenant_id.is_nil() || request.recipient_id.is_nil() {
        return Err(NotificationError::Validation(
            "notification inbox mark-all-read identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxMarkAllUnreadRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

impl NotificationInboxMarkAllUnreadRequest {
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
pub struct NotificationInboxMarkAllUnreadPage {
    pub scanned: u16,
    pub marked_unread: u16,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// Marks one bounded exact-recipient page of seen or read notifications as unread.
///
/// Selection is stable and cursor-based. Every selected row delegates to the exact state owner,
/// which clears seen/read timestamps and keeps archived terminal. Already-unread and archived rows
/// are outside selection. Empty and foreign scopes return an empty page without exposing any
/// notification identity. No recipient-policy, source, target, or delivery owner is invoked.
#[derive(Clone)]
pub struct NotificationInboxMarkAllUnreadService {
    db: DatabaseConnection,
    state: NotificationInboxStateService,
}

impl NotificationInboxMarkAllUnreadService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db: db.clone(),
            state: NotificationInboxStateService::new(db),
        }
    }

    pub async fn mark_page(
        &self,
        request: NotificationInboxMarkAllUnreadRequest,
    ) -> NotificationResult<NotificationInboxMarkAllUnreadPage> {
        validate_mark_all_unread_request(&request)?;
        let cursor = request
            .cursor
            .as_deref()
            .map(decode_inbox_cursor)
            .transpose()?;
        let limit = request.bounded_limit();

        let mut select = notification::Entity::find()
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .filter(
                Condition::any()
                    .add(notification::Column::State.eq(NotificationState::Seen))
                    .add(notification::Column::State.eq(NotificationState::Read)),
            );
        if let Some(cursor) = cursor {
            select = select.filter(
                Condition::any()
                    .add(notification::Column::CreatedAt.lt(cursor.created_at.to_owned()))
                    .add(
                        Condition::all()
                            .add(notification::Column::CreatedAt.eq(cursor.created_at))
                            .add(notification::Column::Id.lt(cursor.id)),
                    ),
            );
        }

        let mut rows = select
            .order_by_desc(notification::Column::CreatedAt)
            .order_by_desc(notification::Column::Id)
            .limit(limit + 1)
            .all(&self.db)
            .await?;
        let has_more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let next_cursor = has_more
            .then(|| rows.last().map(encode_inbox_cursor))
            .flatten();
        let scanned = rows.len() as u16;
        let mut marked_unread = 0_u16;

        for stored in rows {
            if matches!(
                self.state
                    .mark_unread(NotificationInboxStateRequest {
                        tenant_id: request.tenant_id,
                        recipient_id: request.recipient_id,
                        notification_id: stored.id,
                    })
                    .await?,
                NotificationInboxStateDecision::Available { changed: true, .. }
            ) {
                marked_unread += 1;
            }
        }

        Ok(NotificationInboxMarkAllUnreadPage {
            scanned,
            marked_unread,
            next_cursor,
            has_more,
        })
    }
}

fn validate_mark_all_unread_request(
    request: &NotificationInboxMarkAllUnreadRequest,
) -> NotificationResult<()> {
    if request.tenant_id.is_nil() || request.recipient_id.is_nil() {
        return Err(NotificationError::Validation(
            "notification inbox mark-all-unread identity must not be nil".to_string(),
        ));
    }
    Ok(())
}
