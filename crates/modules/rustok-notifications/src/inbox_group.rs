use std::sync::Arc;

use rustok_notifications_api::{
    NotificationPriority, NotificationSourceRegistry, NotificationSourceSlug,
    NotificationTemplateData, NotificationTemplateKey, NotificationTypeKey,
};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::candidate::NotificationRecipientPolicy;
use crate::entities::notification;
use crate::error::{NotificationError, NotificationResult};
use crate::inbox::{
    DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE, MAX_NOTIFICATION_INBOX_PAGE_SIZE, NotificationInboxItem,
    NotificationInboxOpenDecision, NotificationInboxOpenRequest, NotificationInboxOpenService,
    NotificationInboxPage, decode_inbox_cursor, encode_inbox_cursor,
};
use crate::model::{NotificationPriorityValue, NotificationState};

pub const MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES: usize = 191;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxGroupListRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    pub group_key: String,
    #[serde(default)]
    pub state: Option<NotificationState>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

impl NotificationInboxGroupListRequest {
    pub fn bounded_limit(&self) -> u64 {
        let requested = if self.limit == 0 {
            DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE
        } else {
            self.limit
        };
        u64::from(requested.min(MAX_NOTIFICATION_INBOX_PAGE_SIZE))
    }
}

/// Lists one bounded exact-recipient page for one exact stored notification group.
///
/// Raw rows are selected by tenant, recipient, and the opaque group key before current recipient
/// privacy and source target authorization are rechecked through `NotificationInboxOpenService`.
/// The cursor advances by the last scanned raw row, so a fully suppressed group page may be empty
/// while still carrying continuation. Missing groups and foreign recipients are indistinguishably
/// empty. This read mutates no inbox state and creates no delivery attempt.
#[derive(Clone)]
pub struct NotificationInboxGroupListService {
    db: DatabaseConnection,
    open: NotificationInboxOpenService,
}

impl NotificationInboxGroupListService {
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
        request: NotificationInboxGroupListRequest,
    ) -> NotificationResult<NotificationInboxPage> {
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
            .filter(notification::Column::GroupKey.eq(request.group_key.as_str()));
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
                items.push(materialize_item(stored)?);
            }
        }

        Ok(NotificationInboxPage {
            items,
            next_cursor,
            has_more,
        })
    }
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

fn validate_request(request: &NotificationInboxGroupListRequest) -> NotificationResult<()> {
    if request.tenant_id.is_nil() || request.recipient_id.is_nil() {
        return Err(NotificationError::Validation(
            "notification inbox group list identity must not be nil".to_string(),
        ));
    }
    validate_inbox_group_key(&request.group_key)
}

pub(crate) fn validate_inbox_group_key(group_key: &str) -> NotificationResult<()> {
    if group_key.is_empty()
        || group_key != group_key.trim()
        || group_key.len() > MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES
        || group_key.chars().any(char::is_control)
    {
        return Err(NotificationError::Validation(format!(
            "notification inbox group key must contain between 1 and {MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES} safe bytes"
        )));
    }
    Ok(())
}
