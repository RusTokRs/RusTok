use crate::model::StorefrontForumTopicRouteResolution;
#[cfg(feature = "ssr")]
use crate::model::{StorefrontForumTopicRouteDescriptor, StorefrontForumTopicRouteDisposition};

pub async fn resolve_storefront_topic_route_server(
    locale: String,
    short_id: String,
    slug: String,
) -> Result<Option<StorefrontForumTopicRouteResolution>, ApiError> {
    storefront_topic_route_native(locale, short_id, slug)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "forum/storefront-topic-route")]
async fn storefront_topic_route_native(
    locale: String,
    short_id: String,
    slug: String,
) -> Result<Option<StorefrontForumTopicRouteResolution>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{HostRuntimeContext, OptionalAuthContext, RequestContext, TenantContext};
        use rustok_core::SecurityContext;
        use rustok_forum::services::ForumTopicRouteTombstoneVisibilityService;
        use rustok_forum::{
            ForumError, ForumTopicAudienceReadService, ForumTopicReadOperation,
            ForumTopicReadTransport, ForumTopicRouteDisposition, ForumTopicRouteService,
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
        let db = runtime_ctx.db_clone();
        let authenticated = auth.is_some();

        let anonymous_channel_enabled = if authenticated {
            true
        } else if let Some(channel_id) = request.channel_id {
            rustok_channel::ChannelService::new(db.clone())
                .is_module_enabled(channel_id, "forum")
                .await
                .map_err(server_error)?
        } else {
            true
        };
        if !anonymous_channel_enabled {
            return Ok(None);
        }

        let resolution = match ForumTopicRouteService::new(db.clone())
            .resolve(tenant.id, &locale, &short_id, &slug)
            .await
        {
            Ok(resolution) => resolution,
            Err(ForumError::TopicNotFound(_))
            | Err(ForumError::TopicDeleted)
            | Err(ForumError::TopicRouteNotFound) => return Ok(None),
            Err(error) => return Err(server_error(error)),
        };

        if resolution.disposition == ForumTopicRouteDisposition::Gone {
            let gone_channel_enabled = if authenticated {
                if let Some(channel_id) = request.channel_id {
                    rustok_channel::ChannelService::new(db.clone())
                        .is_module_enabled(channel_id, "forum")
                        .await
                        .map_err(server_error)?
                } else {
                    true
                }
            } else {
                anonymous_channel_enabled
            };
            if !gone_channel_enabled {
                return Ok(None);
            }
            let topic_id = resolution.requested_topic_id.ok_or_else(|| {
                ServerFnError::new(
                    "Forum gone route resolution did not provide its historical topic identity",
                )
            })?;
            let disclose = ForumTopicRouteTombstoneVisibilityService::new(db.clone())
                .can_disclose_public_gone(tenant.id, topic_id, request.channel_slug.as_deref())
                .await
                .map_err(server_error)?;
            if !disclose {
                return Ok(None);
            }
            return map_native_topic_route_resolution(resolution);
        }

        let canonical = resolution.canonical.as_ref().ok_or_else(|| {
            ServerFnError::new("Forum topic route resolution did not provide a canonical target")
        })?;
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "forum/storefront-topic-route requires TransactionalEventBus in host runtime context",
                )
            })?;
        let service = match runtime_ctx.shared_get::<SharedForumAudienceFactsPort>() {
            Some(facts) => ForumTopicAudienceReadService::with_audience_facts(db, event_bus, facts),
            None => ForumTopicAudienceReadService::new(db, event_bus),
        };
        let visible = if let Some(auth) = auth.as_ref() {
            let security =
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
            let context = topic_read_audience_port_context(
                ForumTopicReadTransport::NativeServer,
                ForumTopicReadOperation::SelectedTopic,
                tenant.id,
                auth,
                Some(&request),
                canonical.locale.as_str(),
            )
            .map_err(server_error)?;
            service
                .get_authenticated_storefront_visible_with_audience_context(
                    tenant.id,
                    security,
                    context,
                    canonical.topic_id,
                    Some(tenant.default_locale.as_str()),
                )
                .await
                .map_err(server_error)?
        } else {
            service
                .get_public_storefront_visible_with_locale_fallback(
                    tenant.id,
                    canonical.topic_id,
                    canonical.locale.as_str(),
                    Some(tenant.default_locale.as_str()),
                    request.channel_slug.as_deref(),
                )
                .await
                .map_err(server_error)?
        };
        if visible.is_none() {
            return Ok(None);
        }

        map_native_topic_route_resolution(resolution)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (locale, short_id, slug);
        Err(ServerFnError::new(
            "forum/storefront-topic-route requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_native_topic_route_resolution(
    resolution: rustok_forum::ForumTopicRouteResolution,
) -> Result<Option<StorefrontForumTopicRouteResolution>, ServerFnError> {
    let (disposition, canonical) = match resolution.disposition {
        rustok_forum::ForumTopicRouteDisposition::Canonical => (
            StorefrontForumTopicRouteDisposition::Canonical,
            Some(map_native_canonical_descriptor(
                resolution.canonical.ok_or_else(|| {
                    ServerFnError::new(
                        "Forum topic route resolution did not provide a canonical target",
                    )
                })?,
            )),
        ),
        rustok_forum::ForumTopicRouteDisposition::Redirect => (
            StorefrontForumTopicRouteDisposition::Redirect,
            Some(map_native_canonical_descriptor(
                resolution.canonical.ok_or_else(|| {
                    ServerFnError::new(
                        "Forum topic route resolution did not provide a canonical target",
                    )
                })?,
            )),
        ),
        rustok_forum::ForumTopicRouteDisposition::Gone => {
            if resolution.canonical.is_some() {
                return Err(ServerFnError::new(
                    "Forum gone route resolution unexpectedly provided a canonical target",
                ));
            }
            (StorefrontForumTopicRouteDisposition::Gone, None)
        }
    };

    Ok(Some(StorefrontForumTopicRouteResolution {
        requested_locale: resolution.requested_locale,
        requested_short_id: resolution.requested_short_id,
        requested_slug: resolution.requested_slug,
        disposition,
        canonical,
    }))
}

#[cfg(feature = "ssr")]
fn map_native_canonical_descriptor(
    canonical: rustok_forum::ForumTopicRouteDescriptor,
) -> StorefrontForumTopicRouteDescriptor {
    StorefrontForumTopicRouteDescriptor {
        topic_id: canonical.topic_id.to_string(),
        locale: canonical.locale,
        short_id: canonical.short_id,
        slug: canonical.slug,
        path: canonical.path,
    }
}
