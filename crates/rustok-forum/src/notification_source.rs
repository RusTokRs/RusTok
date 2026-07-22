use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::HostRuntimeContext;
use rustok_notifications_api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest,
    NotificationAudienceCandidate, NotificationAudienceCursor, NotificationAudiencePage,
    NotificationOpenAuthorization, NotificationPriority, NotificationProviderError,
    NotificationProviderResult, NotificationSemanticDescriptor, NotificationSourceEventRef,
    NotificationSourceProvider, NotificationSourceProviderFactory, NotificationSourceSlug,
    NotificationTargetKind, NotificationTargetRef, NotificationTargetRoute,
    NotificationTemplateData, NotificationTemplateKey, NotificationTypeKey,
    ResolveNotificationAudienceRequest,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::entities::{
    forum_category_subscription, forum_domain_event, forum_reply, forum_topic,
    forum_topic_channel_access, forum_user_mention,
};
use crate::state_machine::{ReplyStatus, TopicStatus};
use crate::subscription::ForumSubscriptionLevel;

const FORUM_SOURCE: &str = "forum";
const TOPIC_CREATED_TYPE: &str = "forum.topic.created";
const USER_MENTION_ADDED_TYPE: &str = "forum.mention.user_added";
const FORUM_TOPIC_TARGET: &str = "forum.topic";
const FORUM_REPLY_TARGET: &str = "forum.reply";

#[derive(Clone, Default)]
pub(crate) struct ForumNotificationSourceProviderFactory;

impl NotificationSourceProviderFactory for ForumNotificationSourceProviderFactory {
    fn slug(&self) -> NotificationSourceSlug {
        forum_source_slug()
    }

    fn build(
        &self,
        host: &HostRuntimeContext,
    ) -> NotificationProviderResult<Arc<dyn NotificationSourceProvider>> {
        Ok(Arc::new(ForumNotificationSourceProvider::new(
            host.db_clone(),
        )))
    }
}

#[derive(Clone)]
struct ForumNotificationSourceProvider {
    db: DatabaseConnection,
}

#[derive(Clone, Debug)]
struct ForumTargetContext {
    source_kind: String,
    source_id: Uuid,
    topic_id: Uuid,
    category_id: Uuid,
}

#[derive(Clone, Debug)]
enum ForumTargetAvailability {
    Visible(ForumTargetContext),
    Deferred,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize)]
struct ForumUserMentionPayload {
    source_kind: String,
    source_id: Uuid,
    source_revision_id: i64,
    source_locale: String,
    mentioned_user_id: Uuid,
}

impl ForumNotificationSourceProvider {
    fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn load_event(
        &self,
        event: &NotificationSourceEventRef,
    ) -> NotificationProviderResult<forum_domain_event::Model> {
        if event.source() != &forum_source_slug() || !is_supported_event_type(event.event_type()) {
            return Err(NotificationProviderError::InvalidEvent);
        }
        let sequence_no = i64::try_from(event.source_revision())
            .map_err(|_| NotificationProviderError::InvalidEvent)?;
        forum_domain_event::Entity::find()
            .filter(forum_domain_event::Column::TenantId.eq(event.tenant_id()))
            .filter(forum_domain_event::Column::EventId.eq(event.event_id()))
            .filter(forum_domain_event::Column::EventType.eq(event.event_type().as_str()))
            .filter(forum_domain_event::Column::SequenceNo.eq(sequence_no))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?
            .ok_or(NotificationProviderError::InvalidEvent)
    }

