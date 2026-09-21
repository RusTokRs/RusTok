use async_trait::async_trait;
use rustok_core::{
    DomainEvent, EventEnvelope, EventHandler, HandlerResult,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::entities::comment_thread;

pub struct CommentTargetDeletionHandler {
    db: DatabaseConnection,
}

impl CommentTargetDeletionHandler {
    pub fn new(db: DatabaseConnection) -> Self {
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

        // EventEnvelope validation guarantees a tenant-scoped event and a valid target identity.
        // DELETE-by-identity is idempotent, so redelivery after a prior successful cleanup is safe.
        comment_thread::Entity::delete_many()
            .filter(comment_thread::Column::TenantId.eq(envelope.tenant_id))
            .filter(comment_thread::Column::TargetType.eq(target_type))
            .filter(comment_thread::Column::TargetId.eq(*target_id))
            .exec(&self.db)
            .await?;

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rustok_core::{DomainEvent, EventEnvelope, MigrationSource, UserRole};
    use rustok_test_utils::setup_test_db;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set,
    };
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use uuid::Uuid;

    use super::CommentTargetDeletionHandler;
    use crate::{
        CommentsModule, CreateCommentInput, CommentsService,
        migrations,
    };

    async fn setup_comments_db() -> sea_orm::DatabaseConnection {
        let db = setup_test_db().await;
        let manager = SchemaManager::new(&db);
        for migration in CommentsModule.migrations() {
            migration
                .up(&manager)
                .await
                .expect("Comments migration should apply");
        }
        db
    }

    fn event(tenant_id: Uuid, target_id: Uuid) -> EventEnvelope {
        EventEnvelope::new(
            tenant_id,
            None,
            DomainEvent::TargetDeleted {
                target_type: "blog_post".to_string(),
                target_id,
            },
        )
    }

    #[tokio::test]
    async fn target_deletion_removes_thread_and_comments_and_is_idempotent() {
        let db = setup_comments_db().await;
        let tenant_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let service = CommentsService::new(db.clone());

        service
            .create_comment(
                tenant_id,
                rustok_core::SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4())),
                CreateCommentInput {
                    target_type: "blog_post".to_string(),
                    target_id,
                    locale: "en".to_string(),
                    body: CommentsService::test_document_for_tests("hello"),
                    parent_comment_id: None,
                    status: crate::CommentStatus::Pending,
                },
            )
            .await
            .expect("comment should create a target thread");

        let handler = CommentTargetDeletionHandler::new(db.clone());
        let envelope = event(tenant_id, target_id);
        envelope
            .event
            .validate()
            .expect("target deletion event should validate");

        handler
            .handle(&envelope)
            .await
            .expect("target deletion should clean the thread");

        let thread = crate::entities::comment_thread::Entity::find()
            .filter(crate::entities::comment_thread::Column::TenantId.eq(tenant_id))
            .filter(crate::entities::comment_thread::Column::TargetType.eq("blog_post"))
            .filter(crate::entities::comment_thread::Column::TargetId.eq(target_id))
            .one(&db)
            .await
            .expect("thread lookup should succeed");
        assert!(thread.is_none());

        let comments = crate::entities::comment::Entity::find()
            .filter(crate::entities::comment::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await
            .expect("comment lookup should succeed");
        assert!(comments.is_empty());

        handler
            .handle(&envelope)
            .await
            .expect("replayed target deletion should remain idempotent");
    }

    #[test]
    fn handler_owns_only_the_generic_target_deletion_event() {
        let handler = CommentTargetDeletionHandler::new(
            setup_test_db_blocking(),
        );
        let target_id = Uuid::new_v4();
        assert!(handler.handles(&DomainEvent::TargetDeleted {
            target_type: "blog_post".to_string(),
            target_id,
        }));
        assert!(!handler.handles(&DomainEvent::BlogPostDeleted { post_id: target_id }));
    }

    fn setup_test_db_blocking() -> sea_orm::DatabaseConnection {
        // This test only checks the routing predicate; a placeholder connection is never used.
        // The helper is kept local so no async runtime is needed for the assertion.
        futures::executor::block_on(setup_test_db())
    }
}
