#[server(prefix = "/api/fn", endpoint = "search/analytics")]
async fn search_admin_analytics_native(
    days: Option<i32>,
    limit: Option<i32>,
) -> Result<SearchAnalyticsPayload, ServerFnError> {
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

        let snapshot = rustok_search::SearchAnalyticsService::snapshot(
            &app_ctx.db,
            tenant.id,
            normalize_analytics_days(days),
            normalize_analytics_limit(limit),
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_analytics_payload(snapshot))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (days, limit);
        Err(ServerFnError::new(
            "search/analytics requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/dictionary-snapshot")]
async fn search_admin_dictionary_snapshot_native()
-> Result<SearchDictionarySnapshotPayload, ServerFnError> {
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

        let snapshot = rustok_search::SearchDictionaryService::snapshot(&app_ctx.db, tenant.id)
            .await
            .map_err(map_core_error)?;

        Ok(map_dictionary_snapshot(snapshot))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "search/dictionary-snapshot requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/track-click")]
async fn track_search_click_native(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::TenantContext;

        let app_ctx = SearchAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        let query_log_id = query_log_id
            .trim()
            .parse::<i64>()
            .map_err(|_| ServerFnError::new("Invalid query_log_id"))?;
        let document_id = parse_required_uuid(&document_id, "document_id")?;

        rustok_search::SearchAnalyticsService::record_click(
            &app_ctx.db,
            rustok_search::SearchClickRecord {
                tenant_id: tenant.id,
                query_log_id,
                document_id,
                position: position.map(|value| value.max(0) as u32),
                href: href.and_then(|value| {
                    let trimmed = value.trim().to_string();
                    (!trimmed.is_empty()).then_some(trimmed)
                }),
            },
        )
        .await
        .map_err(map_core_error)?;

        Ok(TrackSearchClickPayload {
            success: true,
            tracked: true,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query_log_id, document_id, position, href);
        Err(ServerFnError::new(
            "search/track-click requires the `ssr` feature",
        ))
    }
}
