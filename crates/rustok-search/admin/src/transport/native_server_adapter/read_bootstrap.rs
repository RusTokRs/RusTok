#[server(prefix = "/api/fn", endpoint = "search/bootstrap")]
async fn search_admin_bootstrap_native() -> Result<SearchAdminBootstrap, ServerFnError> {
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

        let module = rustok_search::SearchModule;
        let settings =
            rustok_search::SearchSettingsService::load_effective(&app_ctx.db, Some(tenant.id))
                .await
                .map_err(ServerFnError::new)?;
        let diagnostics = rustok_search::SearchDiagnosticsService::snapshot(&app_ctx.db, tenant.id)
            .await
            .map_err(map_core_error)?;

        Ok(SearchAdminBootstrap {
            available_search_engines: module
                .available_engines()
                .into_iter()
                .map(map_search_engine_descriptor)
                .collect(),
            search_settings_preview: map_search_settings_payload(settings),
            search_diagnostics: map_diagnostics_payload(diagnostics),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "search/bootstrap requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/preview")]
async fn search_admin_preview_native(
    query: String,
    locale: Option<String>,
    ranking_profile: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, TenantContext};
        use std::time::Instant;

        let app_ctx = SearchAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let input = normalize_search_preview_input(SearchPreviewInput {
            query,
            locale,
            channel_id: filters.channel_id,
            tenant_id: None,
            limit: Some(12),
            offset: Some(0),
            ranking_profile,
            preset_key,
            entity_types: Some(filters.entity_types),
            source_modules: Some(filters.source_modules),
            statuses: Some(filters.statuses),
            category_ids: Some(filters.category_ids),
            attribute_filters: Some(search_attribute_filter_inputs(filters.attribute_filters)),
            sort_attribute_code: filters.sort_attribute_code,
            sort_desc: Some(filters.sort_desc),
        })?;
        let transform = rustok_search::SearchDictionaryService::transform_query(
            &app_ctx.db,
            tenant.id,
            &input.query,
        )
        .await
        .map_err(map_core_error)?;
        let settings =
            rustok_search::SearchSettingsService::load_effective(&app_ctx.db, Some(tenant.id))
                .await
                .map_err(ServerFnError::new)?;
        let resolved = resolve_preset_and_ranking(
            &settings.config,
            "search_preview",
            input.preset_key.as_deref(),
            input.ranking_profile.as_deref(),
            input.entity_types.unwrap_or_default(),
            input.source_modules.unwrap_or_default(),
            input.statuses.unwrap_or_default(),
        )?;

        let search_query = rustok_search::SearchQuery {
            tenant_id: Some(tenant.id),
            locale: input.locale,
            channel_id: parse_optional_uuid(input.channel_id.as_deref())?,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile: resolved.ranking_profile,
            preset_key: resolved.preset_key,
            limit: 12,
            offset: 0,
            published_only: false,
            entity_types: resolved.entity_types,
            source_modules: resolved.source_modules,
            statuses: resolved.statuses,
            category_ids: normalize_uuid_values("category_ids", input.category_ids)?,
            attribute_filters: normalize_attribute_filters(input.attribute_filters)?,
            sort_attribute_code: normalize_attribute_code(input.sort_attribute_code)?,
            sort_desc: input.sort_desc.unwrap_or(false),
        };
        let engine = rustok_search::PgSearchEngine::new(app_ctx.db.clone());
        let started_at = Instant::now();
        let result = run_search_with_dictionaries(&app_ctx.db, &engine, search_query.clone()).await;

        finalize_search_result(
            &app_ctx.db,
            "search_preview",
            &search_query,
            started_at,
            result,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query, locale, ranking_profile, preset_key, filters);
        Err(ServerFnError::new(
            "search/preview requires the `ssr` feature",
        ))
    }
}
