#[allow(dead_code)]
pub async fn mark_storefront_category_read_server(
    category_id: String,
    cursor: Option<String>,
    limit: Option<u64>,
    locale: Option<String>,
) -> Result<super::StorefrontForumBulkReadResult, ApiError> {
    storefront_category_mark_read_native(category_id, cursor, limit, locale)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

#[allow(dead_code)]
pub async fn mark_all_storefront_topics_read_server(
    cursor: Option<String>,
    limit: Option<u64>,
    locale: Option<String>,
) -> Result<super::StorefrontForumBulkReadResult, ApiError> {
    storefront_all_topics_mark_read_native(cursor, limit, locale)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "forum/storefront-category-read")]
async fn storefront_category_mark_read_native(
    category_id: String,
    cursor: Option<String>,
    limit: Option<u64>,
    locale: Option<String>,
) -> Result<super::StorefrontForumBulkReadResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{HostRuntimeContext, OptionalAuthContext, RequestContext, TenantContext};
        use rustok_core::SecurityContext;
        use rustok_forum::{
            ForumError, ForumStorefrontReadStateService, ForumTopicReadOperation,
            ForumTopicReadTransport, MarkForumTopicsReadBatchInput, SharedForumAudienceFactsPort,
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
                    "forum/storefront-category-read requires TransactionalEventBus in host runtime context",
                )
            })?;
        let category_id = uuid::Uuid::parse_str(category_id.trim())
            .map_err(|_| ServerFnError::new("category_id must be a valid UUID"))?;
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
            ForumTopicReadOperation::MarkCategoryRead,
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
        let result = match service
            .mark_category_read_audience_visible(
                tenant.id,
                category_id,
                security,
                audience_context,
                MarkForumTopicsReadBatchInput { cursor, limit },
            )
            .await
        {
            Ok(result) => result,
            Err(ForumError::CategoryNotFound(_)) => {
                return Err(ServerFnError::new("Forum category is unavailable"));
            }
            Err(error) => return Err(server_error(error)),
        };
        Ok(map_storefront_bulk_read_result(result))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (category_id, cursor, limit, locale);
        Err(ServerFnError::new(
            "forum/storefront-category-read requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "forum/storefront-all-read")]
async fn storefront_all_topics_mark_read_native(
    cursor: Option<String>,
    limit: Option<u64>,
    locale: Option<String>,
) -> Result<super::StorefrontForumBulkReadResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{HostRuntimeContext, OptionalAuthContext, RequestContext, TenantContext};
        use rustok_core::SecurityContext;
        use rustok_forum::{
            ForumStorefrontReadStateService, ForumTopicReadOperation, ForumTopicReadTransport,
            MarkForumTopicsReadBatchInput, SharedForumAudienceFactsPort,
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
                    "forum/storefront-all-read requires TransactionalEventBus in host runtime context",
                )
            })?;
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
            ForumTopicReadOperation::MarkAllRead,
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
        let result = service
            .mark_all_read_audience_visible(
                tenant.id,
                security,
                audience_context,
                MarkForumTopicsReadBatchInput { cursor, limit },
            )
            .await
            .map_err(server_error)?;
        Ok(map_storefront_bulk_read_result(result))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (cursor, limit, locale);
        Err(ServerFnError::new(
            "forum/storefront-all-read requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_storefront_bulk_read_result(
    result: rustok_forum::MarkForumTopicsReadBatchResult,
) -> super::StorefrontForumBulkReadResult {
    super::StorefrontForumBulkReadResult {
        processed: result.processed,
        next_cursor: result.next_cursor,
        has_more: result.has_more,
        snapshot_at: result.snapshot_at,
    }
}
