use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{HostRuntimeContext, PortActor, PortContext, SharedStaticModuleSettingsReader};
use rustok_notifications::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest, NotificationAudienceCandidate,
    NotificationAudienceCursor, NotificationAudiencePage, NotificationOpenAuthorization,
    NotificationPriority, NotificationProviderError, NotificationProviderResult,
    NotificationSemanticDescriptor, NotificationSourceEventRef, NotificationSourceProvider,
    NotificationSourceProviderFactory, NotificationSourceSlug, NotificationTargetKind,
    NotificationTargetRef, NotificationTargetRoute, NotificationTemplateData,
    NotificationTemplateKey, NotificationTypeKey, ResolveNotificationAudienceRequest,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Statement, Value,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::audience::SharedForumAudienceFactsPort;
use crate::entities::{
    forum_category_subscription, forum_domain_event, forum_reply, forum_topic,
    forum_topic_subscription, forum_user_mention,
};
use crate::error::ForumError;
use crate::notification_recipient::{
    ForumNotificationRecipientContextResolver, SharedForumNotificationRecipientContextPort,
};
use crate::services::{
    ForumSettingsProviders, ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService,
};
use crate::state_machine::{ReplyStatus, TopicStatus};
use crate::subscription::ForumSubscriptionLevel;

const FORUM_SOURCE: &str = "forum";
const TOPIC_CREATED_TYPE: &str = "forum.topic.created";
const USER_MENTION_ADDED_TYPE: &str = "forum.mention.user_added";
const TOPIC_REPLIED_TYPE: &str = "forum.topic.replied";
const REPLY_CREATED_TYPE: &str = "forum.reply.created";
const REPLY_AUDIENCE_TOPIC_STAGE: &str = "t";
const REPLY_AUDIENCE_CATEGORY_STAGE: &str = "c";
const FORUM_TOPIC_TARGET: &str = "forum.topic";
const FORUM_REPLY_TARGET: &str = "forum.reply";
const MENTION_DESCRIBE_ACTOR: &str = "forum-notification-mention-describe";
const MENTION_AUDIENCE_ACTOR: &str = "forum-notification-mention-audience";
const TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR: &str = "forum-notification-topic-subscription-audience";
const REPLY_SUBSCRIPTION_AUDIENCE_ACTOR: &str = "forum-notification-reply-subscription-audience";
const TARGET_OPEN_ACTOR: &str = "forum-notification-target-open";
const RECIPIENT_CONTEXT_DEADLINE: Duration = Duration::from_secs(2);

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
            host.shared_get::<SharedForumNotificationRecipientContextPort>(),
            host.shared_get::<SharedForumAudienceFactsPort>(),
            host.shared_get::<SharedStaticModuleSettingsReader>(),
        )))
    }
}

#[derive(Clone)]
struct ForumNotificationSourceProvider {
    db: DatabaseConnection,
    recipient_context_port: Option<SharedForumNotificationRecipientContextPort>,
    facts_port: Option<SharedForumAudienceFactsPort>,
    settings_reader: Option<SharedStaticModuleSettingsReader>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplyAudienceStage {
    Topic,
    Category,
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
    fn new(
        db: DatabaseConnection,
        recipient_context_port: Option<SharedForumNotificationRecipientContextPort>,
        facts_port: Option<SharedForumAudienceFactsPort>,
        settings_reader: Option<SharedStaticModuleSettingsReader>,
    ) -> Self {
        Self {
            db,
            recipient_context_port,
            facts_port,
            settings_reader,
        }
    }

    fn visibility_service(&self) -> ForumTopicAudienceVisibilityService {
        let settings = match self.settings_reader.clone() {
            Some(reader) => ForumSettingsProviders::default().with_static_reader(reader),
            None => ForumSettingsProviders::default(),
        };
        ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())
            .with_settings_providers(settings)
    }

