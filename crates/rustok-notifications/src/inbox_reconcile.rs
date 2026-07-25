use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use rustok_notifications_api::NotificationSourceRegistry;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::candidate::NotificationRecipientPolicy;
use crate::entities::notification;
use crate::error::{NotificationError, NotificationResult};
use crate::inbox::{
    DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE, MAX_NOTIFICATION_INBOX_CURSOR_BYTES,
    MAX_NOTIFICATION_INBOX_PAGE_SIZE, NotificationInboxOpenDecision, NotificationInboxOpenRequest,
    NotificationInboxOpenService,
};
use crate::inbox_state::{
    NotificationInboxStateDecision, NotificationInboxStateRequest, NotificationInboxStateService,
};
use crate::model::NotificationState;

const RECONCILE_CURSOR_VERSION: &str = "ir1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxReconcileRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

impl NotificationInboxReconcileRequest {
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
pub struct NotificationInboxReconcilePage {
    pub scanned: u16,
    pub archived: u16,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// Rechecks one bounded exact-recipient inbox page and archives rows that are no longer available.
///
/// Raw rows are selected outside foreign owner calls. Each row then reuses the existing open-time
/// privacy and source authorization pipeline. An `Unavailable` decision is persisted through the
/// exact-recipient state owner. Retryable owner failures stop the page; earlier per-row archives are
/// durable and safe to revisit because archive is idempotent.
#[derive(Clone)]
pub struct NotificationInboxReconcileService {
    db: DatabaseConnection,
    open: NotificationInboxOpenService,
    state: NotificationInboxStateService,
}

impl NotificationInboxReconcileService {
    pub fn new(
        db: DatabaseConnection,
        registry: Arc<NotificationSourceRegistry>,
        policy: Arc<dyn NotificationRecipientPolicy>,
    ) -> Self {
        Self {
            db: db.clone(),
            open: NotificationInboxOpenService::new(db.clone(), registry, policy),
            state: NotificationInboxStateService::new(db),
        }
    }

    pub async fn reconcile_page(
        &self,
        request: NotificationInboxReconcileRequest,
    ) -> NotificationResult<NotificationInboxReconcilePage> {
        validate_request(&request)?;
        let cursor = request
            .cursor
            .as_deref()
            .map(decode_cursor)
            .transpose()?;
        let limit = request.bounded_limit();

        let mut select = notification::Entity::find()
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .filter(notification::Column::State.ne(NotificationState::Archived));
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
            .then(|| rows.last().map(encode_cursor))
            .flatten();
        let scanned = rows.len() as u16;
        let mut archived = 0_u16;

        for stored in rows {
            let identity = NotificationInboxStateRequest {
                tenant_id: request.tenant_id,
                recipient_id: request.recipient_id,
                notification_id: stored.id,
            };
            match self
                .open
                .authorize_open(NotificationInboxOpenRequest {
                    tenant_id: identity.tenant_id,
                    recipient_id: identity.recipient_id,
                    notification_id: identity.notification_id,
                })
                .await?
            {
                NotificationInboxOpenDecision::Allowed { .. } => {}
                NotificationInboxOpenDecision::Unavailable => {
                    if matches!(
                        self.state.archive(identity).await?,
                        NotificationInboxStateDecision::Available { changed: true, .. }
                    ) {
                        archived += 1;
                    }
                }
            }
        }

        Ok(NotificationInboxReconcilePage {
            scanned,
            archived,
            next_cursor,
            has_more,
        })
    }
}

#[derive(Clone)]
struct ReconcileCursor {
    created_at: DateTime<FixedOffset>,
    id: Uuid,
}

fn encode_cursor(stored: &notification::Model) -> String {
    format!(
        "{RECONCILE_CURSOR_VERSION}:{}:{}:{}",
        stored.created_at.timestamp(),
        stored.created_at.timestamp_subsec_nanos(),
        stored.id
    )
}

fn decode_cursor(value: &str) -> NotificationResult<ReconcileCursor> {
    if value.is_empty()
        || value.len() > MAX_NOTIFICATION_INBOX_CURSOR_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(invalid_cursor());
    }

    let mut parts = value.splitn(4, ':');
    if parts.next() != Some(RECONCILE_CURSOR_VERSION) {
        return Err(invalid_cursor());
    }
    let seconds = parts
        .next()
        .and_then(|part| part.parse::<i64>().ok())
        .ok_or_else(invalid_cursor)?;
    let nanos = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or_else(invalid_cursor)?;
    let created_at = DateTime::<Utc>::from_timestamp(seconds, nanos)
        .ok_or_else(invalid_cursor)?
        .fixed_offset();
    let id = parts
        .next()
        .and_then(|part| Uuid::parse_str(part).ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(invalid_cursor)?;
    Ok(ReconcileCursor { created_at, id })
}

fn validate_request(request: &NotificationInboxReconcileRequest) -> NotificationResult<()> {
    if request.tenant_id.is_nil() || request.recipient_id.is_nil() {
        return Err(NotificationError::Validation(
            "notification inbox reconciliation identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn invalid_cursor() -> NotificationError {
    NotificationError::Validation("invalid notification inbox reconciliation cursor".to_string())
}
