use leptos::prelude::*;

use super::graphql_adapter::ApiError;
use crate::model::ForumReplyConnection;

#[cfg(feature = "ssr")]
use crate::model::ForumReplyDetail;

pub async fn fetch_storefront_replies_server(
    topic_id: Option<String>,
    locale: Option<String>,
) -> Result<ForumReplyConnection, ApiError> {
    storefront_audience_replies_native(topic_id, locale)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "forum/storefront-audience-replies")]
async fn storefront_audience_replies_native(
    topic_id: Option<String>,
    locale: Option<String>,
) -> Result<ForumReplyConnection, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{
            HostRuntimeContext, OptionalAuthContext, Permission, RequestContext, TenantContext,
            has_any_effective_permission,
        };
        use rustok_core::SecurityContext;
        use rustok_forum::{
            ForumReplyAudienceReadService, ForumReplyReadOperation, ForumReplyReadTransport,
            ListRepliesFilter, ReplyStatus, SharedForumAudienceFactsPort,
            reply_read_audience_port_context,
        };
        use rustok_outbox::TransactionalEventBus;

        let Some(topic_id) = topic_id else {
            return Ok(empty_replies());
        };
        let topic_id = uuid::Uuid::parse_str(topic_id.trim())
            .map_err(|_| ServerFnError::new("topic_id must be a valid UUID"))?;
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
                    "forum/storefront-audience-replies requires TransactionalEventBus in host runtime context",
                )
            })?;
        let effective_locale = normalize_locale(
            locale
                .as_deref()
                .or(Some(request.locale.as_str()))
                .or(Some(tenant.default_locale.as_str())),
        );
        let db = runtime_ctx.db_clone();
        let service = match runtime_ctx.shared_get::<SharedForumAudienceFactsPort>() {
            Some(facts) => ForumReplyAudienceReadService::with_audience_facts(
                db,
                event_bus,
                facts,
            ),
            None => ForumReplyAudienceReadService::new(db, event_bus),
        };
        let filter = ListRepliesFilter {
            locale: Some(effective_locale.clone()),
            page: 1,
            per_page: 20,
        };
        let statuses = [ReplyStatus::Approved];

        let (replies, total) = if let Some(auth) = auth.as_ref().filter(|auth| {
            has_any_effective_permission(&auth.permissions, &[Permission::FORUM_REPLIES_LIST])
        }) {
            let context = reply_read_audience_port_context(
                ForumReplyReadTransport::NativeServer,
                ForumReplyReadOperation::ReplyList,
                tenant.id,
                auth,
                Some(&request),
                effective_locale.as_str(),
            )
            .map_err(server_error)?;
            service
                .list_authenticated_storefront_visible_with_audience_context(
                    tenant.id,
                    SecurityContext::from_permission_snapshot(
                        Some(auth.user_id),
                        &auth.permissions,
                    ),
                    context,
                    topic_id,
                    filter,
                    Some(tenant.default_locale.as_str()),
                    Some(&statuses),
                )
                .await
                .map_err(server_error)?
        } else {
            service
                .list_public_storefront_visible_with_locale_fallback(
                    tenant.id,
                    topic_id,
                    filter,
                    Some(tenant.default_locale.as_str()),
                    request.channel_slug.as_deref(),
                    Some(&statuses),
                )
                .await
                .map_err(server_error)?
        };

        Ok(ForumReplyConnection {
            items: replies.into_iter().map(map_reply).collect(),
            total,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (topic_id, locale);
        Err(ServerFnError::new(
            "forum/storefront-audience-replies requires the `ssr` feature",
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
