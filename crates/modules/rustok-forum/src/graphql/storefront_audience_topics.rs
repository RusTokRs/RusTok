use std::{
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use async_graphql::{Context, Object, Result};
use prometheus::{IntCounterVec, Opts};
use rustok_api::{
    RequestContext, TenantContext,
    graphql::{PaginationInput, require_module_enabled, resolve_graphql_locale},
};
use rustok_outbox::TransactionalEventBus;
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{ForumTopicAudienceListService, TopicListItem};

use super::types::*;

const MODULE_SLUG: &str = "forum";
const STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_EXACT: &str = "exact";
const STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_FALLBACK: &str = "fallback";
const STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_MISSING: &str = "missing";

static FORUM_GRAPHQL_STOREFRONT_TOPIC_LIST_LOCALE_RESOLUTION_TOTAL: OnceLock<IntCounterVec> =
    OnceLock::new();
static FORUM_GRAPHQL_STOREFRONT_TOPIC_LIST_LOCALE_RESOLUTION_REGISTERED: AtomicBool =
    AtomicBool::new(false);

#[derive(Default)]
pub struct ForumStorefrontAudienceTopicsQuery;

#[Object]
impl ForumStorefrontAudienceTopicsQuery {
    /// Public storefront topic pagination through the exact richer-audience owner.
    ///
    /// The response page and total are derived from the same allowed sequence.
    /// Authenticated callers may use this field as the public fallback surface;
    /// user-specific unread composition remains on `forumStorefrontUnreadTopics`.
    async fn forum_storefront_audience_topics(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        category_id: Option<Uuid>,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<ForumTopicConnection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_public_forum_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;
        let request = ctx.data_opt::<RequestContext>();
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let list_started_at = Instant::now();
        let page = ForumTopicAudienceListService::new(db.clone(), event_bus.clone())
            .list_public_storefront_visible_with_locale_fallback(
                tenant_id,
                crate::ListTopicsFilter {
                    category_id,
                    status: None,
                    locale: Some(locale),
                    page: (offset / limit + 1) as u64,
                    per_page: limit as u64,
                },
                Some(tenant.default_locale.as_str()),
                request.and_then(|request| request.channel_slug.as_deref()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.storefront_audience_topics",
            "exact_audience_owner",
            list_started_at.elapsed().as_secs_f64(),
            page.total,
        );
        observe_storefront_topic_list_locale_resolution(&page.items);

        let items = page
            .items
            .into_iter()
            .map(map_topic_list_item)
            .collect::<Vec<_>>();
        metrics::record_read_path_budget(
            "graphql",
            "forum.storefront_audience_topics",
            Some(requested_limit),
            limit as u64,
            items.len(),
        );

        Ok(ForumTopicConnection::new(
            items,
            page.total as i64,
            offset,
            limit,
        ))
    }
}

fn storefront_topic_list_locale_resolution_outcome(
    requested_locale: &str,
    effective_locale: &str,
    available_locale_count: usize,
) -> &'static str {
    if available_locale_count == 0 {
        STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_MISSING
    } else if requested_locale == effective_locale {
        STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_EXACT
    } else {
        STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_FALLBACK
    }
}

fn forum_graphql_storefront_topic_list_locale_resolution_counter() -> Option<&'static IntCounterVec>
{
    let counter = FORUM_GRAPHQL_STOREFRONT_TOPIC_LIST_LOCALE_RESOLUTION_TOTAL.get_or_init(|| {
        IntCounterVec::new(
            Opts::new(
                "rustok_forum_graphql_storefront_topic_list_locale_resolution_total",
                "Forum GraphQL storefront topic-list items by fixed locale resolution outcome",
            ),
            &["outcome"],
        )
        .expect("Forum GraphQL storefront topic-list locale metric descriptor must be valid")
    });

    if FORUM_GRAPHQL_STOREFRONT_TOPIC_LIST_LOCALE_RESOLUTION_REGISTERED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
        && rustok_telemetry::register_runtime_collector(Box::new(counter.clone())).is_err()
    {
        FORUM_GRAPHQL_STOREFRONT_TOPIC_LIST_LOCALE_RESOLUTION_REGISTERED
            .store(false, Ordering::Release);
        return None;
    }

    Some(counter)
}

fn observe_storefront_topic_list_locale_resolution(items: &[TopicListItem]) {
    let Some(counter) = forum_graphql_storefront_topic_list_locale_resolution_counter() else {
        return;
    };

    for item in items {
        let outcome = storefront_topic_list_locale_resolution_outcome(
            &item.requested_locale,
            &item.effective_locale,
            item.available_locales.len(),
        );
        counter.with_label_values(&[outcome]).inc();
    }
}

fn map_topic_list_item(topic: TopicListItem) -> GqlForumTopicListItem {
    GqlForumTopicListItem {
        id: topic.id,
        requested_locale: topic.requested_locale,
        locale: topic.locale,
        effective_locale: topic.effective_locale,
        available_locales: topic.available_locales,
        category_id: topic.category_id,
        author_id: topic.author_id,
        author_profile: None,
        title: topic.title,
        slug: topic.slug,
        metadata: topic.metadata,
        status: topic.status,
        channel_slugs: topic.channel_slugs,
        vote_score: topic.vote_score,
        current_user_vote: topic.current_user_vote,
        is_subscribed: topic.is_subscribed,
        solution_reply_id: topic.solution_reply_id,
        is_pinned: topic.is_pinned,
        is_locked: topic.is_locked,
        reply_count: topic.reply_count,
        created_at: topic.created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_EXACT, STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_FALLBACK,
        STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_MISSING,
        storefront_topic_list_locale_resolution_outcome,
    };

    #[test]
    fn storefront_topic_list_locale_outcomes_are_fixed_and_hide_locale_values() {
        assert_eq!(
            storefront_topic_list_locale_resolution_outcome("ru", "ru", 1),
            STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_EXACT
        );
        assert_eq!(
            storefront_topic_list_locale_resolution_outcome("tenant-secret", "different-secret", 2,),
            STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_FALLBACK
        );
        assert_eq!(
            storefront_topic_list_locale_resolution_outcome("tenant-secret", "tenant-secret", 0,),
            STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_MISSING
        );
    }
}
