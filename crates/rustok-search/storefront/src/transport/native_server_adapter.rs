#[cfg(feature = "ssr")]
use crate::model::{SearchFacetBucket, SearchFacetGroup, SearchPreviewResultItem};
use crate::model::{
    SearchFilterPreset, SearchPreviewFilters, SearchPreviewPayload, SearchSuggestion,
    TrackSearchClickPayload,
};
use leptos::prelude::*;

use super::ApiError;

#[cfg(feature = "ssr")]
const STOREFRONT_SEARCH_SURFACE: &str = "storefront_search";
#[cfg(feature = "ssr")]
const MAX_SEARCH_QUERY_LEN: usize = 256;
#[cfg(feature = "ssr")]
const MAX_FILTER_VALUES: usize = 10;
#[cfg(feature = "ssr")]
const MAX_FILTER_VALUE_LEN: usize = 64;
#[cfg(feature = "ssr")]
const MAX_ATTRIBUTE_FILTERS: usize = 10;
#[cfg(feature = "ssr")]
const MAX_LOCALE_LEN: usize = 16;

pub async fn fetch_search(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    fetch_storefront_search_server(query, locale, preset_key, filters).await
}

pub async fn fetch_suggestions(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ApiError> {
    fetch_storefront_suggestions_server(query, locale).await
}

pub async fn fetch_filter_presets() -> Result<Vec<SearchFilterPreset>, ApiError> {
    fetch_storefront_filter_presets_server().await
}

pub async fn track_search_click(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    track_search_click_server(query_log_id, document_id, position, href).await
}

async fn fetch_storefront_search_server(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    storefront_search_native(query, locale, preset_key, filters)
        .await
        .map_err(ApiError::from)
}

async fn fetch_storefront_filter_presets_server() -> Result<Vec<SearchFilterPreset>, ApiError> {
    storefront_filter_presets_native()
        .await
        .map_err(ApiError::from)
}

async fn fetch_storefront_suggestions_server(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ApiError> {
    storefront_search_suggestions_native(query, locale)
        .await
        .map_err(ApiError::from)
}

async fn track_search_click_server(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    storefront_track_search_click_native(query_log_id, document_id, position, href)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "search/storefront-search")]
async fn storefront_search_native(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{HostRuntimeContext, TenantContext};
        use rustok_search::{
            PgSearchEngine, SearchDictionaryService, SearchEngine, SearchFilterPresetService,
            SearchQuery, SearchQueryLogRecord, SearchRankingProfile, SearchSettingsService,
        };
        use std::time::Instant;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let db = runtime_ctx.db_clone();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let input = normalize_search_input(query, locale, preset_key, filters)?;
        let started_at = Instant::now();
        let transform = SearchDictionaryService::transform_query(&db, tenant.id, &input.query)
            .await
            .map_err(server_error)?;
        let settings = SearchSettingsService::load_effective(&db, Some(tenant.id))
            .await
            .map_err(server_error)?;
        let resolved_preset = SearchFilterPresetService::resolve(
            &settings.config,
            STOREFRONT_SEARCH_SURFACE,
            input.preset_key.as_deref(),
            input.entity_types,
            input.source_modules,
            input.statuses,
        )
        .map_err(server_error)?;
        let ranking_profile = SearchRankingProfile::resolve(
            &settings.config,
            STOREFRONT_SEARCH_SURFACE,
            None,
            resolved_preset.ranking_profile,
        )
        .map_err(server_error)?;
        let search_query = SearchQuery {
            tenant_id: Some(tenant.id),
            locale: input.locale,
            channel_id: input.channel_id,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile,
            preset_key: resolved_preset.preset.map(|preset| preset.key),
            limit: 12,
            offset: 0,
            published_only: true,
            entity_types: resolved_preset.entity_types,
            source_modules: resolved_preset.source_modules,
            statuses: resolved_preset.statuses,
            category_ids: input.category_ids,
            attribute_filters: input.attribute_filters,
            sort_attribute_code: input.sort_attribute_code,
            sort_desc: input.sort_desc,
        };

        let result = PgSearchEngine::new(db.clone())
            .search(search_query.clone())
            .await
            .map_err(server_error)?;
        let result = SearchDictionaryService::apply_query_rules(&db, &search_query, result)
            .await
            .map_err(server_error)?;
        let query_log_id = rustok_search::SearchAnalyticsService::record_query(
            &db,
            SearchQueryLogRecord {
                tenant_id: tenant.id,
                surface: STOREFRONT_SEARCH_SURFACE.to_string(),
                query: search_query.original_query.clone(),
                locale: search_query.locale.clone(),
                engine: result.engine,
                result_count: result.total,
                took_ms: result.took_ms,
                status: "success".to_string(),
                entity_types: search_query.entity_types.clone(),
                source_modules: search_query.source_modules.clone(),
                statuses: search_query.statuses.clone(),
            },
        )
        .await
        .ok()
        .flatten();
        let elapsed_ms = started_at.elapsed().as_millis() as u64;
        Ok(map_search_result(
            result,
            query_log_id.map(|value| value.to_string()),
            search_query.preset_key,
            elapsed_ms,
        ))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query, locale, preset_key, filters);
        Err(ServerFnError::new(
            "search/storefront-search requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/storefront-filter-presets")]
async fn storefront_filter_presets_native() -> Result<Vec<SearchFilterPreset>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{HostRuntimeContext, TenantContext};
        use rustok_search::{SearchFilterPresetService, SearchSettingsService};

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let db = runtime_ctx.db_clone();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let settings = SearchSettingsService::load_effective(&db, Some(tenant.id))
            .await
            .map_err(server_error)?;
        Ok(
            SearchFilterPresetService::list(&settings.config, STOREFRONT_SEARCH_SURFACE)
                .into_iter()
                .map(map_filter_preset)
                .collect(),
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "search/storefront-filter-presets requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/storefront-suggestions")]
async fn storefront_search_suggestions_native(
    query: String,
    locale: Option<String>,
) -> Result<Vec<SearchSuggestion>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{HostRuntimeContext, TenantContext};
        use rustok_search::{SearchSuggestionQuery, SearchSuggestionService};

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let db = runtime_ctx.db_clone();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let query = normalize_query(&query)?;
        let locale = normalize_locale(locale.as_deref())?;
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let suggestions = SearchSuggestionService::suggestions(
            &db,
            SearchSuggestionQuery {
                tenant_id: tenant.id,
                query,
                locale,
                limit: 6,
                published_only: true,
            },
        )
        .await
        .map_err(server_error)?;
        Ok(suggestions.into_iter().map(map_suggestion).collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query, locale);
        Err(ServerFnError::new(
            "search/storefront-suggestions requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/storefront-track-click")]
