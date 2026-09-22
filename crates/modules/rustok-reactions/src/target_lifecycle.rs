use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::events::{EventEnvelope, EventHandler, HandlerResult};
use rustok_events::DomainEvent;
use rustok_reactions_api::ReactionSubjectRegistry;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};

use crate::entities::subject;

pub(crate) struct ReactionTargetDeletionHandler {
    db: DatabaseConnection,
    subjects: Arc<ReactionSubjectRegistry>,
}

impl ReactionTargetDeletionHandler {
    pub(crate) fn new(
        db: DatabaseConnection,
        subjects: Arc<ReactionSubjectRegistry>,
    ) -> Self {
        Self { db, subjects }
    }
}

#[async_trait]
impl EventHandler for ReactionTargetDeletionHandler {
    fn name(&self) -> &'static str {
        "reactions_target_deletion"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::TargetDeleted { .. })
            && !self.subjects.deletion_bindings().is_empty()
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let DomainEvent::TargetDeleted {
            target_type,
            target_id,
        } = &envelope.event
        else {
            return Ok(());
        };

        let bindings = self
            .subjects
            .deletion_bindings()
            .into_iter()
            .filter(|binding| binding.target_type == *target_type)
            .collect::<Vec<_>>();
        if bindings.is_empty() {
            return Ok(());
        }

        let transaction = self.db.begin().await?;
        for binding in bindings {
            subject::Entity::delete_many()
                .filter(subject::Column::TenantId.eq(envelope.tenant_id))
                .filter(subject::Column::SourceSlug.eq(binding.source.as_str()))
                .filter(subject::Column::SubjectKind.eq(binding.kind.as_str()))
                .filter(subject::Column::SubjectId.eq(*target_id))
                .exec(&transaction)
                .await?;
        }
        transaction.commit().await?;

        Ok(())
    }
}
