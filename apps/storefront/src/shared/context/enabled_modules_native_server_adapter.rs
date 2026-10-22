use leptos::prelude::*;

#[server(prefix = "/api/fn", endpoint = "storefront/list-enabled-modules")]
pub(crate) async fn list_enabled_modules() -> Result<Vec<String>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;

        // One implementation per concept: this is the native counterpart
        // of the GraphQL `enabledModules` query, so it resolves through
        // the same owner-issued effective policy (Core/platform defaults
        // plus recorded tenant intent). Reading raw `tenant_modules`
        // rows here under-reported default-enabled modules and, after
        // tri-state `enabled` (NULL = no recorded intent), cannot express
        // effective state at all.
        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let reader = runtime
            .shared_get::<rustok_modules::SharedModuleEffectivePolicyReader>()
            .ok_or_else(|| {
                ServerFnError::new("effective module policy reader is not configured")
            })?;
        let mut modules = reader
            .0
            .resolve(tenant.id)
            .await
            .map_err(|_| ServerFnError::new("effective module policy is unavailable"))?
            .enabled_module_slugs();
        modules.sort();
        Ok(modules)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "storefront/list-enabled-modules requires the `ssr` feature",
        ))
    }
}
