use async_graphql::{Context, Enum, ErrorExtensions, Object, Result, SimpleObject};
use rustok_api::graphql::require_module_enabled;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_channel::ChannelService;
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::services::ForumTopicRouteTombstoneVisibilityService;
use crate::{
    ForumError, ForumTopicReadOperation, ForumTopicReadTransport, ForumTopicRouteDisposition,
    ForumTopicRouteResolution, ForumTopicRouteService, topic_read_audience_port_context,
};

use super::{ForumGraphqlRuntimeData, GqlForumTopicRouteDescriptor};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumTopicRouteQuery;

#[Object]
impl ForumTopicRouteQuery {
    /// Legacy canonical/redirect-only route query retained for existing clients.
    async fn forum_storefront_topic_route(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        locale: String,
        short_id: String,
        slug: String,
    ) -> Result<Option<GqlForumStorefrontTopicRouteResolution>> {
        let Some(resolution) =
            resolve_authorized_public_route(ctx, tenant_id, locale, short_id, slug, false).await?
        else {
            return Ok(None);
        };
        map_legacy_public_route_resolution(resolution)
    }

    /// Resolves one storefront topic route and may disclose `GONE` only through the immutable
    /// FORUM-24J public tombstone decision for the current routed channel.
    async fn forum_storefront_topic_route_decision(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        locale: String,
        short_id: String,
        slug: String,
    ) -> Result<Option<GqlForumStorefrontTopicRouteDecision>> {
        let Some(resolution) =
            resolve_authorized_public_route(ctx, tenant_id, locale, short_id, slug, true).await?
        else {
            return Ok(None);
        };
        map_public_route_decision(resolution).map(Some)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GqlForumStorefrontTopicRouteDisposition {
    Canonical,
    Redirect,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumStorefrontTopicRouteResolution {
    pub requested_locale: String,
    pub requested_short_id: String,
    pub requested_slug: String,
    pub disposition: GqlForumStorefrontTopicRouteDisposition,
    pub canonical: GqlForumTopicRouteDescriptor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GqlForumStorefrontTopicRouteDecisionDisposition {
    Canonical,
    Redirect,
    Gone,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumStorefrontTopicRouteDecision {
    pub requested_locale: String,
    pub requested_short_id: String,
    pub requested_slug: String,
    pub disposition: GqlForumStorefrontTopicRouteDecisionDisposition,
    pub canonical: Option<GqlForumTopicRouteDescriptor>,
}

async fn resolve_authorized_public_route(
    ctx: &Context<'_>,
    requested_tenant_id: Option<Uuid>,
    locale: String,
    short_id: String,
    slug: String,
    disclose_gone: bool,
) -> Result<Option<ForumTopicRouteResolution>> {
    require_module_enabled(ctx, MODULE_SLUG).await?;

    let db = ctx.data::<DatabaseConnection>()?;
    let tenant = ctx.data::<TenantContext>()?;
    let tenant_id = super::resolve_tenant_scope(tenant, requested_tenant_id)?;
    let authenticated = ctx.data_opt::<AuthContext>().is_some();
    let anonymous_channel_enabled = if authenticated {
        true
    } else {
        forum_channel_enabled(ctx).await?
    };
    if !anonymous_channel_enabled {
        return Err(
            async_graphql::Error::new("Forum module is not enabled for this channel")
                .extend_with(|_, extension| extension.set("code", "FORBIDDEN")),
        );
    }

    let resolution = match ForumTopicRouteService::new(db.clone())
        .resolve(tenant_id, &locale, &short_id, &slug)
        .await
    {
        Ok(resolution) => resolution,
        Err(ForumError::TopicNotFound(_))
        | Err(ForumError::TopicDeleted)
        | Err(ForumError::TopicRouteNotFound) => return Ok(None),
        Err(error) => return Err(async_graphql::Error::new(error.to_string())),
    };

    if resolution.disposition == ForumTopicRouteDisposition::Gone {
        if !disclose_gone {
            return Ok(None);
        }
        let gone_channel_enabled = if authenticated {
            forum_channel_enabled(ctx).await?
        } else {
            anonymous_channel_enabled
        };
        if !gone_channel_enabled {
            return Ok(None);
        }
        let topic_id = resolution.requested_topic_id.ok_or_else(|| {
            internal_error(
                "Forum gone route resolution did not provide its historical topic identity",
            )
        })?;
        let disclose = ForumTopicRouteTombstoneVisibilityService::new(db.clone())
            .can_disclose_public_gone(tenant_id, topic_id, public_channel_slug(ctx).as_deref())
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        return Ok(disclose.then_some(resolution));
    }

    let canonical = resolution.canonical.as_ref().ok_or_else(|| {
        internal_error("Forum topic route resolution did not provide a canonical target")
    })?;
    let event_bus = ctx.data::<TransactionalEventBus>()?;
    let runtime = ctx
        .data_opt::<ForumGraphqlRuntimeData>()
        .cloned()
        .unwrap_or_default();
    let service = runtime.topic_audience_read_service(db.clone(), event_bus.clone());
    let visible = if let Some(auth) = ctx.data_opt::<AuthContext>() {
        let security =
            SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
        let audience_context = topic_read_audience_port_context(
            ForumTopicReadTransport::Graphql,
            ForumTopicReadOperation::SelectedTopic,
            tenant_id,
            auth,
            ctx.data_opt::<RequestContext>(),
            canonical.locale.as_str(),
        )?;
        service
            .get_authenticated_storefront_visible_with_audience_context(
                tenant_id,
                security,
                audience_context,
                canonical.topic_id,
                Some(tenant.default_locale.as_str()),
            )
            .await
    } else {
        service
            .get_public_storefront_visible_with_locale_fallback(
                tenant_id,
                canonical.topic_id,
                canonical.locale.as_str(),
                Some(tenant.default_locale.as_str()),
                public_channel_slug(ctx).as_deref(),
            )
            .await
    };
    match visible {
        Ok(Some(_)) => Ok(Some(resolution)),
        Ok(None)
        | Err(ForumError::TopicNotFound(_))
        | Err(ForumError::TopicDeleted)
        | Err(ForumError::TopicRouteNotFound) => Ok(None),
        Err(error) => Err(async_graphql::Error::new(error.to_string())),
    }
}

fn map_legacy_public_route_resolution(
    resolution: ForumTopicRouteResolution,
) -> Result<Option<GqlForumStorefrontTopicRouteResolution>> {
    let disposition = match resolution.disposition {
        ForumTopicRouteDisposition::Canonical => GqlForumStorefrontTopicRouteDisposition::Canonical,
        ForumTopicRouteDisposition::Redirect => GqlForumStorefrontTopicRouteDisposition::Redirect,
        ForumTopicRouteDisposition::Gone => return Ok(None),
    };
    let canonical = resolution.canonical.ok_or_else(|| {
        internal_error("Forum topic route resolution did not provide a canonical target")
    })?;

    Ok(Some(GqlForumStorefrontTopicRouteResolution {
        requested_locale: resolution.requested_locale,
        requested_short_id: resolution.requested_short_id,
        requested_slug: resolution.requested_slug,
        disposition,
        canonical: canonical.into(),
    }))
}

fn map_public_route_decision(
    resolution: ForumTopicRouteResolution,
) -> Result<GqlForumStorefrontTopicRouteDecision> {
    let (disposition, canonical) = match resolution.disposition {
        ForumTopicRouteDisposition::Canonical => (
            GqlForumStorefrontTopicRouteDecisionDisposition::Canonical,
            Some(
                resolution
                    .canonical
                    .ok_or_else(|| {
                        internal_error(
                            "Forum topic route resolution did not provide a canonical target",
                        )
                    })?
                    .into(),
            ),
        ),
        ForumTopicRouteDisposition::Redirect => (
            GqlForumStorefrontTopicRouteDecisionDisposition::Redirect,
            Some(
                resolution
                    .canonical
                    .ok_or_else(|| {
                        internal_error(
                            "Forum topic route resolution did not provide a canonical target",
                        )
                    })?
                    .into(),
            ),
        ),
        ForumTopicRouteDisposition::Gone => {
            if resolution.canonical.is_some() {
                return Err(internal_error(
                    "Forum gone route resolution unexpectedly provided a canonical target",
                ));
            }
            (GqlForumStorefrontTopicRouteDecisionDisposition::Gone, None)
        }
    };

    Ok(GqlForumStorefrontTopicRouteDecision {
        requested_locale: resolution.requested_locale,
        requested_short_id: resolution.requested_short_id,
        requested_slug: resolution.requested_slug,
        disposition,
        canonical,
    })
}

async fn forum_channel_enabled(ctx: &Context<'_>) -> Result<bool> {
    let Some(request_context) = ctx.data_opt::<RequestContext>() else {
        return Ok(true);
    };
    let Some(channel_id) = request_context.channel_id else {
        return Ok(true);
    };
    let db = ctx.data::<DatabaseConnection>()?;
    ChannelService::new(db.clone())
        .is_module_enabled(channel_id, MODULE_SLUG)
        .await
        .map_err(|error| {
            async_graphql::Error::new(format!("Channel module check failed: {error}"))
                .extend_with(|_, extension| extension.set("code", "INTERNAL_SERVER_ERROR"))
        })
}

fn internal_error(message: &'static str) -> async_graphql::Error {
    async_graphql::Error::new(message)
        .extend_with(|_, extension| extension.set("code", "INTERNAL_SERVER_ERROR"))
}

fn public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request| request.channel_slug.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ForumTopicRouteDescriptor;

    fn descriptor() -> ForumTopicRouteDescriptor {
        ForumTopicRouteDescriptor {
            topic_id: Uuid::parse_str("00000000-0000-4000-8000-000000000001").expect("topic id"),
            locale: "en".to_string(),
            short_id: "000000000000".to_string(),
            slug: "welcome".to_string(),
            path: "/en/forum/t/000000000000/welcome".to_string(),
        }
    }

    fn resolution(
        disposition: ForumTopicRouteDisposition,
        canonical: Option<ForumTopicRouteDescriptor>,
    ) -> ForumTopicRouteResolution {
        ForumTopicRouteResolution {
            requested_locale: "en".to_string(),
            requested_short_id: "000000000000".to_string(),
            requested_slug: "old-welcome".to_string(),
            requested_topic_id: Some(
                Uuid::parse_str("00000000-0000-4000-8000-000000000001").expect("topic id"),
            ),
            disposition,
            canonical,
            alias_id: None,
        }
    }

    #[test]
    fn legacy_mapping_keeps_canonical_and_redirect_only() {
        let canonical = map_legacy_public_route_resolution(resolution(
            ForumTopicRouteDisposition::Canonical,
            Some(descriptor()),
        ))
        .expect("canonical mapping")
        .expect("canonical result");
        assert_eq!(
            canonical.disposition,
            GqlForumStorefrontTopicRouteDisposition::Canonical
        );

        assert!(
            map_legacy_public_route_resolution(resolution(ForumTopicRouteDisposition::Gone, None,))
                .expect("gone mapping")
                .is_none()
        );
    }

    #[test]
    fn decision_mapping_exposes_authorized_gone_without_canonical_target() {
        let gone = map_public_route_decision(resolution(ForumTopicRouteDisposition::Gone, None))
            .expect("gone decision");
        assert_eq!(
            gone.disposition,
            GqlForumStorefrontTopicRouteDecisionDisposition::Gone
        );
        assert!(gone.canonical.is_none());
    }

    #[test]
    fn disclosed_nonterminal_decisions_require_canonical_target() {
        assert!(
            map_public_route_decision(resolution(ForumTopicRouteDisposition::Redirect, None,))
                .is_err()
        );
    }
}
