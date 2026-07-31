use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::dto::{
    ForumDomainEventQuery, ForumDomainEventResponse, ForumProjectionOwnerRevisionImpact,
    ForumProjectionOwnerRevisionResponse,
};
use crate::entities::forum_domain_event;
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

const DEFAULT_EVENT_LIMIT: u64 = 50;
const MAX_EVENT_LIMIT: u64 = 100;
pub const MAX_FORUM_PROJECTION_OWNER_REVISION_PAGE: usize = 100;

#[derive(Clone)]
pub struct ForumEventService {
    db: DatabaseConnection,
}

impl ForumEventService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: ForumDomainEventQuery,
    ) -> ForumResult<Vec<ForumDomainEventResponse>> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;

        let after_sequence = query.after_sequence.unwrap_or(0);
        if after_sequence < 0 {
            return Err(ForumError::Validation(
                "after_sequence must not be negative".to_string(),
            ));
        }

        let limit = query
            .limit
            .unwrap_or(DEFAULT_EVENT_LIMIT)
            .clamp(1, MAX_EVENT_LIMIT);

        let mut select = forum_domain_event::Entity::find()
            .filter(forum_domain_event::Column::TenantId.eq(tenant_id))
            .filter(forum_domain_event::Column::SequenceNo.gt(after_sequence));

        if let Some(aggregate_type) = normalize_filter(query.aggregate_type, "aggregate_type")? {
            select = select.filter(forum_domain_event::Column::AggregateType.eq(aggregate_type));
        }
        if let Some(aggregate_id) = query.aggregate_id {
            select = select.filter(forum_domain_event::Column::AggregateId.eq(aggregate_id));
        }
        if let Some(event_type) = normalize_filter(query.event_type, "event_type")? {
            select = select.filter(forum_domain_event::Column::EventType.eq(event_type));
        }

        let events = select
            .order_by_asc(forum_domain_event::Column::SequenceNo)
            .limit(limit)
            .all(&self.db)
            .await?;

        Ok(events
            .into_iter()
            .map(|event| ForumDomainEventResponse {
                sequence_no: event.sequence_no,
                event_id: event.event_id,
                tenant_id: event.tenant_id,
                aggregate_type: event.aggregate_type,
                aggregate_id: event.aggregate_id,
                event_type: event.event_type,
                schema_version: event.schema_version,
                actor_id: event.actor_id,
                payload: event.payload,
                created_at: event.created_at.to_rfc3339(),
            })
            .collect())
    }

    /// Reads the immutable Forum owner journal as a bounded projection-revision
    /// source. This internal owner boundary deliberately omits journal payload,
    /// actor and user-facing authorization state.
    pub async fn list_projection_owner_revisions(
        &self,
        tenant_id: Uuid,
        after_owner_revision: i64,
        limit: usize,
    ) -> ForumResult<Vec<ForumProjectionOwnerRevisionResponse>> {
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "projection owner revision tenant must not be nil".to_string(),
            ));
        }
        if after_owner_revision < 0 {
            return Err(ForumError::Validation(
                "after_owner_revision must not be negative".to_string(),
            ));
        }
        if !(1..=MAX_FORUM_PROJECTION_OWNER_REVISION_PAGE).contains(&limit) {
            return Err(ForumError::Validation(format!(
                "projection owner revision limit must be between 1 and {MAX_FORUM_PROJECTION_OWNER_REVISION_PAGE}"
            )));
        }

        let events = forum_domain_event::Entity::find()
            .filter(forum_domain_event::Column::TenantId.eq(tenant_id))
            .filter(forum_domain_event::Column::SequenceNo.gt(after_owner_revision))
            .order_by_asc(forum_domain_event::Column::SequenceNo)
            .limit(limit as u64)
            .all(&self.db)
            .await?;

        Ok(events
            .into_iter()
            .map(|event| ForumProjectionOwnerRevisionResponse {
                owner_revision: event.sequence_no,
                event_id: event.event_id,
                impact: projection_revision_impact(&event.event_type),
                event_type: event.event_type,
            })
            .collect())
    }
}

fn projection_revision_impact(event_type: &str) -> ForumProjectionOwnerRevisionImpact {
    match event_type {
        "forum.topic.vote_changed"
        | "forum.reply.vote_changed"
        | "forum.category.subscription_changed"
        | "forum.topic.subscription_changed"
        | "forum.mention.user_added"
        | "forum.mention.audience_added" => {
            ForumProjectionOwnerRevisionImpact::NoProjectionChange
        }
        // Unknown future Forum journal types fail safe. Until the owner contract
        // explicitly classifies them, Search reconciliation must assume that the
        // public projection may have changed.
        _ => ForumProjectionOwnerRevisionImpact::FullRebuild,
    }
}

fn normalize_filter(value: Option<String>, field: &str) -> ForumResult<Option<String>> {
    value
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                return Err(ForumError::Validation(format!("{field} must not be empty")));
            }
            if normalized.len() > 96 {
                return Err(ForumError::Validation(format!(
                    "{field} must not exceed 96 characters"
                )));
            }
            Ok(normalized)
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::projection_revision_impact;
    use crate::ForumProjectionOwnerRevisionImpact;

    #[test]
    fn non_projection_events_are_explicitly_classified() {
        for event_type in [
            "forum.topic.vote_changed",
            "forum.reply.vote_changed",
            "forum.category.subscription_changed",
            "forum.topic.subscription_changed",
            "forum.mention.user_added",
            "forum.mention.audience_added",
        ] {
            assert_eq!(
                projection_revision_impact(event_type),
                ForumProjectionOwnerRevisionImpact::NoProjectionChange
            );
        }
    }

    #[test]
    fn projection_and_unknown_events_fail_safe_to_full_rebuild() {
        for event_type in [
            "forum.category.updated",
            "forum.topic.updated",
            "forum.reply.status_changed",
            "forum.solution.marked",
            "forum.topic.tags_changed",
            "forum.future_projection_event",
        ] {
            assert_eq!(
                projection_revision_impact(event_type),
                ForumProjectionOwnerRevisionImpact::FullRebuild
            );
        }
    }
}
