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
    ForumError, ForumTopicReadOperation, ForumTopicReadTransport, ForumTopicRouteDisposition,
    ForumTopicRouteResolution, ForumTopicRouteService, topic_read_audience_port_context,
};

use super::{ForumGraphqlRuntimeData, GqlForumTopicRouteDescriptor};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumTopicRouteQuery;

#[Object]
impl ForumTopicRouteQuery {
    /// Resolves one public Forum topic route after rechecking the canonical topic through the
    /// exact storefront audience contract.
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
            Err(ForumError::TopicNotFound(_))
            | Err(ForumError::TopicDeleted)
            | Err(ForumError::TopicRouteNotFound) => return Ok(None),
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
            Ok(Some(_)) => map_public_route_resolution(resolution),
            Ok(None)
            | Err(ForumError::TopicNotFound(_))
            | Err(ForumError::TopicDeleted)
            | Err(ForumError::TopicRouteNotFound) => Ok(None),
            Err(error) => Err(async_graphql::Error::new(error.to_string())),
        }
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
            topic_id: Uuid::parse_str("00000000-0000-4000-8000-000000000001")
                .expect("topic id"),
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
