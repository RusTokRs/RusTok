use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{HostRuntimeContext, PortActorKind, PortContext, PortError, PortErrorKind};
use rustok_core::SecurityContext;
use rustok_reactions_api::{
    ReactionCatalog, ReactionKey, ReactionProviderError, ReactionProviderResult,
    ReactionSelectionPolicy, ReactionSourceSlug, ReactionSubjectAccess,
    ReactionSubjectAuthorization, ReactionSubjectKind, ReactionSubjectProvider,
    ReactionSubjectProviderFactory, ReactionSubjectRequest,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Statement,
};
use uuid::Uuid;

use crate::audience::SharedForumAudienceFactsPort;
use crate::entities::{forum_reply, forum_reply_revision, forum_topic, forum_topic_revision};
use crate::error::ForumError;
use crate::notification_recipient::{
    ForumNotificationRecipientContextResolver, SharedForumNotificationRecipientContextPort,
};
use crate::services::{ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService};
use crate::state_machine::{ReplyStatus, TopicStatus};

pub const FORUM_REACTION_SOURCE: &str = "forum";
pub const FORUM_TOPIC_REACTION_KIND: &str = "topic";
pub const FORUM_REPLY_REACTION_KIND: &str = "reply";
pub const FORUM_REACTION_V1_KEY: &str = "like";

#[derive(Clone, Default)]
pub struct ForumReactionSubjectProviderFactory;

impl ReactionSubjectProviderFactory for ForumReactionSubjectProviderFactory {
    fn source(&self) -> ReactionSourceSlug {
        forum_reaction_source()
    }

    fn build(
        &self,
        host: &HostRuntimeContext,
    ) -> ReactionProviderResult<Arc<dyn ReactionSubjectProvider>> {
        Ok(Arc::new(ForumReactionSubjectProvider::new(
            host.db_clone(),
            host.shared_get::<SharedForumNotificationRecipientContextPort>(),
            host.shared_get::<SharedForumAudienceFactsPort>(),
        )))
    }
}

#[derive(Clone)]
struct ForumReactionSubjectProvider {
    db: DatabaseConnection,
    recipient_context_port: Option<SharedForumNotificationRecipientContextPort>,
    facts_port: Option<SharedForumAudienceFactsPort>,
}

impl ForumReactionSubjectProvider {
    fn new(
        db: DatabaseConnection,
        recipient_context_port: Option<SharedForumNotificationRecipientContextPort>,
        facts_port: Option<SharedForumAudienceFactsPort>,
    ) -> Self {
        Self {
            db,
            recipient_context_port,
            facts_port,
        }
    }

    async fn authorize_topic(
        &self,
        context: &PortContext,
        request: &ReactionSubjectRequest,
        actor_id: Option<Uuid>,
    ) -> ReactionProviderResult<ReactionSubjectAuthorization> {
        let subject = &request.subject;
        let viewer = self
            .resolve_viewer(context, subject.tenant_id(), actor_id)
            .await?;
        if !ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())
            .is_topic_visible(
                subject.tenant_id(),
                subject.subject_id(),
                context.channel.as_deref(),
                &viewer,
            )
            .await
            .map_err(map_forum_error)?
        {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        }

        let Some(topic) = self
            .load_active_topic(subject.tenant_id(), subject.subject_id())
            .await?
        else {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        };
        if topic.status != TopicStatus::Open {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        }

