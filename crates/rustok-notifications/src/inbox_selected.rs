use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{NotificationError, NotificationResult};
use crate::inbox::MAX_NOTIFICATION_INBOX_PAGE_SIZE;
use crate::inbox_state::{
    NotificationInboxStateDecision, NotificationInboxStateRequest, NotificationInboxStateService,
};

pub const MAX_NOTIFICATION_INBOX_SELECTED_IDS: usize =
    MAX_NOTIFICATION_INBOX_PAGE_SIZE as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationInboxSelectedAction {
    MarkSeen,
    MarkRead,
    MarkUnread,
    Archive,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxSelectedStateRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    pub action: NotificationInboxSelectedAction,
    pub notification_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxSelectedStateResult {
    pub requested: u16,
    pub changed: u16,
    pub not_changed: u16,
}

/// Applies one bounded state action to an explicit set of exact-recipient notifications.
///
/// Input order is preserved and every identity is delegated to the exact state owner. Duplicate,
/// nil, empty, and oversized selections fail before any mutation. Missing, cross-tenant,
/// cross-recipient, already-satisfied, and protected-state rows are all counted as `not_changed`,
/// so the response does not expose which supplied identities exist. Earlier exact transitions stay
/// durable and idempotent if a later database operation fails. No recipient-policy, source, target,
/// or delivery owner is invoked.
#[derive(Clone)]
pub struct NotificationInboxSelectedStateService {
    state: NotificationInboxStateService,
}

impl NotificationInboxSelectedStateService {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            state: NotificationInboxStateService::new(db),
        }
    }

    pub async fn apply(
        &self,
        request: NotificationInboxSelectedStateRequest,
    ) -> NotificationResult<NotificationInboxSelectedStateResult> {
        validate_request(&request)?;
        let requested = request.notification_ids.len() as u16;
        let mut changed = 0_u16;

        for notification_id in request.notification_ids {
            let state_request = NotificationInboxStateRequest {
                tenant_id: request.tenant_id,
                recipient_id: request.recipient_id,
                notification_id,
            };
            let decision = match request.action {
                NotificationInboxSelectedAction::MarkSeen => {
                    self.state.mark_seen(state_request).await?
                }
                NotificationInboxSelectedAction::MarkRead => {
                    self.state.mark_read(state_request).await?
                }
                NotificationInboxSelectedAction::MarkUnread => {
                    self.state.mark_unread(state_request).await?
                }
                NotificationInboxSelectedAction::Archive => {
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

        Ok(NotificationInboxSelectedStateResult {
            requested,
            changed,
            not_changed: requested - changed,
        })
    }
}

fn validate_request(request: &NotificationInboxSelectedStateRequest) -> NotificationResult<()> {
    if request.tenant_id.is_nil() || request.recipient_id.is_nil() {
        return Err(NotificationError::Validation(
            "notification inbox selected-state owner identity must not be nil".to_string(),
        ));
    }
    if request.notification_ids.is_empty() {
        return Err(NotificationError::Validation(
            "notification inbox selected-state selection must not be empty".to_string(),
        ));
    }
    if request.notification_ids.len() > MAX_NOTIFICATION_INBOX_SELECTED_IDS {
        return Err(NotificationError::Validation(format!(
            "notification inbox selected-state selection exceeds {MAX_NOTIFICATION_INBOX_SELECTED_IDS} identities"
        )));
    }

    let mut unique = HashSet::with_capacity(request.notification_ids.len());
    for notification_id in &request.notification_ids {
        if notification_id.is_nil() {
            return Err(NotificationError::Validation(
                "notification inbox selected-state notification identity must not be nil"
                    .to_string(),
            ));
        }
        if !unique.insert(*notification_id) {
            return Err(NotificationError::Validation(
                "notification inbox selected-state selection must not contain duplicates"
                    .to_string(),
            ));
        }
    }
    Ok(())
}
