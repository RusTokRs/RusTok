use async_graphql::{Context, Enum, ErrorExtensions, FieldError, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, RequestContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
};
use rustok_channel::ChannelService;
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ForumError, ForumTopicRouteDisposition, ForumTopicRouteResolution,
    ForumTopicRouteService, TopicService,
};

use super::GqlForumTopicRouteDescriptor;

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumTopicRouteQuery;

#[Object]
impl ForumTopicRouteQuery {
    /// Resolves one public Forum topic route after rechecking the canonical topic through the
    /// existing storefront visibility contract.
    ///
    /// Deleted-route tombstones intentionally remain hidden until Forum owns a visibility
    /// snapshot that can authorize a public `gone` response without disclosing private history.
    async fn forum_storefront_topic_route(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        locale: String,
        short_id: String,
        slug: String,
    ) -> Result<Option<GqlForumStorefrontTopicRouteResolution>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_forum_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let resolution = match ForumTopicRouteService::new(db.clone())
            .resolve(tenant_id, &locale, &short_id, &slug)
            .await
        {
            Ok(resolution) => resolution,
            Err(ForumError::TopicRouteNotFound) => return Ok(None),
            Err(error) => return Err(async_graphql::Error::new(error.to_string())),
        };

        let canonical = match resolution.disposition {
            ForumTopicRouteDisposition::Canonical | ForumTopicRouteDisposition::Redirect => {
                resolution.canonical.as_ref().ok_or_else(|| {
                    async_graphql::Error::new(
                        "Forum topic route resolution did not provide a canonical target",
                    )
                    .extend_with(|_, extension| {
                        extension.set("code", "INTERNAL_SERVER_ERROR")
                    })
                })?
            }
            ForumTopicRouteDisposition::Gone => return Ok(None),
        };

        let topic = match TopicService::new(db.clone(), event_bus.clone())
            .get_with_locale_fallback(
                tenant_id,
                forum_request_security(ctx),
                canonical.topic_id,
                &canonical.locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(topic) => topic,
            Err(ForumError::TopicNotFound(_))
            | Err(ForumError::TopicDeleted)
            | Err(ForumError::TopicRouteNotFound) => return Ok(None),
            Err(error) => return Err(async_graphql::Error::new(error.to_string())),
        };

        if is_public_request(ctx)
            && (topic.status != crate::constants::topic_status::OPEN
                || !is_topic_visible_for_channel(
                    &topic.channel_slugs,
                    public_channel_slug(ctx).as_deref(),
                ))
        {
            return Ok(None);
        }

        map_public_route_resolution(resolution)
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

fn map_public_route_resolution(
    resolution: ForumTopicRouteResolution,
) -> Result<Option<GqlForumStorefrontTopicRouteResolution>> {
    let disposition = match resolution.disposition {
        ForumTopicRouteDisposition::Canonical => {
            GqlForumStorefrontTopicRouteDisposition::Canonical
        }
        ForumTopicRouteDisposition::Redirect => {
            GqlForumStorefrontTopicRouteDisposition::Redirect
        }
        ForumTopicRouteDisposition::Gone => return Ok(None),
    };
    let canonical = resolution.canonical.ok_or_else(|| {
        async_graphql::Error::new(
            "Forum topic route resolution did not provide a canonical target",
        )
        .extend_with(|_, extension| extension.set("code", "INTERNAL_SERVER_ERROR"))
    })?;

    Ok(Some(GqlForumStorefrontTopicRouteResolution {
        requested_locale: resolution.requested_locale,
        requested_short_id: resolution.requested_short_id,
        requested_slug: resolution.requested_slug,
        disposition,
        canonical: canonical.into(),
    }))
}

async fn require_public_forum_channel_enabled(ctx: &Context<'_>) -> Result<()> {
    let db = ctx.data::<DatabaseConnection>()?;
    if ctx.data_opt::<AuthContext>().is_some() {
        return Ok(());
    }
    let Some(request_context) = ctx.data_opt::<RequestContext>() else {
        return Ok(());
    };
    let Some(channel_id) = request_context.channel_id else {
        return Ok(());
    };

    let enabled = ChannelService::new(db.clone())
        .is_module_enabled(channel_id, MODULE_SLUG)
        .await
        .map_err(|error| {
            async_graphql::Error::new(format!("Channel module check failed: {error}"))
                .extend_with(|_, extension| extension.set("code", "INTERNAL_SERVER_ERROR"))
        })?;
    if enabled {
        Ok(())
    } else {
        Err(
            async_graphql::Error::new("Forum module is not enabled for this channel")
                .extend_with(|_, extension| extension.set("code", "FORBIDDEN")),
        )
    }
}

fn resolve_tenant_scope(tenant: &TenantContext, requested_tenant_id: Option<Uuid>) -> Result<Uuid> {
    match requested_tenant_id {
        Some(requested_tenant_id) if requested_tenant_id != tenant.id => {
            Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: tenant scope mismatch",
            ))
        }
        Some(requested_tenant_id) => Ok(requested_tenant_id),
        None => Ok(tenant.id),
    }
}

fn forum_request_security(ctx: &Context<'_>) -> SecurityContext {
    ctx.data_opt::<AuthContext>()
        .map(|auth| {
            SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
        })
        .unwrap_or_else(SecurityContext::public_read)
}

fn is_public_request(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<AuthContext>().is_none()
}

fn public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request| request.channel_slug.clone())
}

fn is_topic_visible_for_channel(channel_slugs: &[String], channel_slug: Option<&str>) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }
    let Some(channel_slug) = channel_slug else {
        return false;
    };
    let normalized = channel_slug.trim().to_ascii_lowercase();
    !normalized.is_empty() && channel_slugs.iter().any(|item| item == &normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ForumTopicRouteDescriptor;

    fn descriptor() -> ForumTopicRouteDescriptor {
        ForumTopicRouteDescriptor {
            topic_id: Uuid::parse_str("00000000-0000-4000-8000-000000000001")
                .expect("topic id"),
            locale: "en".to_string(),
            short_id: "000000000000".to_string(),
            slug: "welcome".to_string(),
            path: "/forum/en/t/000000000000/welcome".to_string(),
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
            requested_topic_id: None,
            disposition,
            canonical,
            alias_id: None,
        }
    }

    #[test]
    fn maps_only_canonical_and_redirect_public_results() {
        let canonical = map_public_route_resolution(resolution(
            ForumTopicRouteDisposition::Canonical,
            Some(descriptor()),
        ))
        .expect("canonical mapping")
        .expect("canonical result");
        assert_eq!(
            canonical.disposition,
            GqlForumStorefrontTopicRouteDisposition::Canonical
        );

        let redirect = map_public_route_resolution(resolution(
            ForumTopicRouteDisposition::Redirect,
            Some(descriptor()),
        ))
        .expect("redirect mapping")
        .expect("redirect result");
        assert_eq!(
            redirect.disposition,
            GqlForumStorefrontTopicRouteDisposition::Redirect
        );
    }

    #[test]
    fn hides_gone_routes_without_a_visibility_snapshot() {
        assert!(
            map_public_route_resolution(resolution(ForumTopicRouteDisposition::Gone, None))
                .expect("gone mapping")
                .is_none()
        );
    }

    #[test]
    fn requires_canonical_target_for_disclosed_results() {
        assert!(
            map_public_route_resolution(resolution(
                ForumTopicRouteDisposition::Redirect,
                None,
            ))
            .is_err()
        );
    }
}
