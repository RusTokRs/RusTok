use async_graphql::{Context, FieldError, Object, Result};
use axum::http::HeaderMap;
use sea_orm::DatabaseConnection;
use std::time::Instant;
use uuid::Uuid;

use crate::{
    PgSearchEngine, SLOW_QUERY_THRESHOLD_MS, SearchAnalyticsService, SearchAttributeFilter,
    SearchDiagnosticsService, SearchDictionaryService, SearchEngine, SearchFilterPresetService,
    SearchModule, SearchQuery, SearchQueryLogRecord, SearchRankingProfile, SearchSettingsService,
    SearchSuggestionQuery, SearchSuggestionService,
};
use rustok_api::{
    AuthContext, Permission, RequestContext, TenantContext, graphql::GraphQLError,
    has_effective_permission,
};
use rustok_telemetry::metrics;

use super::types::{
    LaggingSearchDocumentPayload, SearchAnalyticsPayload, SearchConsistencyIssuePayload,
    SearchDiagnosticsPayload, SearchDictionarySnapshotPayload, SearchEngineDescriptor,
    SearchFilterPresetPayload, SearchFilterPresetsInput, SearchPreviewInput, SearchPreviewPayload,
    SearchSettingsPayload, SearchSuggestionPayload, SearchSuggestionsInput,
};
use super::{SearchGraphqlRateLimitError, SearchGraphqlRateLimiterHandle};

#[derive(Default)]
pub struct SearchQueryRoot;

const MAX_SEARCH_QUERY_LEN: usize = 256;
const MAX_FILTER_VALUES: usize = 10;
const MAX_FILTER_VALUE_LEN: usize = 64;
const MAX_ATTRIBUTE_FILTERS: usize = 10;
const MAX_LOCALE_LEN: usize = 16;
const DEFAULT_ANALYTICS_WINDOW_DAYS: u32 = 7;
const DEFAULT_ANALYTICS_LIMIT: usize = 10;
const DEFAULT_SUGGESTIONS_LIMIT: usize = 6;
const MAX_SUGGESTIONS_LIMIT: usize = 10;
const SEARCH_PREVIEW_SURFACE: &str = "search_preview";
const ADMIN_GLOBAL_SEARCH_SURFACE: &str = "admin_global_search";
const STOREFRONT_SEARCH_SURFACE: &str = "storefront_search";
const STOREFRONT_SUGGESTIONS_SURFACE: &str = "storefront_search_suggestions";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SearchSurfacePolicy {
    surface: &'static str,
    default_limit: usize,
    max_limit: usize,
    published_only: bool,
    requires_settings_read: bool,
    allows_tenant_override: bool,
}

#[derive(Debug, Clone)]
struct NormalizedSearchPreviewInput {
    query: String,
    locale: Option<String>,
    channel_id: Option<Uuid>,
    tenant_id: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
    ranking_profile: Option<String>,
    preset_key: Option<String>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
    category_ids: Vec<Uuid>,
    attribute_filters: Vec<SearchAttributeFilter>,
    sort_attribute_code: Option<String>,
    sort_desc: bool,
}

impl SearchSurfacePolicy {
    const fn search_preview() -> Self {
        Self {
            surface: SEARCH_PREVIEW_SURFACE,
            default_limit: 10,
            max_limit: 50,
            published_only: false,
            requires_settings_read: true,
            allows_tenant_override: true,
        }
    }

    const fn admin_global_search() -> Self {
        Self {
            surface: ADMIN_GLOBAL_SEARCH_SURFACE,
            default_limit: 8,
            max_limit: 20,
            published_only: false,
            requires_settings_read: true,
            allows_tenant_override: true,
        }
    }

    const fn storefront_search() -> Self {
        Self {
            surface: STOREFRONT_SEARCH_SURFACE,
            default_limit: 12,
            max_limit: 50,
            published_only: true,
            requires_settings_read: false,
            allows_tenant_override: false,
        }
    }

