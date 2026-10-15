use crate::model::StorefrontForumCategoryRouteResolution;
#[cfg(feature = "ssr")]
use crate::model::{
    StorefrontForumCategoryRouteDescriptor, StorefrontForumCategoryRouteDisposition,
};

pub async fn resolve_storefront_category_route_server(
    locale: String,
    slug: String,
) -> Result<Option<StorefrontForumCategoryRouteResolution>, ApiError> {
    storefront_category_route_native(locale, slug)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "forum/storefront-category-route")]
async fn storefront_category_route_native(
    locale: String,
    slug: String,
) -> Result<Option<StorefrontForumCategoryRouteResolution>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{HostRuntimeContext, OptionalAuthContext, RequestContext, TenantContext};
        use rustok_core::SecurityContext;
        use rustok_forum::{
            ForumCategoryAudienceReadService, ForumCategoryReadOperation,
            ForumCategoryReadTransport, ForumCategoryRouteService, ForumError,
            SharedForumAudienceFactsPort, category_read_audience_port_context,
        };

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
        let settings_providers = super::native_server_adapter::forum_settings_providers(&runtime_ctx);
        if auth.is_none()
            && !settings_providers.anonymous_reading_allowed(tenant.id).await.map_err(server_error)?
        {
            return Ok(None);
        }

        if !super::native_server_adapter::forum_channel_enabled(&runtime_ctx, tenant.id, &request).await? {
            return Ok(None);
        }

        let resolution = match ForumCategoryRouteService::new(db.clone())
            .resolve(
                tenant.id,
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
            Err(error) => return Err(server_error(error)),
        };

        let service = match runtime_ctx.shared_get::<SharedForumAudienceFactsPort>() {
            Some(facts) => ForumCategoryAudienceReadService::with_audience_facts(db, facts),
            None => ForumCategoryAudienceReadService::new(db),
        }.with_settings_providers(settings_providers);
        let visible = if let Some(auth) = auth.as_ref() {
            let security =
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
            let context = category_read_audience_port_context(
                ForumCategoryReadTransport::NativeServer,
                ForumCategoryReadOperation::SelectedCategory,
                tenant.id,
                auth,
                Some(&request),
                resolution.canonical.locale.as_str(),
            )
            .map_err(server_error)?;
            service
                .get_authenticated_storefront_list_visible_with_audience_context(
                    tenant.id,
                    security,
                    context,
                    resolution.canonical.category_id,
                    Some(tenant.default_locale.as_str()),
                )
                .await
        } else {
            service
                .get_public_storefront_visible_with_locale_fallback(
                    tenant.id,
                    resolution.canonical.category_id,
                    resolution.canonical.locale.as_str(),
                    Some(tenant.default_locale.as_str()),
                )
                .await
        };

        match visible {
            Ok(_) => Ok(Some(map_native_category_route_resolution(resolution))),
            Err(ForumError::CategoryNotFound(_)) | Err(ForumError::CategoryRouteNotFound) => {
                Ok(None)
            }
            Err(error) => Err(server_error(error)),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (locale, slug);
        Err(ServerFnError::new(
            "forum/storefront-category-route requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_native_category_route_resolution(
    resolution: rustok_forum::ForumCategoryRouteResolution,
) -> StorefrontForumCategoryRouteResolution {
    let disposition = match resolution.disposition {
        rustok_forum::ForumCategoryRouteDisposition::Canonical => {
            StorefrontForumCategoryRouteDisposition::Canonical
        }
        rustok_forum::ForumCategoryRouteDisposition::Redirect => {
            StorefrontForumCategoryRouteDisposition::Redirect
        }
    };
    StorefrontForumCategoryRouteResolution {
        requested_locale: resolution.requested_locale,
        requested_slug: resolution.requested_slug,
        disposition,
        canonical: StorefrontForumCategoryRouteDescriptor {
            category_id: resolution.canonical.category_id.to_string(),
            locale: resolution.canonical.locale,
            slug: resolution.canonical.slug,
            path: resolution.canonical.path,
        },
    }
}
