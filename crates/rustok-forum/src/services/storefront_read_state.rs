use std::collections::{HashMap, HashSet};

use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    Statement, Value,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::entities::{forum_reply, forum_topic, forum_topic_revision};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::services::{ForumTopicReadState, ForumTopicReadStateService, MarkForumTopicReadInput};
use crate::state_machine::ReplyStatus;

const MAX_STOREFRONT_UNREAD_TOPIC_IDS: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumTopicUnreadSummary {
    pub topic_id: Uuid,
    pub read_state_explicit: bool,
    pub last_read_position: i64,
    pub last_read_revision: i64,
    pub unread_count: i64,
    pub has_unread_topic_revision: bool,
    pub is_unread: bool,
}

/// Bounded owner facade used by storefront transports after visibility filtering.
///
/// This service never decides storefront visibility. Callers must first obtain a
/// bounded topic page through the Forum storefront-visible topic read contract,
/// then pass only those topic IDs here for canonical unread enrichment.
pub struct ForumStorefrontReadStateService {
    db: DatabaseConnection,
}

impl ForumStorefrontReadStateService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn summarize_topics(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_ids: Vec<Uuid>,
    ) -> ForumResult<Vec<ForumTopicUnreadSummary>> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        let user_id = security.user_id.ok_or_else(|| {
            ForumError::forbidden(
                "Authenticated user context is required for storefront topic unread summaries",
            )
        })?;
        if topic_ids.len() > MAX_STOREFRONT_UNREAD_TOPIC_IDS {
            return Err(ForumError::Validation(format!(
                "Storefront topic unread summaries are limited to {MAX_STOREFRONT_UNREAD_TOPIC_IDS} topic IDs"
            )));
        }

        let mut seen = HashSet::with_capacity(topic_ids.len());
        let topic_ids = topic_ids
            .into_iter()
            .filter(|topic_id| seen.insert(*topic_id))
            .collect::<Vec<_>>();
        if topic_ids.is_empty() {
            return Ok(Vec::new());
        }

        let summaries = unread_summaries(&self.db, tenant_id, user_id, &topic_ids).await?;
        Ok(topic_ids
            .into_iter()
            .filter_map(|topic_id| summaries.get(&topic_id).cloned())
            .collect())
    }

    /// Marks the latest approved reply position and immutable topic revision
    /// observed by the owner service. Content published after this snapshot
    /// remains unread instead of being accidentally acknowledged.
    pub async fn mark_topic_read_current(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumTopicReadState> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        if security.user_id.is_none() {
            return Err(ForumError::forbidden(
                "Authenticated user context is required to mark a storefront topic read",
            ));
        }

        let topic = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))?;
        let last_read_position = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic.id))
            .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
            .order_by_desc(forum_reply::Column::Position)
            .one(&self.db)
            .await?
            .map(|reply| reply.position)
            .unwrap_or(0);
        let last_read_revision = forum_topic_revision::Entity::find()
            .filter(forum_topic_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_revision::Column::TopicId.eq(topic.id))
            .order_by_desc(forum_topic_revision::Column::Id)
            .one(&self.db)
            .await?
            .map(|revision| revision.id)
            .unwrap_or(0);

        ForumTopicReadStateService::new(self.db.clone())
            .mark_topic_read(
                tenant_id,
                topic_id,
                security,
                MarkForumTopicReadInput {
                    last_read_position,
                    last_read_revision,
                },
            )
            .await
    }
}

async fn unread_summaries(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, ForumTopicUnreadSummary>> {
    let placeholders = (0..topic_ids.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        r#"
SELECT
    topic.id AS topic_id,
    state.user_id AS state_user_id,
    COALESCE(state.last_read_position, 0) AS last_read_position,
    COALESCE(state.last_read_revision, 0) AS last_read_revision,
    COUNT(DISTINCT unread_reply.id) AS unread_count,
    COUNT(DISTINCT unread_revision.id) AS unread_revision_count
FROM forum_topics topic
LEFT JOIN forum_topic_read_states state
  ON state.tenant_id = topic.tenant_id
 AND state.topic_id = topic.id
 AND state.user_id = ?
LEFT JOIN forum_replies unread_reply
  ON unread_reply.tenant_id = topic.tenant_id
 AND unread_reply.topic_id = topic.id
 AND unread_reply.status = 'approved'
 AND (
      unread_reply.position > COALESCE(state.last_read_position, 0)
      OR unread_reply.updated_at > state.updated_at
 )
LEFT JOIN forum_topic_revisions unread_revision
  ON unread_revision.tenant_id = topic.tenant_id
 AND unread_revision.topic_id = topic.id
 AND unread_revision.id > COALESCE(state.last_read_revision, 0)
WHERE topic.tenant_id = ?
  AND topic.id IN ({placeholders})
GROUP BY
    topic.id,
    state.user_id,
    state.last_read_position,
    state.last_read_revision
"#,
    );
    let mut values = Vec::<Value>::with_capacity(topic_ids.len() + 2);
    values.push(user_id.into());
    values.push(tenant_id.into());
    for topic_id in topic_ids {
        values.push((*topic_id).into());
    }

    let rows = db
        .query_all(Statement::from_sql_and_values(
            db.get_database_backend(),
            sql,
            values,
        ))
        .await?;
    let mut summaries = HashMap::with_capacity(rows.len());
    for row in rows {
        let topic_id = row.try_get::<Uuid>("", "topic_id")?;
        let read_state_explicit = row
            .try_get::<Option<Uuid>>("", "state_user_id")?
            .is_some();
        let last_read_position = row.try_get::<i64>("", "last_read_position")?;
        let last_read_revision = row.try_get::<i64>("", "last_read_revision")?;
        let unread_count = row.try_get::<i64>("", "unread_count")?;
        let has_unread_topic_revision = row.try_get::<i64>("", "unread_revision_count")? > 0;
        summaries.insert(
            topic_id,
            ForumTopicUnreadSummary {
                topic_id,
                read_state_explicit,
                last_read_position,
                last_read_revision,
                unread_count,
                has_unread_topic_revision,
                is_unread: !read_state_explicit
                    || unread_count > 0
                    || has_unread_topic_revision,
            },
        );
    }
    Ok(summaries)
}
