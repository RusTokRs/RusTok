use leptos::prelude::*;

use super::graphql_adapter::ApiError;
use crate::model::StorefrontForumData;

#[cfg(feature = "ssr")]
use crate::model::{
    ForumCategoryConnection, ForumCategoryListItem, ForumMemberCard, ForumMemberProfileSummary,
    ForumMemberStats, ForumReplyConnection, ForumReplyDetail, ForumTopicConnection,
    ForumTopicDetail, ForumTopicListItem,
};

pub async fn fetch_storefront_forum_server(
    selected_category_id: Option<String>,
    selected_topic_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontForumData, ApiError> {
    storefront_forum_native(selected_category_id, selected_topic_id, locale)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

pub async fn mark_storefront_topic_read_server(
    topic_id: String,
    locale: Option<String>,
) -> Result<(), ApiError> {
    storefront_topic_mark_read_native(topic_id, locale)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "forum/storefront-data")]
async fn storefront_forum_native(
    selected_category_id: Option<String>,
    selected_topic_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontForumData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{
            HostRuntimeContext, OptionalAuthContext, Permission, RequestContext, TenantContext,
            has_any_effective_permission,
        };
        use rustok_core::SecurityContext;
        use rustok_forum::services::user_stats::ForumMemberCardService;
        use rustok_forum::{
            ForumCategoryAudienceReadService, ForumCategoryReadOperation,
            ForumCategoryReadTransport, ForumReplyAudienceReadService, ForumReplyReadOperation,
            ForumReplyReadTransport, ForumStorefrontReadStateService,
            ForumTopicAudienceListService, ForumTopicAudienceReadService, ForumTopicReadOperation,
            ForumTopicReadTransport, ListRepliesFilter, ListTopicsFilter, ReplyStatus,
            SharedForumAudienceFactsPort, category_read_audience_port_context,
            reply_read_audience_port_context, topic_read_audience_port_context,
        };
        use rustok_outbox::TransactionalEventBus;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?
            .0;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "forum/storefront-data requires TransactionalEventBus in host runtime context",
                )
            })?;
        let effective_locale = normalize_locale(
            locale
                .as_deref()
                .or(Some(request.locale.as_str()))
                .or(Some(tenant.default_locale.as_str())),
        );
        let db = runtime_ctx.db_clone();
        let audience_facts = runtime_ctx.shared_get::<SharedForumAudienceFactsPort>();
        let category_audience_service = match audience_facts.clone() {
            Some(facts) => ForumCategoryAudienceReadService::with_audience_facts(db.clone(), facts),
            None => ForumCategoryAudienceReadService::new(db.clone()),
        };
        let topic_audience_service = match audience_facts.clone() {
            Some(facts) => ForumTopicAudienceReadService::with_audience_facts(
                db.clone(),
                event_bus.clone(),
                facts,
            ),
            None => ForumTopicAudienceReadService::new(db.clone(), event_bus.clone()),
        };
        let topic_audience_list_service = match audience_facts.clone() {
            Some(facts) => ForumTopicAudienceListService::with_audience_facts(
                db.clone(),
                event_bus.clone(),
                facts,
            ),
            None => ForumTopicAudienceListService::new(db.clone(), event_bus.clone()),
        };
        let reply_audience_service = match audience_facts.clone() {
            Some(facts) => ForumReplyAudienceReadService::with_audience_facts(
                db.clone(),
                event_bus.clone(),
                facts,
            ),
            None => ForumReplyAudienceReadService::new(db.clone(), event_bus.clone()),
        };
        let read_state_service = match audience_facts {
            Some(facts) => ForumStorefrontReadStateService::with_audience_facts(
                db.clone(),
                event_bus.clone(),
                facts,
            ),
            None => ForumStorefrontReadStateService::new(db.clone(), event_bus.clone()),
        };
        let channel_slug = request.channel_slug.as_deref();

        let category_page = if let Some(auth) = auth.as_ref().filter(|auth| {
            has_any_effective_permission(&auth.permissions, &[Permission::FORUM_CATEGORIES_LIST])
        }) {
            let security =
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
            let audience_context = category_read_audience_port_context(
                ForumCategoryReadTransport::NativeServer,
                ForumCategoryReadOperation::CategoryList,
                tenant.id,
                auth,
                Some(&request),
                effective_locale.as_str(),
            )
            .map_err(server_error)?;
            category_audience_service
                .list_authenticated_storefront_visible_with_audience_context(
                    tenant.id,
                    security,
                    audience_context,
                    1,
                    12,
                    Some(tenant.default_locale.as_str()),
                )
                .await
                .map_err(server_error)?
        } else {
            category_audience_service
                .list_public_storefront_visible_with_locale_fallback(
                    tenant.id,
                    effective_locale.as_str(),
                    1,
                    12,
                    Some(tenant.default_locale.as_str()),
                )
                .await
                .map_err(server_error)?
        };
        let categories = category_page.items;
        let categories_total = category_page.total;

        let requested_topic_id = parse_optional_uuid(selected_topic_id.as_deref(), "topic_id")?;
        let mut selected_topic = match requested_topic_id {
            Some(topic_id) => {
                load_audience_visible_topic(
                    &topic_audience_service,
                    tenant.id,
                    auth.as_ref(),
                    &request,
                    topic_id,
                    effective_locale.as_str(),
                    tenant.default_locale.as_str(),
                )
                .await?
            }
            None => None,
        };

        let requested_category_id =
            parse_optional_uuid(selected_category_id.as_deref(), "category_id")?;
        let category_candidate_id = requested_category_id
            .or_else(|| selected_topic.as_ref().map(|topic| topic.category_id));
        let exact_selected_category = match category_candidate_id {
            Some(category_id) => {
                load_audience_visible_category(
                    &category_audience_service,
                    tenant.id,
                    auth.as_ref(),
                    &request,
                    category_id,
                    effective_locale.as_str(),
                    tenant.default_locale.as_str(),
                )
                .await?
            }
            None => None,
        };
        let resolved_category_id = exact_selected_category
            .as_ref()
            .map(|category| category.id)
            .or_else(|| categories.first().map(|category| category.id));
        let topic_filter = ListTopicsFilter {
            category_id: resolved_category_id,
            status: None,
            locale: Some(effective_locale.clone()),
            page: 1,
            per_page: 20,
        };

        let (topic_items, topics_total, first_topic_id, read_state_available) = if let Some(auth) =
            auth.as_ref().filter(|auth| {
                has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_LIST])
            }) {
            let security =
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
            let audience_context = topic_read_audience_port_context(
                ForumTopicReadTransport::NativeServer,
                ForumTopicReadOperation::TopicList,
                tenant.id,
                auth,
                Some(&request),
                effective_locale.as_str(),
            )
            .map_err(server_error)?;
            let page = read_state_service
                .list_topics_with_unread_audience_visible(
                    tenant.id,
                    security,
                    audience_context,
                    topic_filter,
                    Some(tenant.default_locale.as_str()),
                )
                .await
                .map_err(server_error)?;
            let first_topic_id = page.items.first().map(|item| item.topic.id);
            (
                page.items.into_iter().map(map_unread_topic).collect(),
                page.total,
                first_topic_id,
                true,
            )
        } else {
            let page = topic_audience_list_service
                .list_public_storefront_visible_with_locale_fallback(
                    tenant.id,
                    topic_filter,
                    Some(tenant.default_locale.as_str()),
                    channel_slug,
                )
                .await
                .map_err(server_error)?;
            let first_topic_id = page.items.first().map(|topic| topic.id);
            (
                page.items.into_iter().map(map_topic_list_item).collect(),
                page.total,
                first_topic_id,
                false,
            )
        };

        let resolved_topic_id = requested_topic_id.or(first_topic_id);
        if selected_topic.is_none() {
            if let Some(topic_id) = resolved_topic_id {
                selected_topic = load_audience_visible_topic(
                    &topic_audience_service,
                    tenant.id,
                    auth.as_ref(),
                    &request,
                    topic_id,
                    effective_locale.as_str(),
                    tenant.default_locale.as_str(),
                )
                .await?;
            }
        }

        let replies = if let Some(topic_id) = resolved_topic_id {
            if selected_topic.is_some() {
                let approved_statuses = [ReplyStatus::Approved];
                let (items, total) = if let Some(auth) = auth.as_ref().filter(|auth| {
                    has_any_effective_permission(
                        &auth.permissions,
                        &[Permission::FORUM_REPLIES_LIST],
                    )
                }) {
                    let security = SecurityContext::from_permission_snapshot(
                        Some(auth.user_id),
                        &auth.permissions,
                    );
                    let audience_context = reply_read_audience_port_context(
                        ForumReplyReadTransport::NativeServer,
                        ForumReplyReadOperation::ReplyList,
                        tenant.id,
                        auth,
                        Some(&request),
                        effective_locale.as_str(),
                    )
                    .map_err(server_error)?;
                    reply_audience_service
                        .list_authenticated_storefront_visible_with_audience_context(
                            tenant.id,
                            security,
                            audience_context,
                            topic_id,
                            ListRepliesFilter {
                                locale: Some(effective_locale.clone()),
                                page: 1,
                                per_page: 20,
                            },
                            Some(tenant.default_locale.as_str()),
                            Some(&approved_statuses),
                        )
                        .await
                        .map_err(server_error)?
                } else {
                    reply_audience_service
                        .list_public_storefront_visible_with_locale_fallback(
                            tenant.id,
                            topic_id,
                            ListRepliesFilter {
                                locale: Some(effective_locale.clone()),
                                page: 1,
                                per_page: 20,
                            },
                            Some(tenant.default_locale.as_str()),
                            channel_slug,
                            Some(&approved_statuses),
                        )
                        .await
                        .map_err(server_error)?
                };
                ForumReplyConnection {
                    items: items.into_iter().map(map_reply).collect(),
                    total,
                }
            } else {
                empty_replies()
            }
        } else {
            empty_replies()
        };
        let selected_topic = selected_topic.map(map_topic_detail);
        let may_read_member_cards = auth.as_ref().is_some_and(|auth| {
            has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_READ])
        });
        let member_card_user_ids = if may_read_member_cards {
            storefront_author_ids(&topic_items, selected_topic.as_ref(), &replies)?
        } else {
            Vec::new()
        };
        let member_cards = if member_card_user_ids.is_empty() {
            Vec::new()
        } else {
            ForumMemberCardService::new(db.clone())
                .read_for_audience(
                    tenant.id,
                    storefront_member_card_audience(auth.as_ref()),
                    &member_card_user_ids,
                    Some(effective_locale.as_str()),
                    Some(tenant.default_locale.as_str()),
                )
                .await
                .map_err(server_error)?
                .into_iter()
                .map(map_member_card)
                .collect()
        };

        Ok(StorefrontForumData {
            categories: ForumCategoryConnection {
                items: categories.into_iter().map(map_category).collect(),
                total: categories_total,
            },
            topics: ForumTopicConnection {
                items: topic_items,
                total: topics_total,
            },
            selected_category_id: resolved_category_id.map(|id| id.to_string()),
            selected_topic_id: resolved_topic_id.map(|id| id.to_string()),
            selected_topic,
            replies,
            member_cards,
            read_state_available,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (selected_category_id, selected_topic_id, locale);
        Err(ServerFnError::new(
            "forum/storefront-data requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "forum/storefront-topic-read")]
async fn storefront_topic_mark_read_native(
    topic_id: String,
    locale: Option<String>,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{HostRuntimeContext, OptionalAuthContext, RequestContext, TenantContext};
        use rustok_core::SecurityContext;
        use rustok_forum::{
            ForumError, ForumStorefrontReadStateService, ForumTopicReadOperation,
            ForumTopicReadTransport, SharedForumAudienceFactsPort,
            topic_read_audience_port_context,
        };
        use rustok_outbox::TransactionalEventBus;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?
            .0
            .ok_or_else(|| ServerFnError::new("Authentication required"))?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "forum/storefront-topic-read requires TransactionalEventBus in host runtime context",
                )
            })?;
        let topic_id = uuid::Uuid::parse_str(topic_id.trim())
            .map_err(|_| ServerFnError::new("topic_id must be a valid UUID"))?;
        let effective_locale = normalize_locale(
            locale
                .as_deref()
                .or(Some(request.locale.as_str()))
                .or(Some(tenant.default_locale.as_str())),
        );
        let security =
            SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
        let audience_context = topic_read_audience_port_context(
            ForumTopicReadTransport::NativeServer,
            ForumTopicReadOperation::MarkRead,
            tenant.id,
            &auth,
            Some(&request),
            effective_locale.as_str(),
        )
        .map_err(server_error)?;
        let db = runtime_ctx.db_clone();
        let service = match runtime_ctx.shared_get::<SharedForumAudienceFactsPort>() {
            Some(facts) => {
                ForumStorefrontReadStateService::with_audience_facts(db, event_bus, facts)
            }
            None => ForumStorefrontReadStateService::new(db, event_bus),
        };
        match service
            .mark_topic_read_current_audience_visible(
                tenant.id,
                topic_id,
                security,
                audience_context,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(ForumError::TopicNotFound(_)) => {
                Err(ServerFnError::new("Forum topic is unavailable"))
            }
            Err(error) => Err(server_error(error)),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (topic_id, locale);
        Err(ServerFnError::new(
            "forum/storefront-topic-read requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn normalize_locale(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(rustok_api::PLATFORM_FALLBACK_LOCALE)
        .to_string()
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(
    value: Option<&str>,
    field: &str,
) -> Result<Option<uuid::Uuid>, ServerFnError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            uuid::Uuid::parse_str(value)
                .map_err(|_| ServerFnError::new(format!("{field} must be a valid UUID")))
        })
        .transpose()
}

#[cfg(feature = "ssr")]
async fn load_audience_visible_category(
    service: &rustok_forum::ForumCategoryAudienceReadService,
    tenant_id: uuid::Uuid,
    auth: Option<&rustok_api::AuthContext>,
    request: &rustok_api::RequestContext,
    category_id: uuid::Uuid,
    locale: &str,
    fallback_locale: &str,
) -> Result<Option<rustok_forum::CategoryResponse>, ServerFnError> {
    if let Some(auth) = auth.filter(|auth| {
        rustok_api::has_any_effective_permission(
            &auth.permissions,
            &[rustok_api::Permission::FORUM_CATEGORIES_LIST],
        )
    }) {
        let security = rustok_core::SecurityContext::from_permission_snapshot(
            Some(auth.user_id),
            &auth.permissions,
        );
        let context = rustok_forum::category_read_audience_port_context(
            rustok_forum::ForumCategoryReadTransport::NativeServer,
            rustok_forum::ForumCategoryReadOperation::SelectedCategory,
            tenant_id,
            auth,
            Some(request),
            locale,
        )
        .map_err(server_error)?;
        match service
            .get_authenticated_storefront_list_visible_with_audience_context(
                tenant_id,
                security,
                context,
                category_id,
                Some(fallback_locale),
            )
            .await
        {
            Ok(category) => Ok(Some(category)),
            Err(rustok_forum::ForumError::CategoryNotFound(_)) => Ok(None),
            Err(error) => Err(server_error(error)),
        }
    } else {
        match service
            .get_public_storefront_visible_with_locale_fallback(
                tenant_id,
                category_id,
                locale,
                Some(fallback_locale),
            )
            .await
        {
            Ok(category) => Ok(Some(category)),
            Err(rustok_forum::ForumError::CategoryNotFound(_)) => Ok(None),
            Err(error) => Err(server_error(error)),
        }
    }
}

#[cfg(feature = "ssr")]
async fn load_audience_visible_topic(
    service: &rustok_forum::ForumTopicAudienceReadService,
    tenant_id: uuid::Uuid,
    auth: Option<&rustok_api::AuthContext>,
    request: &rustok_api::RequestContext,
    topic_id: uuid::Uuid,
    locale: &str,
    fallback_locale: &str,
) -> Result<Option<rustok_forum::TopicResponse>, ServerFnError> {
    if let Some(auth) = auth {
        let security = rustok_core::SecurityContext::from_permission_snapshot(
            Some(auth.user_id),
            &auth.permissions,
        );
        let context = rustok_forum::topic_read_audience_port_context(
            rustok_forum::ForumTopicReadTransport::NativeServer,
            rustok_forum::ForumTopicReadOperation::SelectedTopic,
            tenant_id,
            auth,
            Some(request),
            locale,
        )
        .map_err(server_error)?;
        service
            .get_authenticated_storefront_visible_with_audience_context(
                tenant_id,
                security,
                context,
                topic_id,
                Some(fallback_locale),
            )
            .await
            .map_err(server_error)
    } else {
        service
            .get_public_storefront_visible_with_locale_fallback(
                tenant_id,
                topic_id,
                locale,
                Some(fallback_locale),
                request.channel_slug.as_deref(),
            )
            .await
            .map_err(server_error)
    }
}

#[cfg(feature = "ssr")]
fn storefront_member_card_audience(
    auth: Option<&rustok_api::AuthContext>,
) -> rustok_forum::services::user_stats::ForumMemberCardAudience {
    use rustok_forum::services::user_stats::ForumMemberCardAudience;

    match auth {
        None => ForumMemberCardAudience::Anonymous,
        Some(auth) if auth.is_service_principal() => {
            ForumMemberCardAudience::TrustedService { actor_id: None }
        }
        Some(auth) => ForumMemberCardAudience::Authenticated {
            actor_id: auth.user_id,
        },
    }
}

#[cfg(feature = "ssr")]
fn storefront_author_ids(
    topics: &[ForumTopicListItem],
    selected_topic: Option<&ForumTopicDetail>,
    replies: &ForumReplyConnection,
) -> Result<Vec<uuid::Uuid>, ServerFnError> {
    let mut seen = std::collections::HashSet::new();
    let mut user_ids = Vec::new();
    for author_id in topics
        .iter()
        .filter_map(|topic| topic.author_id.as_deref())
        .chain(selected_topic.and_then(|topic| topic.author_id.as_deref()))
        .chain(replies.items.iter().filter_map(|reply| reply.author_id.as_deref()))
    {
        let user_id = uuid::Uuid::parse_str(author_id)
            .map_err(|_| ServerFnError::new("Forum storefront author ID is invalid"))?;
        if seen.insert(user_id) {
            user_ids.push(user_id);
        }
    }
    Ok(user_ids)
}

#[cfg(feature = "ssr")]
fn map_member_card(
    value: rustok_forum::services::user_stats::ForumMemberCard,
) -> ForumMemberCard {
    ForumMemberCard {
        user_id: value.user_id.to_string(),
        profile: ForumMemberProfileSummary {
            user_id: value.profile.user_id.to_string(),
            handle: value.profile.handle,
            display_name: value.profile.display_name,
            tags: value.profile.tags,
            avatar_media_id: value.profile.avatar_media_id.map(|id| id.to_string()),
            preferred_locale: value.profile.preferred_locale,
        },
        forum_stats: ForumMemberStats {
            topic_count: value.forum_stats.topic_count,
            reply_count: value.forum_stats.reply_count,
            solution_count: value.forum_stats.solution_count,
        },
    }
}

#[cfg(feature = "ssr")]
fn map_category(value: rustok_forum::CategoryListItem) -> ForumCategoryListItem {
    ForumCategoryListItem {
        id: value.id.to_string(),
        effective_locale: value.effective_locale,
        name: value.name,
        slug: value.slug,
        description: value.description,
        icon: value.icon,
        color: value.color,
        topic_count: value.topic_count,
        reply_count: value.reply_count,
    }
}

#[cfg(feature = "ssr")]
fn map_topic_list_item(value: rustok_forum::TopicListItem) -> ForumTopicListItem {
    ForumTopicListItem {
        id: value.id.to_string(),
        effective_locale: value.effective_locale,
        category_id: value.category_id.to_string(),
        author_id: value.author_id.map(|id| id.to_string()),
        title: value.title,
        slug: value.slug,
        status: value.status,
        is_pinned: value.is_pinned,
        is_locked: value.is_locked,
        reply_count: value.reply_count,
        created_at: value.created_at,
        read_state_explicit: None,
        last_read_position: None,
        last_read_revision: None,
        unread_count: None,
        has_unread_topic_revision: None,
        is_unread: None,
    }
}

#[cfg(feature = "ssr")]
fn map_unread_topic(value: rustok_forum::ForumStorefrontUnreadTopic) -> ForumTopicListItem {
    ForumTopicListItem {
        id: value.topic.id.to_string(),
        effective_locale: value.topic.effective_locale,
        category_id: value.topic.category_id.to_string(),
        author_id: value.topic.author_id.map(|id| id.to_string()),
        title: value.topic.title,
        slug: value.topic.slug,
        status: value.topic.status,
        is_pinned: value.topic.is_pinned,
        is_locked: value.topic.is_locked,
        reply_count: value.topic.reply_count,
        created_at: value.topic.created_at,
        read_state_explicit: Some(value.read_state_explicit),
        last_read_position: Some(value.last_read_position),
        last_read_revision: Some(value.last_read_revision),
        unread_count: Some(value.unread_count),
        has_unread_topic_revision: Some(value.has_unread_topic_revision),
        is_unread: Some(value.is_unread),
    }
}

#[cfg(feature = "ssr")]
fn map_topic_detail(value: rustok_forum::TopicResponse) -> ForumTopicDetail {
    ForumTopicDetail {
        id: value.id.to_string(),
        effective_locale: value.effective_locale,
        available_locales: value.available_locales,
        category_id: value.category_id.to_string(),
        author_id: value.author_id.map(|id| id.to_string()),
        title: value.title,
        slug: value.slug,
        body: value.body,
        body_plain_text: value.body_plain_text,
        status: value.status,
        tags: value.tags,
        is_pinned: value.is_pinned,
        is_locked: value.is_locked,
        reply_count: value.reply_count,
        created_at: value.created_at,
        updated_at: value.updated_at,
    }
}

#[cfg(feature = "ssr")]
fn map_reply(value: rustok_forum::ReplyResponse) -> ForumReplyDetail {
    ForumReplyDetail {
        id: value.id.to_string(),
        effective_locale: value.effective_locale,
        topic_id: value.topic_id.to_string(),
        author_id: value.author_id.map(|id| id.to_string()),
        content: value.content,
        content_plain_text: value.content_plain_text,
        status: value.status,
        parent_reply_id: value.parent_reply_id.map(|id| id.to_string()),
        created_at: value.created_at,
        updated_at: value.updated_at,
    }
}

#[cfg(feature = "ssr")]
fn empty_replies() -> ForumReplyConnection {
    ForumReplyConnection {
        items: Vec::new(),
        total: 0,
    }
}

#[cfg(feature = "ssr")]
fn server_error(error: impl std::fmt::Display) -> ServerFnError {
    ServerFnError::new(error.to_string())
}
