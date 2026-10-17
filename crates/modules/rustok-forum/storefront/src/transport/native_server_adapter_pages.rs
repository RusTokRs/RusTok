//! Cursor-paged storefront reads for the native (Leptos server-fn) transport.
//!
//! Each fetch continues from the owner cursor the surface already holds, so a page is appended to
//! the list the reader is looking at instead of refetching the first page. There is deliberately no
//! cross-variant fallback: unread-aware and audience-only cursors are produced by different owner
//! services, and handing one service's cursor to the other would reorder the list the reader is
//! already looking at.

use leptos::prelude::*;

use super::graphql_adapter::ApiError;
use crate::model::{ForumReplyPage, ForumTopicPage};

#[cfg(feature = "ssr")]
use super::native_server_adapter::{
    map_reply, map_topic_list_item, map_unread_topic, normalize_locale, parse_optional_uuid,
    server_error,
};

pub async fn fetch_storefront_forum_topic_page_server(
    category_id: Option<String>,
    after: String,
    locale: Option<String>,
    read_state_available: bool,
    selection: crate::model::ForumTopicSelection,
) -> Result<ForumTopicPage, ApiError> {
    storefront_forum_topic_page_native(category_id, after, locale, read_state_available, selection)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

pub async fn fetch_storefront_forum_reply_page_server(
    topic_id: String,
    after: String,
    locale: Option<String>,
) -> Result<ForumReplyPage, ApiError> {
    storefront_forum_reply_page_native(topic_id, after, locale)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

/// One more page of the topic list the surface already holds.
///
/// `read_state_available` names the list variant the first page came from: unread-aware cursors
/// belong to the storefront read-state service and audience cursors to the public list service, so
/// a page fetch continues on the same variant instead of feeding a foreign cursor to the other
/// one. A principal that can no longer use the read-state variant gets an error, never a silent
/// fallback to a differently ordered list.
#[server(prefix = "/api/fn", endpoint = "forum/storefront-topic-page")]
async fn storefront_forum_topic_page_native(
    category_id: Option<String>,
    after: String,
    locale: Option<String>,
    read_state_available: bool,
    selection: crate::model::ForumTopicSelection,
) -> Result<ForumTopicPage, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{
            HostRuntimeContext, OptionalAuthContext, Permission, RequestContext,
            SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
            TenantContext, has_any_effective_permission,
        };
        use rustok_core::SecurityContext;
        use rustok_forum::{
            ForumSettingsProviders, ForumStorefrontReadStateService, ForumTopicAudienceListService,
            ForumTopicReadOperation, ForumTopicReadTransport, ListTopicsFilter,
            SharedForumAudienceFactsPort, topic_read_audience_port_context,
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
        if !super::native_server_adapter::forum_channel_enabled(&runtime_ctx, tenant.id, &request)
            .await?
        {
            return Err(ServerFnError::new(
                "Forum module is not enabled for this channel",
            ));
        }
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "forum/storefront-topic-page requires TransactionalEventBus in host runtime context",
                )
            })?;
        let effective_locale = normalize_locale(
            locale
                .as_deref()
                .or(Some(request.locale.as_str()))
                .or(Some(tenant.default_locale.as_str())),
        );
        let category_id = parse_optional_uuid(category_id.as_deref(), "category_id")?;
        let db = runtime_ctx.db_clone();
        let audience_facts = runtime_ctx.shared_get::<SharedForumAudienceFactsPort>();
        let settings_providers = match (
            runtime_ctx.shared_get::<SharedStaticModuleSettingsReader>(),
            runtime_ctx.shared_get::<SharedStaticModuleSettingsTransactionReader>(),
        ) {
            (Some(static_reader), Some(transactional_reader)) => ForumSettingsProviders::default()
                .with_static_readers(static_reader, transactional_reader),
            (Some(static_reader), None) => {
                ForumSettingsProviders::default().with_static_reader(static_reader)
            }
            (None, _) => ForumSettingsProviders::default(),
        };
        let per_page = settings_providers
            .default_topics_per_page(tenant.id)
            .await
            .map_err(server_error)?;
        let filter = ListTopicsFilter {
            search: None,
            order: Default::default(),
            solved_only: false,
            category_id,
            status: None,
            locale: Some(effective_locale.clone()),
            after: Some(after),
            per_page,
        };
        let filter = super::native_server_adapter::selected_topic_filter(filter, selection);

        if read_state_available {
            let Some(auth) = auth.as_ref().filter(|auth| {
                has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_LIST])
            }) else {
                return Err(ServerFnError::new(
                    "Forum read state is unavailable for this principal",
                ));
            };
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
            let service = match audience_facts {
                Some(facts) => ForumStorefrontReadStateService::with_audience_facts(
                    db.clone(),
                    event_bus.clone(),
                    facts,
                ),
                None => ForumStorefrontReadStateService::new(db.clone(), event_bus.clone()),
            }
            .with_settings_providers(settings_providers.clone());
            let page = service
                .list_topics_with_unread_audience_visible(
                    tenant.id,
                    security,
                    audience_context,
                    filter,
                    Some(tenant.default_locale.as_str()),
                    false, // The native feed shows read and unread topics with read-state badges.
                )
                .await
                .map_err(server_error)?;
            return Ok(ForumTopicPage {
                items: page.items.into_iter().map(map_unread_topic).collect(),
                next_cursor: page.next_cursor,
            });
        }

        if auth.is_some() {
            // The plain continuation is for guests only; authenticated callers
            // cannot drop their viewer context by forging `read_state_available`.
            return Err(ServerFnError::new(
                "Forum read state is unavailable for this principal",
            ));
        }
        let service = match audience_facts {
            Some(facts) => ForumTopicAudienceListService::with_audience_facts(
                db.clone(),
                event_bus.clone(),
                facts,
            ),
            None => ForumTopicAudienceListService::new(db.clone(), event_bus.clone()),
        }
        .with_settings_providers(settings_providers);
        let page = service
            .list_public_storefront_visible_with_locale_fallback(
                tenant.id,
                filter,
                Some(tenant.default_locale.as_str()),
                request.channel_slug.as_deref(),
            )
            .await
            .map_err(server_error)?;
        Ok(ForumTopicPage {
            items: page.items.into_iter().map(map_topic_list_item).collect(),
            next_cursor: page.next_cursor,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (category_id, after, locale, read_state_available, selection);
        Err(ServerFnError::new(
            "forum/storefront-topic-page requires the `ssr` feature",
        ))
    }
}

