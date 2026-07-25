use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::notification;
use crate::error::{NotificationError, NotificationResult};
use crate::model::NotificationState;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxStateRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    pub notification_id: Uuid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxStateSnapshot {
    pub notification_id: Uuid,
    pub state: NotificationState,
    pub seen_at: Option<DateTime<FixedOffset>>,
    pub read_at: Option<DateTime<FixedOffset>>,
    pub archived_at: Option<DateTime<FixedOffset>>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum NotificationInboxStateDecision {
    Available {
        changed: bool,
        snapshot: NotificationInboxStateSnapshot,
    },
    Unavailable,
}

/// Applies monotonic exact-recipient inbox state transitions.
///
/// `mark_seen` advances only `unread -> seen`; `mark_read` advances `unread/seen -> read`;
/// and `archive` advances every non-archived state to `archived`. Requests at the same or a later
/// state are idempotent and preserve all timestamps. Missing and foreign-recipient rows both return
/// `Unavailable`, preventing a cross-recipient or cross-tenant existence oracle.
#[derive(Clone)]
pub struct NotificationInboxStateService {
    db: DatabaseConnection,
}

impl NotificationInboxStateService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn mark_seen(
        &self,
        request: NotificationInboxStateRequest,
    ) -> NotificationResult<NotificationInboxStateDecision> {
        validate_request(&request)?;
        let timestamp = now();
        let result = notification::Entity::update_many()
            .set(notification::ActiveModel {
                state: Set(NotificationState::Seen),
                seen_at: Set(Some(timestamp.to_owned())),
                updated_at: Set(timestamp),
                ..Default::default()
            })
            .filter(notification::Column::Id.eq(request.notification_id))
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .filter(notification::Column::State.eq(NotificationState::Unread))
            .exec(&self.db)
            .await?;
        self.load_decision(request, result.rows_affected > 0).await
    }

    pub async fn mark_read(
        &self,
        request: NotificationInboxStateRequest,
    ) -> NotificationResult<NotificationInboxStateDecision> {
        validate_request(&request)?;
        let timestamp = now();
        let unread = notification::Entity::update_many()
            .set(notification::ActiveModel {
                state: Set(NotificationState::Read),
                seen_at: Set(Some(timestamp.to_owned())),
                read_at: Set(Some(timestamp.to_owned())),
                updated_at: Set(timestamp.to_owned()),
                ..Default::default()
            })
            .filter(notification::Column::Id.eq(request.notification_id))
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .filter(notification::Column::State.eq(NotificationState::Unread))
            .exec(&self.db)
            .await?;
        let changed = if unread.rows_affected > 0 {
            true
        } else {
            notification::Entity::update_many()
                .set(notification::ActiveModel {
                    state: Set(NotificationState::Read),
                    read_at: Set(Some(timestamp.to_owned())),
                    updated_at: Set(timestamp),
                    ..Default::default()
                })
                .filter(notification::Column::Id.eq(request.notification_id))
                .filter(notification::Column::TenantId.eq(request.tenant_id))
                .filter(notification::Column::RecipientId.eq(request.recipient_id))
                .filter(notification::Column::State.eq(NotificationState::Seen))
                .exec(&self.db)
                .await?
                .rows_affected
                > 0
        };
        self.load_decision(request, changed).await
    }

    pub async fn archive(
        &self,
        request: NotificationInboxStateRequest,
    ) -> NotificationResult<NotificationInboxStateDecision> {
        validate_request(&request)?;
        let timestamp = now();
        let result = notification::Entity::update_many()
            .set(notification::ActiveModel {
                state: Set(NotificationState::Archived),
                archived_at: Set(Some(timestamp.to_owned())),
                updated_at: Set(timestamp),
                ..Default::default()
            })
            .filter(notification::Column::Id.eq(request.notification_id))
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .filter(notification::Column::State.ne(NotificationState::Archived))
            .exec(&self.db)
            .await?;
        self.load_decision(request, result.rows_affected > 0).await
    }

    async fn load_decision(
        &self,
        request: NotificationInboxStateRequest,
        changed: bool,
    ) -> NotificationResult<NotificationInboxStateDecision> {
        let stored = notification::Entity::find_by_id(request.notification_id)
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .one(&self.db)
            .await?;
        let Some(stored) = stored else {
            return Ok(NotificationInboxStateDecision::Unavailable);
        };
        Ok(NotificationInboxStateDecision::Available {
            changed,
            snapshot: NotificationInboxStateSnapshot {
                notification_id: stored.id,
                state: stored.state,
                seen_at: stored.seen_at,
                read_at: stored.read_at,
                archived_at: stored.archived_at,
                updated_at: stored.updated_at,
            },
        })
    }
}

fn validate_request(request: &NotificationInboxStateRequest) -> NotificationResult<()> {
    if request.tenant_id.is_nil()
        || request.recipient_id.is_nil()
        || request.notification_id.is_nil()
    {
        return Err(NotificationError::Validation(
            "notification inbox state identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn now() -> DateTime<FixedOffset> {
    Utc::now().fixed_offset()
}
