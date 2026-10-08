use leptos::prelude::*;

use super::canonical_route::ResolvedCanonicalRoute;
use crate::shared::api::configured_tenant_slug;

#[server(prefix = "/api/fn", endpoint = "storefront/resolve-canonical-route")]
pub(crate) async fn resolve_canonical_route(
    tenant_slug: String,
    locale: String,
    route: String,
) -> Result<Option<ResolvedCanonicalRoute>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_content::SharedCanonicalRouteResolver;
        use rustok_core::ModuleRuntimeExtensions;
        use rustok_tenant::TenantService;

        let configured = configured_tenant_slug().ok_or_else(|| {
            ServerFnError::new(
                "storefront canonical-route server function requires a configured host tenant",
            )
        })?;
        if tenant_slug.trim() != configured {
            return Err(ServerFnError::new(
                "storefront canonical-route tenant does not match the configured host tenant",
            ));
        }

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let tenant = TenantService::new(runtime.db_clone())
            .get_tenant_by_slug(tenant_slug.as_str())
            .await
            .map_err(ServerFnError::new)?;
        let extensions = runtime
            .shared_get::<std::sync::Arc<ModuleRuntimeExtensions>>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "canonical route runtime extensions are not initialized; host bootstrap must provide ModuleRuntimeExtensions",
                )
            })?;
        let resolver = extensions
            .get::<SharedCanonicalRouteResolver>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "canonical route resolver is not initialized; host bootstrap must register it",
                )
            })?;
        let resolved = resolver
            .0
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