    async fn load_open_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> NotificationProviderResult<Option<forum_topic::Model>> {
        let topic = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Id.eq(topic_id))
            .filter(forum_topic::Column::Status.eq(TopicStatus::Open))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?;
        let Some(topic) = topic else {
            return Ok(None);
        };
        if !self
            .row_is_active("forum_topics", "id", tenant_id, topic_id)
            .await?
        {
            return Ok(None);
        }
        Ok(Some(topic))
    }

    async fn load_public_target(
        &self,
        tenant_id: Uuid,
        source_kind: &str,
        source_id: Uuid,
    ) -> NotificationProviderResult<ForumTargetAvailability> {
        match source_kind {
            "topic" => {
                let Some(topic) = self.load_open_topic(tenant_id, source_id).await? else {
                    return Ok(ForumTargetAvailability::Unavailable);
                };
                if self.is_channel_restricted(tenant_id, topic.id).await? {
                    return Ok(ForumTargetAvailability::Unavailable);
                }
                Ok(ForumTargetAvailability::Visible(ForumTargetContext {
                    source_kind: "topic".to_string(),
                    source_id: topic.id,
                    topic_id: topic.id,
                    category_id: topic.category_id,
                }))
            }
            "reply" => {
                let reply = forum_reply::Entity::find()
                    .filter(forum_reply::Column::TenantId.eq(tenant_id))
                    .filter(forum_reply::Column::Id.eq(source_id))
                    .one(&self.db)
                    .await
                    .map_err(retryable_database_error)?;
                let Some(reply) = reply else {
                    return Ok(ForumTargetAvailability::Unavailable);
                };
                if !self
                    .row_is_active("forum_replies", "id", tenant_id, reply.id)
                    .await?
                {
                    return Ok(ForumTargetAvailability::Unavailable);
                }
                if reply.status == ReplyStatus::Pending {
                    return Ok(ForumTargetAvailability::Deferred);
                }
                if reply.status != ReplyStatus::Approved {
                    return Ok(ForumTargetAvailability::Unavailable);
                }
                let Some(topic) = self.load_open_topic(tenant_id, reply.topic_id).await? else {
                    return Ok(ForumTargetAvailability::Unavailable);
                };
                if self.is_channel_restricted(tenant_id, topic.id).await? {
                    return Ok(ForumTargetAvailability::Unavailable);
                }
                Ok(ForumTargetAvailability::Visible(ForumTargetContext {
                    source_kind: "reply".to_string(),
                    source_id: reply.id,
                    topic_id: topic.id,
                    category_id: topic.category_id,
                }))
            }
            _ => Err(NotificationProviderError::InvalidEvent),
        }
    }

    async fn row_is_active(
        &self,
        table: &'static str,
        id_column: &'static str,
        tenant_id: Uuid,
        id: Uuid,
    ) -> NotificationProviderResult<bool> {
        self.db
            .query_one(Statement::from_string(
                self.db.get_database_backend(),
                format!(
                    "SELECT 1 AS active FROM {table} WHERE tenant_id = '{tenant_id}' AND {id_column} = '{id}' AND deleted_at IS NULL"
                ),
            ))
            .await
            .map(|row| row.is_some())
            .map_err(retryable_database_error)
    }

    async fn is_channel_restricted(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> NotificationProviderResult<bool> {
        forum_topic_channel_access::Entity::find()
            .filter(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_channel_access::Column::TopicId.eq(topic_id))
            .count(&self.db)
            .await
            .map(|count| count > 0)
            .map_err(retryable_database_error)
    }

    fn parse_user_mention(
        &self,
        event: &forum_domain_event::Model,
    ) -> NotificationProviderResult<ForumUserMentionPayload> {
        if event.event_type != USER_MENTION_ADDED_TYPE || event.schema_version != 1 {
            return Err(NotificationProviderError::InvalidEvent);
        }
        let payload = serde_json::from_value::<ForumUserMentionPayload>(event.payload.clone())
            .map_err(|_| NotificationProviderError::InvalidEvent)?;
        if payload.source_revision_id <= 0
            || payload.source_id.is_nil()
            || payload.mentioned_user_id.is_nil()
            || payload.source_locale.is_empty()
            || payload.source_locale.len() > 32
            || event.aggregate_type != payload.source_kind
            || event.aggregate_id != payload.source_id
        {
            return Err(NotificationProviderError::InvalidEvent);
        }
        Ok(payload)
    }

    async fn user_mention_relation_exists(
        &self,
        tenant_id: Uuid,
        payload: &ForumUserMentionPayload,
    ) -> NotificationProviderResult<bool> {
        forum_user_mention::Entity::find()
            .filter(forum_user_mention::Column::TenantId.eq(tenant_id))
            .filter(forum_user_mention::Column::SourceKind.eq(payload.source_kind.as_str()))
            .filter(forum_user_mention::Column::SourceId.eq(payload.source_id))
            .filter(forum_user_mention::Column::SourceLocale.eq(payload.source_locale.as_str()))
            .filter(
                forum_user_mention::Column::SourceRevisionId.eq(payload.source_revision_id),
            )
            .filter(
                forum_user_mention::Column::MentionedUserId.eq(payload.mentioned_user_id),
            )
            .one(&self.db)
            .await
            .map(|row| row.is_some())
            .map_err(retryable_database_error)
    }

    fn validate_topic_descriptor(
        &self,
        event: &forum_domain_event::Model,
        descriptor: &NotificationSemanticDescriptor,
    ) -> NotificationProviderResult<()> {
        if descriptor.notification_type != topic_created_type()
            || descriptor.target.owner != forum_source_slug()
            || descriptor.target.kind != forum_topic_target_kind()
            || descriptor.target.id != event.aggregate_id
        {
            return Err(NotificationProviderError::Rejected);
        }
        Ok(())
    }

    fn validate_mention_descriptor(
        &self,
        payload: &ForumUserMentionPayload,
        descriptor: &NotificationSemanticDescriptor,
    ) -> NotificationProviderResult<()> {
        let expected_kind = target_kind_for_source(payload.source_kind.as_str())?;
        if descriptor.notification_type != user_mention_added_type()
            || descriptor.target.owner != forum_source_slug()
            || descriptor.target.kind != expected_kind
            || descriptor.target.id != payload.source_id
        {
            return Err(NotificationProviderError::Rejected);
        }
        Ok(())
    }

    fn mention_descriptor(
        &self,
        event: &forum_domain_event::Model,
        payload: &ForumUserMentionPayload,
        target: &ForumTargetContext,
    ) -> NotificationProviderResult<NotificationSemanticDescriptor> {
        let template_data = NotificationTemplateData::try_new(BTreeMap::from([
            ("source_kind".to_string(), payload.source_kind.clone()),
            ("source_id".to_string(), payload.source_id.to_string()),
            (
                "source_revision_id".to_string(),
                payload.source_revision_id.to_string(),
            ),
            ("source_locale".to_string(), payload.source_locale.clone()),
            ("topic_id".to_string(), target.topic_id.to_string()),
            ("category_id".to_string(), target.category_id.to_string()),
        ]))
        .map_err(|_| NotificationProviderError::InvalidEvent)?;
        Ok(NotificationSemanticDescriptor {
            notification_type: user_mention_added_type(),
            template_key: NotificationTemplateKey::new(USER_MENTION_ADDED_TYPE)
                .expect("forum user-mention notification template key must stay valid"),
            target: NotificationTargetRef {
                owner: forum_source_slug(),
                kind: target_kind_for_source(target.source_kind.as_str())?,
                id: target.source_id,
            },
            actor_id: event.actor_id,
            priority: NotificationPriority::Normal,
            template_data,
        })
    }

    fn target_route(
        &self,
        target: &ForumTargetContext,
    ) -> NotificationProviderResult<NotificationTargetRoute> {
        let route = if target.source_kind == "reply" {
            format!(
                "/modules/forum?category={}&topic={}&reply={}",
                target.category_id, target.topic_id, target.source_id
            )
        } else {
            format!(
                "/modules/forum?category={}&topic={}",
                target.category_id, target.topic_id
            )
        };
        NotificationTargetRoute::new(route)
            .map_err(|_| NotificationProviderError::Internal { retryable: false })
    }
}

