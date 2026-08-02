use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::audience::SharedForumAudienceFactsPort;
use crate::dto::{ListRepliesFilter, ReplyListItem, ReplyResponse};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::ReplyStatus;

use super::rbac::enforce_scope;
use super::reply_facade::ReplyService;
use super::topic_audience_visibility::{
    ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService,
};

/// Exact reply read owner. Every reply decision is derived from the audience
/// policy of its parent topic before reply content or pagination is returned.
pub struct ForumReplyAudienceReadService {
    reply_service: ReplyService,
    visibility: ForumTopicAudienceVisibilityService,
}

impl ForumReplyAudienceReadService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self::with_optional_audience_facts(db, event_bus, None)
    }

    pub fn with_audience_facts(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        facts_port: SharedForumAudienceFactsPort,
    ) -> Self {
        Self::with_optional_audience_facts(db, event_bus, Some(facts_port))
    }

    fn with_optional_audience_facts(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        facts_port: Option<SharedForumAudienceFactsPort>,
    ) -> Self {
        Self {
            reply_service: ReplyService::new(db.clone(), event_bus),
            visibility: ForumTopicAudienceVisibilityService::new(db, facts_port),
        }
    }

    /// Exact authenticated owner read for one reply. Closed topics remain
    /// readable to authorized owners, while every category/topic audience layer
    /// is enforced before the reply body is loaded.
    pub async fn get_authenticated_owner_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        reply_id: Uuid,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ReplyResponse> {
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let locale = required_context_locale(&context)?;
        let reply = self.reply_service.find_reply(tenant_id, reply_id).await?;
        let viewer = ForumTopicAudienceViewer::authenticated(security.clone(), context)?;
        if !self
            .visibility
            .is_topic_owner_visible(tenant_id, reply.topic_id, &viewer)
            .await?
        {
            return Err(ForumError::ReplyNotFound(reply_id));
        }

        self.reply_service
            .get_with_locale_fallback(tenant_id, security, reply_id, &locale, fallback_locale)
            .await
    }

    /// Exact anonymous public read for one reply. Typed status and parent-topic
    /// visibility are resolved before the reply body is loaded. Missing, denied,
    /// non-approved and route-channel-ineligible replies are indistinguishable.
    pub async fn get_public_storefront_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<Option<ReplyResponse>> {
        let security = SecurityContext::public_read();
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let reply = match self.reply_service.find_reply(tenant_id, reply_id).await {
            Ok(reply) => reply,
            Err(ForumError::ReplyNotFound(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        if statuses.is_some_and(|allowed| !allowed.contains(&reply.status)) {
            return Ok(None);
        }
        let viewer = ForumTopicAudienceViewer::public();
        if !self
            .visibility
            .is_topic_visible(tenant_id, reply.topic_id, channel_slug, &viewer)
            .await?
        {
            return Ok(None);
        }

        match self
            .reply_service
            .get_with_locale_fallback(tenant_id, security, reply_id, locale, fallback_locale)
            .await
        {
            Ok(reply) => Ok(Some(reply)),
            Err(ForumError::ReplyNotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Exact authenticated REST-style reply list through parent-topic owner
    /// visibility. Denied and missing parent topics both resolve as absent.
    pub async fn list_authenticated_owner_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<ReplyListItem>, u64)> {
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        let locale = required_context_locale(&context)?;
        let viewer = ForumTopicAudienceViewer::authenticated(security.clone(), context)?;
        self.require_owner_topic_visible(tenant_id, topic_id, &viewer)
            .await?;
        filter.locale = Some(locale);
        self.reply_service
            .list_for_topic_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
            )
            .await
    }

    /// Exact authenticated GraphQL owner list returning full reply responses.
    pub async fn list_response_authenticated_owner_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<(Vec<ReplyResponse>, u64)> {
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        let locale = required_context_locale(&context)?;
        let viewer = ForumTopicAudienceViewer::authenticated(security.clone(), context)?;
        self.require_owner_topic_visible(tenant_id, topic_id, &viewer)
            .await?;
        filter.locale = Some(locale);
        self.reply_service
            .list_response_for_topic_by_statuses_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
                statuses,
            )
            .await
    }

    /// Exact public storefront reply list. Missing, closed, channel-denied or
    /// richer-audience-denied parent topics all return an empty connection.
    pub async fn list_public_storefront_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<(Vec<ReplyResponse>, u64)> {
        let security = SecurityContext::public_read();
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        let viewer = ForumTopicAudienceViewer::public();
        if !self
            .visibility
            .is_topic_visible(tenant_id, topic_id, channel_slug, &viewer)
            .await?
        {
            return Ok((Vec::new(), 0));
        }

        self.reply_service
            .list_response_for_topic_by_statuses_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
                statuses,
            )
            .await
    }

    /// Exact authenticated storefront reply list. The route channel and locale
    /// come only from the validated caller context. Denied topics return an empty
    /// connection without exposing the rejecting policy layer.
    pub async fn list_authenticated_storefront_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<(Vec<ReplyResponse>, u64)> {
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        let locale = required_context_locale(&context)?;
        let channel_slug = context.channel.clone();
        let viewer = ForumTopicAudienceViewer::authenticated(security.clone(), context)?;
        if !self
            .visibility
            .is_topic_visible(tenant_id, topic_id, channel_slug.as_deref(), &viewer)
            .await?
        {
            return Ok((Vec::new(), 0));
        }
        filter.locale = Some(locale);

        self.reply_service
            .list_response_for_topic_by_statuses_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
                statuses,
            )
            .await
    }

    async fn require_owner_topic_visible(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        viewer: &ForumTopicAudienceViewer,
    ) -> ForumResult<()> {
        if self
            .visibility
            .is_topic_owner_visible(tenant_id, topic_id, viewer)
            .await?
        {
            Ok(())
        } else {
            Err(ForumError::TopicNotFound(topic_id))
        }
    }
}

fn required_context_locale(context: &PortContext) -> ForumResult<String> {
    let locale = context.locale.trim();
    if locale.is_empty() {
        return Err(ForumError::Validation(
            "Forum reply audience read context locale is unavailable".to_string(),
        ));
    }
    Ok(locale.to_string())
}
