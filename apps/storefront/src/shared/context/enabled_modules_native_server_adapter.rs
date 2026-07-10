use leptos::prelude::*;

#[server(prefix = "/api/fn", endpoint = "storefront/list-enabled-modules")]
pub(crate) async fn list_enabled_modules(
    tenant_slug: String,
) -> Result<Vec<String>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_tenant::TenantService;

        let ctx = expect_context::<AppContext>();
        let service = TenantService::new(ctx.db.clone());
        let tenant = service
            .get_tenant_by_slug(tenant_slug.as_str())
            .await
            .map_err(ServerFnError::new)?;
        let mut modules = service
            .list_tenant_modules(tenant.id)
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .filter(|module| module.enabled)
            .map(|module| module.module_slug)
            .collect::<Vec<_>>();
        modules.sort();
        Ok(modules)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = tenant_slug;
        Err(ServerFnError::new(
            "storefront/list-enabled-modules requires the `ssr` feature",
        ))
    }
}
