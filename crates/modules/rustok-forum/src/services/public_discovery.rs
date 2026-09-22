use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::dto::{CategoryResponse, ReplyResponse, TopicResponse};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::ReplyStatus;

use super::{
    ForumCategoryAudienceReadService, ForumReplyAudienceReadService, ForumTopicAudienceReadService,
};

/// Canonical public discovery owner for cross-consumer surfaces such as SEO,
/// route resolution and Search projection materialization.
///
/// The owner deliberately exposes only the anonymous public decision. Content
/// requiring authentication, trust, Groups, explicit users, roles, inherited
/// category selectors, or an unavailable route channel is absent rather than
/// downgraded to a weaker public approximation.
pub struct ForumPublicDiscoveryService {
    categories: ForumCategoryAudienceReadService,
    topics: ForumTopicAudienceReadService,
    replies: ForumReplyAudienceReadService,
}

impl ForumPublicDiscoveryService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            categories: ForumCategoryAudienceReadService::new(db.clone()),
            topics: ForumTopicAudienceReadService::new(db.clone(), event_bus.clone()),
            replies: ForumReplyAudienceReadService::new(db, event_bus),
        }
    }

    /// Returns a category only when it is visible to an anonymous public
    /// consumer through the inherited base floor and every richer category
    /// audience layer.
    pub async fn get_public_category_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<Option<CategoryResponse>> {
        match self
            .categories
            .get_public_storefront_visible_with_locale_fallback(
                tenant_id,
                category_id,
                locale,
                fallback_locale,
            )
            .await
        {
            Ok(category) => Ok(Some(category)),
            Err(ForumError::CategoryNotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Returns a topic only when the exact current public decision allows the
    /// topic for the supplied route channel. Closed, foreign, inherited-policy
    /// denied, topic-policy denied and route-channel denied targets are all
    /// represented by `None`.
    pub async fn get_public_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> ForumResult<Option<TopicResponse>> {
        self.topics
            .get_public_storefront_visible_with_locale_fallback(
                tenant_id,
                topic_id,
                locale,
                fallback_locale,
                channel_slug,
            )
            .await
    }

    /// Returns one reply only when its typed status is allowed and its parent
    /// topic is exactly visible to an anonymous public consumer. The reply body
    /// is loaded only after the parent decision succeeds.
    pub async fn get_public_reply_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<Option<ReplyResponse>> {
        self.replies
            .get_public_storefront_visible_with_locale_fallback(
                tenant_id,
                reply_id,
                locale,
                fallback_locale,
                channel_slug,
                statuses,
            )
            .await
    }
}
