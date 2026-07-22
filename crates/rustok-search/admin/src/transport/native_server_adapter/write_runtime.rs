#[server(prefix = "/api/fn", endpoint = "search/update-settings")]
async fn update_search_settings_native(
    active_engine: String,
    fallback_engine: Option<String>,
    config: String,
) -> Result<SearchSettingsPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_events::DomainEvent;

        let app_ctx = SearchAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_manage_permission(&auth.permissions)?;

        let active_engine = parse_engine(&active_engine, "active_engine")?;
        let fallback_engine = fallback_engine
            .as_deref()
            .map(|value| parse_engine(value, "fallback_engine"))
            .transpose()?
            .unwrap_or(rustok_search::SearchEngineKind::Postgres);
        ensure_engine_available(active_engine)?;
        ensure_engine_available(fallback_engine)?;
        let config: serde_json::Value = serde_json::from_str(&config)
            .map_err(|err| ServerFnError::new(format!("Invalid JSON in config: {err}")))?;

        let settings = rustok_search::SearchSettingsService::save(
            &app_ctx.db,
            Some(tenant.id),
            active_engine,
            fallback_engine,
            config,
        )
        .await
        .map_err(ServerFnError::new)?;

        let event_bus = app_ctx.transactional_event_bus()?;
        let _ = event_bus
            .publish(
                tenant.id,
                Some(auth.user_id),
                DomainEvent::SearchSettingsChanged {
                    active_engine: active_engine.as_str().to_string(),
                    fallback_engine: fallback_engine.as_str().to_string(),
                    changed_by: auth.user_id,
                },
            )
            .await;

        Ok(map_search_settings_payload(settings))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (active_engine, fallback_engine, config);
        Err(ServerFnError::new(
            "search/update-settings requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/trigger-rebuild")]
async fn trigger_search_rebuild_native(
    target_type: Option<String>,
    target_id: Option<String>,
) -> Result<TriggerSearchRebuildPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_events::DomainEvent;

        let app_ctx = SearchAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_manage_permission(&auth.permissions)?;

        let target_type = target_type
            .unwrap_or_else(|| "search".to_string())
            .trim()
            .to_ascii_lowercase();
        if !matches!(target_type.as_str(), "search" | "content" | "product") {
            return Err(ServerFnError::new(
                "Invalid target_type. Expected one of: search, content, product",
            ));
        }

        let parsed_target_id = target_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_required_uuid(value, "target_id"))
            .transpose()?;
        let event_bus = app_ctx.transactional_event_bus()?;
        event_bus
            .publish(
                tenant.id,
                Some(auth.user_id),
                DomainEvent::ReindexRequested {
                    target_type: target_type.clone(),
                    target_id: parsed_target_id,
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        let _ = event_bus
            .publish(
                tenant.id,
                Some(auth.user_id),
                DomainEvent::SearchRebuildQueued {
                    target_type: target_type.clone(),
                    target_id: parsed_target_id,
                    queued_by: auth.user_id,
                },
            )
            .await;

        Ok(TriggerSearchRebuildPayload {
            success: true,
            queued: true,
            tenant_id: tenant.id.to_string(),
            target_type,
            target_id: parsed_target_id.map(|value| value.to_string()),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (target_type, target_id);
        Err(ServerFnError::new(
            "search/trigger-rebuild requires the `ssr` feature",
        ))
    }
}
