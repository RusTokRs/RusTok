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
use crate::inbox_group::validate_inbox_group_key;
use crate::inbox_state::{
    NotificationInboxStateDecision, NotificationInboxStateRequest, NotificationInboxStateService,
};
use crate::model::NotificationState;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationInboxGroupStateAction {
    MarkRead,
    MarkUnread,
    Archive,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxGroupStateRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    pub group_key: String,
    pub action: NotificationInboxGroupStateAction,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

impl NotificationInboxGroupStateRequest {
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
pub struct NotificationInboxGroupStatePage {
    pub scanned: u16,
    pub changed: u16,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// Applies one bounded state action to one exact stored notification group.
///
/// Selection is restricted by tenant, recipient, opaque group key, and the action's eligible stored
/// states before any mutation. Every selected row delegates to `NotificationInboxStateService`, so
/// exact transition and timestamp invariants remain centralized. Missing, foreign, and already
/// satisfied groups return an empty page without exposing notification identity. Earlier exact
/// transitions remain durable and idempotent if a later database operation fails. No recipient
/// privacy, source, target, or delivery owner is invoked.
#[derive(Clone)]
pub struct NotificationInboxGroupStateService {
    db: DatabaseConnection,
    state: NotificationInboxStateService,
}

impl NotificationInboxGroupStateService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db: db.clone(),
            state: NotificationInboxStateService::new(db),
        }
    }

    pub async fn apply_page(
        &self,
        request: NotificationInboxGroupStateRequest,
    ) -> NotificationResult<NotificationInboxGroupStatePage> {
        validate_request(&request)?;
        let cursor = request
            .cursor
            .as_deref()
            .map(decode_inbox_cursor)
            .transpose()?;
        let limit = request.bounded_limit();
        let action = request.action;

        let mut select = notification::Entity::find()
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .filter(notification::Column::GroupKey.eq(request.group_key.as_str()));
        select = match action {
            NotificationInboxGroupStateAction::MarkRead => select.filter(
                Condition::any()
                    .add(notification::Column::State.eq(NotificationState::Unread))
                    .add(notification::Column::State.eq(NotificationState::Seen)),
            ),
            NotificationInboxGroupStateAction::MarkUnread => select.filter(
                Condition::any()
                    .add(notification::Column::State.eq(NotificationState::Seen))
                    .add(notification::Column::State.eq(NotificationState::Read)),
            ),
            NotificationInboxGroupStateAction::Archive => {
                select.filter(notification::Column::State.ne(NotificationState::Archived))
            }
        };
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
        let mut changed = 0_u16;

        for stored in rows {
            let state_request = NotificationInboxStateRequest {
                tenant_id: request.tenant_id,
                recipient_id: request.recipient_id,
                notification_id: stored.id,
            };
            let decision = match action {
                NotificationInboxGroupStateAction::MarkRead => {
                    self.state.mark_read(state_request).await?
                }
                NotificationInboxGroupStateAction::MarkUnread => {
                    self.state.mark_unread(state_request).await?
                }
                NotificationInboxGroupStateAction::Archive => {
                    self.state.archive(state_request).await?
                }
            };
            if matches!(
                decision,
                NotificationInboxStateDecision::Available { changed: true, .. }
            ) {
                changed += 1;
            }
        }

        Ok(NotificationInboxGroupStatePage {
            scanned,
            changed,
            next_cursor,
            has_more,
        })
    }
}

fn validate_request(request: &NotificationInboxGroupStateRequest) -> NotificationResult<()> {
    if request.tenant_id.is_nil() || request.recipient_id.is_nil() {
        return Err(NotificationError::Validation(
            "notification inbox group-state identity must not be nil".to_string(),
        ));
    }
    validate_inbox_group_key(&request.group_key)
}