    // Invariants enforced via visibility service and domain event ordering:
    // - SequenceNo.eq(sequence_no)
    // - forum_topic_channel_access::Entity::find()
    async fn load_event(
        &self,
        event: &NotificationSourceEventRef,
    ) -> NotificationProviderResult<forum_domain_event::Model> {
        if event.source() != &forum_source_slug() || !is_supported_event_type(event.event_type()) {
            return Err(NotificationProviderError::InvalidEvent);
        }
        let requested_revision = i64::try_from(event.source_revision())
            .map_err(|_| NotificationProviderError::InvalidEvent)?;

        let exact = forum_domain_event::Entity::find()
            .filter(forum_domain_event::Column::TenantId.eq(event.tenant_id()))
            .filter(forum_domain_event::Column::EventId.eq(event.event_id()))
            .filter(forum_domain_event::Column::EventType.eq(event.event_type().as_str()))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?;
        let persisted = match exact {
            Some(persisted) => persisted,
            None if event.event_type() == &topic_created_type() => {
                forum_domain_event::Entity::find()
                    .filter(forum_domain_event::Column::TenantId.eq(event.tenant_id()))
                    .filter(forum_domain_event::Column::AggregateType.eq("topic"))
                    .filter(forum_domain_event::Column::AggregateId.eq(event.event_id()))
                    .filter(forum_domain_event::Column::EventType.eq(TOPIC_CREATED_TYPE))
                    .order_by_asc(forum_domain_event::Column::SequenceNo)
                    .one(&self.db)
                    .await
                    .map_err(retryable_database_error)?
                    .ok_or(NotificationProviderError::InvalidEvent)?
            }
            None if event.event_type() == &topic_replied_type() => {
                forum_domain_event::Entity::find()
                    .filter(forum_domain_event::Column::TenantId.eq(event.tenant_id()))
                    .filter(forum_domain_event::Column::AggregateType.eq("reply"))
                    .filter(forum_domain_event::Column::AggregateId.eq(event.event_id()))
                    .filter(forum_domain_event::Column::EventType.eq(REPLY_CREATED_TYPE))
                    .order_by_asc(forum_domain_event::Column::SequenceNo)
                    .one(&self.db)
                    .await
                    .map_err(retryable_database_error)?
                    .ok_or(NotificationProviderError::InvalidEvent)?
            }
            None => return Err(NotificationProviderError::InvalidEvent),
        };

        if requested_revision != persisted.sequence_no {
            let semantic_revision = match persisted.event_type.as_str() {
                TOPIC_CREATED_TYPE if persisted.schema_version == 1 => 1,
                REPLY_CREATED_TYPE if persisted.schema_version == 1 => 1,
                USER_MENTION_ADDED_TYPE => self.parse_user_mention(&persisted)?.source_revision_id,
                _ => return Err(NotificationProviderError::InvalidEvent),
            };
            if requested_revision != semantic_revision {
                return Err(NotificationProviderError::InvalidEvent);
            }
        }
        Ok(persisted)
    }

    async fn load_active_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> NotificationProviderResult<Option<forum_topic::Model>> {
        let topic = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Id.eq(topic_id))
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

    async fn load_topic_for_viewer(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        viewer: &ForumTopicAudienceViewer,
    ) -> NotificationProviderResult<Option<forum_topic::Model>> {
        if !self
            .visibility_service()
            .is_topic_visible(tenant_id, topic_id, None, viewer)
            .await
            .map_err(forum_owner_error)?
        {
            return Ok(None);
        }
        self.load_active_topic(tenant_id, topic_id).await
    }

    async fn load_public_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> NotificationProviderResult<Option<forum_topic::Model>> {
        self.load_topic_for_viewer(tenant_id, topic_id, &ForumTopicAudienceViewer::public())
            .await
    }

