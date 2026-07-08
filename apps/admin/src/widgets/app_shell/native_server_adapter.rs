use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::header::AdminGlobalSearchItem;
use super::header::AdminGlobalSearchPayload;

#[cfg(feature = "ssr")]
const MAX_ADMIN_SEARCH_QUERY_LEN: usize = 256;

#[server(prefix = "/api/fn", endpoint = "admin/global-search")]
pub(crate) async fn admin_global_search_native(
    query: String,
    limit: i32,
    offset: i32,
) -> Result<AdminGlobalSearchPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::Permission;
        use rustok_api::{has_effective_permission, AuthContext, TenantContext};
        use std::time::Instant;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required"));
        }

        let query = normalize_admin_search_query(&query)?;
        let limit = limit.clamp(1, 20) as usize;
        let offset = offset.max(0) as usize;
        let transform =
            rustok_search::SearchDictionaryService::transform_query(&app_ctx.db, tenant.id, &query)
                .await
                .map_err(ServerFnError::new)?;
        let settings =
            rustok_search::SearchSettingsService::load_effective(&app_ctx.db, Some(tenant.id))
                .await
                .map_err(ServerFnError::new)?;
        let ranking_profile = rustok_search::SearchRankingProfile::resolve(
            &settings.config,
            "admin_global_search",
            None,
            None,
        )
        .map_err(|err| ServerFnError::new(err.to_string()))?;

        let search_query = rustok_search::SearchQuery {
            tenant_id: Some(tenant.id),
            locale: None,
            channel_id: None,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile,
            preset_key: None,
            limit,
            offset,
            published_only: false,
            entity_types: Vec::new(),
            source_modules: Vec::new(),
            statuses: Vec::new(),
            category_ids: Vec::new(),
            attribute_filters: Vec::new(),
            sort_attribute_code: None,
            sort_desc: false,
        };
        let engine = rustok_search::PgSearchEngine::new(app_ctx.db.clone());
        let started_at = Instant::now();
        let result = rustok_search::SearchEngine::search(&engine, search_query.clone()).await;
        let result = match result {
            Ok(result) => {
                rustok_search::SearchDictionaryService::apply_query_rules(
                    &app_ctx.db,
                    &search_query,
                    result,
                )
                .await
            }
            Err(error) => Err(error),
        }
        .map_err(ServerFnError::new)?;

        let query_log_id = record_admin_search_query_log(
            &app_ctx.db,
            &search_query,
            result.engine.as_str(),
            result.total,
            result.took_ms.max(started_at.elapsed().as_millis() as u64),
        )
        .await;

        Ok(AdminGlobalSearchPayload {
            items: result
                .items
                .into_iter()
                .map(|item| {
                    let url = derive_admin_search_result_url(&item);
                    AdminGlobalSearchItem {
                        id: item.id.to_string(),
                        entity_type: item.entity_type,
                        source_module: item.source_module,
                        title: item.title,
                        snippet: item.snippet,
                        score: item.score,
                        locale: item.locale,
                        url,
                        payload: serde_json::json!({
                            "queryLogId": query_log_id,
                            "payload": item.payload,
                        })
                        .to_string(),
                    }
                })
                .collect(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query, limit, offset);
        Err(ServerFnError::new(
            "admin/global-search requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn normalize_admin_search_query(value: &str) -> Result<String, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.len() > MAX_ADMIN_SEARCH_QUERY_LEN {
        return Err(ServerFnError::new(format!(
            "Search query exceeds the maximum length of {MAX_ADMIN_SEARCH_QUERY_LEN} characters"
        )));
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err(ServerFnError::new(
            "Search query contains unsupported control characters",
        ));
    }
    Ok(trimmed.to_string())
}

#[cfg(feature = "ssr")]
async fn record_admin_search_query_log(
    db: &sea_orm::DatabaseConnection,
    search_query: &rustok_search::SearchQuery,
    engine: &str,
    result_count: u64,
    took_ms: u64,
) -> Option<i64> {
    let tenant_id = search_query.tenant_id?;
    let engine = rustok_search::SearchEngineKind::try_from_str(engine)?;

    rustok_search::SearchAnalyticsService::record_query(
        db,
        rustok_search::SearchQueryLogRecord {
            tenant_id,
            surface: "admin_global_search".to_string(),
            query: search_query.original_query.clone(),
            locale: search_query.locale.clone(),
            engine,
            result_count,
            took_ms,
            status: "success".to_string(),
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
fn derive_admin_search_result_url(item: &rustok_search::SearchResultItem) -> Option<String> {
    match item.entity_type.as_str() {
        "node" => {
            let module_slug = if item.source_module.trim().is_empty() {
                "content"
            } else {
                item.source_module.as_str()
            };
            Some(format!("/modules/{module_slug}?id={}", item.id))
        }
        "product" => Some(format!("/modules/search/playground?focusId={}", item.id)),
        _ => None,
    }
}
