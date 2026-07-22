#[server(prefix = "/api/fn", endpoint = "search/upsert-synonym")]
async fn upsert_search_synonym_native(
    term: String,
    synonyms: Vec<String>,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
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

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::upsert_synonym(
            &app_ctx.db,
            tenant.id,
            &term,
            synonyms,
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (term, synonyms);
        Err(ServerFnError::new(
            "search/upsert-synonym requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/delete-synonym")]
async fn delete_search_synonym_native(
    synonym_id: String,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
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

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::delete_synonym(
            &app_ctx.db,
            tenant.id,
            parse_required_uuid(&synonym_id, "synonym_id")?,
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = synonym_id;
        Err(ServerFnError::new(
            "search/delete-synonym requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/add-stop-word")]
async fn add_search_stop_word_native(
    value: String,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
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

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::add_stop_word(&app_ctx.db, tenant.id, &value)
            .await
            .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = value;
        Err(ServerFnError::new(
            "search/add-stop-word requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/delete-stop-word")]
async fn delete_search_stop_word_native(
    stop_word_id: String,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
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

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::delete_stop_word(
            &app_ctx.db,
            tenant.id,
            parse_required_uuid(&stop_word_id, "stop_word_id")?,
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = stop_word_id;
        Err(ServerFnError::new(
            "search/delete-stop-word requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/upsert-pin-rule")]
async fn upsert_search_pin_rule_native(
    query_text: String,
    document_id: String,
    pinned_position: Option<i32>,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
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

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::upsert_pin_rule(
            &app_ctx.db,
            tenant.id,
            &query_text,
            parse_required_uuid(&document_id, "document_id")?,
            pinned_position.unwrap_or(1).clamp(1, 50) as u32,
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query_text, document_id, pinned_position);
        Err(ServerFnError::new(
            "search/upsert-pin-rule requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/delete-query-rule")]
async fn delete_search_query_rule_native(
    query_rule_id: String,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
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

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::delete_query_rule(
            &app_ctx.db,
            tenant.id,
            parse_required_uuid(&query_rule_id, "query_rule_id")?,
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query_rule_id;
        Err(ServerFnError::new(
            "search/delete-query-rule requires the `ssr` feature",
        ))
    }
}
