#[server(prefix = "/api/fn", endpoint = "search/filter-presets")]
async fn search_admin_filter_presets_native(
    surface: String,
) -> Result<Vec<SearchFilterPresetPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = SearchAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let surface = normalize_surface(&surface)?;
        let settings =
            rustok_search::SearchSettingsService::load_effective(&app_ctx.db, Some(tenant.id))
                .await
                .map_err(ServerFnError::new)?;

        Ok(
            rustok_search::SearchFilterPresetService::list(&settings.config, &surface)
                .into_iter()
                .map(|value| SearchFilterPresetPayload {
                    key: value.key,
                    label: value.label,
                    entity_types: value.entity_types,
                    source_modules: value.source_modules,
                    statuses: value.statuses,
                    ranking_profile: value
                        .ranking_profile
                        .map(|value| value.as_str().to_string()),
                })
                .collect(),
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = surface;
        Err(ServerFnError::new(
            "search/filter-presets requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/lagging-documents")]
async fn search_admin_lagging_documents_native(
    limit: Option<i32>,
) -> Result<Vec<LaggingSearchDocumentPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = SearchAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let rows = rustok_search::SearchDiagnosticsService::lagging_documents(
            &app_ctx.db,
            tenant.id,
            normalize_limit(limit, 25, 100),
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_lagging_documents(rows))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "search/lagging-documents requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/consistency-issues")]
async fn search_admin_consistency_issues_native(
    limit: Option<i32>,
) -> Result<Vec<SearchConsistencyIssuePayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = SearchAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let rows = rustok_search::SearchDiagnosticsService::consistency_issues(
            &app_ctx.db,
            tenant.id,
            normalize_limit(limit, 25, 100),
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_consistency_issues(rows))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "search/consistency-issues requires the `ssr` feature",
        ))
    }
}
