use chrono::Utc;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    TransactionTrait, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::entities::{
    forum_reply, forum_topic, forum_topic_read_state, forum_topic_revision,
};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::state_machine::ReplyStatus;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MarkForumTopicReadInput {
    pub last_read_position: i64,
    pub last_read_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumTopicReadState {
    pub tenant_id: Uuid,
    pub topic_id: Uuid,
    pub user_id: Option<Uuid>,
    pub last_read_position: i64,
    pub last_read_revision: i64,
    pub explicit: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

pub struct ForumTopicReadStateService {
    db: DatabaseConnection,
}

impl ForumTopicReadStateService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get_topic_read_state(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumTopicReadState> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        ensure_topic_exists(&self.db, tenant_id, topic_id).await?;

        let Some(user_id) = security.user_id else {
            return Ok(implicit_state(tenant_id, topic_id, None));
        };

        let state = forum_topic_read_state::Entity::find()
            .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_read_state::Column::TopicId.eq(topic_id))
            .filter(forum_topic_read_state::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?;

        Ok(state
            .map(explicit_state)
            .unwrap_or_else(|| implicit_state(tenant_id, topic_id, Some(user_id))))
    }

    pub async fn mark_topic_read(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: MarkForumTopicReadInput,
    ) -> ForumResult<ForumTopicReadState> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = security.user_id.ok_or_else(|| {
            ForumError::forbidden("Authenticated user context is required to mark a topic read")
        })?;
        validate_nonnegative(&input)?;

        let txn = self.db.begin().await?;
        let topic = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))?;

        let latest_public_position = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic.id))
            .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
            .order_by_desc(forum_reply::Column::Position)
            .one(&txn)
            .await?
            .map(|reply| reply.position)
            .unwrap_or(0);
        if input.last_read_position > latest_public_position {
            return Err(ForumError::Validation(format!(
                "Forum topic read position {} exceeds latest public position {latest_public_position}",
                input.last_read_position
            )));
        }

        let latest_topic_revision = forum_topic_revision::Entity::find()
            .filter(forum_topic_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_revision::Column::TopicId.eq(topic.id))
            .order_by_desc(forum_topic_revision::Column::Id)
            .one(&txn)
            .await?
            .map(|revision| revision.id)
            .unwrap_or(0);
        if input.last_read_revision > latest_topic_revision {
            return Err(ForumError::Validation(format!(
                "Forum topic read revision {} exceeds latest topic revision {latest_topic_revision}",
                input.last_read_revision
            )));
        }

        let now = Utc::now();
        forum_topic_read_state::Entity::insert(forum_topic_read_state::ActiveModel {
            tenant_id: Set(tenant_id),
            topic_id: Set(topic.id),
            user_id: Set(user_id),
            last_read_position: Set(input.last_read_position),
            last_read_revision: Set(input.last_read_revision),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        })
        .on_conflict(
            OnConflict::columns([
                forum_topic_read_state::Column::TenantId,
                forum_topic_read_state::Column::TopicId,
                forum_topic_read_state::Column::UserId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&txn)
        .await?;

        forum_topic_read_state::Entity::update_many()
            .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_read_state::Column::TopicId.eq(topic.id))
            .filter(forum_topic_read_state::Column::UserId.eq(user_id))
            .filter(
                forum_topic_read_state::Column::LastReadPosition.lt(input.last_read_position),
            )
            .set(forum_topic_read_state::ActiveModel {
                last_read_position: Set(input.last_read_position),
                updated_at: Set(now.into()),
                ..Default::default()
            })
            .exec(&txn)
            .await?;

        forum_topic_read_state::Entity::update_many()
            .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_read_state::Column::TopicId.eq(topic.id))
            .filter(forum_topic_read_state::Column::UserId.eq(user_id))
            .filter(
                forum_topic_read_state::Column::LastReadRevision.lt(input.last_read_revision),
            )
            .set(forum_topic_read_state::ActiveModel {
                last_read_revision: Set(input.last_read_revision),
                updated_at: Set(now.into()),
                ..Default::default()
            })
            .exec(&txn)
            .await?;

        let state = forum_topic_read_state::Entity::find()
            .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_read_state::Column::TopicId.eq(topic.id))
            .filter(forum_topic_read_state::Column::UserId.eq(user_id))
            .one(&txn)
            .await?
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum topic read state disappeared during monotonic update".to_string(),
                )
            })?;
        txn.commit().await?;

        Ok(explicit_state(state))
    }
}

fn validate_nonnegative(input: &MarkForumTopicReadInput) -> ForumResult<()> {
    if input.last_read_position < 0 {
        return Err(ForumError::Validation(
            "Forum topic read position must be nonnegative".to_string(),
        ));
    }
    if input.last_read_revision < 0 {
        return Err(ForumError::Validation(
            "Forum topic read revision must be nonnegative".to_string(),
        ));
    }
    Ok(())
}

async fn ensure_topic_exists(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    forum_topic::Entity::find_by_id(topic_id)
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .map(|_| ())
        .ok_or(ForumError::TopicNotFound(topic_id))
}

fn explicit_state(model: forum_topic_read_state::Model) -> ForumTopicReadState {
    ForumTopicReadState {
        tenant_id: model.tenant_id,
        topic_id: model.topic_id,
        user_id: Some(model.user_id),
        last_read_position: model.last_read_position,
        last_read_revision: model.last_read_revision,
        explicit: true,
        created_at: Some(model.created_at.to_rfc3339()),
        updated_at: Some(model.updated_at.to_rfc3339()),
    }
}

fn implicit_state(
    tenant_id: Uuid,
    topic_id: Uuid,
    user_id: Option<Uuid>,
) -> ForumTopicReadState {
    ForumTopicReadState {
        tenant_id,
        topic_id,
        user_id,
        last_read_position: 0,
        last_read_revision: 0,
        explicit: false,
        created_at: None,
        updated_at: None,
    }
}