#[async_trait]
impl NotificationSourceProvider for ForumNotificationSourceProvider {
    fn slug(&self) -> NotificationSourceSlug {
        forum_source_slug()
    }

    fn display_name(&self) -> &'static str {
        "Forum"
    }

    fn supported_types(&self) -> Vec<NotificationTypeKey> {
        vec![topic_created_type(), user_mention_added_type()]
    }

    async fn describe_event(
        &self,
        request: DescribeNotificationRequest,
    ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>> {
        let event = self.load_event(&request.event).await?;
        match event.event_type.as_str() {
            TOPIC_CREATED_TYPE => {
                if event.aggregate_type != "topic" || event.schema_version != 1 {
                    return Err(NotificationProviderError::InvalidEvent);
                }
                let Some(topic) = self
                    .load_open_topic(event.tenant_id, event.aggregate_id)
                    .await?
                else {
                    return Ok(None);
                };
                if self
                    .is_channel_restricted(event.tenant_id, event.aggregate_id)
                    .await?
                {
                    return Ok(None);
                }
                let template_data = NotificationTemplateData::try_new(BTreeMap::from([
                    ("topic_id".to_string(), topic.id.to_string()),
                    ("category_id".to_string(), topic.category_id.to_string()),
                ]))
                .map_err(|_| NotificationProviderError::InvalidEvent)?;
                Ok(Some(NotificationSemanticDescriptor {
                    notification_type: topic_created_type(),
                    template_key: NotificationTemplateKey::new(TOPIC_CREATED_TYPE)
                        .expect("forum topic notification template key must stay valid"),
                    target: NotificationTargetRef {
                        owner: forum_source_slug(),
                        kind: forum_topic_target_kind(),
                        id: topic.id,
                    },
                    actor_id: event.actor_id.or(topic.author_id),
                    priority: NotificationPriority::Normal,
                    template_data,
                }))
            }
            USER_MENTION_ADDED_TYPE => {
                let payload = self.parse_user_mention(&event)?;
                if !self
                    .user_mention_relation_exists(event.tenant_id, &payload)
                    .await?
                {
                    return Err(NotificationProviderError::InvalidEvent);
                }
                match self
                    .load_public_target(event.tenant_id, &payload.source_kind, payload.source_id)
                    .await?
                {
                    ForumTargetAvailability::Visible(target) => {
                        self.mention_descriptor(&event, &payload, &target).map(Some)
                    }
                    ForumTargetAvailability::Deferred => {
                        Err(NotificationProviderError::Internal { retryable: true })
                    }
                    ForumTargetAvailability::Unavailable => Ok(None),
                }
            }
            _ => Err(NotificationProviderError::InvalidEvent),
        }
    }

    async fn resolve_audience(
        &self,
        request: ResolveNotificationAudienceRequest,
    ) -> NotificationProviderResult<NotificationAudiencePage> {
        let event = self.load_event(&request.event).await?;
        match event.event_type.as_str() {
            TOPIC_CREATED_TYPE => {
                self.validate_topic_descriptor(&event, &request.descriptor)?;
                let Some(topic) = self
                    .load_open_topic(event.tenant_id, event.aggregate_id)
                    .await?
                else {
                    return Ok(NotificationAudiencePage::empty());
                };
                if self
                    .is_channel_restricted(event.tenant_id, event.aggregate_id)
                    .await?
                {
                    return Ok(NotificationAudiencePage::empty());
                }

                let limit = request.bounded_limit();
                if limit == 0 {
                    return Err(NotificationProviderError::Rejected);
                }
                let cursor = request
                    .cursor
                    .as_ref()
                    .map(|cursor| Uuid::parse_str(cursor.as_str()))
                    .transpose()
                    .map_err(|_| NotificationProviderError::InvalidEvent)?;
                let mut query = forum_category_subscription::Entity::find()
                    .filter(forum_category_subscription::Column::TenantId.eq(event.tenant_id))
                    .filter(forum_category_subscription::Column::CategoryId.eq(topic.category_id))
                    .filter(forum_category_subscription::Column::NotifyNewTopics.eq(true))
                    .filter(
                        forum_category_subscription::Column::Level
                            .ne(ForumSubscriptionLevel::Muted),
                    )
                    .order_by_asc(forum_category_subscription::Column::UserId);
                if let Some(cursor) = cursor {
                    query = query
                        .filter(forum_category_subscription::Column::UserId.gt(cursor));
                }
                if let Some(actor_id) = event.actor_id.or(topic.author_id) {
                    query = query
                        .filter(forum_category_subscription::Column::UserId.ne(actor_id));
                }
                let mut subscriptions = query
                    .limit((limit + 1) as u64)
                    .all(&self.db)
                    .await
                    .map_err(retryable_database_error)?;
                let has_more = subscriptions.len() > limit;
                subscriptions.truncate(limit);
                let next_cursor = if has_more {
                    subscriptions
                        .last()
                        .map(|subscription| {
                            NotificationAudienceCursor::new(subscription.user_id.to_string())
                        })
                        .transpose()
                        .map_err(|_| NotificationProviderError::Internal { retryable: false })?
                } else {
                    None
                };
                let recipients = subscriptions
                    .into_iter()
                    .map(|subscription| NotificationAudienceCandidate {
                        recipient_id: subscription.user_id,
                    })
                    .collect();
                NotificationAudiencePage::try_new(recipients, next_cursor)
                    .map_err(|_| NotificationProviderError::Internal { retryable: false })
            }
            USER_MENTION_ADDED_TYPE => {
                let payload = self.parse_user_mention(&event)?;
                self.validate_mention_descriptor(&payload, &request.descriptor)?;
                if request.bounded_limit() == 0 {
                    return Err(NotificationProviderError::Rejected);
                }
                if request.cursor.is_some() {
                    return Ok(NotificationAudiencePage::empty());
                }
                if !self
                    .user_mention_relation_exists(event.tenant_id, &payload)
                    .await?
                {
                    return Err(NotificationProviderError::InvalidEvent);
                }
                match self
                    .load_public_target(event.tenant_id, &payload.source_kind, payload.source_id)
                    .await?
                {
                    ForumTargetAvailability::Visible(_) => {}
                    ForumTargetAvailability::Deferred => {
                        return Err(NotificationProviderError::Internal { retryable: true });
                    }
                    ForumTargetAvailability::Unavailable => {
                        return Ok(NotificationAudiencePage::empty());
                    }
                }
                if event.actor_id == Some(payload.mentioned_user_id) {
                    return Ok(NotificationAudiencePage::empty());
                }
                NotificationAudiencePage::try_new(
                    vec![NotificationAudienceCandidate {
                        recipient_id: payload.mentioned_user_id,
                    }],
                    None,
                )
                .map_err(|_| NotificationProviderError::Internal { retryable: false })
            }
            _ => Err(NotificationProviderError::InvalidEvent),
        }
    }

    async fn authorize_target_open(
        &self,
        request: AuthorizeNotificationTargetRequest,
    ) -> NotificationProviderResult<NotificationOpenAuthorization> {
        if request.target.owner != forum_source_slug() {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }
        let source_kind = if request.target.kind == forum_topic_target_kind() {
            "topic"
        } else if request.target.kind == forum_reply_target_kind() {
            "reply"
        } else {
            return Ok(NotificationOpenAuthorization::Unavailable);
        };
        match self
            .load_public_target(request.tenant_id, source_kind, request.target.id)
            .await?
        {
            ForumTargetAvailability::Visible(target) => Ok(NotificationOpenAuthorization::Allowed {
                route: self.target_route(&target)?,
            }),
            ForumTargetAvailability::Deferred | ForumTargetAvailability::Unavailable => {
                Ok(NotificationOpenAuthorization::Unavailable)
            }
        }
    }
}

