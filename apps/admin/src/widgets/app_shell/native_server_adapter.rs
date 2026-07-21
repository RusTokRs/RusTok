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
        use rustok_api::Permission;
        use rustok_api::{AuthContext, TenantContext, has_effective_permission};
        use std::time::Instant;

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
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
        let transform = rustok_search::SearchDictionaryService::transform_query(
            runtime.db(),
            tenant.id,
            &query,
        )
        .await
        .map_err(ServerFnError::new)?;
        let settings =
            rustok_search::SearchSettingsService::load_effective(runtime.db(), Some(tenant.id))
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
        let engine = rustok_search::PgSearchEngine::new(runtime.db_clone());
        let started_at = Instant::now();
        let result = rustok_search::SearchEngine::search(&engine, search_query.clone()).await;
        let result = match result {
            Ok(result) => {
                rustok_search::SearchDictionaryService::apply_query_rules(
                    runtime.db(),
                    &search_query,
                    result,
                )
                .await
            }
            Err(error) => Err(error),
        }
        .map_err(ServerFnError::new)?;

        let query_log_id = record_admin_search_query_log(
            runtime.db(),
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
                .filter(|item| {
                    required_admin_search_permission(&item.entity_type, &item.source_module)
                        .is_some_and(|permission| {
                            has_effective_permission(&auth.permissions, &permission)
                        })
                })
                .map(|item| {
                    let url = rustok_search::canonical_search_result_url(&item);
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
fn required_admin_search_permission(
    entity_type: &str,
    source_module: &str,
) -> Option<rustok_api::Permission> {
    use rustok_api::Permission;

    match (entity_type.trim(), source_module.trim()) {
        ("product", _) => Some(Permission::PRODUCTS_READ),
        ("blog_post", "blog" | "rustok-blog") => Some(Permission::BLOG_POSTS_READ),
        ("node", "" | "content" | "rustok-content") => Some(Permission::NODES_READ),
        ("node", "blog" | "rustok-blog") => Some(Permission::BLOG_POSTS_READ),
        ("node", "pages" | "rustok-pages") => Some(Permission::PAGES_READ),
        ("node", "flex" | "rustok-flex") => Some(Permission::FLEX_ENTRIES_READ),
        _ => None,
    }
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

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::required_admin_search_permission;
    use rustok_api::Permission;

    #[test]
    fn search_result_types_map_to_domain_permissions() {
        assert_eq!(
            required_admin_search_permission("product", "catalog"),
            Some(Permission::PRODUCTS_READ)
        );
        assert_eq!(
            required_admin_search_permission("blog_post", "blog"),
            Some(Permission::BLOG_POSTS_READ)
        );
        assert_eq!(
            required_admin_search_permission("node", "blog"),
            Some(Permission::BLOG_POSTS_READ)
        );
        assert_eq!(
            required_admin_search_permission("node", "pages"),
            Some(Permission::PAGES_READ)
        );
    }

    #[test]
    fn unknown_search_sources_fail_closed() {
        assert_eq!(required_admin_search_permission("secret", "unknown"), None);
        assert_eq!(required_admin_search_permission("blog_post", "content"), None);
        assert_eq!(required_admin_search_permission("node", "unknown"), None);
    }
}
