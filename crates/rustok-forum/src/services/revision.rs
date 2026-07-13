use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::{ReplyRevisionResponse, TopicRevisionResponse};
use crate::entities::{forum_reply_revision, forum_topic_revision};
use crate::error::ForumResult;
use crate::services::rbac::enforce_scope;

const MAX_REVISION_PAGE_SIZE: u64 = 100;

pub struct RevisionService {
    db: DatabaseConnection,
}

impl RevisionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
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

        Ok(query
            .order_by_desc(forum_topic_revision::Column::Id)
            .limit(limit.clamp(1, MAX_REVISION_PAGE_SIZE))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|row| TopicRevisionResponse {
                id: row.id,
                topic_id: row.topic_id,
                locale: row.locale,
                title: row.title,
                slug: row.slug,
                body: row.body,
                body_format: row.body_format,
                metadata: row.metadata,
                revision_reason: row.revision_reason,
                created_at: row.created_at.to_rfc3339(),
            })
            .collect())
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

        Ok(query
            .order_by_desc(forum_reply_revision::Column::Id)
            .limit(limit.clamp(1, MAX_REVISION_PAGE_SIZE))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|row| ReplyRevisionResponse {
                id: row.id,
                reply_id: row.reply_id,
                locale: row.locale,
                body: row.body,
                body_format: row.body_format,
                revision_reason: row.revision_reason,
                created_at: row.created_at.to_rfc3339(),
            })
            .collect())
    }
}
