use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::audience::SharedForumAudienceFactsPort;
use crate::dto::{ListTopicsFilter, MAX_FORUM_READ_LIMIT, TopicListItem};
use crate::error::{ForumError, ForumResult};

use super::rbac::enforce_scope;
use super::topic_audience_visibility::{
    ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService,
};
use super::engagement_mode::ForumSettingsProviders;
use super::topic_facade::TopicService;

const FORUM_TOPIC_AUDIENCE_SCAN_PAGE_SIZE: u64 = MAX_FORUM_READ_LIMIT;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ForumTopicAudiencePage {
    pub items: Vec<TopicListItem>,
    pub next_cursor: Option<String>,
}

/// Which read contract a topic-list scan enforces.
///
/// Both variants share one bounded keyset scan. They differ only in the base candidate
/// query and in the per-topic predicate, so the owner and storefront paths cannot drift
/// in how they page or count visible topics.
#[derive(Clone, Copy)]
enum TopicListAudience<'a> {
    /// Storefront contract: open topics, route channel, and every audience layer.
    Storefront { channel_slug: Option<&'a str> },
    /// Owner/admin contract: every status, the inherited category floor, and every
    /// audience layer. Channel visibility is not part of the owner read.
    Owner,
}

/// Exact topic-list owner over the canonical base visibility query and every persisted
/// category/topic audience layer.
///
/// The owner scans the base candidate set in bounded database pages, applies the richer
/// decision before output pagination, and derives `items` and the next cursor from the
/// same allowed sequence. This prevents hidden topics from producing sparse pages or
/// leaking through a pre-audience count.
pub struct ForumTopicAudienceListService {
    topic_service: TopicService,
    visibility: ForumTopicAudienceVisibilityService,
}

impl ForumTopicAudienceListService {
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

    /// Applies tenant settings (for example `show_locked_topics_in_lists`) to the
    /// base topic read path, so the audience-aware list honours the same flags as
    /// the plain list.
    pub fn with_settings_providers(mut self, settings: ForumSettingsProviders) -> Self {
        self.topic_service = self.topic_service.with_settings_providers(settings);
        self
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

    pub async fn list_public_storefront_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> ForumResult<ForumTopicAudiencePage> {
        self.list_visible(
            tenant_id,
            SecurityContext::public_read(),
            ForumTopicAudienceViewer::public(),
            filter,
            fallback_locale,
            TopicListAudience::Storefront { channel_slug },
        )
        .await
    }

    pub async fn list_authenticated_storefront_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        mut filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ForumTopicAudiencePage> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        let locale = context.locale.trim().to_string();
        if locale.is_empty() {
            return Err(ForumError::Validation(
                "Forum topic audience list context locale is unavailable".to_string(),
            ));
        }
        let channel_slug = context.channel.clone();
        let viewer = ForumTopicAudienceViewer::authenticated(security.clone(), context)?;
        filter.locale = Some(locale);
        self.list_visible(
            tenant_id,
            security,
            viewer,
            filter,
            fallback_locale,
            TopicListAudience::Storefront {
                channel_slug: channel_slug.as_deref(),
            },
        )
        .await
    }

    /// Exact authenticated owner/admin topic list. The effective locale comes from the
    /// validated caller context and overrides any locale in the filter so the page and the
    /// audience facts always describe the same request.
    pub async fn list_authenticated_owner_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        mut filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ForumTopicAudiencePage> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        let locale = context.locale.trim().to_string();
        if locale.is_empty() {
            return Err(ForumError::Validation(
                "Forum topic audience list context locale is unavailable".to_string(),
            ));
        }
        let viewer = ForumTopicAudienceViewer::authenticated(security.clone(), context)?;
        filter.locale = Some(locale);
        self.list_visible(
            tenant_id,
            security,
            viewer,
            filter,
            fallback_locale,
            TopicListAudience::Owner,
        )
        .await
    }

    async fn list_visible(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        viewer: ForumTopicAudienceViewer,
        filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        audience: TopicListAudience<'_>,
    ) -> ForumResult<ForumTopicAudiencePage> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;
        if !(1..=MAX_FORUM_READ_LIMIT).contains(&filter.per_page) {
            return Err(ForumError::Validation(format!(
                "Forum topic audience page size must be between 1 and {MAX_FORUM_READ_LIMIT}"
            )));
        }
        let per_page = filter.per_page as usize;

        // Walk owner keyset batches from the cursor, keeping only topics the audience can
        // see. The cursor advances over scanned candidates; a page is closed only when one
        // more visible topic exists, so the last page never comes back empty.
        let mut items: Vec<TopicListItem> = Vec::with_capacity(per_page);
        let mut candidate_after = filter.after.clone();
        let mut has_more = false;
        'scan: loop {
            let mut candidate_filter = filter.clone();
            candidate_filter.after = candidate_after.clone();
            candidate_filter.per_page = FORUM_TOPIC_AUDIENCE_SCAN_PAGE_SIZE;

            let candidates = match audience {
                TopicListAudience::Storefront { channel_slug } => {
                    self.topic_service
                        .list_storefront_visible_with_locale_fallback(
                            tenant_id,
                            security.clone(),
                            candidate_filter,
                            fallback_locale,
                            channel_slug,
                        )
                        .await?
                }
                TopicListAudience::Owner => {
                    self.topic_service
                        .list_with_locale_fallback(
                            tenant_id,
                            security.clone(),
                            candidate_filter,
                            fallback_locale,
                        )
                        .await?
                }
            };

            if candidates.items.is_empty() {
                break;
            }

            for topic in candidates.items {
                let visible = match audience {
                    TopicListAudience::Storefront { channel_slug } => {
                        self.visibility
                            .is_topic_visible(tenant_id, topic.id, channel_slug, &viewer)
                            .await?
                    }
                    TopicListAudience::Owner => {
                        self.visibility
                            .is_topic_owner_visible(tenant_id, topic.id, &viewer)
                            .await?
                    }
                };
                if visible {
                    if items.len() == per_page {
                        has_more = true;
                        break 'scan;
                    }
                    items.push(topic);
                }
            }

            match candidates.next_cursor {
                Some(cursor) => candidate_after = Some(cursor),
                None => break,
            }
        }

        let next_cursor = if has_more {
            items
                .last()
                .and_then(|topic| topic.sort_key)
                .map(|key| key.encode())
        } else {
            None
        };

        Ok(ForumTopicAudiencePage { items, next_cursor })
    }
}