        let current_revision = self
            .current_topic_revision(subject.tenant_id(), topic.id)
            .await?;
        self.allow_exact_revision(request, current_revision)
    }

    async fn authorize_reply(
        &self,
        context: &PortContext,
        request: &ReactionSubjectRequest,
        actor_id: Option<Uuid>,
    ) -> ReactionProviderResult<ReactionSubjectAuthorization> {
        let subject = &request.subject;
        let Some(initial_reply) = self
            .load_active_reply(subject.tenant_id(), subject.subject_id())
            .await?
        else {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        };
        if initial_reply.status != ReplyStatus::Approved {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        }

        let viewer = self
            .resolve_viewer(context, subject.tenant_id(), actor_id)
            .await?;
        if !ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())
            .is_topic_visible(
                subject.tenant_id(),
                initial_reply.topic_id,
                context.channel.as_deref(),
                &viewer,
            )
            .await
            .map_err(map_forum_error)?
        {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        }

        let Some(reply) = self
            .load_active_reply(subject.tenant_id(), subject.subject_id())
            .await?
        else {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        };
        if reply.status != ReplyStatus::Approved || reply.topic_id != initial_reply.topic_id {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        }
        let Some(topic) = self
            .load_active_topic(subject.tenant_id(), reply.topic_id)
            .await?
        else {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        };
        if topic.status != TopicStatus::Open {
            return Ok(ReactionSubjectAuthorization::Unavailable);
        }

        let current_revision = self
            .current_reply_revision(subject.tenant_id(), reply.id)
            .await?;
        self.allow_exact_revision(request, current_revision)
    }

    fn allow_exact_revision(
        &self,
        request: &ReactionSubjectRequest,
        current_revision: u64,
    ) -> ReactionProviderResult<ReactionSubjectAuthorization> {
        if request.subject.subject_revision() != current_revision {
            return Err(ReactionProviderError::Conflict);
        }
        Ok(ReactionSubjectAuthorization::Allowed {
            canonical_subject: request.subject.clone(),
            catalog: forum_reaction_catalog_v1()?,
        })
    }

    async fn resolve_viewer(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
    ) -> ReactionProviderResult<ForumTopicAudienceViewer> {
        let Some(actor_id) = actor_id else {
            return Ok(ForumTopicAudienceViewer::public());
        };

        match context.actor.kind {
            PortActorKind::User => {
                let context_actor = Uuid::parse_str(&context.actor.id)
                    .map_err(|_| ReactionProviderError::InvalidRequest)?;
                if context_actor != actor_id {
                    return Err(ReactionProviderError::InvalidRequest);
                }
                let security =
                    SecurityContext::try_from_port_context(context).map_err(map_port_error)?;
                ForumTopicAudienceViewer::authenticated(security, context.clone())
                    .map_err(map_forum_error)
            }
            PortActorKind::Service | PortActorKind::System => {
                let recipient = ForumNotificationRecipientContextResolver::new(
                    self.recipient_context_port.clone(),
                )
                .resolve(context.clone(), tenant_id, actor_id)
                .await
                .map_err(map_forum_error)?;
                recipient.into_topic_viewer().map_err(map_forum_error)
            }
        }
    }

    async fn load_active_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ReactionProviderResult<Option<forum_topic::Model>> {
        let topic = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Id.eq(topic_id))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        let Some(topic) = topic else {
            return Ok(None);
        };
        if !self
            .row_is_active("forum_topics", tenant_id, topic_id)
            .await?
        {
            return Ok(None);
        }
        Ok(Some(topic))
    }

    async fn load_active_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ReactionProviderResult<Option<forum_reply::Model>> {
        let reply = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::Id.eq(reply_id))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        let Some(reply) = reply else {
            return Ok(None);
        };
        if !self
            .row_is_active("forum_replies", tenant_id, reply_id)
            .await?
        {
            return Ok(None);
        }
        Ok(Some(reply))
    }

    async fn row_is_active(
        &self,
        table: &'static str,
        tenant_id: Uuid,
        id: Uuid,
    ) -> ReactionProviderResult<bool> {
        self.db
            .query_one(Statement::from_string(
                self.db.get_database_backend(),
                format!(
                    "SELECT 1 AS active FROM {table} WHERE tenant_id = '{tenant_id}' AND id = '{id}' AND deleted_at IS NULL"
                ),
            ))
            .await
            .map(|row| row.is_some())
            .map_err(database_error)
    }

    async fn current_topic_revision(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ReactionProviderResult<u64> {
        let latest = forum_topic_revision::Entity::find()
            .select_only()
            .column(forum_topic_revision::Column::Id)
            .filter(forum_topic_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_revision::Column::TopicId.eq(topic_id))
            .order_by_desc(forum_topic_revision::Column::Id)
            .into_tuple::<i64>()
            .one(&self.db)
            .await
            .map_err(database_error)?;
        current_revision_after(latest)
    }

    async fn current_reply_revision(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ReactionProviderResult<u64> {
        let latest = forum_reply_revision::Entity::find()
            .select_only()
            .column(forum_reply_revision::Column::Id)
            .filter(forum_reply_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_revision::Column::ReplyId.eq(reply_id))
            .order_by_desc(forum_reply_revision::Column::Id)
            .into_tuple::<i64>()
            .one(&self.db)
            .await
            .map_err(database_error)?;
        current_revision_after(latest)
    }
}

#[async_trait]
impl ReactionSubjectProvider for ForumReactionSubjectProvider {
    fn source(&self) -> ReactionSourceSlug {
        forum_reaction_source()
    }

    fn display_name(&self) -> &'static str {
        "Forum"
    }

    fn supported_kinds(&self) -> Vec<ReactionSubjectKind> {
        vec![forum_topic_reaction_kind(), forum_reply_reaction_kind()]
    }

    async fn authorize(
        &self,
        context: PortContext,
        request: ReactionSubjectRequest,
    ) -> ReactionProviderResult<ReactionSubjectAuthorization> {
        request
            .validate()
            .map_err(|_| ReactionProviderError::InvalidRequest)?;
        let tenant_id = Uuid::parse_str(&context.tenant_id)
            .map_err(|_| ReactionProviderError::InvalidRequest)?;
        if tenant_id != request.subject.tenant_id()
            || request.subject.source().as_str() != FORUM_REACTION_SOURCE
        {
            return Err(ReactionProviderError::InvalidRequest);
        }

        let actor_id = actor_id_for_access(&request.access);
        match request.subject.kind().as_str() {
            FORUM_TOPIC_REACTION_KIND => self.authorize_topic(&context, &request, actor_id).await,
            FORUM_REPLY_REACTION_KIND => self.authorize_reply(&context, &request, actor_id).await,
            _ => Err(ReactionProviderError::InvalidRequest),
        }
    }
}

