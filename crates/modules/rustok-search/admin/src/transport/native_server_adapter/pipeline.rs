#[cfg(feature = "ssr")]
async fn run_search_with_dictionaries(
    db: &sea_orm::DatabaseConnection,
    engine: &rustok_search::PgSearchEngine,
    search_query: rustok_search::SearchQuery,
) -> rustok_core::Result<rustok_search::SearchResult> {
    let result = rustok_search::SearchEngine::search(engine, search_query.clone()).await?;
    rustok_search::SearchDictionaryService::apply_query_rules(db, &search_query, result).await
}

#[cfg(feature = "ssr")]
fn classify_search_error(error: &rustok_core::Error) -> &'static str {
    match error {
        rustok_core::Error::Database(_) => "database",
        rustok_core::Error::Validation(_) => "validation",
        rustok_core::Error::External(_) => "external",
        rustok_core::Error::NotFound(_) => "not_found",
        rustok_core::Error::Forbidden(_) => "forbidden",
        rustok_core::Error::Auth(_) => "auth",
        rustok_core::Error::Cache(_) => "cache",
        rustok_core::Error::Serialization(_) => "serialization",
        rustok_core::Error::Scripting(_) => "scripting",
        rustok_core::Error::InvalidIdFormat(_) => "invalid_id",
    }
}

#[cfg(feature = "ssr")]
async fn record_search_query_log(
    db: &sea_orm::DatabaseConnection,
    surface: &str,
    search_query: &rustok_search::SearchQuery,
    engine: &str,
    result_count: u64,
    took_ms: u64,
    status: &str,
) -> Option<i64> {
    let tenant_id = search_query.tenant_id?;
    let engine_kind = rustok_search::SearchEngineKind::try_from_str(engine)?;

    rustok_search::SearchAnalyticsService::record_query(
        db,
        rustok_search::SearchQueryLogRecord {
            tenant_id,
            surface: surface.to_string(),
            query: search_query.original_query.clone(),
            locale: search_query.locale.clone(),
            engine: engine_kind,
            result_count,
            took_ms,
            status: status.to_string(),
            entity_types: search_query.entity_types.clone(),
            source_modules: search_query.source_modules.clone(),
            statuses: search_query.statuses.clone(),
        },
    )
    .await
    .ok()
    .flatten()
}

#[cfg(feature = "ssr")]
async fn finalize_search_result(
    db: &sea_orm::DatabaseConnection,
    surface: &str,
    search_query: &rustok_search::SearchQuery,
    started_at: std::time::Instant,
    result: rustok_core::Result<rustok_search::SearchResult>,
) -> Result<SearchPreviewPayload, ServerFnError> {
    match result {
        Ok(result) => {
            let query_log_id = record_search_query_log(
                db,
                surface,
                search_query,
                result.engine.as_str(),
                result.total,
                result.took_ms,
                "success",
            )
            .await;
            Ok(map_search_preview_payload(
                result,
                search_query.preset_key.clone(),
                query_log_id,
            ))
        }
        Err(error) => {
            let _ = record_search_query_log(
                db,
                surface,
                search_query,
                "postgres",
                0,
                started_at.elapsed().as_millis() as u64,
                classify_search_error(&error),
            )
            .await;
            Err(map_core_error(error))
        }
    }
}

#[cfg(feature = "ssr")]
fn map_search_engine_descriptor(
    value: rustok_search::SearchConnectorDescriptor,
) -> crate::model::SearchEngineDescriptor {
    crate::model::SearchEngineDescriptor {
        kind: value.kind.as_str().to_string(),
        label: value.label,
        provided_by: value.provided_by,
        enabled: value.enabled,
        default_engine: value.default_engine,
    }
}

#[cfg(feature = "ssr")]
fn map_search_settings_payload(
    value: rustok_search::SearchSettingsRecord,
) -> SearchSettingsPayload {
    SearchSettingsPayload {
        tenant_id: value.tenant_id.map(|tenant_id| tenant_id.to_string()),
        active_engine: value.active_engine.as_str().to_string(),
        fallback_engine: value.fallback_engine.as_str().to_string(),
        config: value.config.to_string(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}
