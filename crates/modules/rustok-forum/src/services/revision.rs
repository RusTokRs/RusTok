use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::{ReplyRevisionResponse, TopicRevisionResponse};
use crate::entities::{forum_reply_revision, forum_topic_revision};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

const MAX_REVISION_PAGE_SIZE: u64 = 100;

pub struct RevisionService {
    db: DatabaseConnection,
}

impl RevisionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Return the exact current Forum-owned topic revision used by consumers
    /// that need a stable owner revision. Callers must authorize topic access
    /// before exposing this value outside the Forum owner boundary.
    pub async fn current_topic_revision(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<u64> {
        let latest = forum_topic_revision::Entity::find()
            .select_only()
            .column(forum_topic_revision::Column::Id)
            .filter(forum_topic_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_revision::Column::TopicId.eq(topic_id))
            .order_by_desc(forum_topic_revision::Column::Id)
            .into_tuple::<i64>()
            .one(&self.db)
            .await?;
        current_revision_after(latest)
    }

    /// Return the exact current Forum-owned reply revision. Authorization stays
    /// with the caller because this service owns revision state, not visibility.
    pub async fn current_reply_revision(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<u64> {
        let latest = forum_reply_revision::Entity::find()
            .select_only()
            .column(forum_reply_revision::Column::Id)
            .filter(forum_reply_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_revision::Column::ReplyId.eq(reply_id))
            .order_by_desc(forum_reply_revision::Column::Id)
            .into_tuple::<i64>()
            .one(&self.db)
            .await?;
        current_revision_after(latest)
    }

    #[instrument(skip(self, security))]
    pub async fn list_topic_revisions(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: Option<&str>,
        limit: u64,
        security: SecurityContext,
    ) -> ForumResult<Vec<TopicRevisionResponse>> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;

        let mut query = forum_topic_revision::Entity::find()
            .filter(forum_topic_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_revision::Column::TopicId.eq(topic_id));
        if let Some(locale) = locale {
            query = query.filter(forum_topic_revision::Column::Locale.eq(locale));
        }

        query
            .order_by_desc(forum_topic_revision::Column::Id)
            .limit(limit.clamp(1, MAX_REVISION_PAGE_SIZE))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|row| {
                Ok(TopicRevisionResponse {
                    id: row.id,
                    topic_id: row.topic_id,
                    locale: row.locale,
                    title: row.title,
                    slug: row.slug,
                    body: crate::richtext::project_stored_discussion(&row.body)?.view,
                    metadata: row.metadata,
                    revision_reason: row.revision_reason,
                    created_at: row.created_at.to_rfc3339(),
                })
            })
            .collect::<ForumResult<Vec<_>>>()
    }

    #[instrument(skip(self, security))]
    pub async fn list_reply_revisions(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        locale: Option<&str>,
        limit: u64,
        security: SecurityContext,
    ) -> ForumResult<Vec<ReplyRevisionResponse>> {
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;

        let mut query = forum_reply_revision::Entity::find()
            .filter(forum_reply_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_revision::Column::ReplyId.eq(reply_id));
        if let Some(locale) = locale {
            query = query.filter(forum_reply_revision::Column::Locale.eq(locale));
        }

        query
            .order_by_desc(forum_reply_revision::Column::Id)
            .limit(limit.clamp(1, MAX_REVISION_PAGE_SIZE))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|row| {
                Ok(ReplyRevisionResponse {
                    id: row.id,
                    reply_id: row.reply_id,
                    locale: row.locale,
                    body: crate::richtext::project_stored_discussion(&row.body)?.view,
                    revision_reason: row.revision_reason,
                    created_at: row.created_at.to_rfc3339(),
                })
            })
            .collect::<ForumResult<Vec<_>>>()
    }
}

fn current_revision_after(latest: Option<i64>) -> ForumResult<u64> {
    match latest {
        None => Ok(1),
        Some(latest) => u64::try_from(latest)
            .ok()
            .and_then(|revision| revision.checked_add(1))
            .filter(|revision| *revision > 0)
            .ok_or_else(ForumError::relation_revision_unavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::current_revision_after;

    #[test]
    fn owner_revision_is_positive_and_advances_after_captured_history() {
        assert_eq!(current_revision_after(None).expect("initial revision"), 1);
        assert_eq!(
            current_revision_after(Some(41)).expect("advanced revision"),
            42
        );
        assert!(current_revision_after(Some(-1)).is_err());
        assert!(current_revision_after(Some(i64::MAX)).is_ok());
    }
}