fn actor_id_for_access(access: &ReactionSubjectAccess) -> Option<Uuid> {
    match access {
        ReactionSubjectAccess::Read { actor_id } => *actor_id,
        ReactionSubjectAccess::Apply { command } => Some(command.identity().actor_id()),
    }
}

fn current_revision_after(latest: Option<i64>) -> ReactionProviderResult<u64> {
    match latest {
        None => Ok(1),
        Some(latest) => u64::try_from(latest)
            .ok()
            .and_then(|revision| revision.checked_add(1))
            .filter(|revision| *revision > 0)
            .ok_or(ReactionProviderError::Internal { retryable: false }),
    }
}

fn forum_reaction_catalog_v1() -> ReactionProviderResult<ReactionCatalog> {
    ReactionCatalog::try_new(
        ReactionSelectionPolicy::Single,
        vec![
            ReactionKey::new(FORUM_REACTION_V1_KEY)
                .map_err(|_| ReactionProviderError::Internal { retryable: false })?,
        ],
    )
    .map_err(|_| ReactionProviderError::Internal { retryable: false })
}

fn forum_reaction_source() -> ReactionSourceSlug {
    ReactionSourceSlug::new(FORUM_REACTION_SOURCE)
        .expect("Forum reaction source constant must remain valid")
}

fn forum_topic_reaction_kind() -> ReactionSubjectKind {
    ReactionSubjectKind::new(FORUM_TOPIC_REACTION_KIND)
        .expect("Forum topic reaction kind constant must remain valid")
}

fn forum_reply_reaction_kind() -> ReactionSubjectKind {
    ReactionSubjectKind::new(FORUM_REPLY_REACTION_KIND)
        .expect("Forum reply reaction kind constant must remain valid")
}

fn database_error(_error: sea_orm::DbErr) -> ReactionProviderError {
    ReactionProviderError::Internal { retryable: true }
}

fn map_port_error(error: PortError) -> ReactionProviderError {
    match error.kind {
        PortErrorKind::Validation => ReactionProviderError::InvalidRequest,
        PortErrorKind::Forbidden | PortErrorKind::NotFound => ReactionProviderError::Unavailable,
        PortErrorKind::Conflict => ReactionProviderError::Conflict,
        PortErrorKind::Timeout | PortErrorKind::Unavailable => {
            ReactionProviderError::CapabilityUnavailable { retryable: true }
        }
        PortErrorKind::InvariantViolation => ReactionProviderError::Internal { retryable: false },
    }
}

fn map_forum_error(error: ForumError) -> ReactionProviderError {
    match error {
        ForumError::CapabilityUnavailable { .. } => {
            ReactionProviderError::CapabilityUnavailable { retryable: false }
        }
        ForumError::CapabilityFailure { retryable, .. } => {
            ReactionProviderError::CapabilityUnavailable { retryable }
        }
        ForumError::Validation(_) => ReactionProviderError::InvalidRequest,
        ForumError::RelationRevisionConflict => ReactionProviderError::Conflict,
        ForumError::Database(_) | ForumError::Internal(_) => {
            ReactionProviderError::Internal { retryable: true }
        }
        ForumError::Content(_) => ReactionProviderError::Internal { retryable: false },
        _ => ReactionProviderError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use rustok_reactions_api::ReactionSelectionPolicy;

    use super::*;

    #[test]
    fn forum_catalog_is_single_like_without_vote_reinterpretation() {
        let catalog = forum_reaction_catalog_v1().expect("fixed catalog should be valid");
        assert_eq!(catalog.selection(), ReactionSelectionPolicy::Single);
        assert_eq!(catalog.keys().len(), 1);
        assert_eq!(catalog.keys()[0].as_str(), FORUM_REACTION_V1_KEY);
    }

    #[test]
    fn current_revision_is_positive_and_advances_after_captured_history() {
        assert_eq!(current_revision_after(None).expect("initial revision"), 1);
        assert_eq!(
            current_revision_after(Some(41)).expect("advanced revision"),
            42
        );
        assert!(current_revision_after(Some(-1)).is_err());
    }
}
