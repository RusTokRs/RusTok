use leptos::prelude::*;

use super::canonical_route::ResolvedCanonicalRoute;

#[server(prefix = "/api/fn", endpoint = "storefront/resolve-canonical-route")]
pub(crate) async fn resolve_canonical_route(
    tenant_slug: String,
    locale: String,
    route: String,
) -> Result<Option<ResolvedCanonicalRoute>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_content::CanonicalUrlService;
        use rustok_tenant::TenantService;

        let ctx = expect_context::<AppContext>();
        let tenant = TenantService::new(ctx.db.clone())
            .get_tenant_by_slug(tenant_slug.as_str())
            .await
            .map_err(ServerFnError::new)?;
        let resolved = CanonicalUrlService::new(ctx.db.clone())
            .resolve_route(tenant.id, locale.as_str(), route.as_str())
            .await
            .map_err(ServerFnError::new)?;
        Ok(resolved.map(|resolved| ResolvedCanonicalRoute {
            target_kind: resolved.target_kind,
            target_id: resolved.target_id.to_string(),
            locale: resolved.locale,
            matched_url: resolved.matched_url,
            canonical_url: resolved.canonical_url,
            redirect_required: resolved.redirect_required,
        }))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_slug, locale, route);
        Err(ServerFnError::new(
            "storefront/resolve-canonical-route requires the `ssr` feature",
        ))
    }
}
