use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use rustok_notifications_api::{
    AuthorizeNotificationTargetRequest, NotificationOpenAuthorization, NotificationPriority,
    NotificationSourceRegistry, NotificationSourceSlug, NotificationTargetKind,
    NotificationTargetRef, NotificationTargetRoute, NotificationTemplateData,
    NotificationTemplateKey, NotificationTypeKey,
};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::candidate::{
    NotificationRecipientPolicy, NotificationRecipientPolicyDecision,
    NotificationRecipientPolicyRequest,
};
use crate::entities::notification;
use crate::error::{NotificationError, NotificationResult};
use crate::model::{NotificationPriorityValue, NotificationState};

pub const DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE: u16 = 20;
pub const MAX_NOTIFICATION_INBOX_PAGE_SIZE: u16 = 64;
pub const MAX_NOTIFICATION_INBOX_CURSOR_BYTES: usize = 128;

const INBOX_CURSOR_VERSION: &str = "i1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxOpenRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    pub notification_id: Uuid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum NotificationInboxOpenDecision {
    Allowed { route: NotificationTargetRoute },
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxListRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    #[serde(default)]
    pub state: Option<NotificationState>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

impl NotificationInboxListRequest {
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
pub struct NotificationInboxItem {
    pub id: Uuid,
    pub source: NotificationSourceSlug,
    pub notification_type: NotificationTypeKey,
    pub template_key: NotificationTemplateKey,
    pub actor_id: Option<Uuid>,
    pub priority: NotificationPriority,
    pub state: NotificationState,
    pub template_data: NotificationTemplateData,
    pub seen_at: Option<DateTime<FixedOffset>>,
    pub read_at: Option<DateTime<FixedOffset>>,
    pub archived_at: Option<DateTime<FixedOffset>>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxPage {
    pub items: Vec<NotificationInboxItem>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// Authorizes one stored notification target at the moment an exact recipient opens it.
///
/// This service intentionally returns only a fresh owner-provided route. It does not expose the
/// stored notification row, mutate inbox state, or enqueue a delivery attempt. Missing and
/// foreign-recipient rows both fail closed as `Unavailable`, preventing a notification-existence
/// oracle across recipients or tenants. Owned rows must pass current recipient privacy policy
/// before the source provider is asked to authorize the target.
#[derive(Clone)]
pub struct NotificationInboxOpenService {
    db: DatabaseConnection,
    registry: Arc<NotificationSourceRegistry>,
    policy: Arc<dyn NotificationRecipientPolicy>,
}

impl NotificationInboxOpenService {
    pub fn new(
        db: DatabaseConnection,
        registry: Arc<NotificationSourceRegistry>,
        policy: Arc<dyn NotificationRecipientPolicy>,
    ) -> Self {
        Self {
            db,
            registry,
            policy,
        }
    }

    pub async fn authorize_open(
        &self,
        request: NotificationInboxOpenRequest,
    ) -> NotificationResult<NotificationInboxOpenDecision> {
        validate_open_request(&request)?;

        let stored = notification::Entity::find_by_id(request.notification_id)
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .one(&self.db)
            .await?;
        let Some(stored) = stored else {
            return Ok(NotificationInboxOpenDecision::Unavailable);
        };

        let source = NotificationSourceSlug::new(stored.source_slug.clone())
            .map_err(|_| NotificationError::InvalidDescriptor)?;
        let target = NotificationTargetRef {
            owner: NotificationSourceSlug::new(stored.target_owner)
                .map_err(|_| NotificationError::InvalidDescriptor)?,
            kind: NotificationTargetKind::new(stored.target_kind)
                .map_err(|_| NotificationError::InvalidDescriptor)?,
            id: stored.target_id,
        };
        if target.id.is_nil() {
            return Err(NotificationError::InvalidDescriptor);
        }

        match self
            .policy
            .evaluate(NotificationRecipientPolicyRequest {
                tenant_id: request.tenant_id,
                recipient_id: request.recipient_id,
                actor_id: stored.actor_id,
                source_slug: source.as_str().to_string(),
                source_event_id: stored.source_event_id,
                source_revision: stored.source_revision,
                notification_type: stored.notification_type,
                target: target.clone(),
            })
            .await
        {
            Ok(NotificationRecipientPolicyDecision::Allow) => {}
            Ok(NotificationRecipientPolicyDecision::Suppress { .. }) => {
                return Ok(NotificationInboxOpenDecision::Unavailable);
            }
            Err(error) => {
                return Err(NotificationError::RecipientPolicyFailure {
                    retryable: error.retryable,
                });
            }
        }

        let provider = self
            .registry
            .get(&source)
            .ok_or(NotificationError::SourceUnavailable)?;
        match provider
            .authorize_target_open(AuthorizeNotificationTargetRequest {
                tenant_id: request.tenant_id,
                recipient_id: request.recipient_id,
                target,
            })
            .await
            .map_err(NotificationError::from)?
        {
            NotificationOpenAuthorization::Allowed { route } => {
                Ok(NotificationInboxOpenDecision::Allowed { route })
            }
            NotificationOpenAuthorization::Unavailable => {
                Ok(NotificationInboxOpenDecision::Unavailable)
            }
        }
    }
}

/// Lists a bounded raw inbox page and keeps only notifications that still pass the exact open-time
/// recipient and source authorization pipeline.
///
/// The cursor advances by the last scanned raw row, not the last returned item. A page may therefore
/// contain no items while still returning a next cursor. This preserves bounded work and forward
/// progress when current privacy or source policy suppresses a contiguous range of stored rows.
#[derive(Clone)]
pub struct NotificationInboxListService {
    db: DatabaseConnection,
    open: NotificationInboxOpenService,
}

impl NotificationInboxListService {
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
        request: NotificationInboxListRequest,
    ) -> NotificationResult<NotificationInboxPage> {
        validate_list_request(&request)?;
        let cursor = request
            .cursor
            .as_deref()
            .map(decode_inbox_cursor)
            .transpose()?;
        let limit = request.bounded_limit();

        let mut select = notification::Entity::find()
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id));
        if let Some(state) = request.state {
            select = select.filter(notification::Column::State.eq(state));
        }
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

