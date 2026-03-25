use async_graphql::{Context, FieldError, Object, Result};
use loco_rs::app::AppContext;
use std::time::Instant;
use uuid::Uuid;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::services::rbac_service::RbacService;
use rustok_search::{
    PgSearchEngine, SearchAnalyticsService, SearchDiagnosticsService, SearchDictionaryService,
    SearchEngine, SearchModule, SearchQuery, SearchQueryLogRecord, SearchSettingsService,
};
use rustok_telemetry::metrics;

use super::types::{
    LaggingSearchDocumentPayload, SearchAnalyticsPayload, SearchDiagnosticsPayload,
    SearchDictionarySnapshotPayload, SearchEngineDescriptor, SearchPreviewInput,
    SearchPreviewPayload, SearchSettingsPayload,
};

#[derive(Default)]
pub struct SearchQueryRoot;

const MAX_SEARCH_QUERY_LEN: usize = 256;
const MAX_FILTER_VALUES: usize = 10;
const MAX_FILTER_VALUE_LEN: usize = 64;
const MAX_LOCALE_LEN: usize = 16;
const DEFAULT_ANALYTICS_WINDOW_DAYS: u32 = 7;
const DEFAULT_ANALYTICS_LIMIT: usize = 10;

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
        let engine = PgSearchEngine::new(app_ctx.db.clone());
        let started_at = Instant::now();
        let search_query = SearchQuery {
            tenant_id: Some(tenant_id),
            locale: input.locale,
            original_query: transform.original_query,
            query: transform.effective_query,
            limit: effective_limit,
            offset: input.offset.unwrap_or(0).max(0) as usize,
            published_only: false,
            entity_types: input.entity_types.unwrap_or_default(),
            source_modules: input.source_modules.unwrap_or_default(),
            statuses: input.statuses.unwrap_or_default(),
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
        let engine = PgSearchEngine::new(app_ctx.db.clone());
        let started_at = Instant::now();
        let search_query = SearchQuery {
            tenant_id: Some(tenant_id),
            locale: input.locale,
            original_query: transform.original_query,
            query: transform.effective_query,
            limit: effective_limit,
            offset: input.offset.unwrap_or(0).max(0) as usize,
            published_only: false,
            entity_types: input.entity_types.unwrap_or_default(),
            source_modules: input.source_modules.unwrap_or_default(),
            statuses: input.statuses.unwrap_or_default(),
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
        let engine = PgSearchEngine::new(app_ctx.db.clone());
        let requested_limit = input.limit;
        let effective_limit = requested_limit.unwrap_or(12).clamp(1, 50) as usize;
        let started_at = Instant::now();
        let transform =
            SearchDictionaryService::transform_query(&app_ctx.db, tenant.id, &input.query)
                .await
                .map_err(map_search_module_error)?;

        let search_query = SearchQuery {
            tenant_id: Some(tenant.id),
            locale: input.locale,
            original_query: transform.original_query,
            query: transform.effective_query,
            limit: effective_limit,
            offset: input.offset.unwrap_or(0).max(0) as usize,
            published_only: true,
            entity_types: input.entity_types.unwrap_or_default(),
            source_modules: input.source_modules.unwrap_or_default(),
            statuses: input.statuses.unwrap_or_default(),
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
        entity_types: Some(entity_types),
        source_modules: Some(source_modules),
        statuses: Some(statuses),
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
