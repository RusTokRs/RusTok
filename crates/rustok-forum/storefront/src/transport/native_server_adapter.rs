use leptos::prelude::*;

use super::graphql_adapter::ApiError;
use crate::model::StorefrontForumData;

#[cfg(feature = "ssr")]
use crate::model::{
    ForumCategoryConnection, ForumCategoryListItem, ForumReplyConnection, ForumReplyDetail,
    ForumTopicConnection, ForumTopicDetail, ForumTopicListItem,
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

#[server(prefix = "/api/fn", endpoint = "forum/storefront-data")]
async fn storefront_forum_native(
    selected_category_id: Option<String>,
    selected_topic_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontForumData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{HostRuntimeContext, RequestContext, TenantContext};
        use rustok_core::SecurityContext;
        use rustok_forum::{
            CategoryService, ListRepliesFilter, ListTopicsFilter, ReplyService, ReplyStatus,
            TopicService,
        };
        use rustok_outbox::TransactionalEventBus;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let tenant = expect_context::<TenantContext>();
        let request = expect_context::<RequestContext>();
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "forum/storefront-data requires TransactionalEventBus in host runtime context",
                )
            })?;
        let security = SecurityContext::public_read();
        let effective_locale = normalize_locale(
            locale
                .as_deref()
                .or(Some(request.locale.as_str()))
                .or(Some(tenant.default_locale.as_str())),
        );
        let db = runtime_ctx.db_clone();
        let category_service = CategoryService::new(db.clone());
        let topic_service = TopicService::new(db.clone(), event_bus.clone());
        let reply_service = ReplyService::new(db, event_bus);
        let channel_slug = request.channel_slug.as_deref();

        let (categories, categories_total) = category_service
            .list_paginated_with_locale_fallback(
                tenant.id,
                security.clone(),
                effective_locale.as_str(),
                1,
                12,
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(server_error)?;

        let requested_topic_id = parse_optional_uuid(selected_topic_id.as_deref(), "topic_id")?;
        let mut selected_topic = match requested_topic_id {
            Some(topic_id) => {
                load_visible_topic(
                    &topic_service,
                    tenant.id,
                    security.clone(),
                    topic_id,
                    effective_locale.as_str(),
                    tenant.default_locale.as_str(),
                    channel_slug,
                )
                .await?
            }
            None => None,
        };

        let resolved_category_id =
            parse_optional_uuid(selected_category_id.as_deref(), "category_id")?
                .or_else(|| selected_topic.as_ref().map(|topic| topic.category_id))
                .or_else(|| categories.first().map(|category| category.id));

        let (topics, topics_total) = topic_service
            .list_storefront_visible_with_locale_fallback(
                tenant.id,
                security.clone(),
                ListTopicsFilter {
                    category_id: resolved_category_id,
                    status: None,
                    locale: Some(effective_locale.clone()),
                    page: 1,
                    per_page: 20,
                },
                Some(tenant.default_locale.as_str()),
                channel_slug,
            )
            .await
            .map_err(server_error)?;

        let resolved_topic_id = requested_topic_id.or_else(|| topics.first().map(|topic| topic.id));

        if selected_topic.is_none() {
            if let Some(topic_id) = resolved_topic_id {
                selected_topic = load_visible_topic(
                    &topic_service,
                    tenant.id,
                    security.clone(),
                    topic_id,
                    effective_locale.as_str(),
                    tenant.default_locale.as_str(),
                    channel_slug,
                )
                .await?;
            }
        }

        let replies = if let Some(topic_id) = resolved_topic_id {
            let topic_visible = selected_topic.as_ref().is_some_and(|topic| {
                is_storefront_topic_visible(
                    topic.status.as_str(),
                    &topic.channel_slugs,
                    channel_slug,
                )
            });
            if topic_visible {
                let (items, total) = reply_service
                    .list_response_for_topic_by_statuses_with_locale_fallback(
                        tenant.id,
                        security,
                        topic_id,
                        ListRepliesFilter {
                            locale: Some(effective_locale),
                            page: 1,
                            per_page: 20,
                        },
                        Some(tenant.default_locale.as_str()),
                        Some(&[ReplyStatus::Approved]),
                    )
                    .await
                    .map_err(server_error)?;
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

        Ok(StorefrontForumData {
            categories: ForumCategoryConnection {
                items: categories.into_iter().map(map_category).collect(),
                total: categories_total,
            },
            topics: ForumTopicConnection {
                items: topics.into_iter().map(map_topic_list_item).collect(),
                total: topics_total,
            },
            selected_category_id: resolved_category_id.map(|id| id.to_string()),
            selected_topic_id: resolved_topic_id.map(|id| id.to_string()),
            selected_topic: selected_topic.map(map_topic_detail),
            replies,
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
async fn load_visible_topic(
    service: &rustok_forum::TopicService,
    tenant_id: uuid::Uuid,
    security: rustok_core::SecurityContext,
    topic_id: uuid::Uuid,
    locale: &str,
    fallback_locale: &str,
    channel_slug: Option<&str>,
) -> Result<Option<rustok_forum::TopicResponse>, ServerFnError> {
    match service
        .get_with_locale_fallback(tenant_id, security, topic_id, locale, Some(fallback_locale))
        .await
    {
        Ok(topic)
            if is_storefront_topic_visible(&topic.status, &topic.channel_slugs, channel_slug) =>
        {
            Ok(Some(topic))
        }
        Ok(_) => Ok(None),
        Err(rustok_forum::ForumError::TopicNotFound(_)) => Ok(None),
        Err(error) => Err(server_error(error)),
    }
}

#[cfg(feature = "ssr")]
fn is_storefront_topic_visible(
    status: &str,
    channel_slugs: &[String],
    channel_slug: Option<&str>,
) -> bool {
    status == rustok_forum::topic_status::OPEN
        && is_topic_visible_for_channel(channel_slugs, channel_slug)
}

#[cfg(feature = "ssr")]
fn is_topic_visible_for_channel(channel_slugs: &[String], channel_slug: Option<&str>) -> bool {
    channel_slugs.is_empty()
        || channel_slug.is_some_and(|slug| channel_slugs.iter().any(|value| value == slug))
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
        title: value.title,
        slug: value.slug,
        status: value.status,
        is_pinned: value.is_pinned,
        is_locked: value.is_locked,
        reply_count: value.reply_count,
        created_at: value.created_at,
    }
}

#[cfg(feature = "ssr")]
fn map_topic_detail(value: rustok_forum::TopicResponse) -> ForumTopicDetail {
    ForumTopicDetail {
        id: value.id.to_string(),
        effective_locale: value.effective_locale,
        available_locales: value.available_locales,
        category_id: value.category_id.to_string(),
        title: value.title,
        slug: value.slug,
        body: value.body,
        body_format: value.body_format,
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
        content: value.content,
        content_format: value.content_format,
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