/// One more page of replies for the opened topic, on the same audience variant and approved-status
/// filter the first page used.
#[server(prefix = "/api/fn", endpoint = "forum/storefront-reply-page")]
async fn storefront_forum_reply_page_native(
    topic_id: String,
    after: String,
    locale: Option<String>,
) -> Result<ForumReplyPage, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{
            HostRuntimeContext, OptionalAuthContext, RequestContext,
            SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
            TenantContext,
        };
        use rustok_core::SecurityContext;
        use rustok_forum::{
            ForumReplyAudienceReadService, ForumReplyReadOperation, ForumReplyReadTransport,
            ForumSettingsProviders, ListRepliesFilter, ReplyStatus, SharedForumAudienceFactsPort,
            reply_read_audience_port_context,
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
        if !super::native_server_adapter::forum_channel_enabled(&runtime_ctx, tenant.id, &request)
            .await?
        {
            return Err(ServerFnError::new(
                "Forum module is not enabled for this channel",
            ));
        }
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "forum/storefront-reply-page requires TransactionalEventBus in host runtime context",
                )
            })?;
        let effective_locale = normalize_locale(
            locale
                .as_deref()
                .or(Some(request.locale.as_str()))
                .or(Some(tenant.default_locale.as_str())),
        );
        let topic_id = uuid::Uuid::parse_str(topic_id.trim())
            .map_err(|_| ServerFnError::new("topic_id must be a valid UUID"))?;
        let db = runtime_ctx.db_clone();
        let audience_facts = runtime_ctx.shared_get::<SharedForumAudienceFactsPort>();
        let settings_providers = match (
            runtime_ctx.shared_get::<SharedStaticModuleSettingsReader>(),
            runtime_ctx.shared_get::<SharedStaticModuleSettingsTransactionReader>(),
        ) {
            (Some(static_reader), Some(transactional_reader)) => ForumSettingsProviders::default()
                .with_static_readers(static_reader, transactional_reader),
            (Some(static_reader), None) => {
                ForumSettingsProviders::default().with_static_reader(static_reader)
            }
            (None, _) => ForumSettingsProviders::default(),
        };
        let per_page = settings_providers
            .default_replies_per_page(tenant.id)
            .await
            .map_err(server_error)?;
        let filter = ListRepliesFilter {
            locale: Some(effective_locale.clone()),
            after: Some(after),
            per_page,
        };
        let approved_statuses = [ReplyStatus::Approved];
        let service = match audience_facts {
            Some(facts) => ForumReplyAudienceReadService::with_audience_facts(
                db.clone(),
                event_bus.clone(),
                facts,
            ),
            None => ForumReplyAudienceReadService::new(db.clone(), event_bus.clone()),
        }
        .with_settings_providers(settings_providers);

        if let Some(auth) = auth.as_ref() {
            let security =
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
            let audience_context = reply_read_audience_port_context(
                ForumReplyReadTransport::NativeServer,
                ForumReplyReadOperation::ReplyList,
                tenant.id,
                auth,
                Some(&request),
                effective_locale.as_str(),
            )
            .map_err(server_error)?;
            let page = service
                .list_authenticated_storefront_visible_with_audience_context(
                    tenant.id,
                    security,
                    audience_context,
                    topic_id,
                    filter,
                    Some(tenant.default_locale.as_str()),
                    Some(&approved_statuses),
                )
                .await
                .map_err(server_error)?;
            return Ok(ForumReplyPage {
                items: page.items.into_iter().map(map_reply).collect(),
                next_cursor: page.next_cursor,
            });
        }

        let page = service
            .list_public_storefront_visible_with_locale_fallback(
                tenant.id,
                topic_id,
                filter,
                Some(tenant.default_locale.as_str()),
                request.channel_slug.as_deref(),
                Some(&approved_statuses),
            )
            .await
            .map_err(server_error)?;
        Ok(ForumReplyPage {
            items: page.items.into_iter().map(map_reply).collect(),
            next_cursor: page.next_cursor,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (topic_id, after, locale);
        Err(ServerFnError::new(
            "forum/storefront-reply-page requires the `ssr` feature",
        ))
    }
}