async fn storefront_track_search_click_native(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{HostRuntimeContext, TenantContext};
        use rustok_search::{SearchAnalyticsService, SearchClickRecord};

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let db = runtime_ctx.db_clone();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let query_log_id = query_log_id
            .trim()
            .parse::<i64>()
            .map_err(|_| ServerFnError::new("Invalid query_log_id"))?;
        let document_id = uuid::Uuid::parse_str(document_id.trim())
            .map_err(|_| ServerFnError::new("Invalid document_id"))?;

        SearchAnalyticsService::record_click(
            &db,
            SearchClickRecord {
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
        .map_err(server_error)?;

        Ok(TrackSearchClickPayload {
            success: true,
            tracked: true,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query_log_id, document_id, position, href);
        Err(ServerFnError::new(
            "search/storefront-track-click requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
struct NormalizedSearchInput {
    query: String,
    locale: Option<String>,
    channel_id: Option<uuid::Uuid>,
    preset_key: Option<String>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
    category_ids: Vec<uuid::Uuid>,
    attribute_filters: Vec<rustok_search::SearchAttributeFilter>,
    sort_attribute_code: Option<String>,
    sort_desc: bool,
}

#[cfg(feature = "ssr")]
fn normalize_search_input(
    query: String,
    locale: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<NormalizedSearchInput, ServerFnError> {
    Ok(NormalizedSearchInput {
        query: normalize_query(&query)?,
        locale: normalize_locale(locale.as_deref())?,
        channel_id: parse_optional_uuid(filters.channel_id.as_deref())?,
        preset_key: normalize_preset_key(preset_key)?,
        entity_types: normalize_filter_values("entity_types", Some(filters.entity_types))?,
        source_modules: normalize_filter_values("source_modules", Some(filters.source_modules))?,
        statuses: normalize_filter_values("statuses", Some(filters.statuses))?,
        category_ids: normalize_uuid_values("category_ids", Some(filters.category_ids))?,
        attribute_filters: normalize_attribute_filters(filters.attribute_filters)?,
        sort_attribute_code: normalize_attribute_code(filters.sort_attribute_code)?,
        sort_desc: filters.sort_desc,
    })
}

#[cfg(feature = "ssr")]
fn normalize_query(value: &str) -> Result<String, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.len() > MAX_SEARCH_QUERY_LEN {
        return Err(ServerFnError::new(format!(
            "Search query exceeds the maximum length of {MAX_SEARCH_QUERY_LEN} characters"
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
fn normalize_locale(value: Option<&str>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() > MAX_LOCALE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(ServerFnError::new("Invalid locale format"));
    }
    Ok(Some(value.to_ascii_lowercase()))
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(value: Option<&str>) -> Result<Option<uuid::Uuid>, ServerFnError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| uuid::Uuid::parse_str(value).map_err(|_| ServerFnError::new("Invalid UUID")))
        .transpose()
}

#[cfg(feature = "ssr")]
fn normalize_filter_values(
    field_name: &str,
    values: Option<Vec<String>>,
) -> Result<Vec<String>, ServerFnError> {
    let values = values.unwrap_or_default();
    if values.len() > MAX_FILTER_VALUES {
        return Err(ServerFnError::new(format!(
            "{field_name} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        )));
    }
    values
        .into_iter()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                return Err(ServerFnError::new(format!(
                    "{field_name} contains an empty value"
                )));
            }
            if normalized.len() > MAX_FILTER_VALUE_LEN
                || !normalized
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
            {
                return Err(ServerFnError::new(format!(
                    "{field_name} contains an invalid value"
                )));
            }
            Ok(normalized)
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn normalize_uuid_values(
    field_name: &str,
    values: Option<Vec<String>>,
) -> Result<Vec<uuid::Uuid>, ServerFnError> {
    let values = values.unwrap_or_default();
    if values.len() > MAX_FILTER_VALUES {
        return Err(ServerFnError::new(format!(
            "{field_name} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        )));
    }
    values
        .into_iter()
        .map(|value| {
            uuid::Uuid::parse_str(value.trim())
                .map_err(|_| ServerFnError::new(format!("{field_name} contains an invalid UUID")))
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn normalize_attribute_code(value: Option<String>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    validate_attribute_code("sort_attribute_code", &value)?;
    Ok(Some(value))
}

#[cfg(feature = "ssr")]
fn normalize_preset_key(value: Option<String>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if value.len() > MAX_FILTER_VALUE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
    {
        return Err(ServerFnError::new("Invalid preset key"));
    }
    Ok(Some(value))
}

#[cfg(feature = "ssr")]
fn normalize_attribute_filters(
    filters: Vec<crate::model::SearchAttributeFilter>,
) -> Result<Vec<rustok_search::SearchAttributeFilter>, ServerFnError> {
    if filters.len() > MAX_ATTRIBUTE_FILTERS {
        return Err(ServerFnError::new(format!(
            "attribute_filters exceeds the maximum size of {MAX_ATTRIBUTE_FILTERS} filters"
        )));
    }
    filters
        .into_iter()
        .map(|filter| {
            let attribute_code = filter.attribute_code.trim().to_ascii_lowercase();
            validate_attribute_code("attribute_code", &attribute_code)?;
            let values = normalize_filter_values("attribute_filter.values", Some(filter.values))?;
            Ok(rustok_search::SearchAttributeFilter {
                attribute_code,
                values,
                min: normalize_attribute_bound("attribute_filter.min", filter.min)?,
                max: normalize_attribute_bound("attribute_filter.max", filter.max)?,
            })
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn validate_attribute_code(field_name: &str, value: &str) -> Result<(), ServerFnError> {
    if value.is_empty()
        || value.len() > MAX_FILTER_VALUE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(ServerFnError::new(format!(
            "{field_name} contains an invalid value"
        )));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn normalize_attribute_bound(
    field_name: &str,
    value: Option<String>,
) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if value.len() > MAX_FILTER_VALUE_LEN || value.chars().any(|ch| ch.is_control()) {
        return Err(ServerFnError::new(format!(
            "{field_name} contains an invalid value"
        )));
    }
    Ok(Some(value))
}

#[cfg(feature = "ssr")]
fn map_search_result(
    value: rustok_search::SearchResult,
    query_log_id: Option<String>,
    preset_key: Option<String>,
    elapsed_ms: u64,
) -> SearchPreviewPayload {
    SearchPreviewPayload {
        query_log_id,
        preset_key,
        total: value.total,
        took_ms: value.took_ms.max(elapsed_ms),
        engine: value.engine.as_str().to_string(),
        ranking_profile: value.ranking_profile.as_str().to_string(),
        items: value.items.into_iter().map(map_result_item).collect(),
        facets: value.facets.into_iter().map(map_facet_group).collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_result_item(value: rustok_search::SearchResultItem) -> SearchPreviewResultItem {
    let url = rustok_search::canonical_search_result_url(&value);
    SearchPreviewResultItem {
        id: value.id.to_string(),
        entity_type: value.entity_type,
        source_module: value.source_module,
        title: value.title,
        snippet: value.snippet,
        score: value.score,
        locale: value.locale,
        url,
        payload: serde_json::to_string(&value.payload).unwrap_or_else(|_| "{}".to_string()),
    }
}

#[cfg(feature = "ssr")]
fn map_facet_group(value: rustok_search::engine::SearchFacetGroup) -> SearchFacetGroup {
    SearchFacetGroup {
        name: value.name,
        buckets: value.buckets.into_iter().map(map_facet_bucket).collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_facet_bucket(value: rustok_search::engine::SearchFacetBucket) -> SearchFacetBucket {
    SearchFacetBucket {
        value: value.value,
        label: value.label,
        count: value.count,
    }
}

#[cfg(feature = "ssr")]
fn map_filter_preset(value: rustok_search::SearchFilterPreset) -> SearchFilterPreset {
    SearchFilterPreset {
        key: value.key,
        label: value.label,
        entity_types: value.entity_types,
        source_modules: value.source_modules,
        statuses: value.statuses,
        ranking_profile: value
            .ranking_profile
            .map(|value| value.as_str().to_string()),
    }
}

#[cfg(feature = "ssr")]
fn map_suggestion(value: rustok_search::SearchSuggestion) -> SearchSuggestion {
    SearchSuggestion {
        text: value.text,
        kind: value.kind.as_str().to_string(),
        document_id: value.document_id.map(|value| value.to_string()),
        entity_type: value.entity_type,
        source_module: value.source_module,
        locale: value.locale,
        url: value.url,
        score: value.score,
    }
}

#[cfg(feature = "ssr")]
fn server_error(error: impl std::fmt::Display) -> ServerFnError {
    ServerFnError::new(error.to_string())
}
