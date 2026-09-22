use async_trait::async_trait;
use rustok_core::events::{EventEnvelope, EventHandler, HandlerResult};
use rustok_events::DomainEvent;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::entities::comment_thread;

pub(crate) struct CommentTargetDeletionHandler {
    db: DatabaseConnection,
}

impl CommentTargetDeletionHandler {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EventHandler for CommentTargetDeletionHandler {
    fn name(&self) -> &'static str {
        "comments_target_deletion"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::TargetDeleted { .. })
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let DomainEvent::TargetDeleted {
            target_type,
            target_id,
        } = &envelope.event
        else {
            return Ok(());
        };

        comment_thread::Entity::delete_many()
            .filter(comment_thread::Column::TenantId.eq(envelope.tenant_id))
            .filter(comment_thread::Column::TargetType.eq(target_type))
            .filter(comment_thread::Column::TargetId.eq(*target_id))
            .exec(&self.db)
            .await?;

        Ok(())
    }
}
