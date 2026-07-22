use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[cfg(feature = "ssr")]
use crate::model::SearchAttributeFilter;
use crate::model::{
    LaggingSearchDocumentPayload, SearchAdminBootstrap, SearchAnalyticsPayload,
    SearchConsistencyIssuePayload, SearchDictionaryMutationPayload,
    SearchDictionarySnapshotPayload, SearchFilterPresetPayload, SearchPreviewFilters,
    SearchPreviewPayload, SearchSettingsPayload, TrackSearchClickPayload,
    TriggerSearchRebuildPayload,
};
#[cfg(feature = "ssr")]
use rustok_api::HostRuntimeContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

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

#[cfg(feature = "ssr")]
struct SearchAdminRuntime {
    db: sea_orm::DatabaseConnection,
    host: HostRuntimeContext,
}

#[cfg(feature = "ssr")]
impl SearchAdminRuntime {
    fn from_host(host: HostRuntimeContext) -> Self {
        Self {
            db: host.db_clone(),
            host,
        }
    }

    fn transactional_event_bus(
        &self,
    ) -> Result<rustok_outbox::TransactionalEventBus, ServerFnError> {
        self.host
            .shared_get::<rustok_outbox::TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "Search admin requires TransactionalEventBus in host runtime context",
                )
            })
    }
}

#[cfg(feature = "ssr")]
#[derive(Debug, Serialize)]
struct SearchPreviewInput {
    query: String,
    locale: Option<String>,
    #[serde(rename = "channelId")]
    channel_id: Option<String>,
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
    #[serde(rename = "rankingProfile")]
    ranking_profile: Option<String>,
    #[serde(rename = "presetKey")]
    preset_key: Option<String>,
    #[serde(rename = "entityTypes")]
    entity_types: Option<Vec<String>>,
    #[serde(rename = "sourceModules")]
    source_modules: Option<Vec<String>>,
    statuses: Option<Vec<String>>,
    #[serde(rename = "categoryIds")]
    category_ids: Option<Vec<String>>,
    #[serde(rename = "attributeFilters")]
    attribute_filters: Option<Vec<SearchAttributeFilterInput>>,
    #[serde(rename = "sortAttributeCode")]
    sort_attribute_code: Option<String>,
    #[serde(rename = "sortDesc")]
    sort_desc: Option<bool>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Serialize)]
struct SearchAttributeFilterInput {
    #[serde(rename = "attributeCode")]
    attribute_code: String,
    values: Option<Vec<String>>,
    min: Option<String>,
    max: Option<String>,
}

