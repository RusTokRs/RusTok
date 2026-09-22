use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::notification;
use crate::error::{NotificationError, NotificationResult};
use crate::model::NotificationState;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxUnreadCountRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationInboxUnreadCount {
    pub unread_count: u64,
}

/// Counts exact-recipient inbox rows whose owner state is unread.
///
/// The owner query is scoped by tenant and recipient before applying the unread-state filter.
/// Missing, cross-tenant, and empty recipient scopes all return zero, so this read does not expose
/// notification identity. It does not invoke recipient privacy, source authorization, target, or
/// delivery owners and does not mutate inbox or delivery state.
#[derive(Clone)]
pub struct NotificationInboxUnreadCountService {
    db: DatabaseConnection,
}

impl NotificationInboxUnreadCountService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn count_unread(
        &self,
        request: NotificationInboxUnreadCountRequest,
    ) -> NotificationResult<NotificationInboxUnreadCount> {
        validate_request(&request)?;
        let unread_count = notification::Entity::find()
            .filter(notification::Column::TenantId.eq(request.tenant_id))
            .filter(notification::Column::RecipientId.eq(request.recipient_id))
            .filter(notification::Column::State.eq(NotificationState::Unread))
            .count(&self.db)
            .await?;
        Ok(NotificationInboxUnreadCount { unread_count })
    }
}

fn validate_request(request: &NotificationInboxUnreadCountRequest) -> NotificationResult<()> {
    if request.tenant_id.is_nil() || request.recipient_id.is_nil() {
        return Err(NotificationError::Validation(
            "notification inbox unread count identity must not be nil".to_string(),
        ));
    }
    Ok(())
}