fn retryable_database_error(_error: sea_orm::DbErr) -> NotificationProviderError {
    NotificationProviderError::Internal { retryable: true }
}

fn is_supported_event_type(event_type: &NotificationTypeKey) -> bool {
    event_type == &topic_created_type() || event_type == &user_mention_added_type()
}

fn target_kind_for_source(
    source_kind: &str,
) -> NotificationProviderResult<NotificationTargetKind> {
    match source_kind {
        "topic" => Ok(forum_topic_target_kind()),
        "reply" => Ok(forum_reply_target_kind()),
        _ => Err(NotificationProviderError::InvalidEvent),
    }
}

fn forum_source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(FORUM_SOURCE)
        .expect("forum notification source slug must stay valid")
}

fn topic_created_type() -> NotificationTypeKey {
    NotificationTypeKey::new(TOPIC_CREATED_TYPE)
        .expect("forum topic-created notification type must stay valid")
}

fn user_mention_added_type() -> NotificationTypeKey {
    NotificationTypeKey::new(USER_MENTION_ADDED_TYPE)
        .expect("forum user-mention notification type must stay valid")
}

fn forum_topic_target_kind() -> NotificationTargetKind {
    NotificationTargetKind::new(FORUM_TOPIC_TARGET)
        .expect("forum topic notification target kind must stay valid")
}

fn forum_reply_target_kind() -> NotificationTargetKind {
    NotificationTargetKind::new(FORUM_REPLY_TARGET)
        .expect("forum reply notification target kind must stay valid")
}