#[cfg(feature = "ssr")]
fn search_attribute_filter_inputs(
    filters: Vec<SearchAttributeFilter>,
) -> Vec<SearchAttributeFilterInput> {
    filters
        .into_iter()
        .map(|filter| SearchAttributeFilterInput {
            attribute_code: filter.attribute_code,
            values: (!filter.values.is_empty()).then_some(filter.values),
            min: filter.min,
            max: filter.max,
        })
        .collect()
}
pub async fn fetch_bootstrap(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<SearchAdminBootstrap, ApiError> {
    search_admin_bootstrap_native()
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_search_preview(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    query: String,
    locale: Option<String>,
    ranking_profile: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    search_admin_preview_native(query, locale, ranking_profile, preset_key, filters)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_filter_presets(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    surface: &str,
) -> Result<Vec<SearchFilterPresetPayload>, ApiError> {
    search_admin_filter_presets_native(surface.to_string())
        .await
        .map_err(ApiError::from)
}

pub async fn trigger_search_rebuild(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
) -> Result<TriggerSearchRebuildPayload, ApiError> {
    trigger_search_rebuild_native(target_type, target_id)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_lagging_documents(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<LaggingSearchDocumentPayload>, ApiError> {
    search_admin_lagging_documents_native(limit)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_consistency_issues(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<SearchConsistencyIssuePayload>, ApiError> {
    search_admin_consistency_issues_native(limit)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_search_analytics(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    days: Option<i32>,
    limit: Option<i32>,
) -> Result<SearchAnalyticsPayload, ApiError> {
    search_admin_analytics_native(days, limit)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_dictionary_snapshot(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<SearchDictionarySnapshotPayload, ApiError> {
    search_admin_dictionary_snapshot_native()
        .await
        .map_err(ApiError::from)
}

pub async fn track_search_click(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    track_search_click_native(query_log_id, document_id, position, href)
        .await
        .map_err(ApiError::from)
}

pub async fn update_search_settings(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    active_engine: String,
    fallback_engine: Option<String>,
    config: String,
) -> Result<SearchSettingsPayload, ApiError> {
    update_search_settings_native(active_engine, fallback_engine, config)
        .await
        .map_err(ApiError::from)
}

pub async fn upsert_search_synonym(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    term: String,
    synonyms: Vec<String>,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    upsert_search_synonym_native(term, synonyms)
        .await
        .map_err(ApiError::from)
}

pub async fn delete_search_synonym(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    synonym_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    delete_search_synonym_native(synonym_id)
        .await
        .map_err(ApiError::from)
}

pub async fn add_search_stop_word(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    value: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    add_search_stop_word_native(value)
        .await
        .map_err(ApiError::from)
}

pub async fn delete_search_stop_word(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    stop_word_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    delete_search_stop_word_native(stop_word_id)
        .await
        .map_err(ApiError::from)
}

pub async fn upsert_search_pin_rule(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    query_text: String,
    document_id: String,
    pinned_position: Option<i32>,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    upsert_search_pin_rule_native(query_text, document_id, pinned_position)
        .await
        .map_err(ApiError::from)
}

pub async fn delete_search_query_rule(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    query_rule_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    delete_search_query_rule_native(query_rule_id)
        .await
        .map_err(ApiError::from)
}

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

#[cfg(feature = "ssr")]
struct ResolvedSearchInput {
    preset_key: Option<String>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
    ranking_profile: rustok_search::SearchRankingProfile,
}

#[cfg(feature = "ssr")]
fn normalize_search_preview_input(
    input: SearchPreviewInput,
) -> Result<SearchPreviewInput, ServerFnError> {
    Ok(SearchPreviewInput {
        query: normalize_query(&input.query)?,
        locale: normalize_locale(input.locale.as_deref())?,
        channel_id: input.channel_id,
        tenant_id: input.tenant_id,
        limit: input.limit,
        offset: input.offset,
        ranking_profile: normalize_ranking_profile(input.ranking_profile)?,
        preset_key: normalize_preset_key(input.preset_key)?,
        entity_types: Some(normalize_filter_values("entity_types", input.entity_types)?),
        source_modules: Some(normalize_filter_values(
            "source_modules",
            input.source_modules,
        )?),
        statuses: Some(normalize_filter_values("statuses", input.statuses)?),
        category_ids: input.category_ids,
        attribute_filters: input.attribute_filters,
        sort_attribute_code: input.sort_attribute_code,
        sort_desc: input.sort_desc,
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
fn normalize_attribute_filters(
    filters: Option<Vec<SearchAttributeFilterInput>>,
) -> Result<Vec<rustok_search::SearchAttributeFilter>, ServerFnError> {
    let filters = filters.unwrap_or_default();
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
            let values = normalize_filter_values("attribute_filter.values", filter.values)?;
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
fn normalize_ranking_profile(value: Option<String>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    rustok_search::SearchRankingProfile::try_from_str(&value)
        .map(|_| Some(value))
        .ok_or_else(|| ServerFnError::new("Unsupported ranking profile"))
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
fn normalize_surface(value: &str) -> Result<String, ServerFnError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(ServerFnError::new("Invalid search surface"));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(ServerFnError::new("Invalid search surface"));
    }
    Ok(normalized)
}

#[cfg(feature = "ssr")]
fn resolve_preset_and_ranking(
    config: &serde_json::Value,
    surface: &str,
    preset_key: Option<&str>,
    requested_ranking_profile: Option<&str>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
) -> Result<ResolvedSearchInput, ServerFnError> {
    let resolved_preset = rustok_search::SearchFilterPresetService::resolve(
        config,
        surface,
        preset_key,
        entity_types,
        source_modules,
        statuses,
    )
    .map_err(map_core_error)?;
    let ranking_profile = rustok_search::SearchRankingProfile::resolve(
        config,
        surface,
        requested_ranking_profile,
        resolved_preset.ranking_profile,
    )
    .map_err(map_core_error)?;

    Ok(ResolvedSearchInput {
        preset_key: resolved_preset.preset.map(|preset| preset.key),
        entity_types: resolved_preset.entity_types,
        source_modules: resolved_preset.source_modules,
        statuses: resolved_preset.statuses,
        ranking_profile,
    })
}

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

#[cfg(feature = "ssr")]
fn map_search_preview_payload(
    value: rustok_search::SearchResult,
    preset_key: Option<String>,
    query_log_id: Option<i64>,
) -> SearchPreviewPayload {
    SearchPreviewPayload {
        query_log_id: query_log_id.map(|value| value.to_string()),
        preset_key,
        items: value
            .items
            .into_iter()
            .map(|item| {
                let url = derive_search_result_url(&item);
                crate::model::SearchPreviewResultItem {
                    id: item.id.to_string(),
                    entity_type: item.entity_type,
                    source_module: item.source_module,
                    title: item.title,
                    snippet: item.snippet,
                    score: item.score,
                    locale: item.locale,
                    url,
                    payload: item.payload.to_string(),
                }
            })
            .collect(),
        total: value.total,
        took_ms: value.took_ms,
        engine: value.engine.as_str().to_string(),
        ranking_profile: value.ranking_profile.as_str().to_string(),
        facets: value
            .facets
            .into_iter()
            .map(|facet| crate::model::SearchFacetGroup {
                name: facet.name,
                buckets: facet
                    .buckets
                    .into_iter()
                    .map(|bucket| crate::model::SearchFacetBucket {
                        value: bucket.value,
                        label: bucket.label,
                        count: bucket.count,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_diagnostics_payload(
    value: rustok_search::SearchDiagnosticsSnapshot,
) -> crate::model::SearchDiagnosticsPayload {
    crate::model::SearchDiagnosticsPayload {
        tenant_id: value.tenant_id.to_string(),
        total_documents: value.total_documents,
        public_documents: value.public_documents,
        content_documents: value.content_documents,
        product_documents: value.product_documents,
        stale_documents: value.stale_documents,
        missing_documents: value.missing_documents,
        orphaned_documents: value.orphaned_documents,
        newest_indexed_at: value.newest_indexed_at.map(|value| value.to_rfc3339()),
        oldest_indexed_at: value.oldest_indexed_at.map(|value| value.to_rfc3339()),
        max_lag_seconds: value.max_lag_seconds,
        state: value.state,
    }
}

#[cfg(feature = "ssr")]
fn map_lagging_documents(
    rows: Vec<rustok_search::LaggingSearchDocument>,
) -> Vec<LaggingSearchDocumentPayload> {
    rows.into_iter()
        .map(|value| LaggingSearchDocumentPayload {
            document_key: value.document_key,
            document_id: value.document_id.to_string(),
            source_module: value.source_module,
            entity_type: value.entity_type,
            locale: value.locale,
            status: value.status,
            is_public: value.is_public,
            title: value.title,
            updated_at: value.updated_at.to_rfc3339(),
            indexed_at: value.indexed_at.to_rfc3339(),
            lag_seconds: value.lag_seconds,
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn map_consistency_issues(
    rows: Vec<rustok_search::SearchConsistencyIssue>,
) -> Vec<SearchConsistencyIssuePayload> {
    rows.into_iter()
        .map(|value| SearchConsistencyIssuePayload {
            issue_kind: value.issue_kind,
            document_key: value.document_key,
            document_id: value.document_id.to_string(),
            source_module: value.source_module,
            entity_type: value.entity_type,
            locale: value.locale,
            status: value.status,
            title: value.title,
            updated_at: value.updated_at.to_rfc3339(),
            indexed_at: value.indexed_at.map(|value| value.to_rfc3339()),
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn map_analytics_payload(value: rustok_search::SearchAnalyticsSnapshot) -> SearchAnalyticsPayload {
    SearchAnalyticsPayload {
        summary: crate::model::SearchAnalyticsSummaryPayload {
            window_days: value.summary.window_days,
            total_queries: value.summary.total_queries,
            successful_queries: value.summary.successful_queries,
            zero_result_queries: value.summary.zero_result_queries,
            zero_result_rate: value.summary.zero_result_rate,
            slow_queries: value.summary.slow_queries,
            slow_query_rate: value.summary.slow_query_rate,
            avg_took_ms: value.summary.avg_took_ms,
            avg_results_per_query: value.summary.avg_results_per_query,
            unique_queries: value.summary.unique_queries,
            clicked_queries: value.summary.clicked_queries,
            total_clicks: value.summary.total_clicks,
            click_through_rate: value.summary.click_through_rate,
            abandonment_queries: value.summary.abandonment_queries,
            abandonment_rate: value.summary.abandonment_rate,
            last_query_at: value.summary.last_query_at.map(|value| value.to_rfc3339()),
        },
        top_queries: map_analytics_rows(value.top_queries),
        zero_result_queries: map_analytics_rows(value.zero_result_queries),
        slow_queries: map_analytics_rows(value.slow_queries),
        low_ctr_queries: map_analytics_rows(value.low_ctr_queries),
        abandonment_queries: map_analytics_rows(value.abandonment_queries),
        intelligence_candidates: value
            .intelligence_candidates
            .into_iter()
            .map(|value| crate::model::SearchAnalyticsInsightRowPayload {
                query: value.query,
                hits: value.hits,
                zero_result_hits: value.zero_result_hits,
                clicks: value.clicks,
                click_through_rate: value.click_through_rate,
                abandonment_rate: value.abandonment_rate,
                recommendation: value.recommendation,
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_analytics_rows(
    rows: Vec<rustok_search::SearchAnalyticsQueryRow>,
) -> Vec<crate::model::SearchAnalyticsQueryRowPayload> {
    rows.into_iter()
        .map(|value| crate::model::SearchAnalyticsQueryRowPayload {
            query: value.query,
            hits: value.hits,
            zero_result_hits: value.zero_result_hits,
            clicks: value.clicks,
            avg_took_ms: value.avg_took_ms,
            avg_results: value.avg_results,
            click_through_rate: value.click_through_rate,
            abandonment_rate: value.abandonment_rate,
            last_seen_at: value.last_seen_at.to_rfc3339(),
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn map_dictionary_snapshot(
    value: rustok_search::SearchDictionarySnapshot,
) -> SearchDictionarySnapshotPayload {
    SearchDictionarySnapshotPayload {
        synonyms: value
            .synonyms
            .into_iter()
            .map(|value| crate::model::SearchSynonymPayload {
                id: value.id.to_string(),
                term: value.term,
                synonyms: value.synonyms,
                updated_at: value.updated_at.to_rfc3339(),
            })
            .collect(),
        stop_words: value
            .stop_words
            .into_iter()
            .map(|value| crate::model::SearchStopWordPayload {
                id: value.id.to_string(),
                value: value.value,
                updated_at: value.updated_at.to_rfc3339(),
            })
            .collect(),
        query_rules: value
            .query_rules
            .into_iter()
            .map(|value| crate::model::SearchQueryRulePayload {
                id: value.id.to_string(),
                query_text: value.query_text,
                query_normalized: value.query_normalized,
                rule_kind: value.rule_kind,
                document_id: value.document_id.to_string(),
                entity_type: value.entity_type,
                source_module: value.source_module,
                title: value.title,
                pinned_position: value.pinned_position,
                updated_at: value.updated_at.to_rfc3339(),
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_dictionary_mutation_payload(success: bool) -> SearchDictionaryMutationPayload {
    SearchDictionaryMutationPayload { success }
}

#[cfg(feature = "ssr")]
fn derive_search_result_url(value: &rustok_search::SearchResultItem) -> Option<String> {
    match value.entity_type.as_str() {
        "product" => Some(format!("/store/products/{}", value.id)),
        "node" => Some(format!(
            "/modules/content?id={}{}",
            value.id,
            if value.source_module.is_empty() || value.source_module == "content" {
                String::new()
            } else {
                format!("&kind={}", value.source_module)
            }
        )),
        _ => None,
    }
}

#[cfg(feature = "ssr")]
fn map_core_error(error: rustok_core::Error) -> ServerFnError {
    ServerFnError::new(error.to_string())
}

#[cfg(feature = "ssr")]
fn normalize_analytics_days(value: Option<i32>) -> u32 {
    value.unwrap_or(7).clamp(1, 30) as u32
}

#[cfg(feature = "ssr")]
fn normalize_analytics_limit(value: Option<i32>) -> usize {
    value.unwrap_or(10).clamp(1, 25) as usize
}

#[cfg(feature = "ssr")]
fn normalize_limit(value: Option<i32>, default: i32, max: i32) -> usize {
    value.unwrap_or(default).clamp(1, max) as usize
}

#[cfg(feature = "ssr")]
fn ensure_settings_read_permission(
    permissions: &[rustok_api::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_effective_permission(permissions, &rustok_api::Permission::SETTINGS_READ) {
        return Err(ServerFnError::new("settings:read required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_settings_manage_permission(
    permissions: &[rustok_api::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_effective_permission(permissions, &rustok_api::Permission::SETTINGS_MANAGE)
    {
        return Err(ServerFnError::new("settings:manage required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_required_uuid(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(value: Option<&str>) -> Result<Option<uuid::Uuid>, ServerFnError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new("Invalid UUID"))
        })
        .transpose()
}

#[cfg(feature = "ssr")]
fn parse_engine(
    value: &str,
    field_name: &str,
) -> Result<rustok_search::SearchEngineKind, ServerFnError> {
    rustok_search::SearchEngineKind::try_from_str(value)
        .ok_or_else(|| ServerFnError::new(format!("Invalid {field_name}: unsupported engine")))
}

#[cfg(feature = "ssr")]
fn ensure_engine_available(engine: rustok_search::SearchEngineKind) -> Result<(), ServerFnError> {
    let module = rustok_search::SearchModule;
    if module
        .available_engines()
        .into_iter()
        .any(|descriptor| descriptor.enabled && descriptor.kind == engine)
    {
        Ok(())
    } else {
        Err(ServerFnError::new(format!(
            "Engine '{}' is not installed in the current runtime",
            engine.as_str()
        )))
    }
}
