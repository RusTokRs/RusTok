use async_graphql::{Context, Enum, ErrorExtensions, Object, Result, SimpleObject};
use rustok_api::graphql::require_module_enabled;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_channel::ChannelService;
use rustok_core::SecurityContext;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ForumCategoryReadOperation, ForumCategoryReadTransport, ForumCategoryRouteDescriptor,
    ForumCategoryRouteDisposition, ForumCategoryRouteResolution, ForumCategoryRouteService,
    ForumError, category_read_audience_port_context,
};

use super::ForumGraphqlRuntimeData;

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumCategoryRouteQuery;

#[Object]
impl ForumCategoryRouteQuery {
    /// Resolves one localized storefront category route only after the canonical
    /// category passes the exact Forum category audience and channel boundary.
    async fn forum_storefront_category_route(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        locale: String,
        slug: String,
    ) -> Result<Option<GqlForumStorefrontCategoryRouteResolution>> {
        let Some(resolution) =
            resolve_authorized_public_category_route(ctx, tenant_id, locale, slug).await?
        else {
            return Ok(None);
        };
        Ok(Some(map_public_category_route_resolution(resolution)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GqlForumStorefrontCategoryRouteDisposition {
    Canonical,
    Redirect,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumStorefrontCategoryRouteDescriptor {
    pub category_id: Uuid,
    pub locale: String,
    pub slug: String,
    pub path: String,
}

impl From<ForumCategoryRouteDescriptor> for GqlForumStorefrontCategoryRouteDescriptor {
    fn from(value: ForumCategoryRouteDescriptor) -> Self {
        Self {
            category_id: value.category_id,
            locale: value.locale,
            slug: value.slug,
            path: value.path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumStorefrontCategoryRouteResolution {
    pub requested_locale: String,
    pub requested_slug: String,
    pub disposition: GqlForumStorefrontCategoryRouteDisposition,
    pub canonical: GqlForumStorefrontCategoryRouteDescriptor,
}

async fn resolve_authorized_public_category_route(
    ctx: &Context<'_>,
    requested_tenant_id: Option<Uuid>,
    locale: String,
    slug: String,
) -> Result<Option<ForumCategoryRouteResolution>> {
    require_module_enabled(ctx, MODULE_SLUG).await?;

    let db = ctx.data::<DatabaseConnection>()?;
    let tenant = ctx.data::<TenantContext>()?;
    let tenant_id = super::resolve_tenant_scope(tenant, requested_tenant_id)?;
    if !forum_channel_enabled(ctx).await? {
        return Ok(None);
    }

    let resolution = match ForumCategoryRouteService::new(db.clone())
        .resolve(
            tenant_id,
            &locale,
            &slug,
            Some(tenant.default_locale.as_str()),
        )
        .await
    {
        Ok(resolution) => resolution,
        Err(ForumError::CategoryNotFound(_)) | Err(ForumError::CategoryRouteNotFound) => {
            return Ok(None);
        }
        Err(error) => return Err(internal_error(error.to_string())),
    };

    let runtime = ctx
        .data_opt::<ForumGraphqlRuntimeData>()
        .cloned()
        .unwrap_or_default();
    let service = runtime.category_audience_read_service(db.clone());
    let visible = if let Some(auth) = ctx.data_opt::<AuthContext>() {
        let security =
            SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
        let audience_context = category_read_audience_port_context(
            ForumCategoryReadTransport::Graphql,
            ForumCategoryReadOperation::SelectedCategory,
            tenant_id,
            auth,
            ctx.data_opt::<RequestContext>(),
            resolution.canonical.locale.as_str(),
        )
        .map_err(|error| internal_error(error.to_string()))?;
        service
            .get_authenticated_storefront_list_visible_with_audience_context(
                tenant_id,
                security,
                audience_context,
                resolution.canonical.category_id,
                Some(tenant.default_locale.as_str()),
            )
            .await
    } else {
        service
            .get_public_storefront_visible_with_locale_fallback(
                tenant_id,
                resolution.canonical.category_id,
                resolution.canonical.locale.as_str(),
                Some(tenant.default_locale.as_str()),
            )
            .await
    };

    match visible {
        Ok(_) => Ok(Some(resolution)),
        Err(ForumError::CategoryNotFound(_)) | Err(ForumError::CategoryRouteNotFound) => Ok(None),
        Err(error) => Err(internal_error(error.to_string())),
    }
}

fn map_public_category_route_resolution(
    resolution: ForumCategoryRouteResolution,
) -> GqlForumStorefrontCategoryRouteResolution {
    let disposition = match resolution.disposition {
        ForumCategoryRouteDisposition::Canonical => {
            GqlForumStorefrontCategoryRouteDisposition::Canonical
        }
        ForumCategoryRouteDisposition::Redirect => {
            GqlForumStorefrontCategoryRouteDisposition::Redirect
        }
    };

    GqlForumStorefrontCategoryRouteResolution {
        requested_locale: resolution.requested_locale,
        requested_slug: resolution.requested_slug,
        disposition,
        canonical: resolution.canonical.into(),
    }
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
        .map_err(|error| internal_error(format!("Channel module check failed: {error}")))
}

fn internal_error(message: impl Into<String>) -> async_graphql::Error {
    async_graphql::Error::new(message.into())
        .extend_with(|_, extension| extension.set("code", "INTERNAL_SERVER_ERROR"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> ForumCategoryRouteDescriptor {
        ForumCategoryRouteDescriptor {
            category_id: Uuid::parse_str("00000000-0000-4000-8000-000000000001")
                .expect("category id"),
            locale: "en".to_string(),
            slug: "general".to_string(),
            path: "/en/forum/c/general".to_string(),
        }
    }

    #[test]
    fn mapping_exposes_only_public_category_route_shape() {
        let mapped = map_public_category_route_resolution(ForumCategoryRouteResolution {
            requested_locale: "en".to_string(),
            requested_slug: "old-general".to_string(),
            disposition: ForumCategoryRouteDisposition::Redirect,
            canonical: descriptor(),
            alias_id: Some(Uuid::new_v4()),
        });

        assert_eq!(
            mapped.disposition,
            GqlForumStorefrontCategoryRouteDisposition::Redirect
        );
        assert_eq!(mapped.canonical.path, "/en/forum/c/general");
    }

    #[test]
    fn canonical_mapping_preserves_owner_descriptor() {
        let mapped = map_public_category_route_resolution(ForumCategoryRouteResolution {
            requested_locale: "en".to_string(),
            requested_slug: "general".to_string(),
            disposition: ForumCategoryRouteDisposition::Canonical,
            canonical: descriptor(),
            alias_id: None,
        });

        assert_eq!(
            mapped.disposition,
            GqlForumStorefrontCategoryRouteDisposition::Canonical
        );
        assert_eq!(mapped.canonical.slug, "general");
    }
}
