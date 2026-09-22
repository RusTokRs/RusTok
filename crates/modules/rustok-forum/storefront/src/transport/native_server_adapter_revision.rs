pub async fn fetch_storefront_topic_current_revision_server(
    topic_id: String,
    locale: Option<String>,
) -> Result<Option<String>, ApiError> {
    storefront_topic_current_revision_native(topic_id, locale)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

pub async fn fetch_storefront_reply_current_revision_server(
    reply_id: String,
    locale: Option<String>,
) -> Result<Option<String>, ApiError> {
    storefront_reply_current_revision_native(reply_id, locale)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

#[server(
    prefix = "/api/fn",
    endpoint = "forum/storefront-topic-current-revision"
)]
async fn storefront_topic_current_revision_native(
    topic_id: String,
    locale: Option<String>,
) -> Result<Option<String>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{HostRuntimeContext, OptionalAuthContext, RequestContext, TenantContext};
        use rustok_forum::{
            ForumTopicAudienceReadService, RevisionService, SharedForumAudienceFactsPort,
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
                    "forum/storefront-topic-current-revision requires TransactionalEventBus in host runtime context",
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
        let db = runtime_ctx.db_clone();
        let audience_facts = runtime_ctx.shared_get::<SharedForumAudienceFactsPort>();
        let topic_audience_service = match audience_facts {
            Some(facts) => {
                ForumTopicAudienceReadService::with_audience_facts(db.clone(), event_bus, facts)
            }
            None => ForumTopicAudienceReadService::new(db.clone(), event_bus),
        };
        let topic = load_audience_visible_topic(
            &topic_audience_service,
            tenant.id,
            auth.as_ref(),
            &request,
            topic_id,
            effective_locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;
        let Some(topic) = topic else {
            return Ok(None);
        };
        let revision = RevisionService::new(db)
            .current_topic_revision(tenant.id, topic.id)
            .await
            .map_err(server_error)?;
        Ok(Some(revision.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (topic_id, locale);
        Err(ServerFnError::new(
            "forum/storefront-topic-current-revision requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "forum/storefront-reply-current-revision"
)]
async fn storefront_reply_current_revision_native(
    reply_id: String,
    locale: Option<String>,
) -> Result<Option<String>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{HostRuntimeContext, OptionalAuthContext, RequestContext, TenantContext};
        use rustok_core::SecurityContext;
        use rustok_forum::{
            ForumReplyAudienceReadService, ForumReplyReadOperation, ForumReplyReadTransport,
            ReplyStatus, RevisionService, SharedForumAudienceFactsPort,
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
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "forum/storefront-reply-current-revision requires TransactionalEventBus in host runtime context",
                )
            })?;
        let reply_id = uuid::Uuid::parse_str(reply_id.trim())
            .map_err(|_| ServerFnError::new("reply_id must be a valid UUID"))?;
        let effective_locale = normalize_locale(
            locale
                .as_deref()
                .or(Some(request.locale.as_str()))
                .or(Some(tenant.default_locale.as_str())),
        );
        let db = runtime_ctx.db_clone();
        let audience_facts = runtime_ctx.shared_get::<SharedForumAudienceFactsPort>();
        let reply_audience_service = match audience_facts {
            Some(facts) => {
                ForumReplyAudienceReadService::with_audience_facts(db.clone(), event_bus, facts)
            }
            None => ForumReplyAudienceReadService::new(db.clone(), event_bus),
        };
        let approved_statuses = [ReplyStatus::Approved];
        let reply = if let Some(auth) = auth.as_ref() {
            let security =
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
            let context = reply_read_audience_port_context(
                ForumReplyReadTransport::NativeServer,
                ForumReplyReadOperation::SelectedReply,
                tenant.id,
                auth,
                Some(&request),
                effective_locale.as_str(),
            )
            .map_err(server_error)?;
            reply_audience_service
                .get_authenticated_storefront_visible_with_audience_context(
                    tenant.id,
                    security,
                    context,
                    reply_id,
                    Some(tenant.default_locale.as_str()),
                    Some(&approved_statuses),
                )
                .await
                .map_err(server_error)?
        } else {
            reply_audience_service
                .get_public_storefront_visible_with_locale_fallback(
                    tenant.id,
                    reply_id,
                    effective_locale.as_str(),
                    Some(tenant.default_locale.as_str()),
                    request.channel_slug.as_deref(),
                    Some(&approved_statuses),
                )
                .await
                .map_err(server_error)?
        };
        let Some(reply) = reply else {
            return Ok(None);
        };
        let revision = RevisionService::new(db)
            .current_reply_revision(tenant.id, reply.id)
            .await
            .map_err(server_error)?;
        Ok(Some(revision.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (reply_id, locale);
        Err(ServerFnError::new(
            "forum/storefront-reply-current-revision requires the `ssr` feature",
        ))
    }
}
