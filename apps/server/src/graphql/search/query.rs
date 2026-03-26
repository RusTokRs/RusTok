use async_graphql::{Context, FieldError, Object, Result};
use axum::http::HeaderMap;
use loco_rs::app::AppContext;
use std::time::Instant;
use uuid::Uuid;

use crate::common::RequestContext;
use crate::context::{AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::middleware::rate_limit::{
    extract_client_id_pub, RateLimitCheckError, SharedSearchRateLimiter,
};
use crate::services::rbac_service::RbacService;
use rustok_search::{
    PgSearchEngine, SearchAnalyticsService, SearchDiagnosticsService, SearchDictionaryService,
    SearchEngine, SearchFilterPresetService, SearchModule, SearchQuery, SearchQueryLogRecord,
    SearchRankingProfile, SearchSettingsService, SearchSuggestionQuery, SearchSuggestionService,
    SLOW_QUERY_THRESHOLD_MS,
};
use rustok_telemetry::metrics;

use super::types::{
    LaggingSearchDocumentPayload, SearchAnalyticsPayload, SearchConsistencyIssuePayload,
    SearchDiagnosticsPayload, SearchDictionarySnapshotPayload, SearchEngineDescriptor,
    SearchFilterPresetPayload, SearchFilterPresetsInput, SearchPreviewInput, SearchPreviewPayload,
    SearchSettingsPayload, SearchSuggestionPayload, SearchSuggestionsInput,
};

#[derive(Default)]
pub struct SearchQueryRoot;

const MAX_SEARCH_QUERY_LEN: usize = 256;
const MAX_FILTER_VALUES: usize = 10;
const MAX_FILTER_VALUE_LEN: usize = 64;
const MAX_LOCALE_LEN: usize = 16;
const DEFAULT_ANALYTICS_WINDOW_DAYS: u32 = 7;
const DEFAULT_ANALYTICS_LIMIT: usize = 10;
const DEFAULT_SUGGESTIONS_LIMIT: usize = 6;
const MAX_SUGGESTIONS_LIMIT: usize = 10;

#[Object]
impl SearchQueryRoot {
    /// Returns the list of search engines available in the current runtime.
    /// External engines appear only when their connector crates are installed.
    async fn available_search_engines(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<SearchEngineDescriptor>> {
        ensure_settings_read_permission(ctx).await?;

        let module = SearchModule;
        Ok(module
            .available_engines()
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Returns the effective search settings for the current tenant.
    /// This is the first search GraphQL surface and is intentionally read-only.
    async fn search_settings_preview(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
    ) -> Result<SearchSettingsPayload> {
        ensure_settings_read_permission(ctx).await?;

        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = Some(resolve_tenant_scope(tenant, tenant_id)?);

        let settings = SearchSettingsService::load_effective(&app_ctx.db, tenant_id)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(settings.into())
    }

    /// Returns diagnostics for the current tenant search storage and lag state.
    async fn search_diagnostics(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
    ) -> Result<SearchDiagnosticsPayload> {
        ensure_settings_read_permission(ctx).await?;

        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let snapshot = SearchDiagnosticsService::snapshot(&app_ctx.db, tenant_id)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(snapshot.into())
    }

    /// Returns the latest lagging search documents for diagnostics/debugging in admin.
    async fn search_lagging_documents(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        limit: Option<i32>,
    ) -> Result<Vec<LaggingSearchDocumentPayload>> {
        ensure_settings_read_permission(ctx).await?;

        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let rows = SearchDiagnosticsService::lagging_documents(
            &app_ctx.db,
            tenant_id,
            limit.unwrap_or(25).clamp(1, 100) as usize,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Returns missing/orphaned projection records for the current tenant.
    async fn search_consistency_issues(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        limit: Option<i32>,
    ) -> Result<Vec<SearchConsistencyIssuePayload>> {
        ensure_settings_read_permission(ctx).await?;

        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let rows = SearchDiagnosticsService::consistency_issues(
            &app_ctx.db,
            tenant_id,
            limit.unwrap_or(25).clamp(1, 100) as usize,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Returns aggregated search analytics for the current tenant.
    async fn search_analytics(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        days: Option<i32>,
        limit: Option<i32>,
    ) -> Result<SearchAnalyticsPayload> {
        ensure_settings_read_permission(ctx).await?;

        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let days = normalize_analytics_days(days);
        let limit = normalize_analytics_limit(limit);

        let snapshot = SearchAnalyticsService::snapshot(&app_ctx.db, tenant_id, days, limit)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(snapshot.into())
    }

    /// Returns the current tenant-owned search dictionaries and query rules.
    async fn search_dictionary_snapshot(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
    ) -> Result<SearchDictionarySnapshotPayload> {
        ensure_settings_read_permission(ctx).await?;

        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let snapshot = SearchDictionaryService::snapshot(&app_ctx.db, tenant_id)
            .await
            .map_err(map_search_module_error)?;

        Ok(snapshot.into())
    }

    /// Returns tenant-local filter presets configured for a given search surface.
    async fn search_filter_presets(
        &self,
        ctx: &Context<'_>,
        input: SearchFilterPresetsInput,
    ) -> Result<Vec<SearchFilterPresetPayload>> {
        ensure_settings_read_permission(ctx).await?;

        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id =
            resolve_tenant_scope(tenant, parse_optional_uuid(input.tenant_id.as_deref())?)?;
        let surface = normalize_surface(&input.surface)?;
        let settings = SearchSettingsService::load_effective(&app_ctx.db, Some(tenant_id))
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(SearchFilterPresetService::list(&settings.config, &surface)
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Executes a PostgreSQL-backed search preview over rustok-search owned search documents.
    async fn search_preview(
        &self,
        ctx: &Context<'_>,
        input: SearchPreviewInput,
    ) -> Result<SearchPreviewPayload> {
        ensure_settings_read_permission(ctx).await?;

        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let input = normalize_search_preview_input(input)?;
        let requested_limit = input.limit;
        let effective_limit = requested_limit.unwrap_or(10).clamp(1, 50) as usize;
        let tenant_id =
            resolve_tenant_scope(tenant, parse_optional_uuid(input.tenant_id.as_deref())?)?;
        let transform =
            SearchDictionaryService::transform_query(&app_ctx.db, tenant_id, &input.query)
                .await
                .map_err(map_search_module_error)?;
        let settings = SearchSettingsService::load_effective(&app_ctx.db, Some(tenant_id))
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let resolved = resolve_preset_and_ranking(
            &settings.config,
            "search_preview",
            input.preset_key.as_deref(),
            input.ranking_profile.as_deref(),
            input.entity_types.unwrap_or_default(),
            input.source_modules.unwrap_or_default(),
            input.statuses.unwrap_or_default(),
        )?;
        let engine = PgSearchEngine::new(app_ctx.db.clone());
        let started_at = Instant::now();
        let search_query = SearchQuery {
            tenant_id: Some(tenant_id),
            locale: input.locale,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile: resolved.ranking_profile,
            preset_key: resolved.preset_key,
            limit: effective_limit,
            offset: input.offset.unwrap_or(0).max(0) as usize,
            published_only: false,
            entity_types: resolved.entity_types,
            source_modules: resolved.source_modules,
            statuses: resolved.statuses,
        };
        let result = run_search_with_dictionaries(&app_ctx.db, &engine, search_query.clone()).await;
        finalize_search_result(
            &app_ctx.db,
            "search_preview",
            &search_query,
            requested_limit,
            effective_limit,
            started_at,
            result,
        )
        .await
    }

    /// Executes host-level admin search for global navigation and quick-open flows.
    async fn admin_global_search(
        &self,
        ctx: &Context<'_>,
        input: SearchPreviewInput,
    ) -> Result<SearchPreviewPayload> {
        ensure_settings_read_permission(ctx).await?;

        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let input = normalize_search_preview_input(input)?;
        let requested_limit = input.limit;
        let effective_limit = requested_limit.unwrap_or(8).clamp(1, 20) as usize;
        let tenant_id =
            resolve_tenant_scope(tenant, parse_optional_uuid(input.tenant_id.as_deref())?)?;
        let transform =
            SearchDictionaryService::transform_query(&app_ctx.db, tenant_id, &input.query)
                .await
                .map_err(map_search_module_error)?;
        let settings = SearchSettingsService::load_effective(&app_ctx.db, Some(tenant_id))
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let resolved = resolve_preset_and_ranking(
            &settings.config,
            "admin_global_search",
            input.preset_key.as_deref(),
            input.ranking_profile.as_deref(),
            input.entity_types.unwrap_or_default(),
            input.source_modules.unwrap_or_default(),
            input.statuses.unwrap_or_default(),
        )?;
        let engine = PgSearchEngine::new(app_ctx.db.clone());
        let started_at = Instant::now();
        let search_query = SearchQuery {
            tenant_id: Some(tenant_id),
            locale: input.locale,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile: resolved.ranking_profile,
            preset_key: resolved.preset_key,
            limit: effective_limit,
            offset: input.offset.unwrap_or(0).max(0) as usize,
            published_only: false,
            entity_types: resolved.entity_types,
            source_modules: resolved.source_modules,
            statuses: resolved.statuses,
        };
        let result = run_search_with_dictionaries(&app_ctx.db, &engine, search_query.clone()).await;
        finalize_search_result(
            &app_ctx.db,
            "admin_global_search",
            &search_query,
            requested_limit,
            effective_limit,
            started_at,
            result,
        )
        .await
    }

    /// Executes public storefront search over published content and published products only.
    async fn storefront_search(
        &self,
        ctx: &Context<'_>,
        input: SearchPreviewInput,
    ) -> Result<SearchPreviewPayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let input = normalize_search_preview_input(input)?;
        enforce_storefront_rate_limit(ctx, "storefront_search").await?;
        let engine = PgSearchEngine::new(app_ctx.db.clone());
        let requested_limit = input.limit;
        let effective_limit = requested_limit.unwrap_or(12).clamp(1, 50) as usize;
        let started_at = Instant::now();
        let transform =
            SearchDictionaryService::transform_query(&app_ctx.db, tenant.id, &input.query)
                .await
                .map_err(map_search_module_error)?;
        let settings = SearchSettingsService::load_effective(&app_ctx.db, Some(tenant.id))
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let resolved = resolve_preset_and_ranking(
            &settings.config,
            "storefront_search",
            input.preset_key.as_deref(),
            input.ranking_profile.as_deref(),
            input.entity_types.unwrap_or_default(),
            input.source_modules.unwrap_or_default(),
            input.statuses.unwrap_or_default(),
        )?;

        let search_query = SearchQuery {
            tenant_id: Some(tenant.id),
            locale: input.locale,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile: resolved.ranking_profile,
            preset_key: resolved.preset_key,
            limit: effective_limit,
            offset: input.offset.unwrap_or(0).max(0) as usize,
            published_only: true,
            entity_types: resolved.entity_types,
            source_modules: resolved.source_modules,
            statuses: resolved.statuses,
        };

        let result = run_search_with_dictionaries(&app_ctx.db, &engine, search_query.clone()).await;
        finalize_search_result(
            &app_ctx.db,
            "storefront_search",
            &search_query,
            requested_limit,
            effective_limit,
            started_at,
            result,
        )
        .await
    }

    /// Returns public storefront suggestions and autocomplete candidates.
    async fn storefront_search_suggestions(
        &self,
        ctx: &Context<'_>,
        input: SearchSuggestionsInput,
    ) -> Result<Vec<SearchSuggestionPayload>> {
        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let input = normalize_search_suggestions_input(input)?;
        enforce_storefront_rate_limit(ctx, "storefront_search_suggestions").await?;
        let tenant_id =
            resolve_tenant_scope(tenant, parse_optional_uuid(input.tenant_id.as_deref())?)?;

        if input.query.is_empty() {
            return Ok(Vec::new());
        }

        let suggestions = SearchSuggestionService::suggestions(
            &app_ctx.db,
            SearchSuggestionQuery {
                tenant_id,
                query: input.query,
                locale: input.locale,
                limit: normalize_suggestions_limit(input.limit),
                published_only: true,
            },
        )
        .await
        .map_err(map_search_module_error)?;

        Ok(suggestions.into_iter().map(Into::into).collect())
    }

    /// Returns public storefront filter presets configured for storefront search.
    async fn storefront_search_filter_presets(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<SearchFilterPresetPayload>> {
        let app_ctx = ctx.data::<AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let settings = SearchSettingsService::load_effective(&app_ctx.db, Some(tenant.id))
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(
            SearchFilterPresetService::list(&settings.config, "storefront_search")
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    }
}

async fn ensure_settings_read_permission(ctx: &Context<'_>) -> Result<()> {
    let app_ctx = ctx.data::<AppContext>()?;
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<TenantContext>()?;

    let can_read = RbacService::has_permission(
        &app_ctx.db,
        &tenant.id,
        &auth.user_id,
        &rustok_core::Permission::SETTINGS_READ,
    )
    .await
    .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

    if !can_read {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "settings:read required",
        ));
    }

    Ok(())
}

fn parse_optional_uuid(value: Option<&str>) -> Result<Option<Uuid>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| Uuid::parse_str(value).map_err(|_| FieldError::new("Invalid UUID")))
        .transpose()
}

fn resolve_tenant_scope(tenant: &TenantContext, requested_tenant_id: Option<Uuid>) -> Result<Uuid> {
    match requested_tenant_id {
        Some(requested_tenant_id) if requested_tenant_id != tenant.id => {
            Err(<FieldError as GraphQLError>::permission_denied(
                "cross-tenant search access is not allowed",
            ))
        }
        _ => Ok(tenant.id),
    }
}

fn normalize_search_preview_input(input: SearchPreviewInput) -> Result<SearchPreviewInput> {
    let query = normalize_query(&input.query)?;
    let locale = normalize_locale(input.locale.as_deref())?;
    let entity_types = normalize_filter_values("entity_types", input.entity_types)?;
    let source_modules = normalize_filter_values("source_modules", input.source_modules)?;
    let statuses = normalize_filter_values("statuses", input.statuses)?;

    Ok(SearchPreviewInput {
        query,
        locale,
        tenant_id: input.tenant_id,
        limit: input.limit,
        offset: input.offset,
        ranking_profile: normalize_ranking_profile(input.ranking_profile)?,
        preset_key: normalize_preset_key(input.preset_key)?,
        entity_types: Some(entity_types),
        source_modules: Some(source_modules),
        statuses: Some(statuses),
    })
}

fn normalize_search_suggestions_input(
    input: SearchSuggestionsInput,
) -> Result<SearchSuggestionsInput> {
    Ok(SearchSuggestionsInput {
        query: normalize_query(&input.query)?,
        locale: normalize_locale(input.locale.as_deref())?,
        tenant_id: input.tenant_id,
        limit: input.limit,
    })
}

fn normalize_query(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.len() > MAX_SEARCH_QUERY_LEN {
        return Err(FieldError::new(format!(
            "Search query exceeds the maximum length of {MAX_SEARCH_QUERY_LEN} characters"
        )));
    }

    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err(FieldError::new(
            "Search query contains unsupported control characters",
        ));
    }

    Ok(trimmed.to_string())
}

fn normalize_locale(value: Option<&str>) -> Result<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if value.len() > MAX_LOCALE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(FieldError::new("Invalid locale format"));
    }

    Ok(Some(value.to_ascii_lowercase()))
}

fn normalize_filter_values(field_name: &str, values: Option<Vec<String>>) -> Result<Vec<String>> {
    let values = values.unwrap_or_default();
    if values.len() > MAX_FILTER_VALUES {
        return Err(FieldError::new(format!(
            "{field_name} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        )));
    }

    values
        .into_iter()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                return Err(FieldError::new(format!(
                    "{field_name} contains an empty value"
                )));
            }
            if normalized.len() > MAX_FILTER_VALUE_LEN
                || !normalized
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
            {
                return Err(FieldError::new(format!(
                    "{field_name} contains an invalid value"
                )));
            }
            Ok(normalized)
        })
        .collect()
}

fn normalize_analytics_days(value: Option<i32>) -> u32 {
    value
        .unwrap_or(DEFAULT_ANALYTICS_WINDOW_DAYS as i32)
        .clamp(1, 30) as u32
}

fn normalize_analytics_limit(value: Option<i32>) -> usize {
    value.unwrap_or(DEFAULT_ANALYTICS_LIMIT as i32).clamp(1, 25) as usize
}

fn normalize_suggestions_limit(value: Option<i32>) -> usize {
    value
        .unwrap_or(DEFAULT_SUGGESTIONS_LIMIT as i32)
        .clamp(1, MAX_SUGGESTIONS_LIMIT as i32) as usize
}

fn normalize_ranking_profile(value: Option<String>) -> Result<Option<String>> {
    let Some(value) = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    SearchRankingProfile::try_from_str(&value)
        .map(|_| Some(value))
        .ok_or_else(|| FieldError::new("Unsupported ranking profile"))
}

fn normalize_preset_key(value: Option<String>) -> Result<Option<String>> {
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
        return Err(FieldError::new("Invalid preset key"));
    }

    Ok(Some(value))
}

fn resolve_preset_and_ranking(
    config: &serde_json::Value,
    surface: &str,
    preset_key: Option<&str>,
    requested_ranking_profile: Option<&str>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
) -> Result<ResolvedSearchInput> {
    let resolved_preset = SearchFilterPresetService::resolve(
        config,
        surface,
        preset_key,
        entity_types,
        source_modules,
        statuses,
    )
    .map_err(map_search_module_error)?;
    let ranking_profile = SearchRankingProfile::resolve(
        config,
        surface,
        requested_ranking_profile,
        resolved_preset.ranking_profile,
    )
    .map_err(map_search_module_error)?;

    Ok(ResolvedSearchInput {
        preset_key: resolved_preset.preset.map(|preset| preset.key),
        entity_types: resolved_preset.entity_types,
        source_modules: resolved_preset.source_modules,
        statuses: resolved_preset.statuses,
        ranking_profile,
    })
}

fn normalize_surface(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(FieldError::new("Invalid search surface"));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(FieldError::new("Invalid search surface"));
    }
    Ok(normalized)
}

struct ResolvedSearchInput {
    preset_key: Option<String>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
    ranking_profile: SearchRankingProfile,
}

async fn enforce_storefront_rate_limit(ctx: &Context<'_>, surface: &'static str) -> Result<()> {
    let app_ctx = ctx.data::<AppContext>()?;
    let Some(shared) = app_ctx.shared_store.get::<SharedSearchRateLimiter>() else {
        return Ok(());
    };

    let tenant = ctx.data::<TenantContext>()?;
    let request_context = ctx.data::<RequestContext>()?;
    let auth = ctx.data_opt::<AuthContext>();
    let headers = ctx.data_opt::<HeaderMap>();
    let rate_limit_key =
        build_storefront_rate_limit_key(tenant, request_context, auth, headers, surface);

    match shared.0.check_rate_limit(&rate_limit_key).await {
        Ok(_) => {
            metrics::record_search_rate_limit_outcome(surface, shared.0.namespace(), "allowed");
            Ok(())
        }
        Err(RateLimitCheckError::Exceeded(exceeded)) => {
            metrics::record_rate_limit_exceeded(shared.0.namespace());
            metrics::record_search_rate_limit_outcome(surface, shared.0.namespace(), "exceeded");
            Err(FieldError::new(format!(
                "Search rate limit exceeded. Retry after {} seconds",
                exceeded.retry_after
            )))
        }
        Err(RateLimitCheckError::BackendUnavailable(reason)) => {
            metrics::record_rate_limit_backend_unavailable(shared.0.namespace());
            metrics::record_search_rate_limit_outcome(
                surface,
                shared.0.namespace(),
                "backend_unavailable",
            );
            tracing::error!(
                surface,
                tenant_id = %tenant.id,
                %reason,
                "Storefront search rate limit backend unavailable"
            );
            Err(<FieldError as GraphQLError>::internal_error(
                "Search rate limit backend unavailable",
            ))
        }
    }
}

fn build_storefront_rate_limit_key(
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: Option<&AuthContext>,
    headers: Option<&HeaderMap>,
    surface: &str,
) -> String {
    let client_key = headers
        .map(extract_client_id_pub)
        .filter(|value| value != "ip:unknown")
        .or_else(|| {
            auth.map(|auth| format!("user:{}", auth.user_id))
                .or_else(|| {
                    request_context
                        .user_id
                        .map(|user_id| format!("user:{user_id}"))
                })
        })
        .unwrap_or_else(|| "anonymous".to_string());

    format!("tenant:{}:{surface}:{client_key}", tenant.id)
}

async fn finalize_search_result(
    db: &sea_orm::DatabaseConnection,
    surface: &str,
    search_query: &SearchQuery,
    requested_limit: Option<i32>,
    effective_limit: usize,
    started_at: Instant,
    result: rustok_core::Result<rustok_search::SearchResult>,
) -> Result<SearchPreviewPayload> {
    let duration = started_at.elapsed();

    match result {
        Ok(result) => {
            metrics::record_search_query(
                surface,
                result.engine.as_str(),
                "success",
                duration.as_secs_f64(),
                result.items.len() as u64,
            );
            if result.took_ms >= SLOW_QUERY_THRESHOLD_MS {
                metrics::record_search_slow_query(surface, result.engine.as_str());
            }
            metrics::record_read_path_budget(
                "graphql",
                surface,
                requested_limit.map(|value| value.max(0) as u64),
                effective_limit as u64,
                result.items.len(),
            );
            metrics::record_read_path_query(
                "graphql",
                surface,
                "fts_search",
                result.took_ms as f64 / 1000.0,
                result.total,
            );
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
            let mut payload: SearchPreviewPayload = result.into();
            payload.query_log_id = query_log_id.map(|value| value.to_string());
            payload.preset_key = search_query.preset_key.clone();
            Ok(payload)
        }
        Err(error) => {
            let error_type = classify_search_error(&error);
            metrics::record_search_query(
                surface,
                "postgres",
                error_type,
                duration.as_secs_f64(),
                0,
            );
            metrics::record_module_error("search", error_type, "error");
            let _ = record_search_query_log(
                db,
                surface,
                search_query,
                "postgres",
                0,
                duration.as_millis() as u64,
                error_type,
            )
            .await;

            Err(<FieldError as GraphQLError>::internal_error(
                &error.to_string(),
            ))
        }
    }
}

async fn run_search_with_dictionaries(
    db: &sea_orm::DatabaseConnection,
    engine: &PgSearchEngine,
    search_query: SearchQuery,
) -> rustok_core::Result<rustok_search::SearchResult> {
    let result = engine.search(search_query.clone()).await?;
    SearchDictionaryService::apply_query_rules(db, &search_query, result).await
}

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

async fn record_search_query_log(
    db: &sea_orm::DatabaseConnection,
    surface: &str,
    search_query: &SearchQuery,
    engine: &str,
    result_count: u64,
    took_ms: u64,
    status: &str,
) -> Option<i64> {
    let Some(tenant_id) = search_query.tenant_id else {
        return None;
    };

    let Some(engine_kind) = rustok_search::SearchEngineKind::try_from_str(engine) else {
        return None;
    };

    let record = SearchQueryLogRecord {
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
    };

    match SearchAnalyticsService::record_query(db, record).await {
        Ok(log_id) => log_id,
        Err(error) => {
            metrics::record_module_error("search", classify_search_error(&error), "warning");
            tracing::warn!(
                surface,
                tenant_id = %tenant_id,
                error = %error,
                "Failed to persist search analytics query log"
            );
            None
        }
    }
}

fn map_search_module_error(error: rustok_core::Error) -> FieldError {
    match error {
        rustok_core::Error::Validation(message)
        | rustok_core::Error::NotFound(message)
        | rustok_core::Error::InvalidIdFormat(message) => FieldError::new(message),
        other => <FieldError as GraphQLError>::internal_error(&other.to_string()),
    }
}