        let mut items = Vec::with_capacity(rows.len());
        for stored in rows {
            let decision = self
                .open
                .authorize_open(NotificationInboxOpenRequest {
                    tenant_id: request.tenant_id,
                    recipient_id: request.recipient_id,
                    notification_id: stored.id,
                })
                .await?;
            if matches!(decision, NotificationInboxOpenDecision::Allowed { .. }) {
                items.push(materialize_inbox_item(stored)?);
            }
        }

        Ok(NotificationInboxPage {
            items,
            next_cursor,
            has_more,
        })
    }
}

#[derive(Clone)]
pub(crate) struct InboxCursor {
    pub(crate) created_at: DateTime<FixedOffset>,
    pub(crate) id: Uuid,
}

pub(crate) fn encode_inbox_cursor(stored: &notification::Model) -> String {
    encode_inbox_position_cursor(&stored.created_at, stored.id)
}

pub(crate) fn encode_inbox_position_cursor(created_at: &DateTime<FixedOffset>, id: Uuid) -> String {
    format!(
        "{INBOX_CURSOR_VERSION}:{}:{}:{}",
        created_at.timestamp(),
        created_at.timestamp_subsec_nanos(),
        id
    )
}

pub(crate) fn decode_inbox_cursor(value: &str) -> NotificationResult<InboxCursor> {
    if value.is_empty()
        || value.len() > MAX_NOTIFICATION_INBOX_CURSOR_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(invalid_inbox_cursor());
    }

    let mut parts = value.splitn(4, ':');
    if parts.next() != Some(INBOX_CURSOR_VERSION) {
        return Err(invalid_inbox_cursor());
    }
    let seconds = parts
        .next()
        .and_then(|part| part.parse::<i64>().ok())
        .ok_or_else(invalid_inbox_cursor)?;
    let nanos = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or_else(invalid_inbox_cursor)?;
    let created_at = DateTime::<Utc>::from_timestamp(seconds, nanos)
        .ok_or_else(invalid_inbox_cursor)?
        .fixed_offset();
    let id = parts
        .next()
        .and_then(|part| Uuid::parse_str(part).ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(invalid_inbox_cursor)?;
    Ok(InboxCursor { created_at, id })
}

fn materialize_inbox_item(
    stored: notification::Model,
) -> NotificationResult<NotificationInboxItem> {
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
        template_data: serde_json::from_value(stored.template_data_json)?,
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

fn validate_open_request(request: &NotificationInboxOpenRequest) -> NotificationResult<()> {
    if request.tenant_id.is_nil()
        || request.recipient_id.is_nil()
        || request.notification_id.is_nil()
    {
        return Err(NotificationError::Validation(
            "notification inbox open identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn validate_list_request(request: &NotificationInboxListRequest) -> NotificationResult<()> {
    if request.tenant_id.is_nil() || request.recipient_id.is_nil() {
        return Err(NotificationError::Validation(
            "notification inbox list identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn invalid_inbox_cursor() -> NotificationError {
    NotificationError::Validation("invalid notification inbox cursor".to_string())
}
