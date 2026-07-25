use std::sync::Arc;

use rustok_notifications_api::{
    AuthorizeNotificationTargetRequest, NotificationOpenAuthorization, NotificationSourceRegistry,
    NotificationSourceSlug, NotificationTargetKind, NotificationTargetRef, NotificationTargetRoute,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::notification;
use crate::error::{NotificationError, NotificationResult};

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

/// Authorizes one stored notification target at the moment an exact recipient opens it.
///
/// This service intentionally returns only a fresh owner-provided route. It does not expose the
/// stored notification row, mutate inbox state, or enqueue a delivery attempt. Missing and
/// foreign-recipient rows both fail closed as `Unavailable`, preventing a notification-existence
/// oracle across recipients or tenants.
#[derive(Clone)]
pub struct NotificationInboxOpenService {
    db: DatabaseConnection,
    registry: Arc<NotificationSourceRegistry>,
}

impl NotificationInboxOpenService {
    pub fn new(db: DatabaseConnection, registry: Arc<NotificationSourceRegistry>) -> Self {
        Self { db, registry }
    }

    pub async fn authorize_open(
        &self,
        request: NotificationInboxOpenRequest,
    ) -> NotificationResult<NotificationInboxOpenDecision> {
        validate_request(&request)?;

        let stored = notification::Entity::find_by_id(request.notification_id)
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .one(&self.db)
            .await?;
        let Some(stored) = stored else {
            return Ok(NotificationInboxOpenDecision::Unavailable);
        };

        let source = NotificationSourceSlug::new(stored.source_slug)
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

fn validate_request(request: &NotificationInboxOpenRequest) -> NotificationResult<()> {
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