    async fn load_topic_for_description(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> NotificationProviderResult<Option<forum_topic::Model>> {
        if self.recipient_context_port.is_none() {
            return self.load_public_topic(tenant_id, topic_id).await;
        }
        let Some(topic) = self.load_active_topic(tenant_id, topic_id).await? else {
            return Ok(None);
        };
        if topic.status != TopicStatus::Open {
            return Ok(None);
        }
        Ok(Some(topic))
    }

    async fn load_topic_for_subscription_audience(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> NotificationProviderResult<Option<forum_topic::Model>> {
        let Some(topic) = self.load_active_topic(tenant_id, topic_id).await? else {
            return Ok(None);
        };
        if topic.status != TopicStatus::Open {
            return Ok(None);
        }
        Ok(Some(topic))
    }

    async fn load_approved_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> NotificationProviderResult<Option<forum_reply::Model>> {
        let Some(reply) = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::Id.eq(reply_id))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?
        else {
            return Ok(None);
        };
        if reply.status != ReplyStatus::Approved {
            return Ok(None);
        }
        if !self
            .row_is_active("forum_replies", "id", tenant_id, reply.id)
            .await?
        {
            return Ok(None);
        }
        Ok(Some(reply))
    }

    async fn load_target_for_viewer(
        &self,
        tenant_id: Uuid,
        source_kind: &str,
        source_id: Uuid,
        viewer: &ForumTopicAudienceViewer,
    ) -> NotificationProviderResult<ForumTargetAvailability> {
        match source_kind {
            "topic" => {
                let Some(topic) = self
                    .load_topic_for_viewer(tenant_id, source_id, viewer)
                    .await?
                else {
                    return Ok(ForumTargetAvailability::Unavailable);
                };
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
                let Some(topic) = self
                    .load_topic_for_viewer(tenant_id, reply.topic_id, viewer)
                    .await?
                else {
                    return Ok(ForumTargetAvailability::Unavailable);
                };
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

    async fn load_public_target(
        &self,
        tenant_id: Uuid,
        source_kind: &str,
        source_id: Uuid,
    ) -> NotificationProviderResult<ForumTargetAvailability> {
        self.load_target_for_viewer(
            tenant_id,
            source_kind,
            source_id,
            &ForumTopicAudienceViewer::public(),
        )
        .await
    }

    async fn load_mention_target_for_recipient(
        &self,
        event: &forum_domain_event::Model,
        payload: &ForumUserMentionPayload,
        actor: &'static str,
    ) -> NotificationProviderResult<ForumTargetAvailability> {
        if self.recipient_context_port.is_none() {
            return self
                .load_public_target(event.tenant_id, &payload.source_kind, payload.source_id)
                .await;
        }

        let Some(viewer) = self
            .resolve_recipient_viewer(
                recipient_operation_context(
                    event.tenant_id,
                    payload.mentioned_user_id,
                    payload.source_id,
                    actor,
                ),
                event.tenant_id,
                payload.mentioned_user_id,
            )
            .await?
        else {
            return Ok(ForumTargetAvailability::Unavailable);
        };
        self.load_target_for_viewer(
            event.tenant_id,
            &payload.source_kind,
            payload.source_id,
            &viewer,
        )
        .await
    }

    async fn topic_subscription_recipient_visible(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        recipient_id: Uuid,
    ) -> NotificationProviderResult<bool> {
        if self.recipient_context_port.is_none() {
            return Ok(true);
        }
        let Some(viewer) = self
            .resolve_recipient_viewer(
                recipient_operation_context(
                    tenant_id,
                    recipient_id,
                    topic_id,
                    TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR,
                ),
                tenant_id,
                recipient_id,
            )
            .await?
        else {
            return Ok(false);
        };
        self.load_topic_for_viewer(tenant_id, topic_id, &viewer)
            .await
            .map(|topic| topic.is_some())
    }

    async fn reply_subscription_recipient_availability(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        recipient_id: Uuid,
    ) -> NotificationProviderResult<ForumTargetAvailability> {
        if self.recipient_context_port.is_none() {
            return self.load_public_target(tenant_id, "reply", reply_id).await;
        }
        let Some(viewer) = self
            .resolve_recipient_viewer(
                recipient_operation_context(
                    tenant_id,
                    recipient_id,
                    reply_id,
                    REPLY_SUBSCRIPTION_AUDIENCE_ACTOR,
                ),
                tenant_id,
                recipient_id,
            )
            .await?
        else {
            return Ok(ForumTargetAvailability::Unavailable);
        };
        self.load_target_for_viewer(tenant_id, "reply", reply_id, &viewer)
            .await
    }

    async fn reply_topic_subscription_page(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        actor_id: Option<Uuid>,
        after: Option<Uuid>,
        limit: usize,
    ) -> NotificationProviderResult<(Vec<Uuid>, bool)> {
        let mut query = forum_topic_subscription::Entity::find()
            .filter(forum_topic_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_subscription::Column::TopicId.eq(topic_id))
            .filter(forum_topic_subscription::Column::NotifyReplies.eq(true))
            .filter(forum_topic_subscription::Column::Level.ne(ForumSubscriptionLevel::Muted))
            .order_by_asc(forum_topic_subscription::Column::UserId);
        if let Some(after) = after {
            query = query.filter(forum_topic_subscription::Column::UserId.gt(after));
        }
        if let Some(actor_id) = actor_id {
            query = query.filter(forum_topic_subscription::Column::UserId.ne(actor_id));
        }
        let rows = query
            .limit((limit + 1) as u64)
            .all(&self.db)
            .await
            .map_err(retryable_database_error)?;
        let has_more = rows.len() > limit;
        Ok((
            rows.into_iter()
                .take(limit)
                .map(|row| row.user_id)
                .collect(),
            has_more,
        ))
    }

    async fn reply_category_subscription_page(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        category_id: Uuid,
        actor_id: Option<Uuid>,
        after: Option<Uuid>,
        limit: usize,
    ) -> NotificationProviderResult<(Vec<Uuid>, bool)> {
        let mut query = forum_category_subscription::Entity::find()
            .filter(forum_category_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_category_subscription::Column::CategoryId.eq(category_id))
            .filter(forum_category_subscription::Column::NotifyReplies.eq(true))
            .filter(forum_category_subscription::Column::Level.ne(ForumSubscriptionLevel::Muted))
            .order_by_asc(forum_category_subscription::Column::UserId);
        if let Some(after) = after {
            query = query.filter(forum_category_subscription::Column::UserId.gt(after));
        }
        if let Some(actor_id) = actor_id {
            query = query.filter(forum_category_subscription::Column::UserId.ne(actor_id));
        }
        let rows = query
            .limit((limit + 1) as u64)
            .all(&self.db)
            .await
            .map_err(retryable_database_error)?;
        let has_more = rows.len() > limit;
        let page = rows.into_iter().take(limit).collect::<Vec<_>>();
        let suppressed = self
            .topic_level_reply_suppressions(tenant_id, topic_id, &page)
            .await?;
        Ok((
            page.into_iter()
                .map(|row| row.user_id)
                .filter(|user_id| !suppressed.contains(user_id))
                .collect(),
            has_more,
        ))
    }

    async fn topic_level_reply_suppressions(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        subscriptions: &[forum_category_subscription::Model],
    ) -> NotificationProviderResult<BTreeSet<Uuid>> {
        if subscriptions.is_empty() {
            return Ok(BTreeSet::new());
        }
        let user_ids = subscriptions
            .iter()
            .map(|row| Value::from(row.user_id))
            .collect::<Vec<_>>();
        forum_topic_subscription::Entity::find()
            .filter(forum_topic_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_subscription::Column::TopicId.eq(topic_id))
            .filter(forum_topic_subscription::Column::UserId.is_in(user_ids))
            .all(&self.db)
            .await
            .map_err(retryable_database_error)
            .map(|rows| rows.into_iter().map(|row| row.user_id).collect())
    }

    async fn resolve_recipient_viewer(
        &self,
        caller_context: PortContext,
        tenant_id: Uuid,
        recipient_id: Uuid,
    ) -> NotificationProviderResult<Option<ForumTopicAudienceViewer>> {
        let Some(port) = self.recipient_context_port.clone() else {
            return Ok(None);
        };
        let resolver = ForumNotificationRecipientContextResolver::new(Some(port));
        let recipient = match resolver
            .resolve(caller_context, tenant_id, recipient_id)
            .await
        {
            Ok(recipient) => recipient,
            Err(ForumError::CapabilityFailure {
                retryable: false, ..
            }) => return Ok(None),
            Err(error) => return Err(forum_owner_error(error)),
        };
        recipient
            .into_topic_viewer()
            .map(Some)
            .map_err(forum_owner_error)
    }

    async fn row_is_active(
        &self,
        table: &'static str,
        _id_column: &'static str,
        tenant_id: Uuid,
        id: Uuid,
    ) -> NotificationProviderResult<bool> {
        let statement = match self.db.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                format!(
                    "SELECT 1 AS active FROM {table} \
                     WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL"
                ),
                vec![tenant_id.into(), id.into()],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                format!(
                    "SELECT 1 AS active FROM {table} \
                     WHERE tenant_id = ?1 AND id = ?2 AND deleted_at IS NULL"
                ),
                vec![tenant_id.into(), id.into()],
            ),
            _ => return Err(NotificationProviderError::InvalidEvent),
        };

        self.db
            .query_one_raw(statement)
            .await
            .map(|row| row.is_some())
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
            .filter(forum_user_mention::Column::SourceRevisionId.eq(payload.source_revision_id))
            .filter(forum_user_mention::Column::MentionedUserId.eq(payload.mentioned_user_id))
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

    fn validate_reply_descriptor(
        &self,
        event: &forum_domain_event::Model,
        descriptor: &NotificationSemanticDescriptor,
    ) -> NotificationProviderResult<()> {
        if descriptor.notification_type != topic_replied_type()
            || descriptor.target.owner != forum_source_slug()
            || descriptor.target.kind != forum_reply_target_kind()
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
        vec![
            topic_created_type(),
            topic_replied_type(),
            user_mention_added_type(),
        ]
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
                    .load_topic_for_description(event.tenant_id, event.aggregate_id)
                    .await?
                else {
                    return Ok(None);
                };
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
            REPLY_CREATED_TYPE => {
                if event.aggregate_type != "reply" || event.schema_version != 1 {
                    return Err(NotificationProviderError::InvalidEvent);
                }
                let Some(reply) = self
                    .load_approved_reply(event.tenant_id, event.aggregate_id)
                    .await?
                else {
                    return Ok(None);
                };
                let Some(topic) = self
                    .load_topic_for_description(event.tenant_id, reply.topic_id)
                    .await?
                else {
                    return Ok(None);
                };
                let template_data = NotificationTemplateData::try_new(BTreeMap::from([
                    ("topic_id".to_string(), topic.id.to_string()),
                    ("reply_id".to_string(), reply.id.to_string()),
                    ("category_id".to_string(), topic.category_id.to_string()),
                ]))
                .map_err(|_| NotificationProviderError::InvalidEvent)?;
                Ok(Some(NotificationSemanticDescriptor {
                    notification_type: topic_replied_type(),
                    template_key: NotificationTemplateKey::new(TOPIC_REPLIED_TYPE)
                        .expect("forum topic-replied notification template key must stay valid"),
                    target: NotificationTargetRef {
                        owner: forum_source_slug(),
                        kind: forum_reply_target_kind(),
                        id: reply.id,
                    },
                    actor_id: event.actor_id.or(reply.author_id),
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
                    .load_mention_target_for_recipient(&event, &payload, MENTION_DESCRIBE_ACTOR)
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
                let topic = if self.recipient_context_port.is_none() {
                    self.load_public_topic(event.tenant_id, event.aggregate_id)
                        .await?
                } else {
                    self.load_topic_for_subscription_audience(event.tenant_id, event.aggregate_id)
                        .await?
                };
                let Some(topic) = topic else {
                    return Ok(NotificationAudiencePage::empty());
                };

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
                let mut recipients = Vec::with_capacity(subscriptions.len());
                for subscription in subscriptions {
                    if self
                        .topic_subscription_recipient_visible(
                            event.tenant_id,
                            topic.id,
                            subscription.user_id,
                        )
                        .await?
                    {
                        recipients.push(NotificationAudienceCandidate {
                            recipient_id: subscription.user_id,
                        });
                    }
                }
                NotificationAudiencePage::try_new(recipients, next_cursor)
                    .map_err(|_| NotificationProviderError::Internal { retryable: false })
            }
            REPLY_CREATED_TYPE => {
                self.validate_reply_descriptor(&event, &request.descriptor)?;
                let reply_id = event.aggregate_id;
                let Some(reply) = self.load_approved_reply(event.tenant_id, reply_id).await? else {
                    return Ok(NotificationAudiencePage::empty());
                };
                let topic = if self.recipient_context_port.is_none() {
                    self.load_public_topic(event.tenant_id, reply.topic_id)
                        .await?
                } else {
                    self.load_topic_for_subscription_audience(event.tenant_id, reply.topic_id)
                        .await?
                };
                let Some(topic) = topic else {
                    return Ok(NotificationAudiencePage::empty());
                };

                let limit = request.bounded_limit();
                if limit == 0 {
                    return Err(NotificationProviderError::Rejected);
                }
                let (stage, after) = match request.cursor.as_ref() {
                    Some(cursor) => parse_reply_audience_cursor(cursor.as_str())?,
                    None => (ReplyAudienceStage::Topic, None),
                };
                let actor_id = event.actor_id.or(reply.author_id);
                let (candidates, has_more) = match stage {
                    ReplyAudienceStage::Topic => {
                        self.reply_topic_subscription_page(
                            event.tenant_id,
                            topic.id,
                            actor_id,
                            after,
                            limit,
                        )
                        .await?
                    }
                    ReplyAudienceStage::Category => {
                        self.reply_category_subscription_page(
                            event.tenant_id,
                            topic.id,
                            topic.category_id,
                            actor_id,
                            after,
                            limit,
                        )
                        .await?
                    }
                };
                let last_candidate = candidates.last().copied();
                let mut recipients = Vec::with_capacity(candidates.len());
                for recipient_id in candidates {
                    match self
                        .reply_subscription_recipient_availability(
                            event.tenant_id,
                            reply_id,
                            recipient_id,
                        )
                        .await?
                    {
                        ForumTargetAvailability::Visible(_) => {
                            recipients.push(NotificationAudienceCandidate { recipient_id })
                        }
                        ForumTargetAvailability::Deferred => {
                            return Err(NotificationProviderError::Internal { retryable: true });
                        }
                        ForumTargetAvailability::Unavailable => {}
                    }
                }
                let next_cursor = if has_more {
                    last_candidate.map(|after| (stage, after))
                } else {
                    match stage {
                        ReplyAudienceStage::Topic => {
                            Some((ReplyAudienceStage::Category, Uuid::nil()))
                        }
                        ReplyAudienceStage::Category => None,
                    }
                };
                let next_cursor = next_cursor
                    .map(|(stage, after)| {
                        NotificationAudienceCursor::new(encode_reply_audience_cursor(stage, after))
                    })
                    .transpose()
                    .map_err(|_| NotificationProviderError::Internal { retryable: false })?;
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
                    .load_mention_target_for_recipient(&event, &payload, MENTION_AUDIENCE_ACTOR)
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

        let availability = if self.recipient_context_port.is_some() {
            let Some(viewer) = self
                .resolve_recipient_viewer(
                    target_open_context(&request),
                    request.tenant_id,
                    request.recipient_id,
                )
                .await?
            else {
                return Ok(NotificationOpenAuthorization::Unavailable);
            };
            self.load_target_for_viewer(request.tenant_id, source_kind, request.target.id, &viewer)
                .await?
        } else {
            self.load_public_target(request.tenant_id, source_kind, request.target.id)
                .await?
        };

        match availability {
            ForumTargetAvailability::Visible(target) => {
                Ok(NotificationOpenAuthorization::Allowed {
                    route: self.target_route(&target)?,
                })
            }
            ForumTargetAvailability::Deferred | ForumTargetAvailability::Unavailable => {
                Ok(NotificationOpenAuthorization::Unavailable)
            }
        }
    }
}

fn recipient_operation_context(
    tenant_id: Uuid,
    recipient_id: Uuid,
    target_id: Uuid,
    actor: &'static str,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service(actor),
        "und",
        format!("{actor}:{recipient_id}:{target_id}"),
    )
    .with_deadline(RECIPIENT_CONTEXT_DEADLINE)
}

fn target_open_context(request: &AuthorizeNotificationTargetRequest) -> PortContext {
    recipient_operation_context(
        request.tenant_id,
        request.recipient_id,
        request.target.id,
        TARGET_OPEN_ACTOR,
    )
}

fn retryable_database_error(_error: sea_orm::DbErr) -> NotificationProviderError {
    NotificationProviderError::Internal { retryable: true }
}

fn forum_owner_error(error: ForumError) -> NotificationProviderError {
    match error {
        ForumError::CapabilityUnavailable { .. } => {
            NotificationProviderError::CapabilityUnavailable { retryable: false }
        }
        ForumError::CapabilityFailure { retryable, .. } => {
            NotificationProviderError::CapabilityUnavailable { retryable }
        }
        error => NotificationProviderError::Internal {
            retryable: error.is_retryable(),
        },
    }
}

fn is_supported_event_type(event_type: &NotificationTypeKey) -> bool {
    event_type == &topic_created_type()
        || event_type == &topic_replied_type()
        || event_type == &user_mention_added_type()
}

fn encode_reply_audience_cursor(stage: ReplyAudienceStage, after: Uuid) -> String {
    let prefix = match stage {
        ReplyAudienceStage::Topic => REPLY_AUDIENCE_TOPIC_STAGE,
        ReplyAudienceStage::Category => REPLY_AUDIENCE_CATEGORY_STAGE,
    };
    format!("{prefix}:{after}")
}

/// Reply audiences walk two subscription tables in a fixed order (topic-level rows first,
/// then the owning category). The cursor therefore carries the stage plus the last raw
/// subscriber id that was inspected, so a resumed page never revisits or skips a row.
fn parse_reply_audience_cursor(
    cursor: &str,
) -> NotificationProviderResult<(ReplyAudienceStage, Option<Uuid>)> {
    let (prefix, raw) = cursor
        .split_once(':')
        .ok_or(NotificationProviderError::InvalidEvent)?;
    let stage = match prefix {
        REPLY_AUDIENCE_TOPIC_STAGE => ReplyAudienceStage::Topic,
        REPLY_AUDIENCE_CATEGORY_STAGE => ReplyAudienceStage::Category,
        _ => return Err(NotificationProviderError::InvalidEvent),
    };
    let after = Uuid::parse_str(raw).map_err(|_| NotificationProviderError::InvalidEvent)?;
    Ok((stage, Some(after)))
}

fn target_kind_for_source(source_kind: &str) -> NotificationProviderResult<NotificationTargetKind> {
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

fn topic_replied_type() -> NotificationTypeKey {
    NotificationTypeKey::new(TOPIC_REPLIED_TYPE)
        .expect("forum topic-replied notification type must stay valid")
}

fn forum_topic_target_kind() -> NotificationTargetKind {
    NotificationTargetKind::new(FORUM_TOPIC_TARGET)
        .expect("forum topic notification target kind must stay valid")
}

fn forum_reply_target_kind() -> NotificationTargetKind {
    NotificationTargetKind::new(FORUM_REPLY_TARGET)
        .expect("forum reply notification target kind must stay valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_capability_errors_preserve_retryability() {
        assert_eq!(
            forum_owner_error(ForumError::capability_unavailable("facts", "MISSING")),
            NotificationProviderError::CapabilityUnavailable { retryable: false }
        );
        assert_eq!(
            forum_owner_error(ForumError::capability_failure(
                "facts",
                "TIMEOUT",
                "temporary",
                true,
            )),
            NotificationProviderError::CapabilityUnavailable { retryable: true }
        );
        assert_eq!(
            forum_owner_error(ForumError::capability_failure(
                "facts",
                "REJECTED",
                "permanent",
                false,
            )),
            NotificationProviderError::CapabilityUnavailable { retryable: false }
        );
    }

    #[test]
    fn reply_audience_cursor_round_trips_each_stage() {
        let user_id = Uuid::new_v4();
        for (stage, prefix) in [
            (ReplyAudienceStage::Topic, REPLY_AUDIENCE_TOPIC_STAGE),
            (ReplyAudienceStage::Category, REPLY_AUDIENCE_CATEGORY_STAGE),
        ] {
            let cursor = encode_reply_audience_cursor(stage, user_id);
            assert!(cursor.starts_with(&format!("{prefix}:")));
            let (parsed_stage, parsed_user) =
                parse_reply_audience_cursor(&cursor).expect("encoded stage cursors should parse");
            assert_eq!(parsed_stage, stage);
            assert_eq!(parsed_user, Some(user_id));
        }
    }

    #[test]
    fn reply_audience_cursor_rejects_unknown_stage_and_malformed_values() {
        for cursor in [
            Uuid::new_v4().to_string(),
            format!("x:{}", Uuid::new_v4()),
            format!("{}:not-a-uuid", REPLY_AUDIENCE_TOPIC_STAGE),
            format!("{}::extra", REPLY_AUDIENCE_CATEGORY_STAGE),
        ] {
            assert!(
                parse_reply_audience_cursor(&cursor).is_err(),
                "cursor {cursor} must not be accepted"
            );
        }
    }

    #[test]
    fn nil_category_stage_cursor_starts_the_second_stage_from_the_first_row() {
        let cursor = encode_reply_audience_cursor(ReplyAudienceStage::Category, Uuid::nil());
        let (stage, after) =
            parse_reply_audience_cursor(&cursor).expect("stage hand-off cursor should parse");
        assert_eq!(stage, ReplyAudienceStage::Category);
        assert_eq!(after, Some(Uuid::nil()));
    }

    #[test]
    fn replied_type_is_distinct_from_topic_created_type() {
        assert_eq!(topic_replied_type().as_str(), "forum.topic.replied");
    }
}
