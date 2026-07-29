use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::audience::SharedForumAudienceFactsPort;
use crate::dto::TopicResponse;
use crate::error::{ForumError, ForumResult};

use super::rbac::enforce_scope;
use super::topic_audience::{ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService};
use super::topic_facade::TopicService;

/// Exact storefront topic read composition over the canonical topic owner and
/// every persisted category/topic audience layer.
///
/// Public reads require no optional owner facts. Authenticated reads accept one
/// exact read-only `PortContext`; tenant, actor, locale, route channel, deadline,
/// and claims are therefore supplied by the caller rather than reconstructed
/// from request DTOs inside the Forum owner.
pub struct ForumTopicAudienceReadService {
    topic_service: TopicService,
    visibility: ForumTopicAudienceVisibilityService,
}

impl ForumTopicAudienceReadService {
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
            topic_service: TopicService::new(db.clone(), event_bus),
            visibility: ForumTopicAudienceVisibilityService::new(db, facts_port),
        }
    }

    /// Exact public storefront read. Missing, closed, route-channel denied,
    /// category-denied, and richer-audience-denied topics all resolve as absent.
    pub async fn get_public_storefront_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> ForumResult<Option<TopicResponse>> {
        let security = SecurityContext::public_read();
        let viewer = ForumTopicAudienceViewer::public();
        self.get_visible(
            tenant_id,
            security,
            &viewer,
            topic_id,
            locale,
            fallback_locale,
            channel_slug,
        )
        .await
    }

    /// Exact authenticated storefront read. The effective locale and route
    /// channel come only from the validated caller context. A context whose
    /// tenant or actor differs from `security` fails before topic lookup or an
    /// optional facts-provider call.
    pub async fn get_authenticated_storefront_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        topic_id: Uuid,
        fallback_locale: Option<&str>,
    ) -> ForumResult<Option<TopicResponse>> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let locale = context.locale.trim().to_string();
        if locale.is_empty() {
            return Err(ForumError::Validation(
                "Forum topic audience read context locale is unavailable".to_string(),
            ));
        }
        let channel_slug = context.channel.clone();
        let viewer = ForumTopicAudienceViewer::authenticated(security.clone(), context)?;
        self.get_visible(
            tenant_id,
            security,
            &viewer,
            topic_id,
            &locale,
            fallback_locale,
            channel_slug.as_deref(),
        )
        .await
    }

    async fn get_visible(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        viewer: &ForumTopicAudienceViewer,
        topic_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> ForumResult<Option<TopicResponse>> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        if !self
            .visibility
            .is_topic_visible(tenant_id, topic_id, channel_slug, viewer)
            .await?
        {
            return Ok(None);
        }

        match self
            .topic_service
            .get_with_locale_fallback(tenant_id, security, topic_id, locale, fallback_locale)
            .await
        {
            Ok(topic) => Ok(Some(topic)),
            Err(ForumError::TopicNotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
