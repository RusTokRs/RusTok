use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::HostRuntimeContext;
use rustok_notifications_api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest, NotificationAudienceCandidate,
    NotificationAudienceCursor, NotificationAudiencePage, NotificationOpenAuthorization,
    NotificationPriority, NotificationProviderError, NotificationProviderResult,
    NotificationSemanticDescriptor, NotificationSourceEventRef, NotificationSourceProvider,
    NotificationSourceProviderFactory, NotificationSourceSlug, NotificationTargetKind,
    NotificationTargetRef, NotificationTargetRoute, NotificationTemplateData,
    NotificationTemplateKey, NotificationTypeKey, ResolveNotificationAudienceRequest,
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use uuid::Uuid;

use crate::entities::{
    forum_category_subscription, forum_domain_event, forum_topic, forum_topic_channel_access,
};
use crate::state_machine::TopicStatus;
use crate::subscription::ForumSubscriptionLevel;

const FORUM_SOURCE: &str = "forum";
const TOPIC_CREATED_TYPE: &str = "forum.topic.created";
const FORUM_TOPIC_TARGET: &str = "forum.topic";

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

impl ForumNotificationSourceProvider {
    fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn load_event(
        &self,
        event: &NotificationSourceEventRef,
    ) -> NotificationProviderResult<forum_domain_event::Model> {
        if event.source() != &forum_source_slug() || event.event_type() != &topic_created_type() {
            return Err(NotificationProviderError::InvalidEvent);
        }
        let sequence_no = i64::try_from(event.source_revision())
            .map_err(|_| NotificationProviderError::InvalidEvent)?;
        forum_domain_event::Entity::find()
            .filter(forum_domain_event::Column::TenantId.eq(event.tenant_id()))
            .filter(forum_domain_event::Column::EventId.eq(event.event_id()))
            .filter(forum_domain_event::Column::EventType.eq(TOPIC_CREATED_TYPE))
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
        forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Id.eq(topic_id))
            .filter(forum_topic::Column::Status.eq(TopicStatus::Open))
            .one(&self.db)
            .await
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

    fn validate_descriptor(
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
        vec![topic_created_type()]
    }

    async fn describe_event(
        &self,
        request: DescribeNotificationRequest,
    ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>> {
        let event = self.load_event(&request.event).await?;
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

    async fn resolve_audience(
        &self,
        request: ResolveNotificationAudienceRequest,
    ) -> NotificationProviderResult<NotificationAudiencePage> {
        let event = self.load_event(&request.event).await?;
        self.validate_descriptor(&event, &request.descriptor)?;
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
            .filter(forum_category_subscription::Column::Level.ne(ForumSubscriptionLevel::Muted))
            .order_by_asc(forum_category_subscription::Column::UserId);
        if let Some(cursor) = cursor {
            query = query.filter(forum_category_subscription::Column::UserId.gt(cursor));
        }
        if let Some(actor_id) = event.actor_id.or(topic.author_id) {
            query = query.filter(forum_category_subscription::Column::UserId.ne(actor_id));
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

    async fn authorize_target_open(
        &self,
        request: AuthorizeNotificationTargetRequest,
    ) -> NotificationProviderResult<NotificationOpenAuthorization> {
        if request.target.owner != forum_source_slug()
            || request.target.kind != forum_topic_target_kind()
        {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }
        let Some(topic) = self
            .load_open_topic(request.tenant_id, request.target.id)
            .await?
        else {
            return Ok(NotificationOpenAuthorization::Unavailable);
        };
        if self
            .is_channel_restricted(request.tenant_id, request.target.id)
            .await?
        {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }

        let route = NotificationTargetRoute::new(format!(
            "/modules/forum?category={}&topic={}",
            topic.category_id, topic.id
        ))
        .map_err(|_| NotificationProviderError::Internal { retryable: false })?;
        Ok(NotificationOpenAuthorization::Allowed { route })
    }
}

fn retryable_database_error(_error: sea_orm::DbErr) -> NotificationProviderError {
    NotificationProviderError::Internal { retryable: true }
}

fn forum_source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(FORUM_SOURCE)
        .expect("forum notification source slug must stay valid")
}

fn topic_created_type() -> NotificationTypeKey {
    NotificationTypeKey::new(TOPIC_CREATED_TYPE)
        .expect("forum topic-created notification type must stay valid")
}

fn forum_topic_target_kind() -> NotificationTargetKind {
    NotificationTargetKind::new(FORUM_TOPIC_TARGET)
        .expect("forum topic notification target kind must stay valid")
}