    fn effective_limit(self, requested_limit: Option<i32>) -> usize {
        requested_limit
            .unwrap_or(self.default_limit as i32)
            .clamp(1, self.max_limit as i32) as usize
    }

    fn offset(self, requested_offset: Option<i32>) -> usize {
        requested_offset.unwrap_or(0).max(0) as usize
    }
}

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

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = Some(resolve_tenant_scope(tenant, tenant_id)?);

        let settings = SearchSettingsService::load_effective(db, tenant_id)
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

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let snapshot = SearchDiagnosticsService::snapshot(db, tenant_id)
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

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let rows = SearchDiagnosticsService::lagging_documents(
            db,
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

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let rows = SearchDiagnosticsService::consistency_issues(
            db,
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

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let days = normalize_analytics_days(days);
        let limit = normalize_analytics_limit(limit);

        let snapshot = SearchAnalyticsService::snapshot(db, tenant_id, days, limit)
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

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let snapshot = SearchDictionaryService::snapshot(db, tenant_id)
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

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id =
            resolve_tenant_scope(tenant, parse_optional_uuid(input.tenant_id.as_deref())?)?;
        let surface = normalize_surface(&input.surface)?;
        let settings = SearchSettingsService::load_effective(db, Some(tenant_id))
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
        let policy = SearchSurfacePolicy::search_preview();
        ensure_settings_read_permission(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let input = normalize_search_preview_input(input)?;
        let requested_limit = input.limit;
        let effective_limit = policy.effective_limit(requested_limit);
        let tenant_id =
            resolve_tenant_scope(tenant, parse_optional_uuid(input.tenant_id.as_deref())?)?;
        let transform = SearchDictionaryService::transform_query(db, tenant_id, &input.query)
            .await
            .map_err(map_search_module_error)?;
        let settings = SearchSettingsService::load_effective(db, Some(tenant_id))
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let resolved = resolve_preset_and_ranking(
            &settings.config,
            policy.surface,
            input.preset_key.as_deref(),
            input.ranking_profile.as_deref(),
            input.entity_types.clone(),
            input.source_modules.clone(),
            input.statuses.clone(),
        )?;
        let engine = PgSearchEngine::new(db.clone());
        let started_at = Instant::now();
        let search_query = SearchQuery {
            tenant_id: Some(tenant_id),
            locale: input.locale,
            channel_id: input.channel_id,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile: resolved.ranking_profile,
            preset_key: resolved.preset_key,
            limit: effective_limit,
            offset: policy.offset(input.offset),
            published_only: policy.published_only,
            entity_types: resolved.entity_types,
            source_modules: resolved.source_modules,
            statuses: resolved.statuses,
            category_ids: input.category_ids,
            attribute_filters: input.attribute_filters,
            sort_attribute_code: input.sort_attribute_code,
            sort_desc: input.sort_desc,
        };
        let result = run_search_with_dictionaries(db, &engine, search_query.clone()).await;
        finalize_search_result(
            db,
            policy.surface,
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
        let policy = SearchSurfacePolicy::admin_global_search();
        ensure_settings_read_permission(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let input = normalize_search_preview_input(input)?;
        let requested_limit = input.limit;
        let effective_limit = policy.effective_limit(requested_limit);
        let tenant_id =
            resolve_tenant_scope(tenant, parse_optional_uuid(input.tenant_id.as_deref())?)?;
        let transform = SearchDictionaryService::transform_query(db, tenant_id, &input.query)
            .await
            .map_err(map_search_module_error)?;
        let settings = SearchSettingsService::load_effective(db, Some(tenant_id))
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let resolved = resolve_preset_and_ranking(
            &settings.config,
            policy.surface,
            input.preset_key.as_deref(),
            input.ranking_profile.as_deref(),
            input.entity_types.clone(),
            input.source_modules.clone(),
            input.statuses.clone(),
        )?;
        let engine = PgSearchEngine::new(db.clone());
        let started_at = Instant::now();
        let search_query = SearchQuery {
            tenant_id: Some(tenant_id),
            locale: input.locale,
            channel_id: input.channel_id,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile: resolved.ranking_profile,
            preset_key: resolved.preset_key,
            limit: effective_limit,
            offset: policy.offset(input.offset),
            published_only: policy.published_only,
            entity_types: resolved.entity_types,
            source_modules: resolved.source_modules,
            statuses: resolved.statuses,
            category_ids: input.category_ids,
            attribute_filters: input.attribute_filters,
            sort_attribute_code: input.sort_attribute_code,
            sort_desc: input.sort_desc,
        };
        let result = run_search_with_dictionaries(db, &engine, search_query.clone()).await;
        finalize_search_result(
            db,
            policy.surface,
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
        let policy = SearchSurfacePolicy::storefront_search();
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let input = normalize_search_preview_input(input)?;
        enforce_storefront_rate_limit(ctx, policy.surface).await?;
        let engine = PgSearchEngine::new(db.clone());
        let requested_limit = input.limit;
        let effective_limit = policy.effective_limit(requested_limit);
        let started_at = Instant::now();
        let transform = SearchDictionaryService::transform_query(db, tenant.id, &input.query)
            .await
            .map_err(map_search_module_error)?;
        let settings = SearchSettingsService::load_effective(db, Some(tenant.id))
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        let resolved = resolve_preset_and_ranking(
            &settings.config,
            policy.surface,
            input.preset_key.as_deref(),
            input.ranking_profile.as_deref(),
            input.entity_types.clone(),
            input.source_modules.clone(),
            input.statuses.clone(),
        )?;

        let search_query = SearchQuery {
            tenant_id: Some(tenant.id),
            locale: input.locale,
            channel_id: input.channel_id,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile: resolved.ranking_profile,
            preset_key: resolved.preset_key,
            limit: effective_limit,
            offset: policy.offset(input.offset),
            published_only: policy.published_only,
            entity_types: resolved.entity_types,
            source_modules: resolved.source_modules,
            statuses: resolved.statuses,
            category_ids: input.category_ids,
            attribute_filters: input.attribute_filters,
            sort_attribute_code: input.sort_attribute_code,
            sort_desc: input.sort_desc,
        };

        let result = run_search_with_dictionaries(db, &engine, search_query.clone()).await;
        finalize_search_result(
            db,
            policy.surface,
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
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let input = normalize_search_suggestions_input(input)?;
        enforce_storefront_rate_limit(ctx, STOREFRONT_SUGGESTIONS_SURFACE).await?;
        let tenant_id = resolve_surface_tenant_scope(
            tenant,
            parse_optional_uuid(input.tenant_id.as_deref())?,
            SearchSurfacePolicy::storefront_search(),
        )?;

        if input.query.is_empty() {
            return Ok(Vec::new());
        }

        let suggestions = SearchSuggestionService::suggestions(
            db,
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
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let settings = SearchSettingsService::load_effective(db, Some(tenant.id))
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(
            SearchFilterPresetService::list(&settings.config, STOREFRONT_SEARCH_SURFACE)
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    }
}

async fn ensure_settings_read_permission(ctx: &Context<'_>) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

    if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
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
    resolve_surface_tenant_scope(
        tenant,
        requested_tenant_id,
        SearchSurfacePolicy::search_preview(),
    )
}

fn resolve_surface_tenant_scope(
    tenant: &TenantContext,
    requested_tenant_id: Option<Uuid>,
    policy: SearchSurfacePolicy,
) -> Result<Uuid> {
    match requested_tenant_id {
        Some(requested_tenant_id)
            if !policy.allows_tenant_override || requested_tenant_id != tenant.id =>
        {
            Err(<FieldError as GraphQLError>::permission_denied(
                "cross-tenant search access is not allowed",
            ))
        }
        _ => Ok(tenant.id),
    }
}

fn normalize_search_preview_input(
    input: SearchPreviewInput,
) -> Result<NormalizedSearchPreviewInput> {
    let query = normalize_query(&input.query)?;
    let locale = normalize_locale(input.locale.as_deref())?;
    let entity_types = normalize_filter_values("entity_types", input.entity_types)?;
    let source_modules = normalize_filter_values("source_modules", input.source_modules)?;
    let statuses = normalize_filter_values("statuses", input.statuses)?;
    let category_ids = normalize_uuid_values("category_ids", input.category_ids)?;
    let attribute_filters = normalize_attribute_filters(input.attribute_filters)?;

    Ok(NormalizedSearchPreviewInput {
        query,
        locale,
        channel_id: parse_optional_uuid(input.channel_id.as_deref())?,
        tenant_id: input.tenant_id,
        limit: input.limit,
        offset: input.offset,
        ranking_profile: normalize_ranking_profile(input.ranking_profile)?,
        preset_key: normalize_preset_key(input.preset_key)?,
        entity_types,
        source_modules,
        statuses,
        category_ids,
        attribute_filters,
        sort_attribute_code: normalize_attribute_code(input.sort_attribute_code)?,
        sort_desc: input.sort_desc.unwrap_or(false),
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

fn normalize_uuid_values(field_name: &str, values: Option<Vec<String>>) -> Result<Vec<Uuid>> {
    let values = values.unwrap_or_default();
    if values.len() > MAX_FILTER_VALUES {
        return Err(FieldError::new(format!(
            "{field_name} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        )));
    }

    values
        .into_iter()
        .map(|value| {
            Uuid::parse_str(value.trim())
                .map_err(|_| FieldError::new(format!("{field_name} contains an invalid UUID")))
        })
        .collect()
}

fn normalize_attribute_code(value: Option<String>) -> Result<Option<String>> {
    let Some(value) = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    validate_attribute_code("sort_attribute_code", &value)?;
    Ok(Some(value))
}

fn normalize_attribute_filters(
    filters: Option<Vec<super::types::SearchAttributeFilterInput>>,
) -> Result<Vec<SearchAttributeFilter>> {
    let filters = filters.unwrap_or_default();
    if filters.len() > MAX_ATTRIBUTE_FILTERS {
        return Err(FieldError::new(format!(
            "attribute_filters exceeds the maximum size of {MAX_ATTRIBUTE_FILTERS} filters"
        )));
    }

    filters
        .into_iter()
        .map(|filter| {
            let attribute_code = filter.attribute_code.trim().to_ascii_lowercase();
            validate_attribute_code("attribute_code", &attribute_code)?;
            let values = normalize_filter_values("attribute_filter.values", filter.values)?;
            Ok(SearchAttributeFilter {
                attribute_code,
                values,
                min: normalize_attribute_bound("attribute_filter.min", filter.min)?,
                max: normalize_attribute_bound("attribute_filter.max", filter.max)?,
            })
        })
        .collect()
}

fn validate_attribute_code(field_name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_FILTER_VALUE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(FieldError::new(format!(
            "{field_name} contains an invalid value"
        )));
    }

    Ok(())
}

fn normalize_attribute_bound(field_name: &str, value: Option<String>) -> Result<Option<String>> {
    let Some(value) = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    if value.len() > MAX_FILTER_VALUE_LEN || value.chars().any(|ch| ch.is_control()) {
        return Err(FieldError::new(format!(
            "{field_name} contains an invalid value"
        )));
    }

    Ok(Some(value))
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
    let Some(shared) = ctx.data_opt::<SearchGraphqlRateLimiterHandle>() else {
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
        Err(SearchGraphqlRateLimitError::Exceeded(exceeded)) => {
            metrics::record_rate_limit_exceeded(shared.0.namespace());
            metrics::record_search_rate_limit_outcome(surface, shared.0.namespace(), "exceeded");
            Err(FieldError::new(format!(
                "Search rate limit exceeded. Retry after {} seconds",
                exceeded.retry_after
            )))
        }
        Err(SearchGraphqlRateLimitError::BackendUnavailable(reason)) => {
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

fn extract_client_id(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| value.parse::<std::net::IpAddr>().is_ok())
        .map(|value| format!("ip:{value}"))
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| value.parse::<std::net::IpAddr>().is_ok())
                .map(|value| format!("ip:{value}"))
        })
        .unwrap_or_else(|| "ip:unknown".to_string())
}

fn build_storefront_rate_limit_key(
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: Option<&AuthContext>,
    headers: Option<&HeaderMap>,
    surface: &str,
) -> String {
    let client_key = headers
        .map(extract_client_id)
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
    result: rustok_core::Result<crate::SearchResult>,
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
) -> rustok_core::Result<crate::SearchResult> {
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
    let tenant_id = search_query.tenant_id?;
    let engine_kind = crate::SearchEngineKind::try_from_str(engine)?;

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

#[cfg(test)]
mod tests {
    use super::{
        ADMIN_GLOBAL_SEARCH_SURFACE, SEARCH_PREVIEW_SURFACE, STOREFRONT_SEARCH_SURFACE,
        SearchSurfacePolicy, resolve_surface_tenant_scope,
    };
    use rustok_api::TenantContext;
    use uuid::Uuid;

    fn tenant_context(id: Uuid) -> TenantContext {
        TenantContext {
            id,
            slug: "tenant-a".to_string(),
            name: "Tenant A".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    #[test]
    fn search_surface_policy_keeps_preview_admin_and_storefront_limits_separate() {
        let preview = SearchSurfacePolicy::search_preview();
        let admin = SearchSurfacePolicy::admin_global_search();
        let storefront = SearchSurfacePolicy::storefront_search();

        assert_eq!(preview.surface, SEARCH_PREVIEW_SURFACE);
        assert_eq!(preview.effective_limit(None), 10);
        assert_eq!(preview.effective_limit(Some(500)), 50);
        assert!(!preview.published_only);
        assert!(preview.requires_settings_read);

        assert_eq!(admin.surface, ADMIN_GLOBAL_SEARCH_SURFACE);
        assert_eq!(admin.effective_limit(None), 8);
        assert_eq!(admin.effective_limit(Some(500)), 20);
        assert!(!admin.published_only);
        assert!(admin.requires_settings_read);

        assert_eq!(storefront.surface, STOREFRONT_SEARCH_SURFACE);
        assert_eq!(storefront.effective_limit(None), 12);
        assert_eq!(storefront.effective_limit(Some(500)), 50);
        assert_eq!(storefront.offset(Some(-10)), 0);
        assert!(storefront.published_only);
        assert!(!storefront.requires_settings_read);
    }

    #[test]
    fn admin_surface_allows_only_current_tenant_scope() {
        let current_tenant_id = Uuid::new_v4();
        let tenant = tenant_context(current_tenant_id);

        assert_eq!(
            resolve_surface_tenant_scope(
                &tenant,
                Some(current_tenant_id),
                SearchSurfacePolicy::search_preview()
            )
            .expect("current tenant scope should be accepted"),
            current_tenant_id
        );

        assert!(
            resolve_surface_tenant_scope(
                &tenant,
                Some(Uuid::new_v4()),
                SearchSurfacePolicy::search_preview()
            )
            .is_err()
        );
    }

    #[test]
    fn storefront_surface_rejects_explicit_tenant_override() {
        let current_tenant_id = Uuid::new_v4();
        let tenant = tenant_context(current_tenant_id);

        assert_eq!(
            resolve_surface_tenant_scope(&tenant, None, SearchSurfacePolicy::storefront_search())
                .expect("implicit storefront tenant should use host context"),
            current_tenant_id
        );

        assert!(
            resolve_surface_tenant_scope(
                &tenant,
                Some(current_tenant_id),
                SearchSurfacePolicy::storefront_search()
            )
            .is_err()
        );
    }
}
